//! OpenAI-compatible BPE token counting using the `tiktoken` crate.
//!
//! Provides cached encodings for cl100k_base and o200k_base, used across
//! the application for context estimation. All token counts are i64-safe.

use std::sync::LazyLock;

use tiktoken::get_encoding;

/// Cached cl100k_base encoding (used by GPT-3.5, GPT-4, most models).
static CL100K: LazyLock<&'static tiktoken::CoreBpe> =
    LazyLock::new(|| get_encoding("cl100k_base").expect("cl100k_base must be available"));

/// Cached o200k_base encoding (used by GPT-4o series).
static O200K: LazyLock<&'static tiktoken::CoreBpe> =
    LazyLock::new(|| get_encoding("o200k_base").expect("o200k_base must be available"));

/// Selects the appropriate encoding based on model name.
pub(crate) fn model_encoding(model_id: &str) -> &'static tiktoken::CoreBpe {
    if model_id.contains("gpt-4o") {
        *O200K
    } else {
        *CL100K
    }
}

/// Count tokens using the encoding appropriate for the model.
pub(crate) fn count_tokens(model_id: &str, text: &str) -> i64 {
    model_encoding(model_id).count(text) as i64
}

/// Count tokens using cl100k_base (no model-specific selection).
pub(crate) fn count_tokens_cl100k(text: &str) -> i64 {
    CL100K.count(text) as i64
}

/// Count tokens using o200k_base.
pub(crate) fn count_tokens_o200k(text: &str) -> i64 {
    O200K.count(text) as i64
}
