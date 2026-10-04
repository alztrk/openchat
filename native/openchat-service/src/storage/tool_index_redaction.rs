use rusqlite::{Connection, functions::FunctionFlags};

const MAX_INDEX_TEXT_BYTES: usize = 8 * 1024 * 1024;
const OVERSIZED_INDEX_TEXT: &str = "[tool activity omitted: searchable archive limit exceeded]";

const SENSITIVE_KEYS: &[&str] = &[
    "aws_secret_access_key",
    "cerebras_api_key",
    "huggingface_api_key",
    "openrouter_api_key",
    "github_access_token",
    "google_api_key",
    "openai_api_key",
    "mistral_api_key",
    "gemini_api_key",
    "replicate_api_token",
    "groq_api_key",
    "secret_access_key",
    "bearer_token",
    "proxy-authorization",
    "authorization",
    "client_secret",
    "clientsecret",
    "auth_token",
    "api_token",
    "refresh_token",
    "access_token",
    "github_token",
    "private_key",
    "password",
    "passwd",
    "api_key",
    "api-key",
    "apikey",
    "id_token",
    "x-api-key",
];

const TOKEN_PREFIXES: &[(&str, usize)] = &[
    ("github_pat_", 28),
    ("sk-or-v1-", 28),
    ("sk-ant-", 22),
    ("sk_live_", 28),
    ("sk_test_", 28),
    ("rk_live_", 28),
    ("rk_test_", 28),
    ("ghp_", 24),
    ("gho_", 24),
    ("ghu_", 24),
    ("ghs_", 24),
    ("ghr_", 24),
    ("AIza", 32),
    ("gsk_", 24),
    ("glpat-", 26),
    ("pypi-", 32),
    ("csk-", 24),
    ("xoxb-", 26),
    ("xoxp-", 26),
    ("xoxr-", 26),
    ("xoxs-", 26),
    ("xapp-", 28),
    ("npm_", 24),
    ("whsec_", 24),
    ("ya29.", 32),
    ("AKIA", 20),
    ("ASIA", 20),
    ("hf_", 24),
    ("xai-", 24),
    ("r8_", 24),
    ("sk-", 28),
];

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct Range {
    start: usize,
    end: usize,
}

pub(crate) fn register_sqlite_function(connection: &Connection) -> rusqlite::Result<()> {
    connection.create_scalar_function(
        "openchat_redact_credentials",
        1,
        FunctionFlags::SQLITE_DETERMINISTIC,
        |context| {
            let value = context.get::<Option<String>>(0)?;
            Ok(value.map(|text| redact_tool_index_text(&text)))
        },
    )
}

pub(crate) fn redact_tool_index_text(input: &str) -> String {
    if input.len() > MAX_INDEX_TEXT_BYTES {
        return OVERSIZED_INDEX_TEXT.to_owned();
    }

    let mut ranges = sensitive_value_ranges(input);
    ranges.extend(credential_token_ranges(input, &ranges));
    ranges.sort_unstable_by_key(|range| (range.start, range.end));

    let mut output = String::with_capacity(input.len());
    let mut cursor = 0;
    for range in ranges {
        if range.start < cursor {
            if range.end > cursor {
                cursor = range.end;
            }
            continue;
        }
        output.push_str(&input[cursor..range.start]);
        output.push_str("[REDACTED]");
        cursor = range.end;
    }
    output.push_str(&input[cursor..]);
    output
}

fn sensitive_value_ranges(input: &str) -> Vec<Range> {
    let bytes = input.as_bytes();
    let mut ranges = Vec::new();
    let mut index = 0;
    while index < bytes.len() {
        let Some(key) = SENSITIVE_KEYS
            .iter()
            .find(|key| key_matches_at(input, bytes, index, key))
        else {
            index += 1;
            continue;
        };

        let mut cursor = index + key.len();
        if cursor < bytes.len() && matches!(bytes[cursor], b'\'' | b'"') {
            cursor += 1;
        }
        cursor = skip_ascii_whitespace(bytes, cursor);
        if cursor >= bytes.len() || !matches!(bytes[cursor], b':' | b'=') {
            index += 1;
            continue;
        }
        cursor = skip_ascii_whitespace(bytes, cursor + 1);
        if cursor >= bytes.len() {
            index += key.len();
            continue;
        }

        let quoted = matches!(bytes[cursor], b'\'' | b'"');
        let (start, end) = if quoted {
            let quote = bytes[cursor];
            let start = cursor + 1;
            let mut end = start;
            while end < bytes.len() {
                if bytes[end] == b'\\' {
                    end = (end + 2).min(bytes.len());
                    continue;
                }
                if bytes[end] == quote {
                    break;
                }
                end += 1;
            }
            (start, end)
        } else if is_authorization_key(key) {
            let start = cursor;
            let end = scan_authorization_value_end(bytes, start);
            (start, end)
        } else {
            let start = cursor;
            let end = scan_unquoted_value_end(bytes, start);
            (start, end)
        };
        if end > start {
            ranges.push(Range { start, end });
            index = end;
        } else {
            index += key.len();
        }
    }
    ranges
}

fn credential_token_ranges(input: &str, existing: &[Range]) -> Vec<Range> {
    let bytes = input.as_bytes();
    let mut ranges = Vec::new();
    let mut index = 0;
    let mut existing_index = 0;
    while index < bytes.len() {
        while existing_index < existing.len() && existing[existing_index].end <= index {
            existing_index += 1;
        }
        if let Some(range) = existing.get(existing_index)
            && range.start <= index
            && index < range.end
        {
            index = range.end;
            continue;
        }

        let prefix = TOKEN_PREFIXES.iter().find(|(prefix, _)| {
            starts_ascii_case_insensitive(input, index, prefix)
                && (index == 0 || !is_ascii_word(bytes[index - 1]))
        });
        let Some((_, minimum_length)) = prefix else {
            index += 1;
            continue;
        };

        let mut end = index;
        while end < bytes.len() && is_token_byte(bytes[end]) {
            end += 1;
        }
        while end > index && matches!(bytes[end - 1], b'-' | b'_') {
            end -= 1;
        }
        if end.saturating_sub(index) >= *minimum_length {
            ranges.push(Range { start: index, end });
            index = end;
        } else {
            index += 1;
        }
    }
    ranges
}

fn key_matches_at(input: &str, bytes: &[u8], index: usize, key: &str) -> bool {
    starts_ascii_case_insensitive(input, index, key)
        && (index == 0 || !is_ascii_word(bytes[index - 1]))
        && bytes
            .get(index + key.len())
            .is_none_or(|byte| !is_ascii_word(*byte))
}

fn starts_ascii_case_insensitive(input: &str, index: usize, pattern: &str) -> bool {
    let input = input.as_bytes();
    let Some(candidate) = input.get(index..index.saturating_add(pattern.len())) else {
        return false;
    };
    candidate
        .iter()
        .zip(pattern.bytes())
        .all(|(actual, expected)| actual.eq_ignore_ascii_case(&expected))
}

fn skip_ascii_whitespace(bytes: &[u8], mut index: usize) -> usize {
    while bytes.get(index).is_some_and(u8::is_ascii_whitespace) {
        index += 1;
    }
    index
}

fn scan_unquoted_value_end(bytes: &[u8], mut index: usize) -> usize {
    while let Some(byte) = bytes.get(index) {
        if byte.is_ascii_whitespace()
            || matches!(byte, b',' | b';' | b'}' | b']' | b'&' | b'"' | b'\'')
        {
            break;
        }
        index += 1;
    }
    index
}

fn scan_authorization_value_end(bytes: &[u8], mut index: usize) -> usize {
    while let Some(byte) = bytes.get(index) {
        if matches!(
            byte,
            b'\r' | b'\n' | b',' | b';' | b'}' | b']' | b'"' | b'\''
        ) {
            break;
        }
        index += 1;
    }
    while index > 0 && bytes[index - 1].is_ascii_whitespace() {
        index -= 1;
    }
    index
}

fn is_authorization_key(key: &str) -> bool {
    key.eq_ignore_ascii_case("authorization") || key.eq_ignore_ascii_case("proxy-authorization")
}

fn is_ascii_word(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || byte == b'_'
}

fn is_token_byte(byte: u8) -> bool {
    byte.is_ascii_alphanumeric() || matches!(byte, b'_' | b'-' | b'.')
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::{redact_tool_index_text, register_sqlite_function};

    #[test]
    fn redacts_named_secrets_and_known_provider_token_formats() {
        let input = concat!(
            "api_key=short-secret ",
            "openai_api_key=prefixed-secret ",
            "\"client_secret\":\"json-secret\" ",
            "Authorization: Bearer header-secret\n",
            "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890 ",
            "sk-ant-api03-abcdefghijklmnopqrstuvwxyz ",
            "sk-or-v1-abcdefghijklmnopqrstuvwxyz0123456789 ",
            "gsk_abcdefghijklmnopqrstuvwxyz123456 ",
            "AIzaabcdefghijklmnopqrstuvwxyz1234567890 ",
            "hf_abcdefghijklmnopqrstuvwxyz123456 ",
            "csk-abcdefghijklmnopqrstuvwxyz123456 ",
            "npm_abcdefghijklmnopqrstuvwxyz123456 ",
            "ya29.abcdefghijklmnopqrstuvwxyz1234567890 ",
            "github_pat_abcdefghijklmnopqrstuvwxyz0123456789"
        );
        let redacted = redact_tool_index_text(input);

        for secret in [
            "short-secret",
            "prefixed-secret",
            "json-secret",
            "header-secret",
            "sk-proj-abcdefghijklmnopqrstuvwxyz1234567890",
            "sk-ant-api03-abcdefghijklmnopqrstuvwxyz",
            "sk-or-v1-abcdefghijklmnopqrstuvwxyz0123456789",
            "gsk_abcdefghijklmnopqrstuvwxyz123456",
            "AIzaabcdefghijklmnopqrstuvwxyz1234567890",
            "hf_abcdefghijklmnopqrstuvwxyz123456",
            "csk-abcdefghijklmnopqrstuvwxyz123456",
            "npm_abcdefghijklmnopqrstuvwxyz123456",
            "ya29.abcdefghijklmnopqrstuvwxyz1234567890",
            "github_pat_abcdefghijklmnopqrstuvwxyz0123456789",
        ] {
            assert!(!redacted.contains(secret), "secret remained: {secret}");
        }
        assert_eq!(redacted.matches("[REDACTED]").count(), 14);
    }

    #[test]
    fn preserves_non_secret_text_and_does_not_redact_short_prefixes() {
        let input = "ordinary text sk-demo and a café";
        assert_eq!(redact_tool_index_text(input), input);
    }

    #[test]
    fn replaces_oversized_index_text_instead_of_processing_it() {
        let input = "x".repeat(8 * 1024 * 1024 + 1);
        assert_eq!(
            redact_tool_index_text(&input),
            "[tool activity omitted: searchable archive limit exceeded]"
        );
    }

    #[test]
    fn sqlite_function_can_be_called_from_trigger_context() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        register_sqlite_function(&connection).expect("register redaction function");
        let result: String = connection
            .query_row(
                "SELECT openchat_redact_credentials('api_key=private-value')",
                [],
                |row| row.get(0),
            )
            .expect("call SQLite function");
        assert_eq!(result, "api_key=[REDACTED]");
    }
}
