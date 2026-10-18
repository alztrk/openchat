//! Identifies temporary conversations that are intentionally kept in memory.
//!
//! Temporary IDs are validated at the service boundary so these chats are not
//! confused with conversations persisted in the local database.

pub(crate) const ID_PREFIX: &str = "tmp_";

pub(crate) fn is_temporary_conversation_id(value: &str) -> bool {
    value.starts_with(ID_PREFIX)
}

pub(crate) fn is_valid_temporary_conversation_id(value: &str) -> bool {
    value.strip_prefix(ID_PREFIX).is_some_and(|suffix| {
        suffix.len() == 32
            && suffix
                .bytes()
                .all(|byte| byte.is_ascii_hexdigit() && !byte.is_ascii_uppercase())
    })
}

#[cfg(test)]
mod tests {
    use super::{is_temporary_conversation_id, is_valid_temporary_conversation_id};

    #[test]
    fn temporary_ids_are_distinct_from_durable_conversation_ids() {
        assert!(is_temporary_conversation_id(
            "tmp_0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_temporary_conversation_id(
            "0123456789abcdef0123456789abcdef"
        ));
    }

    #[test]
    fn temporary_rpc_ids_require_128_bits_of_lowercase_hex() {
        assert!(is_valid_temporary_conversation_id(
            "tmp_0123456789abcdef0123456789abcdef"
        ));
        assert!(!is_valid_temporary_conversation_id("tmp_short"));
        assert!(!is_valid_temporary_conversation_id(
            "tmp_0123456789abcdef0123456789abcdeG"
        ));
        assert!(!is_valid_temporary_conversation_id(
            "tmp_0123456789ABCDEF0123456789ABCDEF"
        ));
    }
}
