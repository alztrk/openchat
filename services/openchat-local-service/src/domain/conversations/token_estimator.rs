//! Model-aware token estimation shared by context usage and compaction.
//!
//! A supported model name selects its registered tokenizer. Unknown provider
//! model identifiers use an explicitly approximate, conservative text heuristic
//! instead of silently assuming a tokenizer family.

use tiktoken::encoding_for_model;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum TokenEstimateSource {
    ModelTokenizer,
    Heuristic,
}

impl TokenEstimateSource {
    pub(crate) fn as_str(self) -> &'static str {
        match self {
            Self::ModelTokenizer => "model_tokenizer",
            Self::Heuristic => "heuristic",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) struct TokenEstimate {
    pub tokens: i64,
    pub source: TokenEstimateSource,
}

/// Estimates text using a registered model tokenizer, or a conservative
/// fallback when the provider's model identifier is not recognized.
pub(crate) fn estimate_text(model_id: &str, text: &str) -> TokenEstimate {
    if let Some(encoding) = encoding_for_model(model_id) {
        return TokenEstimate {
            tokens: i64::try_from(encoding.count(text)).unwrap_or(i64::MAX),
            source: TokenEstimateSource::ModelTokenizer,
        };
    }

    TokenEstimate {
        tokens: heuristic_text_tokens(text),
        source: TokenEstimateSource::Heuristic,
    }
}

/// Calculates image tokens from visual dimensions and active model family.
pub(crate) fn estimate_image(model_id: &str, width: u32, height: u32) -> i64 {
    if width == 0 || height == 0 {
        return default_image_tokens(model_id);
    }

    let lower = model_id.to_ascii_lowercase();
    if lower.contains("gemini") {
        if width <= 384 && height <= 384 {
            return 258;
        }
        let tiles_x = u64::from((width + 767) / 768);
        let tiles_y = u64::from((height + 767) / 768);
        return i64::try_from(tiles_x.saturating_mul(tiles_y).saturating_mul(258))
            .unwrap_or(i64::MAX);
    }

    if lower.contains("claude") {
        let pixels = u64::from(width).saturating_mul(u64::from(height));
        let tokens = i64::try_from(pixels / 750).unwrap_or(i64::MAX);
        return tokens.clamp(80, 1600);
    }

    let (mut w, mut h) = (f64::from(width), f64::from(height));
    if w > 2048.0 || h > 2048.0 {
        let aspect = w / h;
        if aspect > 1.0 {
            w = 2048.0;
            h = (2048.0 / aspect).round();
        } else {
            h = 2048.0;
            w = (2048.0 * aspect).round();
        }
    }

    let aspect = w / h;
    if w < h {
        w = 768.0;
        h = (768.0 / aspect).round();
    } else {
        h = 768.0;
        w = (768.0 * aspect).round();
    }

    let tiles_x = (w / 512.0).ceil() as i64;
    let tiles_y = (h / 512.0).ceil() as i64;
    85 + 170 * tiles_x.saturating_mul(tiles_y)
}

/// Fallback image token count when dimensions cannot be determined.
pub(crate) fn default_image_tokens(model_id: &str) -> i64 {
    let lower = model_id.to_ascii_lowercase();
    if lower.contains("gemini") {
        1032
    } else if lower.contains("claude") {
        1600
    } else {
        1105
    }
}

/// Parses image dimensions from PNG, GIF, WebP, or JPEG byte headers without full decoding.
pub(crate) fn image_dimensions(bytes: &[u8]) -> Option<(u32, u32)> {
    if bytes.len() >= 24 && &bytes[0..8] == b"\x89PNG\r\n\x1a\n" {
        let width = u32::from_be_bytes([bytes[16], bytes[17], bytes[18], bytes[19]]);
        let height = u32::from_be_bytes([bytes[20], bytes[21], bytes[22], bytes[23]]);
        if width > 0 && height > 0 {
            return Some((width, height));
        }
    }

    if bytes.len() >= 10 && (&bytes[0..6] == b"GIF87a" || &bytes[0..6] == b"GIF89a") {
        let width = u16::from_le_bytes([bytes[6], bytes[7]]) as u32;
        let height = u16::from_le_bytes([bytes[8], bytes[9]]) as u32;
        if width > 0 && height > 0 {
            return Some((width, height));
        }
    }

    if bytes.len() >= 30 && &bytes[0..4] == b"RIFF" && &bytes[8..12] == b"WEBP" {
        if &bytes[12..16] == b"VP8 " && bytes.len() >= 30 {
            let width = (u16::from_le_bytes([bytes[26], bytes[27]]) & 0x3fff) as u32;
            let height = (u16::from_le_bytes([bytes[28], bytes[29]]) & 0x3fff) as u32;
            if width > 0 && height > 0 {
                return Some((width, height));
            }
        } else if &bytes[12..16] == b"VP8L" && bytes.len() >= 25 {
            let b1 = bytes[21];
            let b2 = bytes[22];
            let b3 = bytes[23];
            let b4 = bytes[24];
            let width = 1 + (((b2 as u32 & 0x3f) << 8) | b1 as u32);
            let height =
                1 + (((b4 as u32 & 0xf) << 10) | ((b3 as u32) << 2) | ((b2 as u32 & 0xc0) >> 6));
            return Some((width, height));
        } else if &bytes[12..16] == b"VP8X" && bytes.len() >= 30 {
            let width =
                1 + (bytes[24] as u32 | ((bytes[25] as u32) << 8) | ((bytes[26] as u32) << 16));
            let height =
                1 + (bytes[27] as u32 | ((bytes[28] as u32) << 8) | ((bytes[29] as u32) << 16));
            return Some((width, height));
        }
    }

    if bytes.len() >= 4 && bytes[0] == 0xff && bytes[1] == 0xd8 {
        let mut i = 2;
        while i + 9 < bytes.len() {
            if bytes[i] != 0xff {
                break;
            }
            let marker = bytes[i + 1];
            if matches!(
                marker,
                0xc0 | 0xc1
                    | 0xc2
                    | 0xc3
                    | 0xc5
                    | 0xc6
                    | 0xc7
                    | 0xc9
                    | 0xca
                    | 0xcb
                    | 0xcd
                    | 0xce
                    | 0xcf
            ) {
                let height = u16::from_be_bytes([bytes[i + 5], bytes[i + 6]]) as u32;
                let width = u16::from_be_bytes([bytes[i + 7], bytes[i + 8]]) as u32;
                if width > 0 && height > 0 {
                    return Some((width, height));
                }
                break;
            }
            if marker == 0xd9 || marker == 0xda {
                break;
            }
            let length = u16::from_be_bytes([bytes[i + 2], bytes[i + 3]]) as usize;
            if length < 2 {
                break;
            }
            i += 2 + length;
        }
    }

    None
}

pub(crate) fn source_for_model(model_id: &str) -> TokenEstimateSource {
    if encoding_for_model(model_id).is_some() {
        TokenEstimateSource::ModelTokenizer
    } else {
        TokenEstimateSource::Heuristic
    }
}

#[cfg(test)]
pub(crate) fn supports_model_tokenizer(model_id: &str) -> bool {
    source_for_model(model_id) == TokenEstimateSource::ModelTokenizer
}

/// Estimates ordinary prose by character class and uses UTF-8 bytes as a
/// conservative upper bound for long, dense payloads that may tokenize poorly.
fn heuristic_text_tokens(text: &str) -> i64 {
    let mut ascii_bytes = 0usize;
    let mut punctuation_count = 0usize;
    let mut non_ascii_characters = 0usize;
    let mut whitespace_count = 0usize;
    let mut character_count = 0usize;

    for character in text.chars() {
        character_count = character_count.saturating_add(1);
        if character.is_ascii() {
            ascii_bytes = ascii_bytes.saturating_add(1);
            if character.is_ascii_whitespace() {
                whitespace_count = whitespace_count.saturating_add(1);
            } else if !character.is_ascii_alphanumeric() {
                punctuation_count = punctuation_count.saturating_add(1);
            }
        } else {
            non_ascii_characters = non_ascii_characters.saturating_add(1);
        }
    }

    let is_dense_payload = text.len() >= 128
        && (whitespace_count == 0 || punctuation_count.saturating_mul(2) >= character_count);
    if is_dense_payload {
        return i64::try_from(text.len()).unwrap_or(i64::MAX);
    }

    let ordinary_ascii = ascii_bytes.saturating_sub(punctuation_count);
    i64::try_from(ordinary_ascii.div_ceil(3))
        .unwrap_or(i64::MAX)
        .saturating_add(i64::try_from(punctuation_count).unwrap_or(i64::MAX))
        .saturating_add(
            i64::try_from(non_ascii_characters)
                .unwrap_or(i64::MAX)
                .saturating_mul(2),
        )
}

#[cfg(test)]
mod tests {
    use super::{TokenEstimateSource, estimate_text, supports_model_tokenizer};

    #[test]
    fn uses_registered_model_tokenizer_for_supported_models() {
        assert!(supports_model_tokenizer("gpt-4.1"));
        assert_eq!(
            estimate_text("gpt-4.1", "hello world").source,
            TokenEstimateSource::ModelTokenizer
        );
    }

    #[test]
    fn unknown_models_use_a_conservative_approximation() {
        let estimate = estimate_text("custom-provider-model", "hello {\"key\": 1} 世界");
        assert_eq!(estimate.source, TokenEstimateSource::Heuristic);
        assert!(estimate.tokens >= 2);
    }

    #[test]
    fn heuristic_uses_utf8_bytes_for_dense_payloads() {
        let payload = "x".repeat(256);
        let estimate = estimate_text("custom-provider-model", &payload);

        assert_eq!(estimate.source, TokenEstimateSource::Heuristic);
        assert_eq!(estimate.tokens, payload.len() as i64);
    }

    #[test]
    fn empty_text_has_no_tokens() {
        assert_eq!(estimate_text("unknown-model", "").tokens, 0);
    }

    #[test]
    fn calculates_image_tokens_for_different_providers() {
        assert_eq!(super::estimate_image("gpt-4o", 1920, 1080), 1105);
        assert_eq!(super::estimate_image("gpt-4o", 1280, 720), 1105);
        assert_eq!(super::estimate_image("claude-3-5-sonnet", 1920, 1080), 1600);
        assert_eq!(super::estimate_image("claude-3-5-sonnet", 800, 600), 640);
        assert_eq!(super::estimate_image("gemini-2.0-flash", 300, 300), 258);
        assert_eq!(super::estimate_image("gemini-2.0-flash", 1920, 1080), 1548);
    }

    #[test]
    fn parses_png_and_gif_image_dimensions() {
        let mut png = vec![0u8; 32];
        png[0..8].copy_from_slice(b"\x89PNG\r\n\x1a\n");
        png[16..20].copy_from_slice(&1920u32.to_be_bytes());
        png[20..24].copy_from_slice(&1080u32.to_be_bytes());
        assert_eq!(super::image_dimensions(&png), Some((1920, 1080)));

        let mut gif = vec![0u8; 16];
        gif[0..6].copy_from_slice(b"GIF89a");
        gif[6..8].copy_from_slice(&800u16.to_le_bytes());
        gif[8..10].copy_from_slice(&600u16.to_le_bytes());
        assert_eq!(super::image_dimensions(&gif), Some((800, 600)));
    }
}
