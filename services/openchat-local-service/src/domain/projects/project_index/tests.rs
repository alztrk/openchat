use super::*;
use crate::storage::AppStorage;
use std::path::PathBuf;
use uuid::Uuid;

struct TestDirectory(PathBuf);

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove test directory");
    }
}

struct TestDirectories {
    storage: AppStorage,
    project: PathBuf,
    root: PathBuf,
    _directory: TestDirectory,
}

impl TestDirectories {
    fn new() -> Self {
        let root = TestDirectory(
            std::env::temp_dir().join(format!("openchat-project-index-{}", Uuid::new_v4())),
        );
        let project = root.0.join("project");
        fs::create_dir_all(&project).expect("create project folder");
        let storage =
            AppStorage::open_at(root.0.join("storage")).expect("open primary storage for index");
        let connection = storage.connect().expect("connect to primary storage");
        connection
            .execute_batch(
                "CREATE TABLE projects (
                         id TEXT PRIMARY KEY NOT NULL,
                         name TEXT NOT NULL,
                         folder_path TEXT NOT NULL,
                         created_at INTEGER NOT NULL,
                         updated_at INTEGER NOT NULL
                     );
                     CREATE TABLE project_workspace_roots (
                         root_id TEXT PRIMARY KEY NOT NULL,
                         project_id TEXT NOT NULL REFERENCES projects(id),
                         root_path TEXT NOT NULL,
                         position INTEGER NOT NULL,
                         created_at INTEGER NOT NULL,
                         UNIQUE(project_id, root_path),
                         UNIQUE(project_id, position)
                     );
                     CREATE TABLE conversations (id TEXT PRIMARY KEY NOT NULL);
                     CREATE TABLE messages (
                         id TEXT NOT NULL,
                         conversation_id TEXT NOT NULL REFERENCES conversations(id),
                         role TEXT NOT NULL,
                         content TEXT NOT NULL,
                         status TEXT NOT NULL,
                         created_at INTEGER,
                         output_tokens INTEGER,
                         tool_activities TEXT NOT NULL DEFAULT '[]',
                         PRIMARY KEY (conversation_id, id)
                     );",
            )
            .expect("create the host conversation schema");
        connection
            .execute(
                "INSERT INTO projects (id, name, folder_path, created_at, updated_at)
                     VALUES ('project-1', 'Test project', ?1, 1, 1)",
                [fs::canonicalize(&project)
                    .expect("canonicalize test project")
                    .to_string_lossy()
                    .as_ref()],
            )
            .expect("create the project record");
        connection
            .execute(
                "INSERT INTO project_workspace_roots
                        (root_id, project_id, root_path, position, created_at)
                     VALUES ('primary-root', 'project-1', ?1, 0, 1)",
                [fs::canonicalize(&project)
                    .expect("canonicalize primary workspace root")
                    .to_string_lossy()
                    .as_ref()],
            )
            .expect("create the primary workspace root");
        drop(connection);
        storage
            .initialize_backend_schema()
            .expect("initialize the current backend schema");
        Self {
            storage,
            project,
            root: root.0.clone(),
            _directory: root,
        }
    }

    fn connect(&self) -> StorageConnection<'_> {
        self.storage.connect().expect("connect to primary storage")
    }
}

#[test]
fn incremental_sync_respects_gitignore_and_reuses_unchanged_files() {
    let directories = TestDirectories::new();
    fs::create_dir_all(directories.project.join(".git/info"))
        .expect("create local Git exclude directory");
    fs::write(
        directories.project.join(".git/info/exclude"),
        "locally-excluded.txt\n",
    )
    .expect("write local Git exclude rule");
    fs::write(
        directories.project.join(".gitignore"),
        "ignored.txt\nignored-dir/\n",
    )
    .expect("write ignore rules");
    fs::write(
        directories.project.join("src.rs"),
        "fn greeting() { println!(\"hello\"); }",
    )
    .expect("write source file");
    fs::write(
        directories.project.join("ignored.txt"),
        "private searchable marker",
    )
    .expect("write ignored file");
    fs::write(
        directories.project.join("locally-excluded.txt"),
        "private local exclude marker",
    )
    .expect("write locally excluded file");
    fs::create_dir_all(directories.project.join("ignored-dir")).expect("create ignored directory");
    fs::write(
        directories.project.join("ignored-dir/nested.rs"),
        "ignorednestedmarker",
    )
    .expect("write ignored nested file");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable index");
    drop(connection);
    let first = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index project");
    let connection = directories.connect();
    assert_eq!(first.status, "current");
    assert_eq!(first.indexed_files, 1);
    assert_eq!(first.indexed_symbols, 1);
    assert_eq!(
        search_symbols(&connection, "project-1", "greet", 10)
            .expect("search indexed symbols")
            .first()
            .map(|symbol| (
                symbol.name.as_str(),
                symbol.kind.as_str(),
                symbol.start_line
            )),
        Some(("greeting", "function", 1))
    );
    assert!(
        search_current(&connection, "project-1", &directories.project, "greeting")
            .expect("search index")
            .is_some()
    );
    assert!(
        search_current(
            &connection,
            "project-1",
            &directories.project,
            "private searchable marker",
        )
        .expect("search index")
        .is_none()
    );
    assert!(
        search_current(
            &connection,
            "project-1",
            &directories.project,
            "private local exclude marker",
        )
        .expect("search local Git exclude")
        .is_none()
    );

    let metadata = load_file_metadata(&connection, "project-1").expect("load file metadata");
    let indexed_path = fs::canonicalize(directories.project.join("src.rs"))
        .expect("canonicalize indexed source")
        .to_string_lossy()
        .into_owned();
    let original = metadata
        .get(&indexed_path)
        .expect("indexed Rust file")
        .modified_ns;
    drop(connection);
    let second = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("resync unchanged project");
    let connection = directories.connect();
    let metadata = load_file_metadata(&connection, "project-1").expect("reload file metadata");
    assert_eq!(second.status, "current");
    assert_eq!(
        metadata
            .get(&indexed_path)
            .expect("retained Rust file")
            .modified_ns,
        original
    );
}

#[test]
fn text_search_marks_index_stale_when_source_changes() {
    let directories = TestDirectories::new();
    let source_path = directories.project.join("source.rs");
    fs::write(&source_path, "fn stale_source_marker() {}\n")
        .expect("write the indexed project source");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable project index");
    drop(connection);
    sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index the project source");

    fs::write(&source_path, "fn current_source_marker() {}\n").expect("change the project source");
    let connection = directories.connect();
    assert!(
        search_current_hits(&connection, "project-1", "stale_source_marker")
            .expect("search the stale source")
            .is_empty()
    );
    assert_eq!(
        snapshot(&connection, "project-1")
            .expect("read stale index status")
            .status,
        "stale"
    );
}

#[test]
fn symbol_search_marks_index_stale_when_source_is_removed() {
    let directories = TestDirectories::new();
    let source_path = directories.project.join("source.rs");
    fs::write(&source_path, "fn removed_symbol_marker() {}\n")
        .expect("write the indexed project source");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable project index");
    drop(connection);
    sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index the project source");

    fs::remove_file(&source_path).expect("remove the indexed project source");
    let connection = directories.connect();
    assert!(
        search_symbols(&connection, "project-1", "removed_symbol_marker", 10)
            .expect("search the removed source")
            .is_empty()
    );
    assert_eq!(
        snapshot(&connection, "project-1")
            .expect("read stale index status")
            .status,
        "stale"
    );
}

#[test]
fn prompt_symbol_map_contains_current_declarations_without_source_bodies() {
    let directories = TestDirectories::new();
    let source_path = directories.project.join("src/router.rs");
    fs::create_dir_all(source_path.parent().expect("router source parent"))
        .expect("create project source directory");
    fs::write(
            &source_path,
            "pub struct RequestRouter;\nimpl RequestRouter {\n    pub fn dispatch_request(&self) { let context_only_body_marker = 1; }\n}\n",
        )
        .expect("write project source");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable project index");
    drop(connection);
    sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index project source");

    let connection = directories.connect();
    let map = prompt_symbol_map(
        &connection,
        "project-1",
        "How does RequestRouter dispatch_request work?",
    )
    .expect("build project symbol map")
    .expect("matching declarations produce a map");

    assert!(map.contains("router.rs"));
    assert!(map.contains("RequestRouter"));
    assert!(map.contains("dispatch_request"));
    assert!(map.contains("Relevant declarations"));
    assert!(!map.contains("context_only_body_marker"));
    assert!(map.len() <= MAX_SYMBOL_MAP_CONTEXT_BYTES);
}

#[test]
fn prompt_symbol_map_retrieval_fixture_has_full_expected_precision_and_recall() {
    let directories = TestDirectories::new();
    let source_directory = directories.project.join("src");
    fs::create_dir_all(&source_directory).expect("create fixture source directory");
    for (path, source) in [
        (
            "router.rs",
            "pub struct RequestRouter;\nimpl RequestRouter {\n    pub fn dispatch_request(&self) {}\n}\n",
        ),
        (
            "providers.rs",
            "pub fn normalize_provider_id(value: &str) -> String { value.to_owned() }\n",
        ),
        (
            "workspaces.rs",
            "pub fn resolve_workspace_root(path: &str) -> bool { !path.is_empty() }\n",
        ),
        (
            "tools.rs",
            "pub fn evaluate_tool_permission(mode: &str) -> bool { !mode.is_empty() }\npub fn update_recent_usage() {}\n",
        ),
    ] {
        fs::write(source_directory.join(path), source).expect("write fixture source");
    }

    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable project index");
    drop(connection);
    sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index fixture source");

    let tasks: [(&str, &[&str]); 4] = [
        (
            "Trace RequestRouter dispatch_request",
            &["RequestRouter", "dispatch_request"],
        ),
        ("Find normalize_provider_id", &["normalize_provider_id"]),
        (
            "Explain resolve_workspace_root",
            &["resolve_workspace_root"],
        ),
        (
            "Locate evaluate_tool_permission",
            &["evaluate_tool_permission"],
        ),
    ];
    let connection = directories.connect();
    let mut expected_declarations = 0;
    let mut matched_declarations = 0;
    let mut relevant_results = 0;
    let mut returned_results = 0;

    for (task, expected_names) in tasks {
        let map = prompt_symbol_map(&connection, "project-1", task)
            .expect("build task symbol map")
            .expect("task identifiers return indexed declarations");
        let declarations = map
            .lines()
            .filter(|line| line.starts_with("- "))
            .collect::<Vec<_>>();
        expected_declarations += expected_names.len();
        returned_results += declarations.len();
        for expected_name in expected_names {
            let matches = declarations
                .iter()
                .filter(|line| line.contains(expected_name))
                .count();
            assert_eq!(matches, 1, "{task} should retrieve {expected_name}");
            matched_declarations += matches;
        }
        relevant_results += declarations
            .iter()
            .filter(|line| expected_names.iter().any(|name| line.contains(name)))
            .count();
    }

    assert_eq!(matched_declarations, expected_declarations);
    assert_eq!(relevant_results, returned_results);
}

#[test]
fn prompt_symbol_map_is_bounded_and_escapes_path_control_characters() {
    let hits = (0..MAX_SYMBOL_MAP_RESULTS * 4)
        .map(|index| ProjectSymbolHit {
            path: format!("src/route\n{index}.rs"),
            name: format!("Route{index}"),
            kind: "function".to_owned(),
            start_line: index + 1,
            end_line: index + 1,
            signature: Some("x".repeat(512)),
            container: Some("Router".to_owned()),
            relationships: vec!["implements RouteTarget".to_owned()],
        })
        .collect::<Vec<_>>();
    let map = format_symbol_map(&hits).expect("non-empty symbols produce a map");

    assert!(map.len() <= MAX_SYMBOL_MAP_CONTEXT_BYTES);
    assert!(map.lines().filter(|line| line.starts_with("- ")).count() <= MAX_SYMBOL_MAP_RESULTS);
    assert!(!map.contains("route\n"));
    assert!(map.contains("route "));
}

#[test]
fn prompt_symbol_map_rejects_stale_indexed_sources() {
    let directories = TestDirectories::new();
    let source_path = directories.project.join("source.rs");
    fs::write(&source_path, "fn StaleMapMarker() {}\n").expect("write project source");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable project index");
    drop(connection);
    sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index project source");
    fs::write(&source_path, "fn CurrentMapMarker() {}\n").expect("change project source");

    let connection = directories.connect();
    assert!(
        prompt_symbol_map(&connection, "project-1", "StaleMapMarker")
            .expect("check stale project source")
            .is_none()
    );
    assert_eq!(
        snapshot(&connection, "project-1")
            .expect("read stale index state")
            .status,
        "stale"
    );
}

#[test]
fn incremental_sync_reextracts_symbols_when_the_grammar_version_is_stale() {
    let directories = TestDirectories::new();
    let source_path = directories.project.join("model.dart");
    fs::write(&source_path, "class NewlyIndexedModel {}")
        .expect("write a newly supported language file");
    let indexed_path = fs::canonicalize(source_path)
        .expect("canonicalize the indexed source")
        .to_string_lossy()
        .into_owned();
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable project index");
    drop(connection);

    let initial = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index the Dart source file");
    assert_eq!(initial.status, "current");

    let connection = directories.connect();
    connection
        .execute(
            "UPDATE project_index_files SET symbol_version = 0
                 WHERE project_id = ?1 AND path = ?2",
            params!["project-1", indexed_path],
        )
        .expect("mark the symbol extraction version stale");
    connection
        .execute(
            "DELETE FROM project_index_symbols
                 WHERE project_id = ?1 AND path = ?2",
            params!["project-1", indexed_path],
        )
        .expect("remove symbols from the prior extraction version");
    drop(connection);

    let refreshed = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("re-extract symbols from the unchanged Dart source file");
    assert_eq!(refreshed.status, "current");
    assert_eq!(refreshed.indexed_symbols, 1);
}

#[test]
fn indexes_and_searches_all_configured_workspace_roots() {
    let directories = TestDirectories::new();
    let extra_root = directories.root.join("shared-library");
    fs::create_dir_all(&extra_root).expect("create additional workspace root");
    fs::write(
        directories.project.join("primary.rs"),
        "pub fn primarymarker() {}\n",
    )
    .expect("write primary root source");
    fs::write(extra_root.join("shared.rs"), "pub fn sharedmarker() {}\n")
        .expect("write additional root source");
    let extra_root = fs::canonicalize(extra_root).expect("canonicalize additional root");
    let connection = directories.connect();
    connection
        .execute(
            "INSERT INTO project_workspace_roots
                    (root_id, project_id, root_path, position, created_at)
                 VALUES ('shared-library', 'project-1', ?1, 1, 1)",
            [extra_root.to_string_lossy().as_ref()],
        )
        .expect("register additional workspace root");
    drop(connection);
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable index");
    drop(connection);

    let snapshot = sync_roots(
        &directories.storage.database_queue_handle(),
        "project-1",
        &[directories.project.clone(), extra_root.clone()],
    )
    .expect("index all workspace roots");
    assert_eq!(snapshot.status, "current");
    assert_eq!(snapshot.indexed_files, 2);

    let connection = directories.connect();
    for (query, expected_path) in [
        ("primarymarker", directories.project.join("primary.rs")),
        ("sharedmarker", extra_root.join("shared.rs")),
    ] {
        let symbol = search_symbols(&connection, "project-1", query, 10)
            .expect("search workspace symbols")
            .into_iter()
            .next()
            .expect("find symbol in configured root");
        assert_eq!(
            PathBuf::from(symbol.path),
            fs::canonicalize(expected_path).expect("canonicalize indexed result")
        );
    }
}

#[test]
fn unchanged_supported_files_without_declarations_keep_the_symbol_index_marker() {
    let directories = TestDirectories::new();
    fs::write(directories.project.join("empty.rs"), "// no declarations\n")
        .expect("write source without declarations");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable index");
    drop(connection);

    let first = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index source without declarations");
    assert_eq!(first.indexed_symbols, 0);
    let connection = directories.connect();
    let indexed_path = fs::canonicalize(directories.project.join("empty.rs"))
        .expect("canonicalize empty source")
        .to_string_lossy()
        .into_owned();
    assert!(
        load_file_metadata(&connection, "project-1")
            .expect("load indexed file metadata")
            .get(&indexed_path)
            .is_some_and(|metadata| metadata.symbols_indexed)
    );
    drop(connection);

    let second = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("sync unchanged empty-symbol source");
    assert_eq!(second.status, "current");
    assert_eq!(second.indexed_symbols, 0);
}

#[test]
fn reindex_replaces_old_search_rows_after_file_and_root_changes() {
    let directories = TestDirectories::new();
    let source = directories.project.join("src.rs");
    fs::write(&source, "fn oldmarker() {}\n").expect("write initial source file");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable index");
    drop(connection);
    sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("index initial project");

    fs::write(&source, "fn newmarker() {}\n").expect("update source file");
    let connection = directories.connect();
    drop(connection);
    sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("reindex changed file");
    let connection = directories.connect();
    assert!(
        search_current(&connection, "project-1", &directories.project, "oldmarker")
            .expect("search old file content")
            .is_none()
    );
    assert!(
        search_current(&connection, "project-1", &directories.project, "newmarker")
            .expect("search updated file content")
            .is_some()
    );
    assert!(
        search_symbols(&connection, "project-1", "oldmarker", 10)
            .expect("search removed symbol")
            .is_empty()
    );
    assert_eq!(
        search_symbols(&connection, "project-1", "newmarker", 10)
            .expect("search new symbol")
            .first()
            .map(|symbol| symbol.name.as_str()),
        Some("newmarker")
    );
    drop(connection);

    let replacement_root = directories.root.join("replacement-project");
    fs::create_dir_all(&replacement_root).expect("create replacement project root");
    let canonical_replacement_root =
        fs::canonicalize(&replacement_root).expect("canonicalize replacement project root");
    fs::write(
        replacement_root.join("replacement.rs"),
        "fn replacementmarker() {}\n",
    )
    .expect("write replacement project file");
    let connection = directories.connect();
    connection
        .execute(
            "UPDATE projects SET folder_path = ?1 WHERE id = 'project-1'",
            [canonical_replacement_root.to_string_lossy().as_ref()],
        )
        .expect("update the project's primary folder");
    connection
        .execute(
            "UPDATE project_workspace_roots SET root_path = ?1
                 WHERE project_id = 'project-1' AND position = 0",
            [canonical_replacement_root.to_string_lossy().as_ref()],
        )
        .expect("update the primary workspace root");
    drop(connection);
    let switched = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &canonical_replacement_root,
    )
    .expect("reindex replacement project root");
    let connection = directories.connect();
    assert_eq!(switched.status, "current");
    assert_eq!(switched.indexed_files, 1);
    let previous_root_hit =
        search_current(&connection, "project-1", &replacement_root, "newmarker")
            .expect("search previous root content");
    assert!(
        previous_root_hit.is_none(),
        "previous root search returned {previous_root_hit:?}"
    );
    assert!(
        search_current(
            &connection,
            "project-1",
            &replacement_root,
            "replacementmarker"
        )
        .expect("search replacement root content")
        .is_some()
    );
}

#[test]
#[ignore = "manual project index cold and incremental sync benchmark"]
fn benchmark_project_index_sync() {
    let directories = TestDirectories::new();
    let source_directory = directories.project.join("src");
    fs::create_dir_all(&source_directory).expect("create benchmark source directory");
    let file_count = 5_000;
    let mut project_source_bytes = 0usize;
    let mut target_file_bytes = 0usize;
    for file_index in 0..file_count {
        let source = format!("fn symbol_{file_index}() {{ let value = {file_index}; }}\n");
        project_source_bytes = project_source_bytes.saturating_add(source.len());
        if file_index == file_count - 1 {
            target_file_bytes = source.len();
        }
        fs::write(
            source_directory.join(format!("file_{file_index:05}.rs")),
            source,
        )
        .expect("write benchmark project file");
    }

    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable project benchmark index");
    drop(connection);
    let started = std::time::Instant::now();
    let cold = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("build the benchmark project index");
    let cold_elapsed = started.elapsed();
    let started = std::time::Instant::now();
    let incremental = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("refresh the unchanged benchmark project index");
    let incremental_elapsed = started.elapsed();
    let connection = directories.connect();
    let search_started = std::time::Instant::now();
    let text_hits = search_current_hits(&connection, "project-1", "symbol_4999")
        .expect("search benchmark source text");
    let symbol_hits = search_symbols(&connection, "project-1", "symbol_4999", 10)
        .expect("search benchmark symbols");
    let symbol_map = prompt_symbol_map(&connection, "project-1", "symbol_4999")
        .expect("build benchmark symbol map");
    let search_elapsed = search_started.elapsed();
    let search_output = serde_json::json!({
        "matches": text_hits.iter().map(|hit| {
            serde_json::json!({"path": hit.path, "excerpt": hit.excerpt})
        }).collect::<Vec<_>>(),
        "symbols": symbol_hits.iter().map(|hit| {
            serde_json::json!({
                "path": hit.path,
                "name": hit.name,
                "kind": hit.kind,
                "startLine": hit.start_line,
                "endLine": hit.end_line,
                "signature": hit.signature,
                "container": hit.container,
                "relationships": hit.relationships,
            })
        }).collect::<Vec<_>>(),
        "promptSymbolMap": symbol_map,
    });
    let search_output_bytes = serde_json::to_vec(&search_output)
        .expect("serialize benchmark search output")
        .len();
    let project_source_rough_tokens = project_source_bytes.div_ceil(4);
    let target_file_rough_tokens = target_file_bytes.div_ceil(4);
    let search_result_rough_tokens = search_output_bytes.div_ceil(4);
    let upper_bound_project_reduction_percent = if project_source_rough_tokens == 0 {
        0.0
    } else {
        (1.0 - search_result_rough_tokens as f64 / project_source_rough_tokens as f64) * 100.0
    };

    assert_eq!(cold.indexed_files, file_count);
    assert_eq!(cold.status, "current");
    assert_eq!(incremental.indexed_files, file_count);
    assert_eq!(incremental.status, "current");
    eprintln!(
        "project_index_sync files={file_count} cold_ms={:.3} unchanged_ms={:.3} search_ms={:.3}",
        cold_elapsed.as_secs_f64() * 1000.0,
        incremental_elapsed.as_secs_f64() * 1000.0,
        search_elapsed.as_secs_f64() * 1000.0,
    );
    eprintln!(
        "project_index_context files={file_count} baseline=all_project_source project_source_bytes={project_source_bytes} target_file_bytes={target_file_bytes} search_result_bytes={search_output_bytes} rough_project_tokens={project_source_rough_tokens} rough_target_file_tokens={target_file_rough_tokens} rough_search_result_tokens={search_result_rough_tokens} upper_bound_project_reduction_percent={upper_bound_project_reduction_percent:.2}",
    );
}

#[test]
#[ignore = "manual current-repository project index benchmark"]
fn benchmark_current_repository_project_index() {
    let directories = TestDirectories::new();
    let repository_root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .expect("resolve the current repository root");
    let repository_path = repository_root.to_string_lossy().into_owned();
    let mut connection = directories.connect();
    connection
        .execute(
            "UPDATE projects SET folder_path = ?1 WHERE id = 'project-1'",
            [&repository_path],
        )
        .expect("point the benchmark project at the current repository");
    connection
        .execute(
            "UPDATE project_workspace_roots SET root_path = ?1
                 WHERE project_id = 'project-1' AND root_id = 'primary-root'",
            [&repository_path],
        )
        .expect("point the benchmark workspace root at the current repository");
    set_enabled(&mut connection, "project-1", true)
        .expect("enable current-repository benchmark index");
    drop(connection);

    let started = std::time::Instant::now();
    let cold = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &repository_root,
    )
    .expect("build the current-repository project index");
    let cold_elapsed = started.elapsed();
    let started = std::time::Instant::now();
    let incremental = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &repository_root,
    )
    .expect("refresh the unchanged current-repository project index");
    let incremental_elapsed = started.elapsed();

    let connection = directories.connect();
    let search_started = std::time::Instant::now();
    let text_hits = search_current_hits(&connection, "project-1", "ProjectIndexSnapshot")
        .expect("search current-repository source text");
    let symbol_hits = search_symbols(&connection, "project-1", "IndexedFileMetadata", 10)
        .expect("search current-repository symbols");
    let symbol_map = prompt_symbol_map(
        &connection,
        "project-1",
        "ProjectIndexSnapshot IndexedFileMetadata",
    )
    .expect("build current-repository symbol map");
    let mut all_symbols = connection
        .query(
            "SELECT path, name, kind, start_line, end_line
                 FROM project_index_symbols
                 WHERE project_id = ?1
                 ORDER BY path, name, kind, start_line",
            ["project-1"],
        )
        .expect("read all current-repository symbol declarations");
    let mut all_symbol_map_bytes = 0usize;
    let mut all_symbol_count = 0usize;
    while let Some(row) = all_symbols.next().expect("read indexed symbol row") {
        let path = row.get::<_, String>(0).expect("read indexed symbol path");
        let name = row.get::<_, String>(1).expect("read indexed symbol name");
        let kind = row.get::<_, String>(2).expect("read indexed symbol kind");
        let start_line = row
            .get::<_, i64>(3)
            .expect("read indexed symbol start line");
        let end_line = row.get::<_, i64>(4).expect("read indexed symbol end line");
        let location = if start_line == end_line {
            start_line.to_string()
        } else {
            format!("{start_line}-{end_line}")
        };
        let candidate_line = format!("- {path}:{location} [{kind}] {name}\n");
        all_symbol_map_bytes = all_symbol_map_bytes.saturating_add(candidate_line.len());
        all_symbol_count = all_symbol_count.saturating_add(1);
    }
    let repository_tasks: [(&str, &[&str]); 4] = [
        (
            "Trace DatabaseKey handoff into run_broker_stdio",
            &["DatabaseKey", "run_broker_stdio"],
        ),
        (
            "Trace file access with OpenedDirectory and OpenedFile capabilities",
            &["OpenedDirectory", "OpenedFile"],
        ),
        (
            "Trace design generation with sendCanvasRequest and frameAsAssistantMessage",
            &["sendCanvasRequest", "frameAsAssistantMessage"],
        ),
        (
            "Trace bounded project symbol retrieval through prompt_symbol_map",
            &["prompt_symbol_map"],
        ),
    ];
    let task_map_sizes = repository_tasks.map(|(query, expected_symbols)| {
            let map = prompt_symbol_map(&connection, "project-1", query)
                .expect("build current-repository task symbol map")
                .expect("current-repository task query returns declarations");
            let declarations = map
                .lines()
                .filter(|line| line.starts_with("- "))
                .collect::<Vec<_>>();
            let matched_symbols = expected_symbols
                .iter()
                .filter(|name| declarations.iter().any(|line| line.contains(**name)))
                .count();
            let relevant_declarations = declarations
                .iter()
                .filter(|line| expected_symbols.iter().any(|name| line.contains(*name)))
                .count();
            assert_eq!(
                matched_symbols,
                expected_symbols.len(),
                "task query should retrieve every expected declaration: {query}; returned {declarations:?}"
            );
            assert_eq!(
                relevant_declarations,
                declarations.len(),
                "task query should not include unrelated declarations: {query}; returned {declarations:?}"
            );
            assert!(map.len() <= MAX_SYMBOL_MAP_CONTEXT_BYTES);
            map.len()
        });
    assert_eq!(all_symbol_count, cold.indexed_symbols);
    assert!(all_symbol_map_bytes > MAX_SYMBOL_MAP_CONTEXT_BYTES);
    assert!(task_map_sizes.iter().all(|size| *size > 0));
    let search_elapsed = search_started.elapsed();
    let search_output = serde_json::json!({
        "matches": text_hits.iter().map(|hit| {
            serde_json::json!({"path": hit.path, "excerpt": hit.excerpt})
        }).collect::<Vec<_>>(),
        "symbols": symbol_hits.iter().map(|hit| {
            serde_json::json!({
                "path": hit.path,
                "name": hit.name,
                "kind": hit.kind,
                "startLine": hit.start_line,
                "endLine": hit.end_line,
                "signature": hit.signature,
                "container": hit.container,
                "relationships": hit.relationships,
            })
        }).collect::<Vec<_>>(),
        "promptSymbolMap": symbol_map,
    });
    let search_output_bytes = serde_json::to_vec(&search_output)
        .expect("serialize current-repository search output")
        .len();
    let target_file_bytes = usize::try_from(
        fs::metadata(
            repository_root
                .join("services/openchat-local-service/src/domain/projects/project_index.rs"),
        )
        .expect("read benchmark target file metadata")
        .len(),
    )
    .expect("fit benchmark target file size in memory");
    let project_source_rough_tokens = cold.indexed_bytes.div_ceil(4);
    let target_file_rough_tokens = target_file_bytes.div_ceil(4);
    let search_result_rough_tokens = search_output_bytes.div_ceil(4);
    let upper_bound_project_reduction_percent = if project_source_rough_tokens == 0 {
        0.0
    } else {
        (1.0 - search_result_rough_tokens as f64 / project_source_rough_tokens as f64) * 100.0
    };

    assert_eq!(
        cold.status, "current",
        "index diagnostic: {:?}",
        cold.error_details
    );
    assert_eq!(incremental.status, "current");
    eprintln!(
        "project_index_current_repository files={} bytes={} symbols={} cold_ms={:.3} unchanged_ms={:.3} search_ms={:.3}",
        cold.indexed_files,
        cold.indexed_bytes,
        cold.indexed_symbols,
        cold_elapsed.as_secs_f64() * 1000.0,
        incremental_elapsed.as_secs_f64() * 1000.0,
        search_elapsed.as_secs_f64() * 1000.0,
    );
    eprintln!(
        "project_index_current_repository_context target_file_bytes={target_file_bytes} search_result_bytes={search_output_bytes} rough_project_tokens={project_source_rough_tokens} rough_target_file_tokens={target_file_rough_tokens} rough_search_result_tokens={search_result_rough_tokens} upper_bound_project_reduction_percent={upper_bound_project_reduction_percent:.2}",
    );
    eprintln!(
        "project_index_current_repository_symbol_map all_symbols={all_symbol_count} all_symbols_name_only_bytes={all_symbol_map_bytes} all_symbols_rough_tokens={} retrieval_limit_bytes={MAX_SYMBOL_MAP_CONTEXT_BYTES} task_map_bytes={task_map_sizes:?}",
        all_symbol_map_bytes.div_ceil(4),
    );
}

#[test]
fn scan_read_budget_bounds_file_reads_and_rejects_partial_files() {
    assert_eq!(
        bounded_index_read_limit(0, MAX_FILE_BYTES as u64),
        Some(MAX_FILE_BYTES as u64 + 1)
    );
    assert_eq!(bounded_index_read_limit(MAX_SCANNED_BYTES - 5, 5), Some(5));
    assert_eq!(bounded_index_read_limit(MAX_SCANNED_BYTES - 5, 6), None);
    assert_eq!(bounded_index_read_limit(MAX_SCANNED_BYTES + 1, 0), None);
}

#[test]
fn project_gitignore_size_line_and_rule_limits_are_enforced_before_indexing() {
    let directories = TestDirectories::new();
    let oversized_file = directories.project.join(".gitignore");
    fs::write(
        &oversized_file,
        vec![b' '; MAX_GITIGNORE_FILE_BYTES.saturating_add(1)],
    )
    .expect("write oversized ignore file");
    let size_error =
        validate_gitignore_limits(&directories.project, &mut GitignoreLimits::default())
            .expect_err("oversized ignore file must be rejected");
    assert_eq!(size_error.code, "project_index_gitignore_limits_exceeded");
    assert!(
        size_error
            .details
            .as_deref()
            .is_some_and(|details| details.contains("byte limit"))
    );

    fs::write(
        &oversized_file,
        format!("{}\n", "x".repeat(MAX_GITIGNORE_LINE_BYTES + 1)),
    )
    .expect("write overlong ignore rule");
    let line_error =
        validate_gitignore_limits(&directories.project, &mut GitignoreLimits::default())
            .expect_err("overlong ignore rule must be rejected");
    assert!(
        line_error
            .details
            .as_deref()
            .is_some_and(|details| details.contains("line exceeds"))
    );

    fs::write(
        &oversized_file,
        (0..=MAX_GITIGNORE_RULES)
            .map(|index| format!("ignored-{index}\n"))
            .collect::<String>(),
    )
    .expect("write too many ignore rules");
    let rule_error =
        validate_gitignore_limits(&directories.project, &mut GitignoreLimits::default())
            .expect_err("too many ignore rules must be rejected");
    assert!(
        rule_error
            .details
            .as_deref()
            .is_some_and(|details| details.contains("rule limit"))
    );
}

#[test]
fn project_gitignore_accepts_file_and_line_sizes_at_their_limits() {
    let directories = TestDirectories::new();
    let gitignore = directories.project.join(".gitignore");
    let mut contents = Vec::with_capacity(MAX_GITIGNORE_FILE_BYTES);
    let full_line_count = MAX_GITIGNORE_FILE_BYTES / (MAX_GITIGNORE_LINE_BYTES + 1);
    let remaining_bytes =
        MAX_GITIGNORE_FILE_BYTES - full_line_count * (MAX_GITIGNORE_LINE_BYTES + 1);
    for _ in 0..full_line_count {
        contents.extend(std::iter::repeat_n(b' ', MAX_GITIGNORE_LINE_BYTES));
        contents.push(b'\n');
    }
    contents.extend(std::iter::repeat_n(b' ', remaining_bytes - 1));
    contents.push(b'\n');
    assert_eq!(contents.len(), MAX_GITIGNORE_FILE_BYTES);
    fs::write(&gitignore, contents).expect("write ignore file at its byte limit");

    validate_gitignore_limits(&directories.project, &mut GitignoreLimits::default())
        .expect("ignore file and lines at their limits must pass");
}

#[test]
fn project_gitignore_aggregate_file_and_byte_limits_are_enforced() {
    let directories = TestDirectories::new();
    let gitignore = directories.project.join(".gitignore");
    fs::write(&gitignore, "ignored-output/\n").expect("write valid ignore file");

    let mut byte_limited = GitignoreLimits {
        total_bytes: MAX_GITIGNORE_TOTAL_BYTES,
        ..GitignoreLimits::default()
    };
    let byte_error = validate_gitignore_file(&gitignore, &mut byte_limited)
        .expect_err("aggregate ignore bytes must be limited");
    assert!(
        byte_error
            .details
            .as_deref()
            .is_some_and(|details| details.contains("total project .gitignore size"))
    );

    let mut file_limited = GitignoreLimits::default();
    for index in 0..MAX_GITIGNORE_FILES {
        file_limited
            .files
            .insert(PathBuf::from(format!("previous-ignore-{index}")));
    }
    let file_error = validate_gitignore_file(&gitignore, &mut file_limited)
        .expect_err("aggregate ignore file count must be limited");
    assert!(
        file_error
            .details
            .as_deref()
            .is_some_and(|details| details.contains("ignore file limit"))
    );
}

#[test]
fn project_gitignore_files_are_checked_in_nested_and_parent_directories() {
    let directories = TestDirectories::new();
    let parent = directories.root.join(".gitignore");
    let nested = directories.project.join("nested/.gitignore");
    fs::create_dir_all(nested.parent().expect("nested parent")).expect("create nested dir");
    fs::write(
        &parent,
        format!("{}\n", "x".repeat(MAX_GITIGNORE_LINE_BYTES + 1)),
    )
    .expect("write oversized parent rule");
    fs::write(&nested, "nested-output/\n").expect("write nested ignore rule");

    let error = validate_gitignore_limits(&directories.project, &mut GitignoreLimits::default())
        .expect_err("oversized parent ignore rule must be rejected");
    assert!(
        error
            .details
            .as_deref()
            .is_some_and(|details| details.contains("line exceeds"))
    );

    fs::write(&parent, "parent-rule\n").expect("replace parent ignore rule");
    fs::write(
        &nested,
        format!("{}\n", "x".repeat(MAX_GITIGNORE_LINE_BYTES + 1)),
    )
    .expect("write oversized nested rule");
    let nested_error =
        validate_gitignore_limits(&directories.project, &mut GitignoreLimits::default())
            .expect_err("oversized nested ignore rule must be rejected");
    assert!(
        nested_error
            .details
            .as_deref()
            .is_some_and(|details| details.contains("line exceeds"))
    );
}

#[test]
fn oversized_project_gitignore_does_not_change_the_saved_index() {
    let directories = TestDirectories::new();
    fs::write(
        directories.project.join("src.rs"),
        "fn should_not_be_indexed() {}",
    )
    .expect("write project source");
    fs::write(
        directories.project.join(".gitignore"),
        vec![b' '; MAX_GITIGNORE_FILE_BYTES.saturating_add(1)],
    )
    .expect("write oversized ignore file");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable index");
    drop(connection);

    let error = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect_err("oversized ignore file must stop indexing");
    assert_eq!(error.code, "project_index_gitignore_limits_exceeded");
    let connection = directories.connect();
    let snapshot = snapshot(&connection, "project-1").expect("read unchanged index state");
    assert_eq!(snapshot.indexed_files, 0);
    assert_eq!(snapshot.status, "empty");
}

#[test]
fn project_file_reads_cannot_escape_the_root_directory_capability() {
    let directories = TestDirectories::new();
    let outside_file = directories.root.join("outside.txt");
    fs::write(&outside_file, "outside project content").expect("write outside file");
    let root_directory = Dir::open_ambient_dir(&directories.project, ambient_authority())
        .expect("open project root capability");

    assert!(
        root_directory
            .open(PathBuf::from("../outside.txt"))
            .is_err()
    );
}

#[test]
fn hard_linked_project_files_keep_the_index_stale_and_unavailable() {
    let directories = TestDirectories::new();
    let outside_file = directories.root.join("outside-secret.txt");
    fs::write(&outside_file, "private_project_marker").expect("write external file content");
    fs::hard_link(&outside_file, directories.project.join("linked.txt"))
        .expect("create project hard link");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable index");
    drop(connection);

    let snapshot = sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("sync project");

    assert_eq!(snapshot.status, "stale");
    assert_eq!(snapshot.indexed_files, 0);
    let connection = directories.connect();
    assert!(
        search_current(
            &connection,
            "project-1",
            &directories.project,
            "private project marker",
        )
        .expect("search stale index")
        .is_none()
    );
}

#[test]
fn disabling_and_clearing_index_have_explicit_states() {
    let directories = TestDirectories::new();
    fs::write(directories.project.join("src.rs"), "fn useful_code() {}")
        .expect("write source file");
    let mut connection = directories.connect();
    set_enabled(&mut connection, "project-1", true).expect("enable index");
    drop(connection);
    sync(
        &directories.storage.database_queue_handle(),
        "project-1",
        &directories.project,
    )
    .expect("sync project");
    let mut connection = directories.connect();
    let disabled = set_enabled(&mut connection, "project-1", false).expect("disable index");
    assert!(!disabled.enabled);
    assert_eq!(disabled.status, "disabled");
    assert!(
        search_current(
            &connection,
            "project-1",
            &directories.project,
            "useful code",
        )
        .expect("search disabled index")
        .is_none()
    );
    let cleared = clear(&mut connection, "project-1").expect("clear index");
    assert_eq!(cleared.indexed_files, 0);
    assert_eq!(cleared.status, "empty");
}
