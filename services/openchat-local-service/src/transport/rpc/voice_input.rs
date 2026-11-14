//! Local-only speech model management and transcription RPCs.

use std::{
    path::{Path, PathBuf},
    sync::{
        Arc, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use serde_json::{Value, json};
use tokio::{fs, sync::watch};
use zeroize::Zeroizing;

use crate::{
    hugging_face,
    protocol::{EventSink, ServiceError},
    storage::AppStorage,
    voice_input::{
        MAX_PCM16_AUDIO_BYTES, Pcm16MonoAudio, VoiceInputError, VoiceLanguage, WhisperTranscriber,
    },
};

use super::required_string;

const WHISPER_REPOSITORY: &str = "ggerganov/whisper.cpp";
const WHISPER_REVISION: &str = "5359861c739e955e79d9a303bcbc70fb988958b1";
const WHISPER_FILE: &str = "ggml-base.bin";
const WHISPER_SIZE_BYTES: u64 = 147_951_465;
const WHISPER_SHA256: &str = "60ed5bc3dd14eea856493d334349b405782ddcaf0028d4b5df4088345fba2efe";

const VAD_REPOSITORY: &str = "ggml-org/whisper-vad";
const VAD_REVISION: &str = "9ffd54a1e1ee413ddf265af9913beaf518d1639b";
const VAD_FILE: &str = "ggml-silero-v6.2.0.bin";
const VAD_SIZE_BYTES: u64 = 885_098;
const VAD_SHA256: &str = "2aa269b785eeb53a82983a20501ddf7c1d9c48e33ab63a41391ac6c9f7fb6987";
const MODEL_DIRECTORY: &str = "voice-input";
const MODEL_INSTALL_PROGRESS_EVENT: &str = "voice.input.model.install.progress";
const MAX_PCM16_BASE64_BYTES: usize = MAX_PCM16_AUDIO_BYTES.div_ceil(3) * 4;

static MODEL_INSTALL_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();
static TRANSCRIPTION_LOCK: OnceLock<tokio::sync::Mutex<()>> = OnceLock::new();

struct VoiceModelAsset {
    repository: &'static str,
    revision: &'static str,
    file_name: &'static str,
    size_bytes: u64,
    sha256: &'static str,
}

const VOICE_MODEL_ASSETS: [VoiceModelAsset; 2] = [
    VoiceModelAsset {
        repository: WHISPER_REPOSITORY,
        revision: WHISPER_REVISION,
        file_name: WHISPER_FILE,
        size_bytes: WHISPER_SIZE_BYTES,
        sha256: WHISPER_SHA256,
    },
    VoiceModelAsset {
        repository: VAD_REPOSITORY,
        revision: VAD_REVISION,
        file_name: VAD_FILE,
        size_bytes: VAD_SIZE_BYTES,
        sha256: VAD_SHA256,
    },
];

/// Dispatches local voice model status, opt-in installation, and transcription.
pub(super) async fn dispatch(
    method: &str,
    params: &Value,
    request_id: &Value,
    storage: &AppStorage,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    match method {
        "voice.input.model.status" => model_status(storage, cancellation).await,
        "voice.input.model.install" => {
            install_models(storage, request_id, events, cancellation).await
        }
        "voice.input.model.uninstall" => uninstall_models(storage, cancellation).await,
        "voice.input.transcribe" => transcribe(storage, params, cancellation).await,
        _ => Err(ServiceError::new(
            "method_not_found",
            "The requested local service method is not available.",
            false,
        )),
    }
}

async fn model_status(
    storage: &AppStorage,
    cancellation: &watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    let directory = voice_model_directory(storage);
    let whisper_path = directory.join(WHISPER_FILE);
    let vad_path = directory.join(VAD_FILE);
    let whisper_installed =
        asset_is_valid(&whisper_path, &VOICE_MODEL_ASSETS[0], cancellation).await?;
    let vad_installed = asset_is_valid(&vad_path, &VOICE_MODEL_ASSETS[1], cancellation).await?;

    Ok(json!({
        "installed": whisper_installed && vad_installed,
        "modelBytes": WHISPER_SIZE_BYTES,
        "vadBytes": VAD_SIZE_BYTES,
    }))
}

async fn install_models(
    storage: &AppStorage,
    request_id: &Value,
    events: &EventSink,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    check_cancelled(cancellation)?;
    let install_lock = MODEL_INSTALL_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = tokio::select! {
        guard = install_lock.lock() => guard,
        _ = cancellation.changed() => return Err(cancelled_error()),
    };

    let directory = voice_model_directory(storage);
    fs::create_dir_all(&directory)
        .await
        .map_err(|_| model_storage_error())?;

    let total_bytes: u64 = VOICE_MODEL_ASSETS
        .iter()
        .map(|asset| asset.size_bytes)
        .sum();
    let mut completed_bytes = 0_u64;
    for (index, asset) in VOICE_MODEL_ASSETS.iter().enumerate() {
        let target = directory.join(asset.file_name);
        if asset_is_valid(&target, asset, cancellation).await? {
            completed_bytes = completed_bytes.saturating_add(asset.size_bytes);
            continue;
        }
        remove_invalid_asset(&target).await?;
        hugging_face::download_pinned_asset(
            asset.repository,
            asset.revision,
            asset.file_name,
            asset.size_bytes,
            asset.sha256,
            &target,
            MODEL_INSTALL_PROGRESS_EVENT,
            completed_bytes,
            total_bytes,
            index + 1,
            VOICE_MODEL_ASSETS.len(),
            request_id,
            events,
            cancellation,
        )
        .await?;
        completed_bytes = completed_bytes.saturating_add(asset.size_bytes);
    }

    Ok(json!({
        "installed": true,
        "modelBytes": WHISPER_SIZE_BYTES,
        "vadBytes": VAD_SIZE_BYTES,
    }))
}

async fn uninstall_models(
    storage: &AppStorage,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    check_cancelled(cancellation)?;
    let install_lock = MODEL_INSTALL_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _install_guard = tokio::select! {
        guard = install_lock.lock() => guard,
        _ = cancellation.changed() => return Err(cancelled_error()),
    };
    let transcription_lock = TRANSCRIPTION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _transcription_guard = tokio::select! {
        guard = transcription_lock.lock() => guard,
        _ = cancellation.changed() => return Err(cancelled_error()),
    };

    let directory = voice_model_directory(storage);
    for asset in &VOICE_MODEL_ASSETS {
        let path = directory.join(asset.file_name);
        match fs::remove_file(path).await {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(_) => return Err(model_storage_error()),
        }
    }

    Ok(json!({ "removed": true }))
}

async fn transcribe(
    storage: &AppStorage,
    params: &Value,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<Value, ServiceError> {
    check_cancelled(cancellation)?;
    let encoded_audio = required_string(params, "pcm16Base64")?;
    if encoded_audio.is_empty() || encoded_audio.len() > MAX_PCM16_BASE64_BYTES {
        return Err(invalid_audio_error());
    }

    let sample_rate = params
        .get("sampleRate")
        .and_then(Value::as_u64)
        .and_then(|rate| u32::try_from(rate).ok())
        .ok_or_else(invalid_audio_error)?;
    let language = VoiceLanguage::parse(required_string(params, "language")?)
        .map_err(map_voice_input_error)?;
    let audio = decode_audio(encoded_audio, sample_rate)?;

    let transcription_lock = TRANSCRIPTION_LOCK.get_or_init(|| tokio::sync::Mutex::new(()));
    let _guard = tokio::select! {
        guard = transcription_lock.lock() => guard,
        _ = cancellation.changed() => return Err(cancelled_error()),
    };

    let directory = voice_model_directory(storage);
    let whisper_path = directory.join(WHISPER_FILE);
    let vad_path = directory.join(VAD_FILE);
    for (path, asset) in [
        (&whisper_path, &VOICE_MODEL_ASSETS[0]),
        (&vad_path, &VOICE_MODEL_ASSETS[1]),
    ] {
        if !asset_is_valid(path, asset, cancellation).await? {
            return Err(model_not_installed_error());
        }
    }

    let cancel_flag = Arc::new(AtomicBool::new(false));
    let worker_cancel_flag = Arc::clone(&cancel_flag);
    let task = tokio::task::spawn_blocking(move || {
        let transcriber =
            WhisperTranscriber::from_model_path(&whisper_path).map_err(map_voice_input_error)?;
        let result = transcriber
            .transcribe(audio, &language, Some(&vad_path), Some(&worker_cancel_flag))
            .map_err(map_voice_input_error)?;
        Ok::<_, ServiceError>(result.into_text())
    });
    tokio::pin!(task);

    tokio::select! {
        result = &mut task => {
            let text = result
                .map_err(|_| transcription_failed_error())??;
            Ok(json!({ "text": text }))
        }
        _ = cancellation.changed() => {
            cancel_flag.store(true, Ordering::Release);
            let _ = task.await;
            Err(cancelled_error())
        }
    }
}

fn decode_audio(encoded_audio: &str, sample_rate: u32) -> Result<Pcm16MonoAudio, ServiceError> {
    let bytes = Zeroizing::new(
        STANDARD
            .decode(encoded_audio)
            .map_err(|_| invalid_audio_error())?,
    );
    if bytes.len() % size_of::<i16>() != 0 {
        return Err(invalid_audio_error());
    }
    let samples = bytes
        .chunks_exact(size_of::<i16>())
        .map(|chunk| i16::from_le_bytes([chunk[0], chunk[1]]))
        .collect();
    Pcm16MonoAudio::new(samples, sample_rate, 1).map_err(map_voice_input_error)
}

async fn asset_is_valid(
    path: &Path,
    asset: &VoiceModelAsset,
    cancellation: &watch::Receiver<bool>,
) -> Result<bool, ServiceError> {
    let metadata = match fs::metadata(path).await {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(false),
        Err(_) => return Err(model_storage_error()),
    };
    if !metadata.is_file() || metadata.len() != asset.size_bytes {
        return Ok(false);
    }
    hugging_face::verify_file_sha256(path, asset.size_bytes, Some(asset.sha256), cancellation).await
}

async fn remove_invalid_asset(path: &Path) -> Result<(), ServiceError> {
    match fs::metadata(path).await {
        Ok(_) => fs::remove_file(path)
            .await
            .map_err(|_| model_storage_error()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(_) => Err(model_storage_error()),
    }
}

fn voice_model_directory(storage: &AppStorage) -> PathBuf {
    storage.root().join("models").join(MODEL_DIRECTORY)
}

fn check_cancelled(cancellation: &watch::Receiver<bool>) -> Result<(), ServiceError> {
    if *cancellation.borrow() {
        Err(cancelled_error())
    } else {
        Ok(())
    }
}

fn map_voice_input_error(error: VoiceInputError) -> ServiceError {
    match error {
        VoiceInputError::Cancelled => cancelled_error(),
        VoiceInputError::EmptyAudio => invalid_audio_error(),
        VoiceInputError::AudioTooLarge { .. }
        | VoiceInputError::UnsupportedSampleRate { .. }
        | VoiceInputError::UnsupportedChannelCount { .. } => invalid_audio_error(),
        VoiceInputError::UnsupportedLanguage(_) => ServiceError::new(
            "voice_language_unsupported",
            "The selected speech recognition language is not supported.",
            false,
        ),
        VoiceInputError::ModelPathUnavailable | VoiceInputError::VadModelPathUnavailable => {
            model_not_installed_error()
        }
        VoiceInputError::ModelPathNotUtf8 | VoiceInputError::ModelPathContainsNull => {
            model_storage_error()
        }
        VoiceInputError::Engine(_) => transcription_failed_error(),
    }
}

fn invalid_audio_error() -> ServiceError {
    ServiceError::new(
        "voice_audio_invalid",
        "The recorded audio is empty, too large, or has an unsupported format.",
        false,
    )
}

fn model_not_installed_error() -> ServiceError {
    ServiceError::new(
        "voice_model_not_installed",
        "Install the local speech model before recording voice input.",
        false,
    )
}

fn model_storage_error() -> ServiceError {
    ServiceError::new(
        "voice_model_storage_failed",
        "The local speech model could not be read or saved.",
        true,
    )
}

fn transcription_failed_error() -> ServiceError {
    ServiceError::new(
        "voice_transcription_failed",
        "Local speech transcription failed. Try recording again.",
        true,
    )
}

fn cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "Voice transcription or model installation was cancelled.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use base64::{Engine as _, engine::general_purpose::STANDARD};

    use super::decode_audio;

    #[test]
    fn decodes_little_endian_pcm16_for_local_transcription() {
        let bytes = [-32_768_i16, 0, 32_767]
            .into_iter()
            .flat_map(i16::to_le_bytes)
            .collect::<Vec<_>>();
        let encoded = STANDARD.encode(bytes);

        assert!(decode_audio(&encoded, 16_000).is_ok());
    }

    #[test]
    fn rejects_invalid_base64_odd_byte_counts_and_unsupported_sample_rates() {
        assert!(decode_audio("invalid", 16_000).is_err());
        assert!(decode_audio(&STANDARD.encode([0_u8]), 16_000).is_err());
        assert!(decode_audio(&STANDARD.encode([0_u8, 0_u8]), 8_000).is_err());
    }
}
