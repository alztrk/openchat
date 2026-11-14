//! Local Whisper transcription over validated in-memory PCM audio.
//!
//! This module deliberately owns no audio capture, persistence, networking, or
//! RPC code. Callers provide an already captured 16 kHz mono PCM16 buffer and
//! local model paths. Whisper output is returned as data and is never printed.

use std::{
    fmt,
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
    },
};

#[cfg(windows)]
use std::path::{Component, Prefix};

use whisper_rs::{
    FullParams, SamplingStrategy, WhisperContext, WhisperContextParameters, WhisperError,
    convert_integer_to_float_audio, get_lang_id,
};
use zeroize::Zeroizing;

pub(crate) const SAMPLE_RATE_HZ: u32 = 16_000;
pub(crate) const MONO_CHANNEL_COUNT: u16 = 1;
pub(crate) const MAX_PCM16_AUDIO_BYTES: usize = 16 * 1024 * 1024;
const MAX_INFERENCE_THREADS: usize = 8;
const MAX_PCM16_AUDIO_SAMPLES: usize = MAX_PCM16_AUDIO_BYTES / size_of::<i16>();

/// A cancellation flag shared with the caller of a synchronous transcription.
pub(crate) type VoiceCancellation = Arc<AtomicBool>;

/// An in-memory PCM16 buffer that has passed the voice input format checks.
#[derive(Debug)]
pub(crate) struct Pcm16MonoAudio {
    samples: Zeroizing<Vec<i16>>,
}

impl Pcm16MonoAudio {
    pub(crate) fn new(
        samples: Vec<i16>,
        sample_rate_hz: u32,
        channel_count: u16,
    ) -> Result<Self, VoiceInputError> {
        let samples = Zeroizing::new(samples);
        if sample_rate_hz != SAMPLE_RATE_HZ {
            return Err(VoiceInputError::UnsupportedSampleRate {
                expected: SAMPLE_RATE_HZ,
                actual: sample_rate_hz,
            });
        }
        if channel_count != MONO_CHANNEL_COUNT {
            return Err(VoiceInputError::UnsupportedChannelCount {
                expected: MONO_CHANNEL_COUNT,
                actual: channel_count,
            });
        }
        if samples.is_empty() {
            return Err(VoiceInputError::EmptyAudio);
        }
        if samples.len() > MAX_PCM16_AUDIO_SAMPLES {
            return Err(VoiceInputError::AudioTooLarge {
                max_bytes: MAX_PCM16_AUDIO_BYTES,
                actual_bytes: samples.len().saturating_mul(size_of::<i16>()),
            });
        }
        Ok(Self { samples })
    }

    fn samples(&self) -> &[i16] {
        &self.samples
    }
}

/// The language passed to Whisper's decoder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum VoiceLanguage {
    Auto,
    Code(String),
}

impl VoiceLanguage {
    pub(crate) fn parse(value: &str) -> Result<Self, VoiceInputError> {
        let value = value.trim().to_ascii_lowercase();
        if value == "auto" {
            return Ok(Self::Auto);
        }
        if value.is_empty() || value.as_bytes().contains(&0) || get_lang_id(&value).is_none() {
            return Err(VoiceInputError::UnsupportedLanguage(value));
        }
        Ok(Self::Code(value))
    }
}

/// A successful local transcription result.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct VoiceTranscription {
    text: String,
}

impl VoiceTranscription {
    pub(crate) fn into_text(self) -> String {
        self.text
    }
}

/// A reusable Whisper model context for local transcription requests.
#[derive(Debug)]
pub(crate) struct WhisperTranscriber {
    context: WhisperContext,
}

impl WhisperTranscriber {
    /// Loads a Whisper model from an existing local file.
    pub(crate) fn from_model_path(model_path: &Path) -> Result<Self, VoiceInputError> {
        let model_path = validated_model_path(model_path, ModelPathKind::Transcription)?;
        let context =
            WhisperContext::new_with_params(model_path, WhisperContextParameters::default())
                .map_err(VoiceInputError::Engine)?;
        Ok(Self { context })
    }

    /// Transcribes and consumes validated PCM audio without using a network or writing audio to disk.
    pub(crate) fn transcribe(
        &self,
        audio: Pcm16MonoAudio,
        language: &VoiceLanguage,
        vad_model_path: Option<&Path>,
        cancellation: Option<&VoiceCancellation>,
    ) -> Result<VoiceTranscription, VoiceInputError> {
        if is_cancelled(cancellation) {
            return Err(VoiceInputError::Cancelled);
        }

        let vad_model_path = vad_model_path
            .map(|path| validated_model_path(path, ModelPathKind::Vad))
            .transpose()?;

        let mut samples = Zeroizing::new(vec![0.0_f32; audio.samples().len()]);
        convert_integer_to_float_audio(audio.samples(), &mut samples)
            .map_err(VoiceInputError::Engine)?;
        if is_cancelled(cancellation) {
            return Err(VoiceInputError::Cancelled);
        }

        let mut state = self
            .context
            .create_state()
            .map_err(VoiceInputError::Engine)?;
        let mut params = FullParams::new(SamplingStrategy::Greedy { best_of: 1 });
        params.set_n_threads(default_thread_count());
        params.set_translate(false);
        params.set_no_context(true);
        params.set_no_timestamps(true);
        params.set_print_special(false);
        params.set_print_progress(false);
        params.set_print_realtime(false);
        params.set_print_timestamps(false);

        match language {
            VoiceLanguage::Auto => {
                params.set_language(None);
                params.set_detect_language(true);
            }
            VoiceLanguage::Code(code) => params.set_language(Some(code.as_str())),
        }

        if let Some(vad_model_path) = vad_model_path {
            params.set_vad_model_path(Some(vad_model_path));
            params.enable_vad(true);
        }

        if let Some(cancellation) = cancellation {
            let cancellation = Arc::clone(cancellation);
            let callback: Box<dyn FnMut() -> bool> =
                Box::new(move || cancellation.load(Ordering::Acquire));
            params.set_abort_callback_safe::<_, Box<dyn FnMut() -> bool>>(Some(callback));
        }

        let full_result = state.full(params, &samples);
        if is_cancelled(cancellation) {
            return Err(VoiceInputError::Cancelled);
        }
        full_result.map_err(VoiceInputError::Engine)?;

        let text = state
            .as_iter()
            .map(|segment| segment.to_string())
            .collect::<String>()
            .trim()
            .to_owned();
        Ok(VoiceTranscription { text })
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ModelPathKind {
    Transcription,
    Vad,
}

fn validated_model_path(path: &Path, kind: ModelPathKind) -> Result<&str, VoiceInputError> {
    if is_remote_path(path) || !path.is_file() {
        return Err(match kind {
            ModelPathKind::Transcription => VoiceInputError::ModelPathUnavailable,
            ModelPathKind::Vad => VoiceInputError::VadModelPathUnavailable,
        });
    }
    let path = path.to_str().ok_or(VoiceInputError::ModelPathNotUtf8)?;
    if path.as_bytes().contains(&0) {
        return Err(VoiceInputError::ModelPathContainsNull);
    }
    Ok(path)
}

#[cfg(windows)]
fn is_remote_path(path: &Path) -> bool {
    matches!(
        path.components().next(),
        Some(Component::Prefix(prefix))
            if matches!(prefix.kind(), Prefix::UNC(..) | Prefix::VerbatimUNC(..))
    )
}

#[cfg(not(windows))]
fn is_remote_path(_path: &Path) -> bool {
    false
}

fn is_cancelled(cancellation: Option<&VoiceCancellation>) -> bool {
    cancellation.is_some_and(|flag| flag.load(Ordering::Acquire))
}

fn default_thread_count() -> i32 {
    std::thread::available_parallelism()
        .map(|count| count.get().min(MAX_INFERENCE_THREADS) as i32)
        .unwrap_or(1)
}

#[derive(Debug, Clone)]
pub(crate) enum VoiceInputError {
    EmptyAudio,
    AudioTooLarge {
        max_bytes: usize,
        actual_bytes: usize,
    },
    UnsupportedSampleRate {
        expected: u32,
        actual: u32,
    },
    UnsupportedChannelCount {
        expected: u16,
        actual: u16,
    },
    UnsupportedLanguage(String),
    ModelPathUnavailable,
    VadModelPathUnavailable,
    ModelPathNotUtf8,
    ModelPathContainsNull,
    Cancelled,
    Engine(WhisperError),
}

impl fmt::Display for VoiceInputError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyAudio => formatter.write_str("Voice input contains no audio samples."),
            Self::AudioTooLarge {
                max_bytes,
                actual_bytes,
            } => write!(
                formatter,
                "Voice input is too large: {actual_bytes} bytes exceeds the {max_bytes}-byte limit."
            ),
            Self::UnsupportedSampleRate { expected, actual } => write!(
                formatter,
                "Voice input sample rate {actual} Hz is unsupported; expected {expected} Hz."
            ),
            Self::UnsupportedChannelCount { expected, actual } => write!(
                formatter,
                "Voice input has {actual} channels; expected {expected} mono channel."
            ),
            Self::UnsupportedLanguage(language) => {
                write!(
                    formatter,
                    "Voice input language {language:?} is unsupported."
                )
            }
            Self::ModelPathUnavailable => {
                formatter.write_str("The local Whisper model file is unavailable.")
            }
            Self::VadModelPathUnavailable => {
                formatter.write_str("The local Whisper VAD model file is unavailable.")
            }
            Self::ModelPathNotUtf8 => {
                formatter.write_str("The local Whisper model path is invalid.")
            }
            Self::ModelPathContainsNull => {
                formatter.write_str("The local Whisper model path contains an invalid character.")
            }
            Self::Cancelled => formatter.write_str("Voice transcription was cancelled."),
            Self::Engine(error) => write!(formatter, "Local Whisper transcription failed: {error}"),
        }
    }
}

impl std::error::Error for VoiceInputError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Engine(error) => Some(error),
            _ => None,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn accepts_only_non_empty_16_khz_mono_pcm16_audio() {
        let samples = vec![0_i16, 1_i16];
        assert!(Pcm16MonoAudio::new(samples, SAMPLE_RATE_HZ, MONO_CHANNEL_COUNT).is_ok());
        assert!(matches!(
            Pcm16MonoAudio::new(Vec::new(), SAMPLE_RATE_HZ, MONO_CHANNEL_COUNT),
            Err(VoiceInputError::EmptyAudio)
        ));
        assert!(matches!(
            Pcm16MonoAudio::new(vec![0_i16, 1_i16], 8_000, MONO_CHANNEL_COUNT),
            Err(VoiceInputError::UnsupportedSampleRate {
                expected: SAMPLE_RATE_HZ,
                actual: 8_000,
            })
        ));
        assert!(matches!(
            Pcm16MonoAudio::new(vec![0_i16, 1_i16], SAMPLE_RATE_HZ, 2),
            Err(VoiceInputError::UnsupportedChannelCount {
                expected: MONO_CHANNEL_COUNT,
                actual: 2,
            })
        ));
    }

    #[test]
    fn rejects_audio_that_would_exceed_the_rpc_budget() {
        let samples = vec![0_i16; MAX_PCM16_AUDIO_SAMPLES + 1];
        assert!(matches!(
            Pcm16MonoAudio::new(samples, SAMPLE_RATE_HZ, MONO_CHANNEL_COUNT),
            Err(VoiceInputError::AudioTooLarge {
                max_bytes: MAX_PCM16_AUDIO_BYTES,
                actual_bytes,
            }) if actual_bytes == (MAX_PCM16_AUDIO_SAMPLES + 1) * size_of::<i16>()
        ));
    }

    #[test]
    fn validates_explicit_and_auto_languages() {
        assert!(matches!(
            VoiceLanguage::parse("tr"),
            Ok(VoiceLanguage::Code(code)) if code == "tr"
        ));
        assert!(matches!(
            VoiceLanguage::parse(" AUTO "),
            Ok(VoiceLanguage::Auto)
        ));
        assert!(matches!(
            VoiceLanguage::parse("not-a-language"),
            Err(VoiceInputError::UnsupportedLanguage(_))
        ));
        assert!(matches!(
            VoiceLanguage::parse(""),
            Err(VoiceInputError::UnsupportedLanguage(_))
        ));
        assert!(matches!(
            VoiceLanguage::parse("tr\0"),
            Err(VoiceInputError::UnsupportedLanguage(_))
        ));
    }

    #[test]
    fn rejects_unavailable_model_paths_before_native_loading() {
        assert!(matches!(
            WhisperTranscriber::from_model_path(Path::new("missing-whisper-model.bin")),
            Err(VoiceInputError::ModelPathUnavailable)
        ));
        assert!(matches!(
            validated_model_path(Path::new("missing-whisper-vad.onnx"), ModelPathKind::Vad),
            Err(VoiceInputError::VadModelPathUnavailable)
        ));
    }

    #[test]
    fn cancellation_flag_is_observed_without_running_the_model() {
        let flag = Arc::new(AtomicBool::new(true));
        assert!(is_cancelled(Some(&flag)));
        assert!(!is_cancelled(None));
    }
}
