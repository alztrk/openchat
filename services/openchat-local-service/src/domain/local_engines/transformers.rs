//! Reads bounded Transformers configuration metadata without loading model weights.

use std::{
    fs::{self, File},
    io::Read,
    path::Path,
};

use serde_json::Value;

const MAX_CONFIG_BYTES: u64 = 1024 * 1024;
const MIN_CONTEXT_WINDOW: u64 = 128;
const MAX_CONTEXT_WINDOW: u64 = 2_000_000;
const NESTED_CONFIGS: &[&str] = &[
    "text_config",
    "language_config",
    "llm_config",
    "decoder",
    "model_config",
];
const CONTEXT_FIELDS: &[&str] = &[
    "max_position_embeddings",
    "n_positions",
    "max_seq_len",
    "max_sequence_length",
    "seq_length",
    "context_length",
    "max_context_length",
    "model_max_length",
];

pub(super) fn read_context_window(model_directory: &Path) -> Option<u32> {
    let config_path = model_directory.join("config.json");
    let metadata = fs::symlink_metadata(&config_path).ok()?;
    if !metadata.file_type().is_file() || metadata.len() > MAX_CONFIG_BYTES {
        return None;
    }

    let mut file = File::open(config_path).ok()?;
    let mut contents = Vec::with_capacity(metadata.len() as usize);
    file.by_ref()
        .take(MAX_CONFIG_BYTES + 1)
        .read_to_end(&mut contents)
        .ok()?;
    if contents.len() as u64 > MAX_CONFIG_BYTES {
        return None;
    }

    let config = serde_json::from_slice::<Value>(&contents).ok()?;
    context_window_from_config(&config)
}

fn context_window_from_config(config: &Value) -> Option<u32> {
    let object = config.as_object()?;
    for nested_key in NESTED_CONFIGS {
        if let Some(nested) = object.get(*nested_key)
            && let Some(context_window) = context_window_from_config(nested)
        {
            return Some(context_window);
        }
    }

    CONTEXT_FIELDS.iter().find_map(|field| {
        object
            .get(*field)
            .and_then(Value::as_u64)
            .filter(|value| (MIN_CONTEXT_WINDOW..=MAX_CONTEXT_WINDOW).contains(value))
            .and_then(|value| u32::try_from(value).ok())
    })
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use serde_json::json;

    use super::{context_window_from_config, read_context_window};

    #[test]
    fn reads_common_transformers_context_fields() {
        for config in [
            json!({"max_position_embeddings": 32_768}),
            json!({"n_positions": 16_384}),
            json!({"max_seq_len": 8_192}),
            json!({"max_sequence_length": 4_096}),
            json!({"seq_length": 2_048}),
        ] {
            assert!(context_window_from_config(&config).is_some());
        }
    }

    #[test]
    fn prefers_nested_text_configuration_to_outer_multimodal_limit() {
        let config = json!({
            "max_position_embeddings": 4_096,
            "text_config": {"max_position_embeddings": 131_072},
            "vision_config": {"max_position_embeddings": 8_192}
        });

        assert_eq!(context_window_from_config(&config), Some(131_072));
    }

    #[test]
    fn ignores_missing_sentinel_and_implausible_context_values() {
        for config in [
            json!({}),
            json!({"model_max_length": 1_000_000_000_000_i64}),
            json!({"max_position_embeddings": 0}),
            json!({"max_position_embeddings": 64}),
            json!({"max_position_embeddings": 2_000_001}),
            json!({"max_position_embeddings": "32768"}),
            json!({"max_position_embeddings": 32768.5}),
        ] {
            assert_eq!(context_window_from_config(&config), None);
        }
    }

    #[test]
    fn reads_bounded_config_file_from_model_directory() {
        let directory = temporary_directory();
        fs::create_dir_all(&directory).expect("temporary model directory should be created");
        fs::write(
            directory.join("config.json"),
            r#"{"text_config":{"max_position_embeddings":65536}}"#,
        )
        .expect("model config should be written");

        assert_eq!(read_context_window(&directory), Some(65_536));
        fs::remove_dir_all(directory).expect("temporary model directory should be removed");
    }

    #[test]
    fn ignores_oversized_and_malformed_config_files() {
        let directory = temporary_directory();
        fs::create_dir_all(&directory).expect("temporary model directory should be created");
        let config_path = directory.join("config.json");
        fs::write(&config_path, vec![b' '; 1024 * 1024 + 1])
            .expect("oversized model config should be written");
        assert_eq!(read_context_window(&directory), None);

        fs::write(&config_path, b"not json").expect("malformed config should be written");
        assert_eq!(read_context_window(&directory), None);
        fs::remove_dir_all(directory).expect("temporary model directory should be removed");
    }

    fn temporary_directory() -> PathBuf {
        std::env::temp_dir().join(format!(
            "openchat-transformers-config-{}",
            uuid::Uuid::new_v4()
        ))
    }
}
