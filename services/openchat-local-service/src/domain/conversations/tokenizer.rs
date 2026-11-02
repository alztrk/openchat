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

#[cfg(test)]
mod tests {
    #[test]
    fn token_count_initializes_on_a_small_thread_stack() {
        let worker = std::thread::Builder::new()
            .stack_size(512 * 1024)
            .spawn(|| super::count_tokens("gpt-4", "small stack tokenization"))
            .expect("spawn small-stack worker");

        assert!(
            worker
                .join()
                .expect("tokenizer should not overflow the stack")
                > 0
        );
    }
}
