//! OpenAI-compatible BPE token counting using the `tiktoken` crate.
//!
//! Provides cached encodings for cl100k_base and o200k_base, used across
//! the application for context estimation. All token counts are i64-safe.

use std::sync::LazyLock;

use tiktoken::{encoding_for_model, get_encoding};

/// Cached o200k_base encoding used when a model does not register a custom BPE table.
static O200K: LazyLock<&'static tiktoken::CoreBpe> =
    LazyLock::new(|| get_encoding("o200k_base").expect("o200k_base must be available"));

/// Selects the registered encoding for the model or falls back to `o200k_base`.
pub(crate) fn model_encoding(model_id: &str) -> &'static tiktoken::CoreBpe {
    encoding_for_model(model_id).unwrap_or(*O200K)
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
            .spawn(|| super::count_tokens("unknown-model", "small stack tokenization"))
            .expect("spawn small-stack worker");

        assert!(
            worker
                .join()
                .expect("tokenizer should not overflow the stack")
                > 0
        );
    }
}
