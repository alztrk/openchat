//! Registers the normalization used by SQL-backed message search.

use rusqlite::{Connection, functions::FunctionFlags};

pub(super) fn register_sqlite_function(connection: &Connection) -> rusqlite::Result<()> {
    connection.create_scalar_function(
        "openchat_normalize_text",
        1,
        FunctionFlags::SQLITE_DETERMINISTIC | FunctionFlags::SQLITE_INNOCUOUS,
        |context| {
            let value = context.get::<Option<String>>(0)?;
            Ok(value.map(|text| crate::chatgpt_store::turkish_search::turkish_to_ascii(&text)))
        },
    )
}

#[cfg(test)]
mod tests {
    use rusqlite::{Connection, params};

    use super::register_sqlite_function;

    #[test]
    fn sqlite_function_uses_turkish_search_normalization() {
        let connection = Connection::open_in_memory().expect("open in-memory database");
        register_sqlite_function(&connection).expect("register search normalization");

        let normalized = connection
            .query_row(
                "SELECT openchat_normalize_text(?1)",
                params!["İSTANBUL IŞIK"],
                |row| row.get::<_, String>(0),
            )
            .expect("normalize Turkish search content");

        assert_eq!(normalized, "istanbul isik");
    }
}
