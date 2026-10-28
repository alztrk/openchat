//! Unit and integration tests for file-change tracking, diffs, reverts, and checkpoints.

use super::checkpoints::{
    CheckpointRecoveryState, CheckpointRestoreError, checkpoint_recovery_state,
    mark_checkpoint_recovery_available, persist_checkpoint, restore_checkpoint,
    revert_checkpoint_operation,
};
use super::*;
use crate::storage::AppStorage;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicU64, Ordering},
};
type FileCheckpointChange<'a> = (&'a str, Option<&'a [u8]>, Option<&'a [u8]>);
static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);
struct TestDirectory(PathBuf);
impl TestDirectory {
    fn new(name: &str) -> Self {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!(
            "openchat-file-changes-{name}-{}-{id}",
            std::process::id()
        ));
        fs::create_dir_all(&path).expect("create test directory");
        Self(path)
    }
}
impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test directory");
    }
}
struct TestStorage {
    storage: AppStorage,
    _directory: TestDirectory,
}
impl TestStorage {
    fn new(name: &str) -> Self {
        let directory = TestDirectory::new(name);
        let storage = AppStorage::open_at(directory.0.clone()).expect("open test storage");
        storage                .connect()                .expect("connect test storage")                .execute_batch(                    "CREATE TABLE projects (                        id TEXT PRIMARY KEY NOT NULL,                        name TEXT NOT NULL,                        folder_path TEXT NOT NULL,                        created_at INTEGER NOT NULL,                        updated_at INTEGER NOT NULL                    );                    CREATE TABLE conversations (                        id TEXT PRIMARY KEY NOT NULL,                        title TEXT NOT NULL,                        title_source TEXT NOT NULL DEFAULT 'automatic',                        connection_id TEXT,                        workspace_id TEXT,                        api_key_connection_id TEXT,                        provider_id TEXT,                        model_id TEXT,                        project_id TEXT REFERENCES projects(id) ON DELETE SET NULL,                        is_pinned INTEGER NOT NULL DEFAULT 0,                        created_at INTEGER NOT NULL,                        updated_at INTEGER NOT NULL                    );                    CREATE TABLE messages (                        rowid INTEGER PRIMARY KEY,                        id TEXT NOT NULL,                        conversation_id TEXT NOT NULL                            REFERENCES conversations(id) ON DELETE CASCADE,                        role TEXT NOT NULL,                        content TEXT NOT NULL,                        created_at INTEGER,                        output_tokens INTEGER,                        tokens_per_second REAL,                        elapsed_microseconds INTEGER,                        reasoning_summaries TEXT NOT NULL DEFAULT '[]',                        tool_activities TEXT NOT NULL DEFAULT '[]',                        status TEXT NOT NULL,                        failure_code TEXT,                        UNIQUE (conversation_id, id)                    );",                )                .expect("create desktop chat tables");
        storage
            .initialize_backend_schema()
            .expect("initialize test backend schema");
        Self {
            storage,
            _directory: directory,
        }
    }
    fn add_conversation(&self, conversation_id: &str) {
        ensure_conversation(&self.storage, conversation_id);
    }
}
fn ensure_conversation(storage: &AppStorage, conversation_id: &str) {
    storage            .connect()            .expect("connect test storage")            .execute(                "INSERT OR IGNORE INTO conversations (                    id, title, created_at, updated_at                 ) VALUES (?1, ?1, 1, 1)",                [conversation_id],            )            .expect("insert test conversation");
}
fn record_file_change(
    root: &std::path::Path,
    storage: &AppStorage,
    conversation_id: &str,
    path: &str,
    old: Option<&[u8]>,
    new: Option<&[u8]>,
) -> String {
    ensure_conversation(storage, conversation_id);
    let before = match old {
        Some(content) => {
            fs::create_dir_all(root.join(path).parent().expect("file parent"))
                .expect("create parent");
            fs::write(root.join(path), content).expect("write old file");
            capture_file(root, path).expect("capture old file")
        }
        None => None,
    };
    match new {
        Some(content) => {
            fs::create_dir_all(root.join(path).parent().expect("file parent"))
                .expect("create parent");
            fs::write(root.join(path), content).expect("write new file");
        }
        None => {
            let _ = fs::remove_file(root.join(path));
        }
    }
    let after = capture_file(root, path).expect("capture new file");
    let normalized = path.replace('\\', "/");
    let root_identity = workspace_root_identity(root).expect("capture workspace identity");
    let delta = super::FileChangeDelta {
        root_identity,
        path: normalized,
        before,
        after,
    };
    record_deltas(storage, conversation_id, root, vec![delta])
        .expect("record file change")
        .first()
        .expect("one recorded file change")
        .id
        .clone()
}
fn record_file_checkpoint(
    root: &std::path::Path,
    storage: &AppStorage,
    conversation_id: &str,
    tool_name: &str,
    changes: &[FileCheckpointChange<'_>],
) -> String {
    ensure_conversation(storage, conversation_id);
    let root_identity = workspace_root_identity(root).expect("capture workspace identity");
    let mut deltas = Vec::with_capacity(changes.len());
    for (path, before_content, after_content) in changes {
        let target = root.join(path);
        fs::create_dir_all(target.parent().expect("file parent")).expect("create file parent");
        match before_content {
            Some(content) => fs::write(&target, content).expect("write before file state"),
            None => {
                let _ = fs::remove_file(&target);
            }
        }
        let before = capture_file(root, path).expect("capture before file state");
        match after_content {
            Some(content) => fs::write(&target, content).expect("write after file state"),
            None => {
                let _ = fs::remove_file(&target);
            }
        }
        let after = capture_file(root, path).expect("capture after file state");
        deltas.push(super::FileChangeDelta {
            root_identity,
            path: (*path).to_owned(),
            before,
            after,
        });
    }
    record_deltas(storage, conversation_id, root, deltas.clone())
        .expect("record operation file changes");
    persist_checkpoint(
        storage,
        conversation_id,
        tool_name,
        &[(root.to_path_buf(), deltas)],
    )
    .expect("persist operation checkpoint")
    .expect("operation changed files")
}
#[test]
fn multi_root_tracking_rejects_invalid_root_without_committing_other_roots() {
    let first_root = TestDirectory::new("batch-first-root");
    let second_root = TestDirectory::new("batch-second-root");
    let data = TestStorage::new("batch-tracking-data");
    let conversation_id = "batch-tracking-conversation";
    ensure_conversation(&data.storage, conversation_id);
    let first_identity =
        workspace_root_identity(&first_root.0).expect("capture first root identity");
    let second_identity =
        workspace_root_identity(&second_root.0).expect("capture second root identity");
    let first_content = b"tracked from first root".to_vec();
    let second_content = b"invalid second root".to_vec();
    let grouped_deltas = vec![
        (
            first_root.0.clone(),
            vec![super::FileChangeDelta {
                root_identity: first_identity,
                path: "first.txt".to_owned(),
                before: None,
                after: Some(super::FileSnapshot {
                    hash: super::sha256(&first_content),
                    content: Some(first_content),
                }),
            }],
        ),
        (
            second_root.0.clone(),
            vec![super::FileChangeDelta {
                root_identity: first_identity,
                path: "second.txt".to_owned(),
                before: None,
                after: Some(super::FileSnapshot {
                    hash: super::sha256(&second_content),
                    content: Some(second_content),
                }),
            }],
        ),
    ];
    let result = super::record_deltas_batch(&data.storage, conversation_id, &grouped_deltas);
    assert!(matches!(
        result,
        Err(super::TrackingError::WorkspaceUnavailable)
    ));
    assert!(
        list_changes(&data.storage, conversation_id)
            .expect("list unchanged tracking")
            .is_empty()
    );
    assert_ne!(first_identity, second_identity);
}
#[test]
fn failed_tool_recovery_preserves_later_changes_to_other_files() {
    let root = TestDirectory::new("recovery-workspace");
    let data = TestStorage::new("recovery-data");
    let failed_checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        "recovery-conversation",
        "bash",
        &[("partial.txt", None, Some(b"partial result\n"))],
    );
    mark_checkpoint_recovery_available(&data.storage, "recovery-conversation", &failed_checkpoint)
        .expect("enable recovery for the failed operation");
    let later_checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        "recovery-conversation",
        "write_file",
        &[("later.txt", None, Some(b"later result\n"))],
    );
    assert_eq!(
        checkpoint_recovery_state(&data.storage, "recovery-conversation", &later_checkpoint,),
        Err(CheckpointRestoreError::NotRecoverable)
    );
    assert_eq!(
        checkpoint_recovery_state(&data.storage, "recovery-conversation", &failed_checkpoint,)
            .expect("inspect failed operation recovery"),
        CheckpointRecoveryState::Available
    );
    revert_checkpoint_operation(&data.storage, "recovery-conversation", &failed_checkpoint)
        .expect("revert only the failed operation");
    assert!(!root.0.join("partial.txt").exists());
    assert_eq!(
        fs::read_to_string(root.0.join("later.txt")).expect("read later file"),
        "later result\n"
    );
    assert_eq!(
        checkpoint_recovery_state(&data.storage, "recovery-conversation", &failed_checkpoint,)
            .expect("inspect completed recovery"),
        CheckpointRecoveryState::AlreadyRestored
    );
    assert_eq!(
        revert_checkpoint_operation(&data.storage, "recovery-conversation", &failed_checkpoint,)
            .expect_err("prevent repeating recovery"),
        CheckpointRestoreError::AlreadyRestored
    );
}
#[test]
fn failed_tool_recovery_resumes_after_a_partial_filesystem_restore() {
    let root = TestDirectory::new("partial-recovery-workspace");
    let data = TestStorage::new("partial-recovery-data");
    let conversation_id = "partial-recovery-conversation";
    let failed_checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        conversation_id,
        "bash",
        &[
            (
                "first.txt",
                Some(b"before first\n"),
                Some(b"partial first\n"),
            ),
            (
                "second.txt",
                Some(b"before second\n"),
                Some(b"partial second\n"),
            ),
        ],
    );
    mark_checkpoint_recovery_available(&data.storage, conversation_id, &failed_checkpoint)
        .expect("enable recovery for the failed operation");

    fs::write(root.0.join("first.txt"), b"before first\n")
        .expect("simulate interruption after restoring the first file");
    assert_eq!(
        checkpoint_recovery_state(&data.storage, conversation_id, &failed_checkpoint)
            .expect("inspect partially restored operation"),
        CheckpointRecoveryState::Available
    );

    let summaries = revert_checkpoint_operation(&data.storage, conversation_id, &failed_checkpoint)
        .expect("resume restoring the remaining file");

    assert_eq!(summaries.len(), 2);
    assert_eq!(
        fs::read(root.0.join("first.txt")).expect("read first restored file"),
        b"before first\n"
    );
    assert_eq!(
        fs::read(root.0.join("second.txt")).expect("read second restored file"),
        b"before second\n"
    );
    let changes = list_changes(&data.storage, conversation_id).expect("list restored changes");
    assert!(changes.iter().all(|change| change.status == "reverted"));
    assert_eq!(
        checkpoint_recovery_state(&data.storage, conversation_id, &failed_checkpoint)
            .expect("inspect completed recovery"),
        CheckpointRecoveryState::AlreadyRestored
    );
}
#[test]
fn checkpoint_restore_and_recovery_reject_a_replaced_root_with_matching_contents() {
    let root = TestDirectory::new("replaced-root-checkpoint");
    let data = TestStorage::new("replaced-root-checkpoint-data");
    let checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        "replaced-root-checkpoint-conversation",
        "write_file",
        &[("note.txt", None, Some(b"after\n"))],
    );
    mark_checkpoint_recovery_available(
        &data.storage,
        "replaced-root-checkpoint-conversation",
        &checkpoint,
    )
    .expect("enable failed operation recovery");
    let moved_root = root.0.with_extension("original");
    fs::rename(&root.0, &moved_root).expect("move original workspace root");
    fs::create_dir_all(&root.0).expect("create replacement workspace root");
    fs::write(root.0.join("note.txt"), b"after\n").expect("seed matching replacement file");
    assert_eq!(
        restore_checkpoint(
            &data.storage,
            "replaced-root-checkpoint-conversation",
            &checkpoint,
        )
        .expect_err("reject restore into replacement root"),
        CheckpointRestoreError::Conflict
    );
    assert_eq!(
        revert_checkpoint_operation(
            &data.storage,
            "replaced-root-checkpoint-conversation",
            &checkpoint,
        )
        .expect_err("reject recovery into replacement root"),
        CheckpointRestoreError::Conflict
    );
    assert_eq!(
        fs::read(root.0.join("note.txt")).expect("read replacement file"),
        b"after\n"
    );
    fs::remove_dir_all(moved_root).expect("remove moved original workspace");
}
#[test]
fn identityless_checkpoint_history_cannot_restore_or_recover_files() {
    let root = TestDirectory::new("identityless-checkpoint");
    let data = TestStorage::new("identityless-checkpoint-data");
    let checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        "identityless-checkpoint-conversation",
        "write_file",
        &[("note.txt", None, Some(b"after\n"))],
    );
    mark_checkpoint_recovery_available(
        &data.storage,
        "identityless-checkpoint-conversation",
        &checkpoint,
    )
    .expect("enable failed operation recovery");
    data.storage            .connect()            .expect("connect test storage")            .execute(                "UPDATE conversation_file_checkpoint_files                 SET root_identity = NULL WHERE checkpoint_id = ?1",                [&checkpoint],            )            .expect("simulate a legacy identity-less checkpoint");
    assert_eq!(
        checkpoint_recovery_state(
            &data.storage,
            "identityless-checkpoint-conversation",
            &checkpoint,
        ),
        Err(CheckpointRestoreError::NotRecoverable)
    );
    assert_eq!(
        revert_checkpoint_operation(
            &data.storage,
            "identityless-checkpoint-conversation",
            &checkpoint,
        )
        .expect_err("refuse identity-less checkpoint recovery"),
        CheckpointRestoreError::NotRecoverable
    );
    assert_eq!(
        restore_checkpoint(
            &data.storage,
            "identityless-checkpoint-conversation",
            &checkpoint,
        )
        .expect_err("refuse identity-less checkpoint restore"),
        CheckpointRestoreError::NotRecoverable
    );
    assert_eq!(
        fs::read(root.0.join("note.txt")).expect("read current file"),
        b"after\n"
    );
}
#[test]
fn failed_tool_recovery_keeps_every_file_when_any_path_conflicts() {
    let root = TestDirectory::new("recovery-conflict-workspace");
    let data = TestStorage::new("recovery-conflict-data");
    let failed_checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        "recovery-conflict-conversation",
        "bash",
        &[
            ("first.txt", None, Some(b"partial first\n")),
            ("second.txt", None, Some(b"partial second\n")),
        ],
    );
    mark_checkpoint_recovery_available(
        &data.storage,
        "recovery-conflict-conversation",
        &failed_checkpoint,
    )
    .expect("enable recovery for the failed operation");
    fs::write(root.0.join("second.txt"), "external change\n")
        .expect("write external conflicting edit");
    assert_eq!(
        checkpoint_recovery_state(
            &data.storage,
            "recovery-conflict-conversation",
            &failed_checkpoint,
        )
        .expect("inspect conflicting recovery"),
        CheckpointRecoveryState::Conflict
    );
    assert_eq!(
        revert_checkpoint_operation(
            &data.storage,
            "recovery-conflict-conversation",
            &failed_checkpoint,
        )
        .expect_err("refuse conflicting recovery"),
        CheckpointRestoreError::Conflict
    );
    assert_eq!(
        fs::read_to_string(root.0.join("first.txt")).expect("read untouched first file"),
        "partial first\n"
    );
    assert_eq!(
        fs::read_to_string(root.0.join("second.txt")).expect("read conflicting file"),
        "external change\n"
    );
}
#[test]
fn failed_tool_recovery_refuses_a_path_touched_by_a_later_tool() {
    let root = TestDirectory::new("recovery-later-tool-workspace");
    let data = TestStorage::new("recovery-later-tool-data");
    let failed_checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        "recovery-later-tool-conversation",
        "bash",
        &[("shared.txt", Some(b"before\n"), Some(b"partial\n"))],
    );
    mark_checkpoint_recovery_available(
        &data.storage,
        "recovery-later-tool-conversation",
        &failed_checkpoint,
    )
    .expect("enable recovery for the failed operation");
    record_file_checkpoint(
        &root.0,
        &data.storage,
        "recovery-later-tool-conversation",
        "write_file",
        &[("shared.txt", Some(b"partial\n"), Some(b"later result\n"))],
    );
    assert_eq!(
        checkpoint_recovery_state(
            &data.storage,
            "recovery-later-tool-conversation",
            &failed_checkpoint,
        )
        .expect("inspect later tool conflict"),
        CheckpointRecoveryState::Conflict
    );
    assert_eq!(
        revert_checkpoint_operation(
            &data.storage,
            "recovery-later-tool-conversation",
            &failed_checkpoint,
        )
        .expect_err("preserve the later tool result"),
        CheckpointRestoreError::Conflict
    );
    assert_eq!(
        fs::read_to_string(root.0.join("shared.txt")).expect("read later tool result"),
        "later result\n"
    );
}
#[test]
fn restoring_a_workspace_checkpoint_records_the_restore_without_relocking() {
    let root = TestDirectory::new("workspace-restore-workspace");
    let data = TestStorage::new("workspace-restore-data");
    let first_checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        "workspace-restore-conversation",
        "write_file",
        &[("shared.txt", Some(b"before\n"), Some(b"first\n"))],
    );
    record_file_checkpoint(
        &root.0,
        &data.storage,
        "workspace-restore-conversation",
        "write_file",
        &[("shared.txt", Some(b"first\n"), Some(b"later\n"))],
    );
    restore_checkpoint(
        &data.storage,
        "workspace-restore-conversation",
        &first_checkpoint,
    )
    .expect("restore the selected workspace checkpoint");
    assert_eq!(
        fs::read_to_string(root.0.join("shared.txt")).expect("read restored file"),
        "first\n"
    );
}
#[test]
fn checkpoint_persistence_failure_rolls_back_files_and_tracking() {
    let root = TestDirectory::new("workspace-restore-persistence-failure");
    let data = TestStorage::new("workspace-restore-persistence-failure-data");
    let conversation_id = "workspace-restore-persistence-failure-conversation";
    let first_checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        conversation_id,
        "write_file",
        &[("shared.txt", Some(b"before\n"), Some(b"first\n"))],
    );
    record_file_checkpoint(
        &root.0,
        &data.storage,
        conversation_id,
        "write_file",
        &[("shared.txt", Some(b"first\n"), Some(b"later\n"))],
    );
    data.storage            .connect()            .expect("connect test storage")            .execute_batch(                "CREATE TRIGGER reject_restore_checkpoint                 BEFORE INSERT ON conversation_file_checkpoints                 WHEN NEW.tool_name = 'restore_checkpoint'                 BEGIN                    SELECT RAISE(ABORT, 'simulated checkpoint persistence failure');                 END;",            )            .expect("install checkpoint persistence failure");
    assert_eq!(
        restore_checkpoint(&data.storage, conversation_id, &first_checkpoint)
            .expect_err("report checkpoint persistence failure"),
        CheckpointRestoreError::Unavailable
    );
    assert_eq!(
        fs::read(root.0.join("shared.txt")).expect("read rolled-back file"),
        b"later\n",
        "a persistence failure must restore the file contents from before the restore"
    );
    assert_eq!(
        super::checkpoints::list_checkpoints(&data.storage, conversation_id)
            .expect("list persisted checkpoints")
            .len(),
        2,
        "a failed restore must not leave a restore checkpoint"
    );
    let change = list_changes(&data.storage, conversation_id)
        .expect("list restored tracking state")
        .into_iter()
        .find(|change| change.path == "shared.txt")
        .expect("find the tracked file");
    revert_change(&data.storage, conversation_id, &change.id)
        .expect("tracking must match the rolled-back file contents");
    assert_eq!(
        fs::read(root.0.join("shared.txt")).expect("read file after tracked revert"),
        b"before\n",
        "the repaired tracking entry must remain usable after rollback"
    );
}
#[test]
fn restoring_an_already_current_workspace_checkpoint_is_a_successful_noop() {
    let root = TestDirectory::new("workspace-restore-noop-workspace");
    let data = TestStorage::new("workspace-restore-noop-data");
    let checkpoint = record_file_checkpoint(
        &root.0,
        &data.storage,
        "workspace-restore-noop-conversation",
        "write_file",
        &[("shared.txt", Some(b"before\n"), Some(b"current\n"))],
    );
    let summaries = restore_checkpoint(
        &data.storage,
        "workspace-restore-noop-conversation",
        &checkpoint,
    )
    .expect("restoring the current checkpoint is a successful no-op");
    assert!(summaries.is_empty());
    assert_eq!(
        fs::read_to_string(root.0.join("shared.txt")).expect("read unchanged file"),
        "current\n"
    );
}
#[test]
fn tracks_text_changes_and_reverts_to_the_first_version() {
    let root = TestDirectory::new("workspace");
    let data = TestStorage::new("data");
    data.add_conversation("conversation-1");
    fs::write(root.0.join("note.txt"), "first\nsecond\n").expect("write initial file");
    let before = capture_file(&root.0, "note.txt")
        .expect("capture before")
        .expect("initial file exists");
    fs::write(root.0.join("note.txt"), "first\nchanged\nthird\n").expect("write changed file");
    let after = capture_file(&root.0, "note.txt")
        .expect("capture after")
        .expect("changed file exists");
    let root_identity = workspace_root_identity(&root.0).expect("capture workspace identity");
    let summaries = record_deltas(
        &data.storage,
        "conversation-1",
        &root.0,
        vec![super::FileChangeDelta {
            root_identity,
            path: "note.txt".to_owned(),
            before: Some(before),
            after: Some(after),
        }],
    )
    .expect("save change");
    assert_eq!(summaries[0].added_lines, Some(2));
    assert_eq!(summaries[0].removed_lines, Some(1));
    assert!(summaries[0].can_revert);
    assert!(
        !data
            ._directory
            .0
            .join(LEGACY_FILE_CHANGE_DIRECTORY)
            .exists()
    );
    let storage_directory = data._directory.0.clone();
    let TestStorage {
        storage,
        _directory,
    } = data;
    drop(storage);
    let reopened = AppStorage::open_at(storage_directory).expect("reopen persistent test storage");
    assert_eq!(
        list_changes(&reopened, "conversation-1")
            .expect("load persisted change history")
            .len(),
        1
    );
    let diff = read_diff(&reopened, "conversation-1", &summaries[0].id).expect("read diff");
    assert!(
        diff["diff"]
            .as_str()
            .is_some_and(|text| text.contains("+third"))
    );
    let changes =
        revert_change(&reopened, "conversation-1", &summaries[0].id).expect("revert change");
    assert_eq!(changes[0].status, "reverted");
    assert_eq!(
        fs::read_to_string(root.0.join("note.txt")).expect("read restored file"),
        "first\nsecond\n"
    );
    drop(reopened);
    drop(_directory);
}
#[test]
fn tracks_created_and_deleted_files_and_detects_external_edits_before_revert() {
    let root = TestDirectory::new("workspace");
    let data = TestStorage::new("data");
    let created_id = record_file_change(
        &root.0,
        &data.storage,
        "conversation-2",
        "new.txt",
        None,
        Some(b"created\n"),
    );
    revert_change(&data.storage, "conversation-2", &created_id).expect("remove created file");
    assert!(!root.0.join("new.txt").exists());
    let deleted_id = record_file_change(
        &root.0,
        &data.storage,
        "conversation-2",
        "gone.txt",
        Some(b"restore me\n"),
        None,
    );
    revert_change(&data.storage, "conversation-2", &deleted_id).expect("restore deleted file");
    assert_eq!(
        fs::read_to_string(root.0.join("gone.txt")).expect("read restored file"),
        "restore me\n"
    );
    let conflict_id = record_file_change(
        &root.0,
        &data.storage,
        "conversation-2",
        "gone.txt",
        Some(b"restore me\n"),
        Some(b"assistant edit\n"),
    );
    fs::write(root.0.join("gone.txt"), "user edit\n").expect("write user edit");
    assert!(matches!(
        revert_change(&data.storage, "conversation-2", &conflict_id),
        Err(RevertError::Conflict)
    ));
    assert_eq!(
        fs::read_to_string(root.0.join("gone.txt")).expect("read conflicted file"),
        "user edit\n"
    );
}
#[test]
fn tracked_revert_restores_file_when_manifest_update_fails() {
    let root = TestDirectory::new("tracked-revert-manifest-failure");
    let data = TestStorage::new("tracked-revert-manifest-failure-data");
    let conversation_id = "tracked-revert-manifest-failure-conversation";
    let id = record_file_change(
        &root.0,
        &data.storage,
        conversation_id,
        "note.txt",
        Some(b"before\n"),
        Some(b"after\n"),
    );
    data.storage            .connect()            .expect("connect test storage")            .execute_batch(                "CREATE TRIGGER reject_tracked_revert                 BEFORE UPDATE OF status ON conversation_file_changes                 WHEN NEW.status = 'reverted'                 BEGIN                    SELECT RAISE(ABORT, 'simulated manifest failure');                 END;",            )            .expect("install tracked revert failure");
    assert_eq!(
        revert_change(&data.storage, conversation_id, &id)
            .expect_err("report manifest failure after restoring the file"),
        RevertError::Unavailable
    );
    assert_eq!(
        fs::read(root.0.join("note.txt")).expect("read file after compensated failure"),
        b"after\n"
    );
    let change = list_changes(&data.storage, conversation_id)
        .expect("read manifest after compensated failure")
        .into_iter()
        .find(|change| change.id == id)
        .expect("find tracked change");
    assert_eq!(change.status, "active");
}
#[test]
fn hunk_revert_restores_file_when_manifest_update_fails() {
    let root = TestDirectory::new("hunk-revert-manifest-failure");
    let data = TestStorage::new("hunk-revert-manifest-failure-data");
    let conversation_id = "hunk-revert-manifest-failure-conversation";
    let original = b"first changed\nsecond\nthird changed\n";
    let id = record_file_change(
        &root.0,
        &data.storage,
        conversation_id,
        "note.txt",
        Some(b"first\nsecond\nthird\n"),
        Some(original),
    );
    data.storage            .connect()            .expect("connect test storage")            .execute_batch(                "CREATE TRIGGER reject_hunk_revert                 BEFORE UPDATE OF expected_hash ON conversation_file_changes                 WHEN NEW.expected_hash IS NOT OLD.expected_hash                 BEGIN                    SELECT RAISE(ABORT, 'simulated manifest failure');                 END;",            )            .expect("install hunk revert failure");
    assert_eq!(
        revert_hunk(&data.storage, conversation_id, &id, 0)
            .expect_err("report hunk manifest failure"),
        RevertError::Unavailable
    );
    assert_eq!(
        fs::read(root.0.join("note.txt")).expect("read file after compensated failure"),
        original
    );
    let change = list_changes(&data.storage, conversation_id)
        .expect("read manifest after compensated failure")
        .into_iter()
        .find(|change| change.id == id)
        .expect("find tracked change");
    assert_eq!(change.status, "active");
}
#[test]
fn revert_all_reports_a_failed_revert_instead_of_suppressing_it() {
    let root = TestDirectory::new("revert-all-manifest-failure");
    let data = TestStorage::new("revert-all-manifest-failure-data");
    let conversation_id = "revert-all-manifest-failure-conversation";
    let id = record_file_change(
        &root.0,
        &data.storage,
        conversation_id,
        "note.txt",
        Some(b"before\n"),
        Some(b"after\n"),
    );
    data.storage            .connect()            .expect("connect test storage")            .execute_batch(                "CREATE TRIGGER reject_revert_all                 BEFORE UPDATE OF status ON conversation_file_changes                 WHEN NEW.status = 'reverted'                 BEGIN                    SELECT RAISE(ABORT, 'simulated manifest failure');                 END;",            )            .expect("install revert-all failure");
    assert_eq!(
        revert_all_changes(&data.storage, conversation_id)
            .expect_err("surface the individual revert failure"),
        RevertError::Unavailable
    );
    assert_eq!(
        fs::read(root.0.join("note.txt")).expect("read file after compensated failure"),
        b"after\n"
    );
    let change = list_changes(&data.storage, conversation_id)
        .expect("read manifest after compensated failure")
        .into_iter()
        .find(|change| change.id == id)
        .expect("find tracked change");
    assert_eq!(change.status, "active");
}

#[test]
fn revert_all_with_no_eligible_changes_returns_the_current_list() {
    let data = TestStorage::new("revert-all-no-changes-data");
    let conversation_id = "revert-all-no-changes-conversation";
    data.add_conversation(conversation_id);

    assert!(
        revert_all_changes(&data.storage, conversation_id)
            .expect("list changes without eligible reverts")
            .is_empty()
    );
}
#[test]
fn new_file_changes_rebase_identityless_history_to_the_captured_before_state() {
    let root = TestDirectory::new("identityless-change-history");
    let data = TestStorage::new("identityless-change-history-data");
    let conversation_id = "identityless-change-history-conversation";
    let id = record_file_change(
        &root.0,
        &data.storage,
        conversation_id,
        "note.txt",
        Some(b"old baseline\n"),
        Some(b"first result\n"),
    );
    data.storage            .connect()            .expect("connect test storage")            .execute(                "UPDATE conversation_file_changes SET root_identity = NULL                 WHERE conversation_id = ?1 AND id = ?2",                rusqlite::params![conversation_id, id],            )            .expect("simulate identity-less legacy history");
    let updated_id = record_file_change(
        &root.0,
        &data.storage,
        conversation_id,
        "note.txt",
        Some(b"first result\n"),
        Some(b"second result\n"),
    );
    assert_eq!(updated_id, id);
    assert!(
        list_changes(&data.storage, conversation_id)
            .expect("list rebased change")
            .first()
            .is_some_and(|change| change.can_revert)
    );
    revert_change(&data.storage, conversation_id, &updated_id)
        .expect("revert against the new captured baseline");
    assert_eq!(
        fs::read(root.0.join("note.txt")).expect("read rebased file"),
        b"first result\n"
    );
}
#[test]
fn reverting_a_tracked_change_rejects_a_replaced_root_with_matching_contents() {
    let root = TestDirectory::new("replaced-root-change");
    let data = TestStorage::new("replaced-root-change-data");
    let id = record_file_change(
        &root.0,
        &data.storage,
        "replaced-root-change-conversation",
        "note.txt",
        Some(b"before\n"),
        Some(b"after\n"),
    );
    let moved_root = root.0.with_extension("original");
    fs::rename(&root.0, &moved_root).expect("move original workspace root");
    fs::create_dir_all(&root.0).expect("create replacement workspace root");
    fs::write(root.0.join("note.txt"), b"after\n").expect("seed matching replacement file");
    assert!(matches!(
        revert_change(&data.storage, "replaced-root-change-conversation", &id),
        Err(RevertError::Conflict)
    ));
    assert_eq!(
        fs::read(root.0.join("note.txt")).expect("read replacement file"),
        b"after\n"
    );
    fs::remove_dir_all(moved_root).expect("remove moved original workspace");
}
#[test]
fn workspace_snapshot_detects_created_modified_and_deleted_paths() {
    let root = TestDirectory::new("workspace");
    fs::create_dir(root.0.join("target")).expect("create ignored directory");
    fs::write(root.0.join("target/ignored.txt"), "ignored").expect("write ignored file");
    fs::write(root.0.join("keep.txt"), "before").expect("write original");
    fs::write(root.0.join("modify.txt"), "before").expect("write modified file");
    let before = capture_workspace(&root.0).expect("capture before");
    fs::write(root.0.join("modify.txt"), "after").expect("modify file");
    fs::write(root.0.join("new.txt"), "new").expect("create file");
    fs::remove_file(root.0.join("keep.txt")).expect("delete file");
    let after = capture_workspace(&root.0).expect("capture after");
    let deltas = workspace_deltas(&before, &after).expect("compare workspaces");
    assert_eq!(deltas.len(), 3);
    assert_eq!(deltas[0].path, "keep.txt");
    assert_eq!(deltas[1].path, "modify.txt");
    assert_eq!(deltas[2].path, "new.txt");
}
#[test]
fn file_tool_writes_wait_for_checkpoint_lock_while_reads_remain_available() {
    use std::{sync::mpsc, thread, time::Duration};

    let root = TestDirectory::new("file-tool-change-lock");
    fs::write(root.0.join("note.txt"), "before").expect("create workspace file");
    let guard = super::lock_file_changes().expect("hold file-change lock");
    let read = crate::tools::read_file(&root.0, "note.txt", 1, 1)
        .expect("read-only file access remains available");
    assert_eq!(read["lines"][0]["text"], "before");

    let (thread_id_sender, thread_id_receiver) = mpsc::channel();
    let (start_sender, start_receiver) = mpsc::channel();
    let (waiting_sender, waiting_receiver) = mpsc::sync_channel(1);
    let worker_root = root.0.clone();
    let worker = thread::spawn(move || {
        thread_id_sender
            .send(thread::current().id())
            .expect("report writer thread id");
        start_receiver
            .recv()
            .expect("wait for lock-wait observation setup");
        crate::tools::write_file(&worker_root, "note.txt", "after")
    });
    let worker_thread_id = thread_id_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("writer thread starts");
    super::set_file_change_lock_wait_hook(worker_thread_id, waiting_sender);
    start_sender.send(()).expect("start the writer");
    waiting_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("writer observes the held file-change lock");
    let completed_while_locked = worker.is_finished();
    drop(guard);
    worker
        .join()
        .expect("writer thread exits")
        .expect("write succeeds after the lock is released");
    assert!(
        !completed_while_locked,
        "a file-tool mutation must wait while checkpoint or revert owns the lock"
    );
    assert_eq!(
        fs::read_to_string(root.0.join("note.txt")).expect("read updated file"),
        "after"
    );
}
#[test]
fn file_tool_edits_wait_for_checkpoint_lock() {
    use std::{sync::mpsc, thread, time::Duration};

    let root = TestDirectory::new("file-tool-edit-change-lock");
    fs::write(root.0.join("note.txt"), "before").expect("create workspace file");
    let guard = super::lock_file_changes().expect("hold file-change lock");

    let (thread_id_sender, thread_id_receiver) = mpsc::channel();
    let (start_sender, start_receiver) = mpsc::channel();
    let (waiting_sender, waiting_receiver) = mpsc::sync_channel(1);
    let worker_root = root.0.clone();
    let worker = thread::spawn(move || {
        thread_id_sender
            .send(thread::current().id())
            .expect("report editor thread id");
        start_receiver
            .recv()
            .expect("wait for lock-wait observation setup");
        crate::tools::edit_file(&worker_root, "note.txt", "before", "after")
    });
    let worker_thread_id = thread_id_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("editor thread starts");
    super::set_file_change_lock_wait_hook(worker_thread_id, waiting_sender);
    start_sender.send(()).expect("start the editor");
    waiting_receiver
        .recv_timeout(Duration::from_secs(1))
        .expect("editor observes the held file-change lock");
    let completed_while_locked = worker.is_finished();
    drop(guard);
    worker
        .join()
        .expect("editor thread exits")
        .expect("edit succeeds after the lock is released");

    assert!(
        !completed_while_locked,
        "a file-tool edit must wait while checkpoint or revert owns the lock"
    );
    assert_eq!(
        fs::read_to_string(root.0.join("note.txt")).expect("read edited file"),
        "after"
    );
}
#[test]
fn workspace_snapshot_rejects_a_replacement_root_at_the_same_path() {
    let parent = TestDirectory::new("workspace-root-replacement");
    let root = parent.0.join("workspace");
    let displaced_root = parent.0.join("workspace-displaced");
    fs::create_dir(&root).expect("create workspace root");
    fs::write(root.join("shared.txt"), "same bytes\n").expect("write original file");
    let before = capture_workspace(&root).expect("capture original root");
    fs::rename(&root, &displaced_root).expect("move original root");
    fs::create_dir(&root).expect("create replacement root at the same path");
    fs::write(root.join("shared.txt"), "same bytes\n").expect("seed replacement file");
    let after = capture_workspace(&root).expect("capture replacement root");
    assert!(workspace_deltas(&before, &after).is_err());
}
#[test]
fn file_change_tracking_rejects_a_replaced_root_before_commit() {
    let parent = TestDirectory::new("workspace-change-root-swap");
    let root = parent.0.join("workspace");
    let displaced_root = parent.0.join("workspace-displaced");
    let storage = TestStorage::new("workspace-change-root-swap-data");
    storage.add_conversation("workspace-change-root-swap");
    fs::create_dir(&root).expect("create workspace root");
    fs::write(root.join("shared.txt"), "before\n").expect("write initial file");
    let before = capture_workspace(&root).expect("capture before");
    fs::write(root.join("shared.txt"), "after\n").expect("write changed file");
    let after = capture_workspace(&root).expect("capture after");
    let deltas = workspace_deltas(&before, &after).expect("compare workspace snapshots");
    fs::rename(&root, &displaced_root).expect("move captured workspace root");
    fs::create_dir(&root).expect("create replacement root");
    fs::write(root.join("shared.txt"), "after\n").expect("seed replacement file");
    assert!(
        record_deltas(
            &storage.storage,
            "workspace-change-root-swap",
            &root,
            deltas
        )
        .is_err()
    );
    assert!(
        list_changes(&storage.storage, "workspace-change-root-swap")
            .expect("read unchanged tracking")
            .is_empty()
    );
}
#[test]
fn checkpoint_persistence_rejects_a_replaced_root_before_commit() {
    let parent = TestDirectory::new("workspace-checkpoint-root-swap");
    let root = parent.0.join("workspace");
    let displaced_root = parent.0.join("workspace-displaced");
    let storage = TestStorage::new("workspace-checkpoint-root-swap-data");
    storage.add_conversation("workspace-checkpoint-root-swap");
    fs::create_dir(&root).expect("create workspace root");
    fs::write(root.join("shared.txt"), "before\n").expect("write initial file");
    let before = capture_workspace(&root).expect("capture before");
    fs::write(root.join("shared.txt"), "after\n").expect("write changed file");
    let after = capture_workspace(&root).expect("capture after");
    let deltas = workspace_deltas(&before, &after).expect("compare workspace snapshots");
    fs::rename(&root, &displaced_root).expect("move captured workspace root");
    fs::create_dir(&root).expect("create replacement root");
    fs::write(root.join("shared.txt"), "after\n").expect("seed replacement file");
    assert!(
        persist_checkpoint(
            &storage.storage,
            "workspace-checkpoint-root-swap",
            "bash",
            &[(root, deltas)]
        )
        .is_err()
    );
    assert!(
        super::checkpoints::list_checkpoints(&storage.storage, "workspace-checkpoint-root-swap")
            .expect("read unchanged checkpoints")
            .is_empty()
    );
}
#[test]
fn restore_root_check_rejects_a_different_directory_object() {
    let parent = TestDirectory::new("workspace-root-identity");
    let expected_root = parent.0.join("expected");
    let other_root = parent.0.join("other");
    fs::create_dir(&expected_root).expect("create expected root");
    fs::create_dir(&other_root).expect("create other root");
    let (expected_directory, expected_canonical_root) =
        open_workspace_directory(&expected_root).expect("open expected root");
    assert!(
        !workspace_directory_matches_path(&other_root, &expected_directory)
            .expect("compare directory identity")
    );
    assert!(
        workspace_directory_matches_path(&expected_canonical_root, &expected_directory)
            .expect("match original directory identity")
    );
}
#[test]
fn file_growth_after_metadata_read_does_not_retain_unbounded_content() {
    let root = TestDirectory::new("workspace");
    let path = root.0.join("growing.txt");
    let content = vec![b'x'; 2 * 1024 * 1024];
    fs::write(&path, &content).expect("write growing file");
    let directory = cap_std::fs::Dir::open_ambient_dir(&root.0, cap_std::ambient_authority())
        .expect("open workspace capability");
    let (file, _) = open_workspace_file(&directory, std::path::Path::new("growing.txt"))
        .expect("open growing file through workspace root");
    let snapshot = snapshot_file(file, 1).expect("snapshot growing file");
    assert_eq!(snapshot.hash, super::sha256(&content));
    assert!(snapshot.content.is_none());
}
#[test]
fn direct_file_capture_rejects_files_over_the_snapshot_read_limit() {
    let root = TestDirectory::new("oversized-snapshot");
    let path = root.0.join("large.bin");
    fs::File::create(&path)
        .expect("create sparse file")
        .set_len(super::MAX_WORKSPACE_BYTES + 1)
        .expect("set sparse file length");
    assert!(matches!(
        capture_file(&root.0, "large.bin"),
        Err(super::TrackingError::WorkspaceTooLarge)
    ));
}
#[cfg(unix)]
fn create_directory_symlink(
    target: &std::path::Path,
    link: &std::path::Path,
) -> std::io::Result<()> {
    std::os::unix::fs::symlink(target, link)
}
#[cfg(windows)]
fn create_directory_symlink(
    target: &std::path::Path,
    link: &std::path::Path,
) -> std::io::Result<()> {
    std::os::windows::fs::symlink_dir(target, link)
}
#[cfg(any(unix, windows))]
#[test]
fn workspace_capability_operations_reject_external_symlinks() {
    let workspace = TestDirectory::new("symlink-workspace");
    let external = TestDirectory::new("symlink-external");
    let external_file = external.0.join("secret.txt");
    fs::write(&external_file, b"keep private").expect("write external file");
    create_directory_symlink(&external.0, &workspace.0.join("outside"))
        .expect("create external directory symlink");
    assert!(capture_file(&workspace.0, "outside/secret.txt").is_err());
    let root_directory =
        cap_std::fs::Dir::open_ambient_dir(&workspace.0, cap_std::ambient_authority())
            .expect("open workspace capability");
    assert!(
        crate::filesystem::read_bounded_file_from_directory(
            &root_directory,
            std::path::Path::new("outside/secret.txt"),
            64,
        )
        .is_err()
    );
    assert!(
        super::atomic_write_workspace_file_from_directory(
            &root_directory,
            std::path::Path::new("outside/secret.txt"),
            b"replace",
        )
        .is_err()
    );
    assert_eq!(
        fs::read(&external_file).expect("read external file"),
        b"keep private"
    );
}
#[cfg(any(unix, windows))]
#[test]
fn workspace_operations_reject_a_directory_link_added_after_preflight() {
    let workspace = TestDirectory::new("symlink-race-workspace");
    let external = TestDirectory::new("symlink-race-external");
    let external_file = external.0.join("secret.txt");
    fs::write(&external_file, b"keep private").expect("write external file");
    let workspace_link = workspace.0.join("outside");
    fs::create_dir(&workspace_link).expect("create initial workspace directory");
    let root_directory =
        cap_std::fs::Dir::open_ambient_dir(&workspace.0, cap_std::ambient_authority())
            .expect("open workspace capability");
    let relative = std::path::Path::new("outside/secret.txt");

    ensure_no_symlink_path_from_directory(&root_directory, relative)
        .expect("ordinary directory passes the initial path check");
    fs::remove_dir(&workspace_link).expect("remove checked directory");
    create_directory_symlink(&external.0, &workspace_link)
        .expect("replace checked directory with external symlink");

    assert!(capture_file_from_directory(&root_directory, relative).is_err());
    assert!(
        atomic_write_workspace_file_from_directory(&root_directory, relative, b"replace").is_err()
    );
    assert!(remove_workspace_file_from_directory(&root_directory, relative).is_err());
    assert_eq!(
        fs::read(&external_file).expect("read external file after rejected operations"),
        b"keep private"
    );
}
#[cfg(windows)]
#[test]
fn workspace_directory_handles_block_rename_until_released() {
    let workspace = TestDirectory::new("directory-rename-lock-workspace");
    let nested_path = workspace.0.join("nested");
    let file_path = nested_path.join("inner").join("note.txt");
    fs::create_dir_all(file_path.parent().expect("file parent"))
        .expect("create nested directories");
    fs::write(&file_path, b"workspace content").expect("write nested file");
    let moved_root = workspace.0.with_extension("moved-root");
    let moved_nested = workspace.0.join("nested-moved");

    let (root_directory, _) =
        open_workspace_directory(&workspace.0).expect("open workspace capability");
    assert!(
        fs::rename(&workspace.0, &moved_root).is_err(),
        "an open root capability must prevent moving the workspace root"
    );

    let mut read_options = cap_std::fs::OpenOptions::new();
    read_options.read(true);
    let (opened_file, _) = crate::filesystem::open_regular_file_from_directory(
        &root_directory,
        std::path::Path::new("nested/inner/note.txt"),
        &read_options,
    )
    .expect("open file through nested workspace capabilities");
    assert!(
        fs::rename(&nested_path, &moved_nested).is_err(),
        "an open file must keep every nested parent capability alive"
    );

    drop(opened_file);
    fs::rename(&nested_path, &moved_nested)
        .expect("nested directory can move after its capability is released");
    fs::rename(&moved_nested, &nested_path).expect("restore nested directory path");

    drop(root_directory);
    fs::rename(&workspace.0, &moved_root)
        .expect("workspace root can move after its capability is released");
    fs::rename(&moved_root, &workspace.0).expect("restore workspace root path");
}
#[cfg(windows)]
#[test]
fn tracked_revert_target_holds_existing_nested_parent_through_write() {
    let workspace = TestDirectory::new("tracked-revert-parent-lock");
    let nested = workspace.0.join("nested");
    fs::create_dir_all(&nested).expect("create nested workspace directory");
    fs::write(nested.join("note.txt"), b"current").expect("write workspace file");
    let root_identity = workspace_root_identity(&workspace.0)
        .expect("capture workspace root identity")
        .storage_value()
        .expect("serialize workspace root identity");
    let entry = ManifestEntry {
        id: "a".repeat(64),
        root: workspace.0.to_string_lossy().into_owned(),
        root_identity: Some(root_identity),
        path: "nested/note.txt".to_owned(),
        baseline_exists: true,
        baseline_hash: Some(super::sha256(b"baseline")),
        baseline_blob_hash: None,
        expected_exists: true,
        expected_hash: Some(super::sha256(b"current")),
        added_lines: None,
        removed_lines: None,
        is_binary: false,
        diff_available: true,
        status: "active".to_owned(),
    };
    let mut target = super::RevertFileTarget::open(&entry).expect("open tracked revert target");
    let moved = workspace.0.join("nested-moved");

    assert!(target.matches_entry(&entry).expect("verify current file"));
    assert!(fs::rename(&nested, &moved).is_err());
    target
        .write_file(b"baseline")
        .expect("write through pinned parent capability");
    assert_eq!(
        fs::read(nested.join("note.txt")).expect("read reverted file"),
        b"baseline"
    );

    drop(target);
    fs::rename(&nested, &moved).expect("nested directory is movable after revert target drops");
    fs::rename(&moved, &nested).expect("restore nested workspace directory");
}
#[cfg(windows)]
#[test]
fn tracked_revert_target_retains_a_created_nested_parent() {
    let workspace = TestDirectory::new("tracked-revert-created-parent");
    let root_identity = workspace_root_identity(&workspace.0)
        .expect("capture workspace root identity")
        .storage_value()
        .expect("serialize workspace root identity");
    let entry = ManifestEntry {
        id: "b".repeat(64),
        root: workspace.0.to_string_lossy().into_owned(),
        root_identity: Some(root_identity),
        path: "nested/note.txt".to_owned(),
        baseline_exists: false,
        baseline_hash: None,
        baseline_blob_hash: None,
        expected_exists: false,
        expected_hash: None,
        added_lines: None,
        removed_lines: None,
        is_binary: false,
        diff_available: true,
        status: "active".to_owned(),
    };
    let mut target = super::RevertFileTarget::open(&entry).expect("open tracked revert target");
    let nested = workspace.0.join("nested");
    let moved = workspace.0.join("nested-moved");

    assert!(target.matches_entry(&entry).expect("verify missing file"));
    target
        .write_file(b"created")
        .expect("create file through retained parent capability");
    assert_eq!(
        target
            .capture()
            .expect("capture created file")
            .as_ref()
            .and_then(|snapshot| snapshot.content.as_deref()),
        Some(b"created".as_slice())
    );
    assert!(fs::rename(&nested, &moved).is_err());

    drop(target);
    fs::rename(&nested, &moved).expect("created parent is movable after target drops");
}
#[cfg(any(unix, windows))]
#[test]
fn workspace_root_symlink_is_rejected() {
    let workspace = TestDirectory::new("symlink-root-workspace");
    let external = TestDirectory::new("symlink-root-external");
    let external_file = external.0.join("secret.txt");
    fs::write(&external_file, b"keep private").expect("write external file");
    let workspace_link = workspace.0.join("linked-root");
    create_directory_symlink(&external.0, &workspace_link).expect("create workspace-root symlink");

    assert!(open_workspace_directory(&workspace_link).is_err());
    assert!(capture_workspace(&workspace_link).is_err());
    assert_eq!(
        fs::read(&external_file).expect("read external file after rejected root"),
        b"keep private"
    );
}
#[test]
fn bounded_file_read_rejects_growth_past_the_diff_limit() {
    let root = TestDirectory::new("workspace");
    let path = root.0.join("large.txt");
    fs::write(&path, b"12345").expect("write file");
    let bytes = read_bounded_file(&path, 4).expect("read bounded file");
    assert!(bytes.is_none());
}
#[test]
fn a_reverted_file_can_be_changed_again_from_the_original_baseline() {
    let root = TestDirectory::new("workspace");
    let data = TestStorage::new("data");
    let id = record_file_change(
        &root.0,
        &data.storage,
        "conversation-3",
        "again.txt",
        None,
        Some(b"first\n"),
    );
    revert_change(&data.storage, "conversation-3", &id).expect("revert first edit");
    let second_id = record_file_change(
        &root.0,
        &data.storage,
        "conversation-3",
        "again.txt",
        None,
        Some(b"second\n"),
    );
    assert_eq!(id, second_id);
    let changes = list_changes(&data.storage, "conversation-3").expect("list changes");
    assert_eq!(changes[0].status, "active");
}
#[test]
fn deleting_change_history_removes_only_the_requested_conversation() {
    let root = TestDirectory::new("workspace");
    let data = TestStorage::new("data");
    record_file_change(
        &root.0,
        &data.storage,
        "conversation-to-remove",
        "first.txt",
        None,
        Some(b"first\n"),
    );
    record_file_change(
        &root.0,
        &data.storage,
        "conversation-to-keep",
        "second.txt",
        None,
        Some(b"second\n"),
    );
    delete_conversation_changes(&data.storage, "conversation-to-remove")
        .expect("delete selected conversation changes");
    assert!(
        list_changes(&data.storage, "conversation-to-remove")
            .expect("list deleted conversation changes")
            .is_empty()
    );
    assert_eq!(
        list_changes(&data.storage, "conversation-to-keep")
            .expect("list retained conversation changes")
            .len(),
        1
    );
}
#[test]
fn deleting_all_change_history_is_idempotent() {
    let root = TestDirectory::new("workspace");
    let data = TestStorage::new("data");
    record_file_change(
        &root.0,
        &data.storage,
        "conversation-to-remove",
        "first.txt",
        None,
        Some(b"first\n"),
    );
    delete_all_changes(&data.storage).expect("delete all tracked changes");
    delete_all_changes(&data.storage).expect("repeat deleting all tracked changes");
    assert!(
        list_changes(&data.storage, "conversation-to-remove")
            .expect("list removed changes")
            .is_empty()
    );
}
#[test]
fn legacy_manifests_import_once_and_are_removed_after_commit() {
    let data = TestStorage::new("legacy-import");
    let conversation_id = "legacy-conversation";
    data.add_conversation(conversation_id);
    let workspace = TestDirectory::new("legacy-workspace");
    let original = b"before\n";
    let current = b"after\n";
    fs::write(workspace.0.join("note.txt"), current).expect("write current workspace file");
    let canonical_root = workspace
        .0
        .canonicalize()
        .expect("canonicalize legacy workspace");
    let root_string = canonical_root.to_string_lossy().into_owned();
    let change_id = super::change_id(&canonical_root, "note.txt");
    let legacy_directory = data
        ._directory
        .0
        .join(LEGACY_FILE_CHANGE_DIRECTORY)
        .join(super::sha256(conversation_id.as_bytes()));
    let backup_directory = legacy_directory.join("backups");
    fs::create_dir_all(&backup_directory).expect("create legacy backup directory");
    fs::write(
        backup_directory.join(format!("{change_id}.before")),
        original,
    )
    .expect("write legacy baseline");
    let manifest = Manifest {
        version: MANIFEST_VERSION,
        files: vec![ManifestEntry {
            id: change_id.clone(),
            root: root_string,
            root_identity: None,
            path: "note.txt".to_owned(),
            baseline_exists: true,
            baseline_hash: Some(super::sha256(original)),
            baseline_blob_hash: Some(format!("{change_id}.before")),
            expected_exists: true,
            expected_hash: Some(super::sha256(current)),
            added_lines: Some(1),
            removed_lines: Some(1),
            is_binary: false,
            diff_available: true,
            status: "active".to_owned(),
        }],
    };
    fs::write(
        legacy_directory.join("manifest.json"),
        serde_json::to_vec(&manifest).expect("serialize legacy manifest"),
    )
    .expect("write legacy manifest");
    let changes = list_changes(&data.storage, conversation_id).expect("import legacy data");
    assert_eq!(changes.len(), 1);
    assert!(!changes[0].can_revert);
    assert!(!changes[0].diff_available);
    let diff = read_diff(&data.storage, conversation_id, &change_id)
        .expect("read unavailable legacy diff");
    assert_eq!(diff["available"].as_bool(), Some(false));
    assert_eq!(
        revert_change(&data.storage, conversation_id, &change_id)
            .expect_err("refuse an identity-less legacy revert"),
        RevertError::Conflict
    );
    assert_eq!(
        fs::read(workspace.0.join("note.txt")).expect("read the current workspace file"),
        current
    );
    assert!(!legacy_directory.exists());
    assert!(
        !data
            ._directory
            .0
            .join(LEGACY_FILE_CHANGE_DIRECTORY)
            .exists()
    );
    let baseline_hash = super::sha256(original);
    assert!(
        data._directory
            .0
            .join("attachments")
            .join(SNAPSHOT_DIRECTORY)
            .join(&baseline_hash[..SNAPSHOT_SHARD_HEX_LENGTH])
            .join(format!("{baseline_hash}.blob"))
            .is_file()
    );
    assert_eq!(
        list_changes(&data.storage, conversation_id)
            .expect("list imported data again")
            .len(),
        1
    );
}
#[test]
fn malformed_legacy_manifest_is_kept_for_recovery() {
    let data = TestStorage::new("malformed-legacy");
    let conversation_id = "broken-legacy-conversation";
    data.add_conversation(conversation_id);
    let legacy_directory = data
        ._directory
        .0
        .join(LEGACY_FILE_CHANGE_DIRECTORY)
        .join(super::sha256(conversation_id.as_bytes()));
    fs::create_dir_all(&legacy_directory).expect("create legacy directory");
    fs::write(legacy_directory.join("manifest.json"), b"not valid json")
        .expect("write malformed legacy manifest");
    assert!(list_changes(&data.storage, conversation_id).is_err());
    assert!(legacy_directory.join("manifest.json").is_file());
    let missing_manifest_conversation = "missing-manifest-conversation";
    data.add_conversation(missing_manifest_conversation);
    let orphaned_backup = data
        ._directory
        .0
        .join(LEGACY_FILE_CHANGE_DIRECTORY)
        .join(super::sha256(missing_manifest_conversation.as_bytes()))
        .join("backups")
        .join("retained.before");
    fs::create_dir_all(orphaned_backup.parent().expect("backup parent"))
        .expect("create legacy backup directory");
    fs::write(&orphaned_backup, b"preserve unknown legacy data")
        .expect("write orphaned legacy backup");
    assert!(list_changes(&data.storage, missing_manifest_conversation).is_err());
    assert!(orphaned_backup.is_file());
}
#[test]
fn shared_snapshots_are_reclaimed_only_after_the_last_conversation_is_deleted() {
    let first_workspace = TestDirectory::new("shared-workspace-one");
    let second_workspace = TestDirectory::new("shared-workspace-two");
    let data = TestStorage::new("shared-blobs");
    let first_id = record_file_change(
        &first_workspace.0,
        &data.storage,
        "first-conversation",
        "note.txt",
        Some(b"same baseline\n"),
        Some(b"first edit\n"),
    );
    record_file_change(
        &second_workspace.0,
        &data.storage,
        "second-conversation",
        "note.txt",
        Some(b"same baseline\n"),
        Some(b"second edit\n"),
    );
    let blob_path = data
        ._directory
        .0
        .join("attachments")
        .join(SNAPSHOT_DIRECTORY)
        .join(&super::sha256(b"same baseline\n")[..SNAPSHOT_SHARD_HEX_LENGTH])
        .join(format!("{}.blob", super::sha256(b"same baseline\n")));
    assert!(blob_path.is_file());
    assert_eq!(
        data.storage
            .connect()
            .expect("connect test storage")
            .query_row("SELECT COUNT(*) FROM file_change_blobs", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count deduplicated blobs"),
        1
    );
    delete_conversation_changes(&data.storage, "first-conversation")
        .expect("delete first conversation history");
    assert!(blob_path.is_file());
    assert!(
        list_changes(&data.storage, "second-conversation")
            .expect("read second conversation")
            .iter()
            .any(|change| change.id != first_id)
    );
    delete_conversation_changes(&data.storage, "second-conversation")
        .expect("delete last conversation history");
    assert!(!blob_path.exists());
    assert_eq!(
        data.storage
            .connect()
            .expect("connect test storage")
            .query_row("SELECT COUNT(*) FROM file_change_blobs", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count remaining blobs"),
        0
    );
    assert!(
        !data
            ._directory
            .0
            .join(LEGACY_FILE_CHANGE_DIRECTORY)
            .exists()
    );
}
#[test]
fn stale_writing_blob_cleanup_recovers_a_crash_before_record_commit() {
    let data = TestStorage::new("pending-blob");
    let content = b"unreferenced snapshot";
    let hash = super::sha256(content);
    let path = super::snapshot_blob_path(&data._directory.0, &hash, true)
        .expect("create snapshot path")
        .expect("snapshot path should exist");
    fs::write(&path, content).expect("simulate blob written before interrupted record");
    data.storage            .connect()            .expect("connect test storage")            .execute(                "INSERT INTO file_change_blobs (hash, size_bytes, state, created_at_unix_ms)                 VALUES (?1, ?2, 'writing', 1)",                rusqlite::params![hash, content.len() as i64],            )            .expect("persist interrupted blob marker");
    super::cleanup_pending_snapshot_blobs(&data.storage)
        .expect("reclaim interrupted unreferenced snapshot");
    assert!(!path.exists());
    assert_eq!(
        data.storage
            .connect()
            .expect("connect test storage")
            .query_row("SELECT COUNT(*) FROM file_change_blobs", [], |row| {
                row.get::<_, i64>(0)
            })
            .expect("count cleaned snapshots"),
        0
    );
}
#[test]
fn conversation_foreign_key_cascade_marks_its_last_blob_for_cleanup() {
    let workspace = TestDirectory::new("cascade-workspace");
    let data = TestStorage::new("cascade-delete");
    record_file_change(
        &workspace.0,
        &data.storage,
        "deleted-conversation",
        "note.txt",
        Some(b"baseline"),
        Some(b"edited"),
    );
    let hash = super::sha256(b"baseline");
    let blob_path = super::snapshot_blob_path(&data._directory.0, &hash, false)
        .expect("locate snapshot")
        .expect("snapshot exists");
    data.storage
        .connect()
        .expect("connect test storage")
        .execute(
            "DELETE FROM conversations WHERE id = 'deleted-conversation'",
            [],
        )
        .expect("delete conversation with cascading change history");
    assert!(blob_path.is_file());
    assert_eq!(
        data.storage
            .connect()
            .expect("connect test storage")
            .query_row(
                "SELECT state FROM file_change_blobs WHERE hash = ?1",
                [&hash],
                |row| row.get::<_, String>(0),
            )
            .expect("read blob cleanup state"),
        "pending"
    );
    super::cleanup_pending_snapshot_blobs(&data.storage).expect("clean orphaned cascade snapshot");
    assert!(!blob_path.exists());
}
#[test]
fn approve_change_and_revert_hunk_update_tracked_state() {
    let workspace = TestDirectory::new("approve-and-hunk-workspace");
    let data = TestStorage::new("approve-and-hunk-data");
    let baseline = b"line1\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10\n";
    let edited = b"line1-mod\nline2\nline3\nline4\nline5\nline6\nline7\nline8\nline9\nline10-mod\n";
    std::fs::write(workspace.0.join("multi.txt"), edited).expect("write edited file");
    let id = record_file_change(
        &workspace.0,
        &data.storage,
        "hunk-conversation",
        "multi.txt",
        Some(baseline),
        Some(edited),
    ); // Revert only the first hunk (hunk 0), preserving the second hunk at line 10.
    let after_hunk =
        super::revert_hunk(&data.storage, "hunk-conversation", &id, 0).expect("revert first hunk");
    assert_eq!(after_hunk[0].status, "active");
    assert!(!after_hunk[0].approved);
    let on_disk =
        std::fs::read_to_string(workspace.0.join("multi.txt")).expect("read updated file");
    assert!(on_disk.starts_with("line1\n"));
    assert!(on_disk.ends_with("line10-mod\n")); // Approve the remaining change.
    let after_approve = super::approve_change(&data.storage, "hunk-conversation", &id)
        .expect("approve remaining change");
    assert!(after_approve[0].approved);
    assert!(!after_approve[0].can_revert);
}
