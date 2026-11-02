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
}
