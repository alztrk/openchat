use std::{
    fs::File,
    io::{self, BufReader, Read, Seek, SeekFrom},
    path::Path,
};

const MAGIC: &[u8; 4] = b"GGUF";
const MIN_VERSION: u32 = 2;
const MAX_VERSION: u32 = 3;
const MAX_METADATA_ENTRIES: u64 = 65_536;
const MAX_ARRAY_ELEMENTS: u64 = 1_000_000;
const MAX_KEY_BYTES: u64 = 64 * 1024;
const MAX_ARCHITECTURE_BYTES: u64 = 256;
const MAX_METADATA_BYTES: u64 = 64 * 1024 * 1024;

pub(crate) fn context_window(path: &Path) -> io::Result<Option<i64>> {
    let file = File::open(path)?;
    let file_size = file.metadata()?.len();
    let mut reader = BufReader::new(file);
    let mut magic = [0; 4];
    reader.read_exact(&mut magic)?;
    if &magic != MAGIC {
        return Err(invalid_gguf());
    }

    let version = read_u32(&mut reader)?;
    if !(MIN_VERSION..=MAX_VERSION).contains(&version) {
        return Err(invalid_gguf());
    }
    let _tensor_count = read_u64(&mut reader)?;
    let metadata_count = read_u64(&mut reader)?;
    if metadata_count > MAX_METADATA_ENTRIES {
        return Err(invalid_gguf());
    }

    let metadata_start = reader.stream_position()?;
    let mut architecture = None;
    let mut context_lengths = Vec::new();

    for _ in 0..metadata_count {
        ensure_metadata_limit(&mut reader, metadata_start, file_size)?;
        let key = read_string(&mut reader, file_size, metadata_start, MAX_KEY_BYTES)?;
        let value_type = read_u32(&mut reader)?;

        if key == "general.architecture" && value_type == 8 {
            architecture = Some(read_string(
                &mut reader,
                file_size,
                metadata_start,
                MAX_ARCHITECTURE_BYTES,
            )?);
        } else if key.ends_with(".context_length") {
            let value = read_integer_value(&mut reader, value_type, file_size, metadata_start)?;
            context_lengths.push((key, value));
        } else {
            skip_value(&mut reader, value_type, file_size, metadata_start)?;
        }
        ensure_metadata_limit(&mut reader, metadata_start, file_size)?;
    }

    let Some(architecture) = architecture else {
        return Ok(None);
    };
    let target_key = format!("{architecture}.context_length");
    Ok(context_lengths
        .into_iter()
        .find(|(key, value)| key == &target_key && value.is_some_and(|value| value > 0))
        .and_then(|(_, value)| value))
}

fn read_integer_value<R: Read + Seek>(
    reader: &mut R,
    value_type: u32,
    file_size: u64,
    metadata_start: u64,
) -> io::Result<Option<i64>> {
    let value = match value_type {
        0 => Some(i64::from(read_u8(reader)?)),
        1 => Some(i64::from(read_i8(reader)?)),
        2 => Some(i64::from(read_u16(reader)?)),
        3 => Some(i64::from(read_i16(reader)?)),
        4 => Some(i64::from(read_u32(reader)?)),
        5 => Some(i64::from(read_i32(reader)?)),
        10 => i64::try_from(read_u64(reader)?).ok(),
        11 => Some(read_i64(reader)?),
        _ => {
            skip_value(reader, value_type, file_size, metadata_start)?;
            None
        }
    };
    ensure_metadata_limit(reader, metadata_start, file_size)?;
    Ok(value)
}

fn skip_value<R: Read + Seek>(
    reader: &mut R,
    value_type: u32,
    file_size: u64,
    metadata_start: u64,
) -> io::Result<()> {
    match value_type {
        0 | 1 | 7 => skip_bytes(reader, 1, file_size, metadata_start),
        2 | 3 => skip_bytes(reader, 2, file_size, metadata_start),
        4..=6 => skip_bytes(reader, 4, file_size, metadata_start),
        8 => skip_string(reader, file_size, metadata_start),
        9 => {
            let element_type = read_u32(reader)?;
            let count = read_u64(reader)?;
            if count > MAX_ARRAY_ELEMENTS || element_type == 9 {
                return Err(invalid_gguf());
            }

            if element_type == 8 {
                for _ in 0..count {
                    skip_string(reader, file_size, metadata_start)?;
                    ensure_metadata_limit(reader, metadata_start, file_size)?;
                }
                Ok(())
            } else if let Some(width) = value_width(element_type) {
                let bytes = count.checked_mul(width).ok_or_else(invalid_gguf)?;
                skip_bytes(reader, bytes, file_size, metadata_start)
            } else {
                Err(invalid_gguf())
            }
        }
        10..=12 => skip_bytes(reader, 8, file_size, metadata_start),
        _ => Err(invalid_gguf()),
    }
}

fn value_width(value_type: u32) -> Option<u64> {
    match value_type {
        0 | 1 | 7 => Some(1),
        2 | 3 => Some(2),
        4..=6 => Some(4),
        10..=12 => Some(8),
        _ => None,
    }
}

fn read_string<R: Read + Seek>(
    reader: &mut R,
    file_size: u64,
    metadata_start: u64,
    maximum_bytes: u64,
) -> io::Result<String> {
    let length = read_u64(reader)?;
    if length > maximum_bytes {
        return Err(invalid_gguf());
    }
    ensure_can_skip(reader, length, file_size, metadata_start)?;
    let length = usize::try_from(length).map_err(|_| invalid_gguf())?;
    let mut bytes = vec![0; length];
    reader.read_exact(&mut bytes)?;
    String::from_utf8(bytes).map_err(|_| invalid_gguf())
}

fn skip_string<R: Read + Seek>(
    reader: &mut R,
    file_size: u64,
    metadata_start: u64,
) -> io::Result<()> {
    let length = read_u64(reader)?;
    skip_bytes(reader, length, file_size, metadata_start)
}

fn skip_bytes<R: Read + Seek>(
    reader: &mut R,
    bytes: u64,
    file_size: u64,
    metadata_start: u64,
) -> io::Result<()> {
    let target = ensure_can_skip(reader, bytes, file_size, metadata_start)?;
    reader.seek(SeekFrom::Start(target))?;
    ensure_metadata_limit(reader, metadata_start, file_size)
}

fn ensure_can_skip<R: Seek>(
    reader: &mut R,
    bytes: u64,
    file_size: u64,
    metadata_start: u64,
) -> io::Result<u64> {
    let current = reader.stream_position()?;
    let target = current.checked_add(bytes).ok_or_else(invalid_gguf)?;
    let metadata_limit = metadata_start
        .checked_add(MAX_METADATA_BYTES)
        .ok_or_else(invalid_gguf)?;
    if target > file_size || target > metadata_limit {
        return Err(invalid_gguf());
    }
    Ok(target)
}

fn ensure_metadata_limit<R: Seek>(
    reader: &mut R,
    metadata_start: u64,
    file_size: u64,
) -> io::Result<()> {
    let current = reader.stream_position()?;
    let metadata_limit = metadata_start
        .checked_add(MAX_METADATA_BYTES)
        .ok_or_else(invalid_gguf)?;
    if current > file_size || current > metadata_limit {
        return Err(invalid_gguf());
    }
    Ok(())
}

fn read_u8<R: Read>(reader: &mut R) -> io::Result<u8> {
    let mut bytes = [0; 1];
    reader.read_exact(&mut bytes)?;
    Ok(bytes[0])
}

fn read_i8<R: Read>(reader: &mut R) -> io::Result<i8> {
    Ok(i8::from_le_bytes([read_u8(reader)?]))
}

fn read_u16<R: Read>(reader: &mut R) -> io::Result<u16> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(u16::from_le_bytes(bytes))
}

fn read_i16<R: Read>(reader: &mut R) -> io::Result<i16> {
    let mut bytes = [0; 2];
    reader.read_exact(&mut bytes)?;
    Ok(i16::from_le_bytes(bytes))
}

fn read_u32<R: Read>(reader: &mut R) -> io::Result<u32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(u32::from_le_bytes(bytes))
}

fn read_i32<R: Read>(reader: &mut R) -> io::Result<i32> {
    let mut bytes = [0; 4];
    reader.read_exact(&mut bytes)?;
    Ok(i32::from_le_bytes(bytes))
}

fn read_u64<R: Read>(reader: &mut R) -> io::Result<u64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(u64::from_le_bytes(bytes))
}

fn read_i64<R: Read>(reader: &mut R) -> io::Result<i64> {
    let mut bytes = [0; 8];
    reader.read_exact(&mut bytes)?;
    Ok(i64::from_le_bytes(bytes))
}

fn invalid_gguf() -> io::Error {
    io::Error::new(io::ErrorKind::InvalidData, "Invalid GGUF metadata.")
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use uuid::Uuid;

    use super::context_window;

    struct TestFile(PathBuf);

    impl TestFile {
        fn new(contents: &[u8]) -> Self {
            let path = std::env::temp_dir().join(format!("openchat-gguf-{}.gguf", Uuid::new_v4()));
            fs::write(&path, contents).expect("GGUF fixture should be written");
            Self(path)
        }
    }

    impl Drop for TestFile {
        fn drop(&mut self) {
            let _ = fs::remove_file(&self.0);
        }
    }

    fn push_string(output: &mut Vec<u8>, value: &str) {
        output.extend_from_slice(&(value.len() as u64).to_le_bytes());
        output.extend_from_slice(value.as_bytes());
    }

    fn push_string_value(output: &mut Vec<u8>, key: &str, value: &str) {
        push_string(output, key);
        output.extend_from_slice(&8_u32.to_le_bytes());
        push_string(output, value);
    }

    fn push_uint32_value(output: &mut Vec<u8>, key: &str, value: u32) {
        push_string(output, key);
        output.extend_from_slice(&4_u32.to_le_bytes());
        output.extend_from_slice(&value.to_le_bytes());
    }

    fn gguf(metadata_count: u64, metadata: &[u8]) -> Vec<u8> {
        let mut contents = b"GGUF".to_vec();
        contents.extend_from_slice(&3_u32.to_le_bytes());
        contents.extend_from_slice(&0_u64.to_le_bytes());
        contents.extend_from_slice(&metadata_count.to_le_bytes());
        contents.extend_from_slice(metadata);
        contents
    }

    #[test]
    fn reads_architecture_context_length_without_loading_tensor_data() {
        let mut metadata = Vec::new();
        push_string_value(&mut metadata, "tokenizer.ggml.model", "llama");
        push_uint32_value(&mut metadata, "llama.context_length", 32_768);
        push_string_value(&mut metadata, "general.architecture", "llama");
        let file = TestFile::new(&gguf(3, &metadata));

        assert_eq!(
            context_window(&file.0).expect("metadata should parse"),
            Some(32_768)
        );
    }

    #[test]
    fn skips_string_arrays_and_rejects_truncated_metadata() {
        let mut metadata = Vec::new();
        push_string_value(&mut metadata, "general.architecture", "qwen2");
        push_string(&mut metadata, "tokenizer.ggml.tokens");
        metadata.extend_from_slice(&9_u32.to_le_bytes());
        metadata.extend_from_slice(&8_u32.to_le_bytes());
        metadata.extend_from_slice(&2_u64.to_le_bytes());
        push_string(&mut metadata, "one");
        push_string(&mut metadata, "two");
        push_uint32_value(&mut metadata, "qwen2.context_length", 131_072);
        let file = TestFile::new(&gguf(3, &metadata));
        assert_eq!(
            context_window(&file.0).expect("metadata should parse"),
            Some(131_072)
        );

        let truncated = TestFile::new(&gguf(3, &metadata[..metadata.len() - 1]));
        assert!(context_window(&truncated.0).is_err());
    }

    #[test]
    fn rejects_invalid_files_and_does_not_infer_a_missing_context_length() {
        let invalid = TestFile::new(b"not a GGUF file");
        assert!(context_window(&invalid.0).is_err());

        let mut metadata = Vec::new();
        push_string_value(&mut metadata, "general.architecture", "llama");
        let missing = TestFile::new(&gguf(1, &metadata));
        assert_eq!(
            context_window(&missing.0).expect("metadata should parse"),
            None
        );
    }
}
