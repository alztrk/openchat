//! Stores user-curated notes for one saved project.
//!
//! Notes are stored locally, disabled until explicitly enabled, and bounded
//! before they are included in requests to the configured model provider.

use std::time::UNIX_EPOCH;

use rusqlite::{OptionalExtension, params};
use serde::Serialize;

use crate::{protocol::ServiceError, storage::StorageConnection};

pub(crate) const MAX_ITEM_BYTES: usize = 4 * 1024;
pub(crate) const MAX_TOTAL_BYTES: usize = 8 * 1024;
pub(crate) const MAX_ITEMS: usize = 20;
pub(crate) const MAX_CONTEXT_BYTES: usize = MAX_TOTAL_BYTES + MAX_ITEMS * 2;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// One user-managed note attached to a saved project.
pub(crate) struct ProjectMemoryItem {
    /// Stable item identifier.
    pub(crate) id: String,
    /// User-authored reference note.
    pub(crate) content: String,
    /// Whether this note was added by the user or the assistant.
    pub(crate) origin: String,
    /// Creation time in Unix milliseconds.
    pub(crate) created_at_unix_ms: i64,
    /// Most recent update time in Unix milliseconds.
    pub(crate) updated_at_unix_ms: i64,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
/// Enablement and bounded notes for one saved project.
pub(crate) struct ProjectMemorySnapshot {
    /// Whether notes are included in new agent requests for this project.
    pub(crate) enabled: bool,
    /// Whether the assistant may add notes to this project's memory.
    pub(crate) assistant_saves_enabled: bool,
    /// Whether prior conversations from this project may be retrieved into new runs.
    pub(crate) history_context_enabled: bool,
    /// Notes in stable creation order.
    pub(crate) items: Vec<ProjectMemoryItem>,
    /// UTF-8 bytes stored across all notes.
    pub(crate) total_bytes: usize,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct ProjectMemorySearchHit {
    pub(crate) id: String,
    pub(crate) content: String,
    pub(crate) origin: String,
}

/// Reads the current project memory without enabling it or creating state.
pub(crate) fn snapshot(
    connection: &StorageConnection<'_>,
    project_id: &str,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    ensure_project(connection, project_id)?;
    read_snapshot(connection, project_id)
}

/// Returns a bounded, prompt-ready representation only when project memory is enabled.
pub(crate) fn context(
    connection: &StorageConnection<'_>,
    project_id: &str,
) -> Result<Option<String>, ServiceError> {
    let snapshot = snapshot(connection, project_id)?;
    Ok(context_from_snapshot(&snapshot))
}

pub(crate) fn context_from_snapshot(snapshot: &ProjectMemorySnapshot) -> Option<String> {
    if !snapshot.enabled || snapshot.items.is_empty() {
        return None;
    }
    Some(
        snapshot
            .items
            .iter()
            .map(|item| item.content.as_str())
            .collect::<Vec<_>>()
            .join("\n\n"),
    )
}

/// Searches the small, opt-in project note set without mixing it into the file index.
pub(crate) fn search(
    connection: &StorageConnection<'_>,
    project_id: &str,
    query: &str,
    limit: usize,
) -> Result<Vec<ProjectMemorySearchHit>, ServiceError> {
    let snapshot = snapshot(connection, project_id)?;
    if !snapshot.enabled || query.trim().is_empty() {
        return Ok(Vec::new());
    }
    let mut seen_terms = std::collections::HashSet::new();
    let query_terms = query
        .split(|character: char| !character.is_alphanumeric() && character != '_')
        .filter(|term| term.chars().count() >= 3)
        .map(crate::chatgpt_store::turkish_search::turkish_to_ascii)
        .filter(|term| !term.is_empty() && seen_terms.insert(term.clone()))
        .collect::<Vec<_>>();
    if query_terms.is_empty() {
        return Ok(Vec::new());
    }

    let mut matches = snapshot
        .items
        .into_iter()
        .filter_map(|item| {
            let content = crate::chatgpt_store::turkish_search::turkish_to_ascii(&item.content);
            let content_terms = content
                .split(|character: char| !character.is_alphanumeric() && character != '_')
                .collect::<Vec<_>>();
            let score = query_terms
                .iter()
                .filter(|term| content_terms.contains(&term.as_str()))
                .count();
            (score > 0).then_some((score, item))
        })
        .collect::<Vec<_>>();
    matches.sort_by(|(left_score, left), (right_score, right)| {
        right_score
            .cmp(left_score)
            .then_with(|| right.updated_at_unix_ms.cmp(&left.updated_at_unix_ms))
            .then_with(|| left.id.cmp(&right.id))
    });
    Ok(matches
        .into_iter()
        .take(limit.clamp(1, MAX_ITEMS))
        .map(|(_, item)| ProjectMemorySearchHit {
            id: item.id,
            content: item.content,
            origin: item.origin,
        })
        .collect())
}

/// Enables or disables injection of project notes in future runs.
pub(crate) fn set_enabled(
    connection: &mut StorageConnection<'_>,
    project_id: &str,
    enabled: bool,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    ensure_project(connection, project_id)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO project_memory_state (project_id, enabled)
             VALUES (?1, ?2)
             ON CONFLICT(project_id) DO UPDATE SET
                enabled = excluded.enabled,
                assistant_saves_enabled = CASE
                    WHEN excluded.enabled = 0 THEN 0
                    ELSE project_memory_state.assistant_saves_enabled
                END",
            params![project_id, enabled],
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    read_snapshot(connection, project_id)
}

/// Enables or disables assistant-authored project notes.
pub(crate) fn set_assistant_saves_enabled(
    connection: &mut StorageConnection<'_>,
    project_id: &str,
    enabled: bool,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    ensure_project(connection, project_id)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    if enabled {
        let memory_enabled = transaction
            .query_row(
                "SELECT enabled FROM project_memory_state WHERE project_id = ?1",
                [project_id],
                |row| row.get::<_, bool>(0),
            )
            .optional()
            .map_err(storage_error)?
            .unwrap_or(false);
        if !memory_enabled {
            return Err(ServiceError::new(
                "project_memory_disabled",
                "Enable project memory before allowing the assistant to save notes.",
                false,
            ));
        }
    }
    transaction
        .execute(
            "INSERT INTO project_memory_state (project_id, assistant_saves_enabled)
             VALUES (?1, ?2)
             ON CONFLICT(project_id) DO UPDATE
             SET assistant_saves_enabled = excluded.assistant_saves_enabled",
            params![project_id, enabled],
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    read_snapshot(connection, project_id)
}

/// Enables or disables retrieval from prior conversations attached to this project.
pub(crate) fn set_history_context_enabled(
    connection: &mut StorageConnection<'_>,
    project_id: &str,
    enabled: bool,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    ensure_project(connection, project_id)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    transaction
        .execute(
            "INSERT INTO project_memory_state (project_id, history_context_enabled)
             VALUES (?1, ?2)
             ON CONFLICT(project_id) DO UPDATE
             SET history_context_enabled = excluded.history_context_enabled",
            params![project_id, enabled],
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    read_snapshot(connection, project_id)
}

/// Adds one user-authored note after enforcing item and project limits.
pub(crate) fn add(
    connection: &mut StorageConnection<'_>,
    project_id: &str,
    content: &str,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    let content = validate_content(content)?;
    ensure_project(connection, project_id)?;
    insert_item(connection, project_id, content, "user", None)?;
    read_snapshot(connection, project_id)
}

/// Adds a bounded assistant-authored note only when both project-memory opt-ins are enabled.
pub(crate) fn add_assistant(
    connection: &mut StorageConnection<'_>,
    project_id: &str,
    conversation_id: &str,
    content: &str,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    let content = validate_content(content)?;
    ensure_project(connection, project_id)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    let belongs_to_project = transaction
        .query_row(
            "SELECT EXISTS (
                SELECT 1 FROM conversations WHERE id = ?1 AND project_id = ?2
             )",
            params![conversation_id, project_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage_error)?;
    if !belongs_to_project {
        return Err(ServiceError::new(
            "project_memory_conversation_unavailable",
            "The active conversation is not attached to this project.",
            false,
        ));
    }
    let settings = transaction
        .query_row(
            "SELECT enabled, assistant_saves_enabled FROM project_memory_state
             WHERE project_id = ?1",
            [project_id],
            |row| Ok((row.get::<_, bool>(0)?, row.get::<_, bool>(1)?)),
        )
        .optional()
        .map_err(storage_error)?;
    if !matches!(settings, Some((true, true))) {
        return Err(ServiceError::new(
            "project_memory_assistant_saves_disabled",
            "Project memory and assistant note saving must both be enabled.",
            false,
        ));
    }
    ensure_capacity(&transaction, project_id, content.len(), None)?;
    let now = now_unix_ms()?;
    insert_item_in_transaction(
        &transaction,
        project_id,
        content,
        "assistant",
        Some(conversation_id),
        now,
    )?;
    transaction.commit().map_err(storage_error)?;
    read_snapshot(connection, project_id)
}

fn insert_item(
    connection: &mut StorageConnection<'_>,
    project_id: &str,
    content: &str,
    origin: &str,
    source_conversation_id: Option<&str>,
) -> Result<(), ServiceError> {
    let transaction = connection.transaction().map_err(storage_error)?;
    ensure_capacity(&transaction, project_id, content.len(), None)?;
    let now = now_unix_ms()?;
    insert_item_in_transaction(
        &transaction,
        project_id,
        content,
        origin,
        source_conversation_id,
        now,
    )?;
    transaction.commit().map_err(storage_error)?;
    Ok(())
}

fn insert_item_in_transaction(
    transaction: &crate::storage::StorageTransaction<'_>,
    project_id: &str,
    content: &str,
    origin: &str,
    source_conversation_id: Option<&str>,
    now: i64,
) -> Result<(), ServiceError> {
    transaction
        .execute(
            "INSERT INTO project_memory_items (
                id, project_id, content, created_at_unix_ms, updated_at_unix_ms,
                origin, source_conversation_id
             ) VALUES (?1, ?2, ?3, ?4, ?4, ?5, ?6)",
            params![
                uuid::Uuid::new_v4().simple().to_string(),
                project_id,
                content,
                now,
                origin,
                source_conversation_id,
            ],
        )
        .map_err(storage_error)?;
    Ok(())
}

/// Updates an existing note without changing its creation order.
pub(crate) fn update(
    connection: &mut StorageConnection<'_>,
    project_id: &str,
    item_id: &str,
    content: &str,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    let content = validate_content(content)?;
    ensure_project(connection, project_id)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    let previous_bytes = transaction
        .query_row(
            "SELECT length(CAST(content AS BLOB)) FROM project_memory_items
             WHERE project_id = ?1 AND id = ?2",
            params![project_id, item_id],
            |row| row.get::<_, i64>(0),
        )
        .optional()
        .map_err(storage_error)?
        .ok_or_else(item_not_found_error)?;
    let previous_bytes = usize::try_from(previous_bytes).map_err(storage_error)?;
    ensure_capacity(
        &transaction,
        project_id,
        content.len(),
        Some(previous_bytes),
    )?;
    let now = now_unix_ms()?;
    transaction
        .execute(
            "UPDATE project_memory_items SET content = ?3, updated_at_unix_ms = ?4
             WHERE project_id = ?1 AND id = ?2",
            params![project_id, item_id, content, now],
        )
        .map_err(storage_error)?;
    transaction.commit().map_err(storage_error)?;
    read_snapshot(connection, project_id)
}

/// Deletes one note belonging to the selected project.
pub(crate) fn delete(
    connection: &mut StorageConnection<'_>,
    project_id: &str,
    item_id: &str,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    ensure_project(connection, project_id)?;
    let transaction = connection.transaction().map_err(storage_error)?;
    let deleted = transaction
        .execute(
            "DELETE FROM project_memory_items WHERE project_id = ?1 AND id = ?2",
            params![project_id, item_id],
        )
        .map_err(storage_error)?;
    if deleted == 0 {
        return Err(item_not_found_error());
    }
    transaction.commit().map_err(storage_error)?;
    read_snapshot(connection, project_id)
}

fn read_snapshot(
    connection: &StorageConnection<'_>,
    project_id: &str,
) -> Result<ProjectMemorySnapshot, ServiceError> {
    let (enabled, assistant_saves_enabled, history_context_enabled) = connection
        .query_row(
            "SELECT enabled, assistant_saves_enabled, history_context_enabled
             FROM project_memory_state WHERE project_id = ?1",
            [project_id],
            |row| {
                Ok((
                    row.get::<_, bool>(0)?,
                    row.get::<_, bool>(1)?,
                    row.get::<_, bool>(2)?,
                ))
            },
        )
        .optional()
        .map_err(storage_error)?
        .unwrap_or((false, false, false));
    let rows = connection
        .query_map(
            "SELECT id, content, origin, created_at_unix_ms, updated_at_unix_ms
             FROM project_memory_items WHERE project_id = ?1
             ORDER BY created_at_unix_ms, id",
            [project_id],
            |row| {
                Ok(ProjectMemoryItem {
                    id: row.get(0)?,
                    content: row.get(1)?,
                    origin: row.get(2)?,
                    created_at_unix_ms: row.get(3)?,
                    updated_at_unix_ms: row.get(4)?,
                })
            },
        )
        .map_err(storage_error)?;
    let items = rows
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(storage_error)?;
    let total_bytes = items.iter().map(|item| item.content.len()).sum::<usize>();
    if items.len() > MAX_ITEMS
        || items.iter().any(|item| item.content.len() > MAX_ITEM_BYTES)
        || total_bytes > MAX_TOTAL_BYTES
    {
        return Err(storage_error(()));
    }
    Ok(ProjectMemorySnapshot {
        enabled,
        assistant_saves_enabled,
        history_context_enabled,
        items,
        total_bytes,
    })
}

fn ensure_capacity(
    connection: &crate::storage::StorageTransaction<'_>,
    project_id: &str,
    new_content_bytes: usize,
    previous_content_bytes: Option<usize>,
) -> Result<(), ServiceError> {
    let (count, current_bytes) = connection
        .query_row(
            "SELECT COUNT(*), COALESCE(SUM(length(CAST(content AS BLOB))), 0)
             FROM project_memory_items WHERE project_id = ?1",
            [project_id],
            |row| Ok((row.get::<_, i64>(0)?, row.get::<_, i64>(1)?)),
        )
        .map_err(storage_error)?;
    let count = usize::try_from(count).map_err(storage_error)?;
    let current_bytes = usize::try_from(current_bytes).map_err(storage_error)?;
    let next_count = count + usize::from(previous_content_bytes.is_none());
    let next_bytes = current_bytes
        .saturating_sub(previous_content_bytes.unwrap_or_default())
        .saturating_add(new_content_bytes);
    if next_count > MAX_ITEMS {
        return Err(ServiceError::new(
            "project_memory_item_limit",
            "A project can contain up to 20 memory notes.",
            false,
        ));
    }
    if next_bytes > MAX_TOTAL_BYTES {
        return Err(ServiceError::new(
            "project_memory_size_limit",
            "Project memory is limited to 8 KiB in total.",
            false,
        ));
    }
    Ok(())
}

fn validate_content(content: &str) -> Result<&str, ServiceError> {
    let content = content.trim();
    if content.is_empty() {
        return Err(ServiceError::new(
            "project_memory_invalid_content",
            "A project memory note cannot be empty.",
            false,
        ));
    }
    if content.len() > MAX_ITEM_BYTES {
        return Err(ServiceError::new(
            "project_memory_item_size_limit",
            "A project memory note cannot exceed 4 KiB.",
            false,
        ));
    }
    Ok(content)
}

fn ensure_project(
    connection: &StorageConnection<'_>,
    project_id: &str,
) -> Result<(), ServiceError> {
    let exists = connection
        .query_row(
            "SELECT EXISTS (SELECT 1 FROM projects WHERE id = ?1)",
            [project_id],
            |row| row.get::<_, bool>(0),
        )
        .map_err(storage_error)?;
    if exists {
        Ok(())
    } else {
        Err(ServiceError::new(
            "project_unavailable",
            "The selected project is no longer available.",
            false,
        ))
    }
}

fn item_not_found_error() -> ServiceError {
    ServiceError::new(
        "project_memory_item_not_found",
        "The selected project memory note no longer exists.",
        false,
    )
}

fn storage_error<E>(_: E) -> ServiceError {
    ServiceError::new(
        "project_memory_unavailable",
        "Project memory could not be accessed. Retry the operation.",
        true,
    )
}

fn now_unix_ms() -> Result<i64, ServiceError> {
    let millis = UNIX_EPOCH
        .elapsed()
        .map_err(|_| storage_error(()))?
        .as_millis();
    i64::try_from(millis).map_err(|_| storage_error(()))
}

#[cfg(test)]
mod tests {
    use std::{fs, path::PathBuf};

    use super::{
        MAX_ITEM_BYTES, MAX_ITEMS, add, add_assistant, context, delete, search,
        set_assistant_saves_enabled, set_enabled, snapshot, update,
    };
    use crate::storage::AppStorage;

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let path = std::env::temp_dir().join(format!(
                "openchat-project-memory-test-{}",
                uuid::Uuid::new_v4().simple()
            ));
            fs::create_dir(&path).expect("create isolated test directory");
            Self(path)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove isolated test directory");
        }
    }

    fn test_storage() -> (TestDirectory, AppStorage) {
        let directory = TestDirectory::new();
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        storage
            .connect()
            .expect("connect test storage")
            .execute_batch(
                "CREATE TABLE projects (
                    id TEXT PRIMARY KEY NOT NULL
                );
                CREATE TABLE conversations (
                    id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT NOT NULL REFERENCES projects(id)
                );
                CREATE TABLE project_memory_state (
                    project_id TEXT PRIMARY KEY NOT NULL REFERENCES projects(id),
                    enabled INTEGER NOT NULL DEFAULT 0 CHECK (enabled IN (0, 1)),
                    assistant_saves_enabled INTEGER NOT NULL DEFAULT 0 CHECK (assistant_saves_enabled IN (0, 1)),
                    history_context_enabled INTEGER NOT NULL DEFAULT 0 CHECK (history_context_enabled IN (0, 1))
                );
                CREATE TABLE project_memory_items (
                    id TEXT PRIMARY KEY NOT NULL,
                    project_id TEXT NOT NULL REFERENCES projects(id),
                    content TEXT NOT NULL CHECK (length(content) BETWEEN 1 AND 4096),
                    created_at_unix_ms INTEGER NOT NULL,
                    updated_at_unix_ms INTEGER NOT NULL,
                    origin TEXT NOT NULL CHECK (origin IN ('user', 'assistant')),
                    source_conversation_id TEXT REFERENCES conversations(id) ON DELETE SET NULL
                );
                INSERT INTO projects (id) VALUES ('project-a'), ('project-b');
                INSERT INTO conversations (id, project_id)
                    VALUES ('conversation-a', 'project-a'), ('conversation-b', 'project-b');",
            )
            .expect("create isolated project memory schema");
        (directory, storage)
    }

    #[test]
    fn notes_are_disabled_by_default_and_scoped_to_their_project() {
        let (_directory, storage) = test_storage();
        let mut connection = storage.connect().expect("connect test storage");

        let added = add(
            &mut connection,
            "project-a",
            "  use the local test command  ",
        )
        .expect("add user note");
        assert!(!added.enabled);
        assert_eq!(added.items[0].content, "use the local test command");
        assert_eq!(
            context(&connection, "project-a").expect("read context"),
            None
        );

        let enabled = set_enabled(&mut connection, "project-a", true).expect("enable notes");
        assert_eq!(
            context(&connection, "project-a").expect("read enabled context"),
            Some("use the local test command".to_owned())
        );
        let other_project = snapshot(&connection, "project-b").expect("read other project");
        assert!(!other_project.enabled);
        assert!(other_project.items.is_empty());
        assert!(enabled.enabled);
    }

    #[test]
    fn memory_search_is_opt_in_scoped_and_ranks_matching_notes() {
        let (_directory, storage) = test_storage();
        let mut connection = storage.connect().expect("connect test storage");
        add(
            &mut connection,
            "project-a",
            "Use cargo test for Rust changes",
        )
        .expect("add Rust test note");
        add(
            &mut connection,
            "project-a",
            "Keep API errors typed and stable",
        )
        .expect("add API note");

        assert!(
            search(&connection, "project-a", "cargo test", 5)
                .expect("search disabled notes")
                .is_empty()
        );
        set_enabled(&mut connection, "project-a", true).expect("enable project memory");

        let hits = search(&connection, "project-a", "cargo test", 5).expect("search enabled notes");
        assert_eq!(hits.len(), 1);
        assert_eq!(hits[0].content, "Use cargo test for Rust changes");
        assert!(
            search(&connection, "project-b", "cargo test", 5)
                .expect("search another project")
                .is_empty()
        );
    }

    #[test]
    fn assistant_notes_require_both_opt_ins_and_an_attached_conversation() {
        let (_directory, storage) = test_storage();
        let mut connection = storage.connect().expect("connect test storage");

        let disabled = add_assistant(
            &mut connection,
            "project-a",
            "conversation-a",
            "prefer explicit error states",
        )
        .expect_err("assistant save is disabled by default");
        assert_eq!(disabled.code, "project_memory_assistant_saves_disabled");

        set_enabled(&mut connection, "project-a", true).expect("enable project memory");
        let assistant_saves_disabled = add_assistant(
            &mut connection,
            "project-a",
            "conversation-a",
            "prefer explicit error states",
        )
        .expect_err("assistant save requires its separate opt-in");
        assert_eq!(
            assistant_saves_disabled.code,
            "project_memory_assistant_saves_disabled"
        );

        set_assistant_saves_enabled(&mut connection, "project-a", true)
            .expect("enable assistant saves");
        let unattached = add_assistant(
            &mut connection,
            "project-a",
            "conversation-b",
            "unrelated note",
        )
        .expect_err("conversation from another project is rejected");
        assert_eq!(unattached.code, "project_memory_conversation_unavailable");

        let saved = add_assistant(
            &mut connection,
            "project-a",
            "conversation-a",
            "prefer explicit error states",
        )
        .expect("save assistant note");
        assert_eq!(saved.items[0].origin, "assistant");

        let disabled_again =
            set_enabled(&mut connection, "project-a", false).expect("disable project memory");
        assert!(!disabled_again.enabled);
        assert!(!disabled_again.assistant_saves_enabled);
        assert_eq!(
            context(&connection, "project-a").expect("read disabled context"),
            None
        );
    }

    #[test]
    fn note_updates_and_deletes_cannot_cross_project_boundaries() {
        let (_directory, storage) = test_storage();
        let mut connection = storage.connect().expect("connect test storage");
        let project_a = add(&mut connection, "project-a", "note A").expect("add project A note");
        let project_b = add(&mut connection, "project-b", "note B").expect("add project B note");
        let project_a_id = &project_a.items[0].id;
        let project_b_id = &project_b.items[0].id;

        assert_eq!(
            update(&mut connection, "project-b", project_a_id, "changed")
                .expect_err("cross-project update is rejected")
                .code,
            "project_memory_item_not_found"
        );
        assert_eq!(
            delete(&mut connection, "project-b", project_a_id)
                .expect_err("cross-project delete is rejected")
                .code,
            "project_memory_item_not_found"
        );

        update(&mut connection, "project-a", project_a_id, "updated A")
            .expect("update project A note");
        let project_b_after = snapshot(&connection, "project-b").expect("read project B notes");
        assert_eq!(project_b_after.items[0].id, *project_b_id);
        assert_eq!(project_b_after.items[0].content, "note B");
        delete(&mut connection, "project-a", project_a_id).expect("delete project A note");
        assert!(
            snapshot(&connection, "project-a")
                .expect("read project A notes")
                .items
                .is_empty()
        );
    }

    #[test]
    fn note_count_and_utf8_byte_limits_are_enforced() {
        let (_directory, storage) = test_storage();
        let mut connection = storage.connect().expect("connect test storage");

        let oversized = "é".repeat(MAX_ITEM_BYTES / 2 + 1);
        assert_eq!(
            add(&mut connection, "project-a", &oversized)
                .expect_err("oversized note is rejected")
                .code,
            "project_memory_item_size_limit"
        );

        let maximum_note = "x".repeat(MAX_ITEM_BYTES);
        add(&mut connection, "project-a", &maximum_note).expect("add first maximum note");
        add(&mut connection, "project-a", &maximum_note).expect("add second maximum note");
        assert_eq!(
            add(&mut connection, "project-a", "x")
                .expect_err("project byte limit is enforced")
                .code,
            "project_memory_size_limit"
        );

        for _ in 0..MAX_ITEMS {
            add(&mut connection, "project-b", "x").expect("add bounded note");
        }
        assert_eq!(
            add(&mut connection, "project-b", "x")
                .expect_err("project note count limit is enforced")
                .code,
            "project_memory_item_limit"
        );
    }
}
