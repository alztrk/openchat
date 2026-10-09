use std::{
    fs,
    path::{Path, PathBuf},
    sync::atomic::{AtomicU64, Ordering},
    time::{Duration, Instant},
};

use serde_json::{Value, json};

use super::search::{ascii_shift_table, contains_ascii_case_insensitive};
use super::{get_file_info, list_files, read_file, search_files};

static NEXT_DIRECTORY: AtomicU64 = AtomicU64::new(0);

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let id = NEXT_DIRECTORY.fetch_add(1, Ordering::Relaxed);
        let path = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("target")
            .join(format!("openchat-tools-test-{}-{id}", std::process::id()));
        fs::create_dir(&path).expect("create isolated test directory");
        Self(path)
    }

    fn root(&self) -> &str {
        self.0.to_str().expect("temporary path is valid UTF-8")
    }

    fn write(&self, relative_path: &str, content: &str) {
        let path = self.0.join(relative_path);
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent).expect("create test subdirectory");
        }
        fs::write(path, content).expect("write test file");
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        fs::remove_dir_all(&self.0).expect("remove isolated test directory");
    }
}

#[test]
fn listing_hides_dot_directories_until_their_path_is_explicit() {
    let directory = TestDirectory::new();
    directory.write("visible.txt", "visible");
    directory.write(".private/secret.txt", "secret");

    let root_listing = list_files(directory.root(), "", 0, 100).expect("list root");
    assert_eq!(
        root_listing["entries"],
        json!([{"path": "visible.txt", "type": "file"}])
    );

    let private_listing =
        list_files(directory.root(), ".private", 0, 100).expect("list explicit hidden path");
    assert_eq!(
        private_listing["entries"],
        json!([{"path": ".private/secret.txt", "type": "file"}])
    );
}

#[test]
fn named_project_task_is_loaded_as_a_bounded_terminal_command() {
    let directory = TestDirectory::new();
    directory.write(
        ".openchat/tasks.json",
        r#"{"version":1,"tasks":[{"id":"verify","command":"cargo test","timeoutSeconds":90}]}"#,
    );

    let task = super::project_tasks::load_project_task(Path::new(directory.root()), "verify")
        .expect("load declared project task");
    assert_eq!(task.id, "verify");
    assert_eq!(task.command, "cargo test");
    assert_eq!(task.timeout_seconds, 90);

    let tasks = super::project_tasks::load_project_tasks(Path::new(directory.root()))
        .expect("list declared project tasks");
    assert_eq!(tasks, vec![task]);

    let missing = super::project_tasks::load_project_task(Path::new(directory.root()), "build")
        .expect_err("undeclared tasks must fail explicitly");
    assert_eq!(missing.code, "project_task_not_found");
}

#[test]
fn project_task_tool_is_available_only_for_declared_catalog_entries() {
    let directory = TestDirectory::new();
    assert!(
        super::project_tasks::load_project_tasks_if_present(Path::new(directory.root()))
            .expect("missing task catalog means no named tasks")
            .is_empty()
    );

    directory.write(
        ".openchat/tasks.json",
        r#"{"version":1,"tasks":[{"id":"verify","command":"cargo test","timeoutSeconds":90}]}"#,
    );
    let task_ids = super::project_tasks::load_project_tasks_if_present(Path::new(directory.root()))
        .expect("load task catalog")
        .into_iter()
        .map(|task| task.id)
        .collect::<Vec<_>>();
    let definition = super::project_tasks::tool_definition(&task_ids);
    assert_eq!(
        definition.parameters["properties"]["task"]["enum"],
        json!(["verify"])
    );
}

#[test]
fn disabled_project_task_tool_does_not_block_chat_on_invalid_catalog() {
    let directory = TestDirectory::new();
    directory.write(".openchat/tasks.json", "not json");

    assert!(
        super::project_tasks::load_project_task_ids_if_enabled(
            Some(Path::new(directory.root())),
            false,
        )
        .expect("disabled tools must not load their optional catalog")
        .is_empty()
    );
    assert_eq!(
        super::project_tasks::load_project_task_ids_if_enabled(
            Some(Path::new(directory.root())),
            true,
        )
        .expect_err("enabled tools must report invalid configuration")
        .code,
        "invalid_project_task_catalog"
    );
}

#[test]
fn oversized_project_task_catalog_is_rejected_after_a_bounded_read() {
    let directory = TestDirectory::new();
    directory.write(
        ".openchat/tasks.json",
        &format!("{{\"version\":1,\"tasks\":[]}}{}", " ".repeat(65 * 1024)),
    );

    let error = super::project_tasks::load_project_tasks(Path::new(directory.root()))
        .expect_err("oversized task catalogs must be rejected");
    assert_eq!(error.code, "invalid_project_task_catalog");
}

#[test]
fn list_files_reads_current_directory_contents_each_time() {
    let directory = TestDirectory::new();
    directory.write("src/original.txt", "original\n");
    let initial = list_files(directory.root(), "src", 0, 100).expect("list initial files");
    assert_eq!(initial["entries"].as_array().map(Vec::len), Some(1));

    directory.write("src/added.txt", "added\n");
    let added = list_files(directory.root(), "src", 0, 100).expect("list added file");
    assert!(added["entries"].as_array().is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry["path"] == json!("src/added.txt"))
    }));

    fs::rename(
        directory.0.join("src/added.txt"),
        directory.0.join("src/renamed.txt"),
    )
    .expect("rename listed file");
    let renamed = list_files(directory.root(), "src", 0, 100).expect("list renamed file");
    assert!(renamed["entries"].as_array().is_some_and(|entries| {
        entries
            .iter()
            .any(|entry| entry["path"] == json!("src/renamed.txt"))
            && !entries
                .iter()
                .any(|entry| entry["path"] == json!("src/added.txt"))
    }));

    fs::remove_file(directory.0.join("src/renamed.txt")).expect("remove listed file");
    let removed = list_files(directory.root(), "src", 0, 100).expect("list removed file");
    assert_eq!(removed["entries"].as_array().map(Vec::len), Some(1));
}

#[test]
fn list_files_sorts_only_the_requested_prefix_without_breaking_pages() {
    let directory = TestDirectory::new();
    for file_index in (0..250).rev() {
        directory.write(&format!("src/file_{file_index:03}.txt"), "content\n");
    }

    let first = list_files(directory.root(), "src", 0, 50).expect("list first page");
    let second = list_files(directory.root(), "src", 50, 50).expect("list second page");
    let first_entries = first["entries"].as_array().expect("first page entries");
    let second_entries = second["entries"].as_array().expect("second page entries");

    assert_eq!(first_entries.len(), 50);
    assert_eq!(second_entries.len(), 50);
    assert_eq!(first_entries[0]["path"], json!("src/file_000.txt"));
    assert_eq!(first_entries[49]["path"], json!("src/file_049.txt"));
    assert_eq!(second_entries[0]["path"], json!("src/file_050.txt"));
    assert_eq!(second_entries[49]["path"], json!("src/file_099.txt"));
    assert_eq!(first["nextOffset"], json!(50));
    assert_eq!(second["nextOffset"], json!(100));
}

#[test]
fn generated_directories_are_skipped_by_default_and_searchable_when_selected() {
    let directory = TestDirectory::new();
    directory.write("src/main.rs", "visible marker\n");
    directory.write("node_modules/package/source.js", "dependency marker\n");

    let default = search_files(directory.root(), "", "marker", false, 0, 40)
        .expect("search default workspace scope");
    assert_eq!(default["matches"].as_array().map(Vec::len), Some(1));
    assert_eq!(default["matches"][0]["path"], json!("src/main.rs"));

    let explicit = search_files(directory.root(), "node_modules", "marker", false, 0, 40)
        .expect("search explicitly selected generated directory");
    assert_eq!(
        explicit["matches"][0]["path"],
        json!("node_modules/package/source.js")
    );
}

#[test]
fn gitignore_rules_filter_listing_and_search_but_explicit_scope_can_search_ignored_path() {
    let directory = TestDirectory::new();
    directory.write(".gitignore", "ignored.txt\nignored-dir/\n");
    directory.write("visible.txt", "searchable marker\n");
    directory.write("ignored.txt", "hidden marker\n");
    directory.write("ignored-dir/nested.txt", "nested marker\n");
    directory.write("src/.gitignore", "private.txt\n");
    directory.write("src/public.txt", "nested searchable marker\n");
    directory.write("src/private.txt", "nested hidden marker\n");

    let listing = list_files(directory.root(), "", 0, 100).expect("list workspace root");
    let listed_paths = listing["entries"]
        .as_array()
        .expect("directory entries")
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect::<Vec<_>>();
    assert!(listed_paths.contains(&"visible.txt"));
    assert!(!listed_paths.contains(&"ignored.txt"));
    assert!(!listed_paths.contains(&"ignored-dir"));

    let selected_listing =
        list_files(directory.root(), "ignored-dir", 0, 100).expect("list selected ignored path");
    assert_eq!(
        selected_listing["entries"][0]["path"],
        json!("ignored-dir/nested.txt")
    );
    let nested_listing = list_files(directory.root(), "src", 0, 100).expect("list nested folder");
    let nested_paths = nested_listing["entries"]
        .as_array()
        .expect("nested directory entries")
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(nested_paths, ["src/public.txt"]);

    let default_search =
        search_files(directory.root(), "", "marker", false, 0, 40).expect("search workspace root");
    let default_matches = default_search["matches"]
        .as_array()
        .expect("search matches");
    let default_paths = default_matches
        .iter()
        .filter_map(|matched| matched["path"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(default_paths, ["src/public.txt", "visible.txt"]);

    let explicit_search = search_files(directory.root(), "ignored-dir", "marker", false, 0, 40)
        .expect("search explicitly selected ignored directory");
    assert_eq!(
        explicit_search["matches"][0]["path"],
        json!("ignored-dir/nested.txt")
    );
}

#[test]
fn search_reads_current_file_contents_each_time() {
    let directory = TestDirectory::new();
    directory.write("source.txt", "oldmarker value\n");
    let initial =
        search_files(directory.root(), "", "oldmarker", false, 0, 10).expect("search initial file");
    assert_eq!(initial["matches"].as_array().map(Vec::len), Some(1));

    directory.write("source.txt", "replacement newmarker value\n");
    let old = search_files(directory.root(), "", "oldmarker", false, 0, 10)
        .expect("search replaced file for old content");
    let new = search_files(directory.root(), "", "newmarker", false, 0, 10)
        .expect("search replaced file for new content");
    assert!(old["matches"].as_array().is_some_and(Vec::is_empty));
    assert_eq!(new["matches"].as_array().map(Vec::len), Some(1));
}

#[test]
fn search_paginates_compact_matches_and_can_include_hidden_files() {
    let directory = TestDirectory::new();
    directory.write("a.txt", "needle first\n");
    directory.write("b.txt", "needle second\n");
    directory.write(".private/c.txt", "needle hidden\n");

    let first =
        search_files(directory.root(), "", "needle", false, 0, 1).expect("search first page");
    assert_eq!(first["matches"].as_array().map(Vec::len), Some(1));
    assert_eq!(first["nextOffset"], json!(1));
    let first_text = first["matches"][0]["text"]
        .as_str()
        .expect("match text is a string");
    assert!(["needle first", "needle second"].contains(&first_text));

    let second =
        search_files(directory.root(), "", "needle", false, 1, 1).expect("search last page");
    let second_text = second["matches"][0]["text"]
        .as_str()
        .expect("match text is a string");
    assert!(["needle first", "needle second"].contains(&second_text));
    assert_ne!(first_text, second_text);
    assert!(second["nextOffset"].is_null());

    let all = search_files(directory.root(), "", "needle", true, 0, 40)
        .expect("search including hidden files");
    assert_eq!(all["matches"].as_array().map(Vec::len), Some(3));
}

#[test]
fn direct_search_keeps_pagination_order_after_parallel_file_reads() {
    let directory = TestDirectory::new();
    for file_index in 0..100 {
        directory.write(&format!("src/file_{file_index:03}.txt"), "no match here\n");
    }
    let root = super::canonical_root(directory.root()).expect("canonical workspace root");
    let source_directory = root.join("src");
    let mut paths = fs::read_dir(&source_directory)
        .expect("read source directory")
        .map(|entry| entry.expect("read source entry").path())
        .collect::<Vec<_>>();
    paths.sort_unstable();
    let first_match_path = paths[98].clone();
    let second_match_path = paths[99].clone();
    fs::write(&first_match_path, "parallel marker first\n").expect("write first match");
    fs::write(&second_match_path, "parallel marker second\n").expect("write second match");

    let all =
        super::search::search_files_direct(&root, &root, "parallel marker", false, false, 0, 40)
            .expect("search parallel candidates");
    assert_eq!(
        all["matches"],
        json!([
            {"path": first_match_path.strip_prefix(&root).expect("relative path").to_string_lossy().replace('\\', "/"), "line": 1, "text": "parallel marker first"},
            {"path": second_match_path.strip_prefix(&root).expect("relative path").to_string_lossy().replace('\\', "/"), "line": 1, "text": "parallel marker second"},
        ])
    );

    let first =
        super::search::search_files_direct(&root, &root, "parallel marker", false, false, 0, 1)
            .expect("search first page");
    let second =
        super::search::search_files_direct(&root, &root, "parallel marker", false, false, 1, 1)
            .expect("search second page");
    assert_eq!(first["matches"][0], all["matches"][0]);
    assert_eq!(first["nextOffset"], json!(1));
    assert_eq!(second["matches"][0], all["matches"][1]);
    assert!(second["nextOffset"].is_null());
}

#[test]
fn search_keeps_ascii_case_insensitive_matching_for_unicode_lines() {
    let directory = TestDirectory::new();
    directory.write("unicode.txt", "Kelvin value\n");

    let result = search_files(directory.root(), "", "KELVIN", false, 0, 10)
        .expect("search Unicode text with ASCII query");

    assert_eq!(result["matches"].as_array().map(Vec::len), Some(1));
}

#[test]
fn ascii_search_matches_case_insensitive_reference_cases() {
    let cases = [
        ("A", "a"),
        ("xxAbx", "AB"),
        ("abababa", "aba"),
        ("aaaaab", "aaab"),
        ("no match", "needle"),
    ];

    for (text, query) in cases {
        let needle = query.to_ascii_lowercase();
        let shifts = ascii_shift_table(needle.as_bytes());
        let expected = text
            .as_bytes()
            .windows(query.len())
            .any(|candidate| candidate.eq_ignore_ascii_case(query.as_bytes()));
        assert_eq!(
            contains_ascii_case_insensitive(text.as_bytes(), needle.as_bytes(), &shifts),
            expected,
            "text={text:?}, query={query:?}"
        );
    }
}

#[test]
fn reading_returns_requested_lines_and_continuation() {
    let directory = TestDirectory::new();
    directory.write("notes.txt", "one\ntwo\nthree\n");

    let result = read_file(directory.root(), "notes.txt", 2, 1).expect("read line range");
    assert_eq!(result["lines"], json!([{"line": 2, "text": "two"}]));
    assert_eq!(result["nextLine"], json!(3));
    assert_eq!(
        get_file_info(directory.root(), "notes.txt").expect("file info")["size"],
        14
    );
}

#[test]
fn reading_late_lines_skips_unrequested_bytes_without_decoding_them() {
    let directory = TestDirectory::new();
    let path = directory.0.join("notes.txt");
    fs::write(&path, b"\xff\nrequested line\n").expect("write mixed-encoding file");

    let result = read_file(directory.root(), "notes.txt", 2, 1)
        .expect("read a valid line after invalid bytes");

    assert_eq!(
        result["lines"],
        json!([{"line": 2, "text": "requested line"}])
    );
    assert!(read_file(directory.root(), "notes.txt", 1, 1).is_err());
}

#[test]
fn paths_cannot_escape_the_selected_root() {
    let directory = TestDirectory::new();
    assert!(list_files(directory.root(), "../", 0, 10).is_err());
    assert!(search_files(directory.root(), "../", "secret", false, 0, 10).is_err());
    assert!(read_file(directory.root(), "../outside.txt", 1, 10).is_err());
    assert!(get_file_info(directory.root(), "../outside.txt").is_err());
}

#[test]
#[ignore = "manual tool-output benchmark; run with --ignored --nocapture"]
fn benchmark_tool_latency_and_output_size() {
    let directory = TestDirectory::new();
    let mut dataset_bytes = 0usize;
    for file_index in 0..500 {
        let line_count = if file_index == 0 { 600 } else { 100 };
        let mut content = String::with_capacity(line_count * 90);
        for line_index in 0..line_count {
            content.push_str(&format!(
                "// file={file_index:04} line={line_index:04} {}{}\n",
                "x".repeat(40),
                if line_index == 30 {
                    " needle target"
                } else {
                    ""
                },
            ));
        }
        dataset_bytes += content.len();
        directory.write(&format!("src/file_{file_index:04}.rs"), &content);
    }
    let hidden_content = "needle hidden\n";
    dataset_bytes += hidden_content.len();
    directory.write(".hidden/secret.txt", hidden_content);

    let root = directory.root();
    let measurements = [
        measure("list_files", || {
            list_files(root, "src", 0, 100).expect("benchmark list")
        }),
        measure("list_files_direct_no_app_cache", || {
            let listing = super::read_directory_listing(
                &super::canonical_root(root)
                    .expect("canonical benchmark workspace root")
                    .join("src"),
                false,
                100,
            )
            .expect("benchmark direct list");
            super::directory_listing_response(Path::new("src"), &listing, 0, 100)
        }),
        measure("search_files", || {
            search_files(root, "", "needle", false, 0, 40).expect("benchmark search")
        }),
        measure("search_files_direct_no_app_cache", || {
            let canonical_root =
                super::canonical_root(root).expect("canonical benchmark workspace root");
            super::search::search_files_direct(
                &canonical_root,
                &canonical_root,
                "needle",
                false,
                false,
                0,
                40,
            )
            .expect("benchmark direct search")
        }),
        measure("search_files_no_match", || {
            search_files(root, "", "no-such-marker", false, 0, 40)
                .expect("benchmark complete search")
        }),
        measure("read_file", || {
            read_file(root, "src/file_0000.rs", 1, 200).expect("benchmark read")
        }),
        measure("read_file_start_late", || {
            read_file(root, "src/file_0000.rs", 500, 100).expect("benchmark late read")
        }),
        measure("get_file_info", || {
            get_file_info(root, "src/file_0000.rs").expect("benchmark file info")
        }),
    ];

    eprintln!(
        "dataset_files=501 dataset_bytes={dataset_bytes} benchmark_iterations=500 warmup_iterations=10"
    );
    for measurement in measurements {
        eprintln!(
            "tool={} operation_p50_ms={:.3} operation_p95_ms={:.3} operation_p99_ms={:.3} serialization_p50_us={:.3} serialization_p95_us={:.3} serialization_p99_us={:.3} output_bytes={} output_chars={} rough_tokens_chars_div_4={}",
            measurement.name,
            measurement.p50.as_secs_f64() * 1000.0,
            measurement.p95.as_secs_f64() * 1000.0,
            measurement.p99.as_secs_f64() * 1000.0,
            measurement.serialization_p50.as_secs_f64() * 1_000_000.0,
            measurement.serialization_p95.as_secs_f64() * 1_000_000.0,
            measurement.serialization_p99.as_secs_f64() * 1_000_000.0,
            measurement.output_bytes,
            measurement.output_chars,
            measurement.output_chars.div_ceil(4),
        );
    }
}

#[test]
#[ignore = "manual monorepo-scale benchmark; run with --ignored --nocapture"]
fn benchmark_search_at_monorepo_scales() {
    for file_count in [10_000, 100_000] {
        benchmark_search_scale(file_count);
    }
}

fn benchmark_search_scale(file_count: usize) {
    let directory = TestDirectory::new();
    let mut dataset_bytes = 0usize;
    for file_index in 0..file_count {
        let content = format!("file={file_index:06} {}\n", "content ".repeat(16));
        dataset_bytes += content.len();
        directory.write(&format!("src/file_{file_index:06}.txt"), &content);
    }
    let canonical_source = directory
        .0
        .join("src")
        .canonicalize()
        .expect("canonical benchmark source directory");
    let source_entries = fs::read_dir(&canonical_source)
        .expect("read benchmark source directory")
        .map(|entry| entry.expect("read benchmark source entry").path())
        .collect::<Vec<_>>();
    let late_match_paths = source_entries
        .get(100..141)
        .expect("benchmark has a late search page")
        .to_vec();
    for late_match_path in &late_match_paths {
        let mut late_match_content =
            fs::read_to_string(late_match_path).expect("read benchmark late-match file");
        late_match_content.push_str("late-only-marker\n");
        dataset_bytes += "late-only-marker\n".len();
        fs::write(late_match_path, late_match_content).expect("write benchmark late marker");
    }
    let late_match_relative = Path::new("src").join(
        late_match_paths[0]
            .file_name()
            .and_then(|name| name.to_str())
            .expect("benchmark file name is valid UTF-8"),
    );

    eprintln!("scale={file_count} dataset_bytes={dataset_bytes} warmup_iterations=1");
    let listing_started = Instant::now();
    let listing_cold =
        list_files(directory.root(), "src", 0, 100).expect("benchmark cold directory listing");
    let listing_cold_elapsed = listing_started.elapsed();
    let listing = measure_iterations("list_files", 7, 1, || {
        list_files(directory.root(), "src", 0, 100).expect("benchmark large listing")
    });
    let cold_started = Instant::now();
    let cold = search_files(directory.root(), "", "no-such-marker", false, 0, 40)
        .expect("benchmark cold full search");
    let cold_elapsed = cold_started.elapsed();
    let canonical_root =
        super::canonical_root(directory.root()).expect("canonical benchmark workspace root");
    let source_directory = canonical_root.join("src");
    let direct_listing = measure_iterations("list_files_direct_no_app_cache", 3, 1, || {
        let listing = super::read_directory_listing(&source_directory, false, 100)
            .expect("benchmark direct directory listing");
        super::directory_listing_response(Path::new("src"), &listing, 0, 100)
    });
    let direct_search = measure_iterations("search_files_direct_no_app_cache", 3, 1, || {
        super::search::search_files_direct(
            &canonical_root,
            &canonical_root,
            "no-such-marker",
            false,
            false,
            0,
            40,
        )
        .expect("benchmark direct full search")
    });
    let late_match = measure_iterations("search_files_late_match_no_app_cache", 3, 1, || {
        search_files(directory.root(), "", "late-only-marker", false, 0, 40)
            .expect("benchmark late match search")
    });
    let late_match_output = search_files(directory.root(), "", "late-only-marker", false, 0, 40)
        .expect("verify benchmark late match");
    assert_eq!(
        late_match_output["matches"][0]["path"],
        json!(late_match_relative.to_string_lossy().replace('\\', "/"))
    );
    assert_eq!(
        late_match_output["matches"].as_array().map(Vec::len),
        Some(40)
    );
    assert_eq!(late_match_output["nextOffset"], json!(40));
    let repeated = measure_iterations("search_files_direct_no_app_cache_repeat", 7, 1, || {
        search_files(directory.root(), "", "another-no-marker", false, 0, 40)
            .expect("benchmark repeated direct full search")
    });
    let common = measure_iterations("search_files_common_term_no_app_cache", 7, 1, || {
        search_files(directory.root(), "", "content", false, 0, 40)
            .expect("benchmark common search")
    });
    eprintln!(
        "scale={file_count} tool=list_files_cold elapsed_ms={:.3} output_bytes={} truncated={}",
        listing_cold_elapsed.as_secs_f64() * 1000.0,
        serde_json::to_vec(&listing_cold).map_or(0, |output| output.len()),
        listing_cold["truncated"].as_bool().unwrap_or(true),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={}",
        listing.name,
        listing.p50.as_secs_f64() * 1000.0,
        listing.p95.as_secs_f64() * 1000.0,
        listing.output_bytes,
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={} rough_tokens_chars_div_4={}",
        direct_listing.name,
        direct_listing.p50.as_secs_f64() * 1000.0,
        direct_listing.p95.as_secs_f64() * 1000.0,
        direct_listing.output_bytes,
        direct_listing.output_chars.div_ceil(4),
    );
    eprintln!(
        "scale={file_count} tool=search_files_cold elapsed_ms={:.3} output_bytes={} truncated={}",
        cold_elapsed.as_secs_f64() * 1000.0,
        serde_json::to_vec(&cold).map_or(0, |output| output.len()),
        cold["truncated"].as_bool().unwrap_or(true),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={}",
        repeated.name,
        repeated.p50.as_secs_f64() * 1000.0,
        repeated.p95.as_secs_f64() * 1000.0,
        repeated.output_bytes,
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={} rough_tokens_chars_div_4={}",
        direct_search.name,
        direct_search.p50.as_secs_f64() * 1000.0,
        direct_search.p95.as_secs_f64() * 1000.0,
        direct_search.output_bytes,
        direct_search.output_chars.div_ceil(4),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={} rough_tokens_chars_div_4={}",
        late_match.name,
        late_match.p50.as_secs_f64() * 1000.0,
        late_match.p95.as_secs_f64() * 1000.0,
        late_match.output_bytes,
        late_match.output_chars.div_ceil(4),
    );
    eprintln!(
        "scale={file_count} tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={}",
        common.name,
        common.p50.as_secs_f64() * 1000.0,
        common.p95.as_secs_f64() * 1000.0,
        common.output_bytes,
    );
}

struct Measurement {
    name: &'static str,
    p50: Duration,
    p95: Duration,
    p99: Duration,
    serialization_p50: Duration,
    serialization_p95: Duration,
    serialization_p99: Duration,
    output_bytes: usize,
    output_chars: usize,
}

fn measure(name: &'static str, operation: impl FnMut() -> serde_json::Value) -> Measurement {
    measure_iterations(name, 500, 10, operation)
}

fn measure_iterations(
    name: &'static str,
    iterations: usize,
    warmup_iterations: usize,
    mut operation: impl FnMut() -> serde_json::Value,
) -> Measurement {
    for _ in 0..warmup_iterations {
        let result = operation();
        let _ = serde_json::to_vec(&result).expect("serialize benchmark warmup output");
    }

    let mut durations = Vec::with_capacity(iterations);
    let mut serialization_durations = Vec::with_capacity(iterations);
    let mut output = Vec::new();
    for _ in 0..iterations {
        let started = Instant::now();
        let result = operation();
        durations.push(started.elapsed());

        let serialization_started = Instant::now();
        output = serde_json::to_vec(&result).expect("serialize benchmark output");
        serialization_durations.push(serialization_started.elapsed());
    }
    durations.sort_unstable();
    serialization_durations.sort_unstable();
    let output_text = std::str::from_utf8(&output).expect("tool output is valid UTF-8");
    Measurement {
        name,
        p50: percentile(&durations, 50),
        p95: percentile(&durations, 95),
        p99: percentile(&durations, 99),
        serialization_p50: percentile(&serialization_durations, 50),
        serialization_p95: percentile(&serialization_durations, 95),
        serialization_p99: percentile(&serialization_durations, 99),
        output_bytes: output.len(),
        output_chars: output_text.chars().count(),
    }
}

fn percentile(durations: &[Duration], percentile: usize) -> Duration {
    durations[(durations.len() - 1) * percentile / 100]
}

#[test]
fn opencode_wire_tools_include_bash_and_read_with_industry_standards() {
    let internal_defs = super::definitions();
    assert!(internal_defs.iter().any(|tool| tool.name == "read_file"));
    assert!(internal_defs.iter().any(|tool| tool.name == "write_file"));
    assert!(internal_defs.iter().any(|tool| tool.name == "edit_file"));
    assert!(internal_defs.iter().any(|tool| tool.name == "list_files"));
    assert!(internal_defs.iter().any(|tool| tool.name == "search_files"));
    assert!(
        internal_defs
            .iter()
            .any(|tool| tool.name == "execute_command")
    );

    let wire_tools = super::opencode_wire_tools(&internal_defs);
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("bash"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("read"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("write"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("edit"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("glob"))
    );
    assert!(
        wire_tools
            .iter()
            .any(|t| t.pointer("/function/name").and_then(Value::as_str) == Some("grep"))
    );

    let responses_wire_tools = super::opencode_responses_wire_tools(&internal_defs);
    assert!(
        responses_wire_tools
            .iter()
            .any(|t| t.get("name").and_then(Value::as_str) == Some("bash")
                && t.get("type").and_then(Value::as_str) == Some("function")
                && t.get("parameters").is_some())
    );
    assert!(
        responses_wire_tools
            .iter()
            .any(|t| t.get("name").and_then(Value::as_str) == Some("read")
                && t.get("type").and_then(Value::as_str) == Some("function")
                && t.get("parameters").is_some())
    );

    // Bidirectional translation test
    assert_eq!(super::opencode_wire_name("read_file"), "read");
    assert_eq!(super::opencode_wire_name("write_file"), "write");
    assert_eq!(super::opencode_wire_name("edit_file"), "edit");
    assert_eq!(super::opencode_wire_name("list_files"), "glob");
    assert_eq!(super::opencode_wire_name("search_files"), "grep");
    assert_eq!(super::opencode_wire_name("execute_command"), "bash");

    assert_eq!(super::internal_tool_name(true, "read"), "read_file");
    assert_eq!(super::internal_tool_name(true, "write"), "write_file");
    assert_eq!(super::internal_tool_name(true, "edit"), "edit_file");
    assert_eq!(super::internal_tool_name(true, "glob"), "list_files");
    assert_eq!(super::internal_tool_name(true, "grep"), "search_files");
    assert_eq!(super::internal_tool_name(true, "bash"), "execute_command");
    assert_eq!(super::internal_tool_name(false, "read"), "read");
}

#[test]
fn opencode_mcp_tool_names_are_preserved_for_round_trip_execution() {
    let tool = super::ToolDefinition {
        name: "mcp__local_docs__search".to_owned(),
        description: "Search local docs".to_owned(),
        parameters: serde_json::json!({"type": "object", "properties": {"query": {"type": "string"}}}),
    };

    let wire = super::opencode_wire_tool(&tool);
    assert_eq!(
        wire.pointer("/function/name")
            .and_then(serde_json::Value::as_str),
        Some("mcp__local_docs__search")
    );
    assert_eq!(
        super::internal_tool_name(true, "mcp__local_docs__search"),
        tool.name
    );
}

#[test]
fn write_file_and_edit_file_flow_and_guards() {
    let directory = TestDirectory::new();
    let root = directory.root();

    // 1. Write new file
    let write_res = super::write_file(root, "nested/hello.txt", "Hello OpenChat World!");
    assert!(
        write_res.is_ok(),
        "write_file should succeed for new file in subfolder"
    );
    let val = write_res.unwrap();
    assert_eq!(val["bytesWritten"], 21);
    assert_eq!(val["success"], true);

    // Verify content
    let read_res = super::read_file(root, "nested/hello.txt", 1, 10).unwrap();
    assert_eq!(read_res["lines"][0]["text"], "Hello OpenChat World!");

    // 2. Edit file (single replacement)
    let edit_res = super::edit_file(root, "nested/hello.txt", "World", "Alican");
    assert!(
        edit_res.is_ok(),
        "edit_file should succeed when target matches exactly once"
    );
    let edit_val = edit_res.unwrap();
    assert_eq!(edit_val["replacements"], 1);

    // Verify edited content
    let read_edited = super::read_file(root, "nested/hello.txt", 1, 10).unwrap();
    assert_eq!(read_edited["lines"][0]["text"], "Hello OpenChat Alican!");

    // 3. Ambiguity guard: when oldString matches multiple times
    super::write_file(root, "ambiguous.txt", "test foo bar foo baz").unwrap();
    let amb_res = super::edit_file(root, "ambiguous.txt", "foo", "qux");
    assert!(
        amb_res.is_err(),
        "edit_file must reject multiple occurrences to avoid ambiguous edits"
    );

    // 4. Not found guard
    let not_found_res = super::edit_file(root, "ambiguous.txt", "nonexistent", "qux");
    assert!(
        not_found_res.is_err(),
        "edit_file must reject when target string is not found"
    );

    // 5. Empty target string guard
    let empty_target = super::edit_file(root, "ambiguous.txt", "", "qux");
    assert!(
        empty_target.is_err(),
        "edit_file must reject empty old_string"
    );
}

#[test]
#[ignore = "manual filesystem latency benchmark; run with --ignored --nocapture"]
fn benchmark_write_and_edit_performance_simulation() {
    use std::time::Instant;

    let directory = TestDirectory::new();
    let root = directory.root();

    // Test with a 100 KB payload
    let chunk = "The quick brown fox jumps over the lazy dog. Rust memory safety and zero-cost abstractions.\n";
    let iterations = 100_000 / chunk.len();
    let mut large_text = String::with_capacity(100_000);
    for _ in 0..iterations {
        large_text.push_str(chunk);
    }
    large_text.push_str("UNIQUE_NEEDLE_FOR_BENCHMARK");

    // Measure write latency
    let write_start = Instant::now();
    let write_res = super::write_file(root, "bench_100kb.txt", &large_text);
    let write_duration = write_start.elapsed();
    assert!(write_res.is_ok());
    println!("[BENCHMARK] write_file 100KB: {:?}", write_duration);

    // Warm-up / let OS file creation filter locks settle
    std::thread::sleep(std::time::Duration::from_millis(50));

    // Measure edit latency on settled 100KB file
    let edit_start = Instant::now();
    let edit_res = super::edit_file(
        root,
        "bench_100kb.txt",
        "UNIQUE_NEEDLE_FOR_BENCHMARK",
        "REPLACED_SUCCESSFULLY",
    );
    let edit_duration = edit_start.elapsed();
    assert!(edit_res.is_ok());
    println!(
        "[BENCHMARK] edit_file in 100KB (single-handle): {:?}",
        edit_duration
    );

    // Verify both are well under threshold
    assert!(
        write_duration.as_millis() < 50,
        "write_file took too long: {:?}",
        write_duration
    );
    assert!(
        edit_duration.as_millis() < 50,
        "edit_file took too long: {:?}",
        edit_duration
    );
}

#[test]
fn executor_prepares_read_tool_with_flexible_arguments() {
    use super::{ToolExecutor, ToolPermissionMode};
    use crate::provider_schema::ToolCall;

    let directory = TestDirectory::new();
    directory.write("sample.txt", "line 1\nline 2\nline 3");

    let executor = ToolExecutor::new(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
    );

    let opencode_call = ToolCall {
        id: "call_read_1".to_owned(),
        name: "read".to_owned(),
        arguments: json!({
            "filePath": format!("{}/sample.txt", directory.root()),
            "offset": 2,
            "limit": 5
        }),
    };
    let prepared = executor
        .prepare_call(&opencode_call)
        .expect("prepare opencode read");
    assert!(matches!(
        prepared.operation,
        super::executor::ToolOperation::Read {
            start_line: 2,
            line_count: 5
        }
    ));

    let openchat_call = ToolCall {
        id: "call_read_2".to_owned(),
        name: "read_file".to_owned(),
        arguments: json!({
            "path": format!("{}/sample.txt", directory.root()),
            "startLine": 1,
            "lineCount": 10
        }),
    };
    let prepared_openchat = executor
        .prepare_call(&openchat_call)
        .expect("prepare openchat read_file");
    assert!(matches!(
        prepared_openchat.operation,
        super::executor::ToolOperation::Read {
            start_line: 1,
            line_count: 10
        }
    ));
}

#[test]
fn executor_prepares_and_handles_bash_tool() {
    use super::{ToolExecutor, ToolPermissionMode};
    use crate::provider_schema::ToolCall;

    let directory = TestDirectory::new();
    let executor = ToolExecutor::new(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
    );

    let bash_call = ToolCall {
        id: "call_bash_1".to_owned(),
        name: "bash".to_owned(),
        arguments: json!({
            "command": "git status"
        }),
    };
    let prepared = executor
        .prepare_call(&bash_call)
        .expect("prepare bash call");
    assert!(matches!(
        prepared.operation,
        super::executor::ToolOperation::Bash { .. }
    ));

    let empty_bash_call = ToolCall {
        id: "call_bash_2".to_owned(),
        name: "bash".to_owned(),
        arguments: json!({
            "command": "   "
        }),
    };
    assert!(executor.prepare_call(&empty_bash_call).is_err());

    let send_input_call = ToolCall {
        id: "call_input_1".to_owned(),
        name: "send_terminal_input".to_owned(),
        arguments: json!({
            "terminal_id": "term_test",
            "input": "yes\n"
        }),
    };
    let prepared_input = executor
        .prepare_call(&send_input_call)
        .expect("prepare send_terminal_input call");
    assert!(matches!(
        prepared_input.operation,
        super::executor::ToolOperation::SendTerminalInput { .. }
    ));
}

#[tokio::test]
async fn terminal_manager_runs_command_and_handles_input_and_kill() {
    use super::terminal::TerminalSessionManager;
    let directory = TestDirectory::new();
    let manager = TerminalSessionManager::isolated();

    // 1. Fast command execution test
    let exec_res = manager
        .execute(
            "echo openchat_terminal_ok",
            Path::new(directory.root()),
            Some(10),
            Some(10000),
        )
        .await
        .expect("execute command");
    assert_eq!(exec_res["is_running"], false);
    assert_eq!(exec_res["exit_code"], 0);
    assert!(
        exec_res["output"]
            .as_str()
            .unwrap_or("")
            .contains("openchat_terminal_ok")
    );

    let failing_command = "exit 17";
    let failed = manager
        .execute(
            failing_command,
            Path::new(directory.root()),
            Some(10),
            Some(10000),
        )
        .await
        .expect("execute nonzero command");
    assert_eq!(failed["is_running"], false);
    assert_eq!(failed["exit_code"], 17);

    // 2. Interactive session creation test
    #[cfg(windows)]
    let interactive_cmd = "$line = [Console]::ReadLine(); Write-Host ('INPUT_RECV:' + $line)";
    #[cfg(not(windows))]
    let interactive_cmd = "read line; echo \"INPUT_RECV:$line\"";

    let session = manager
        .create_session(interactive_cmd, Path::new(directory.root()), Some(30))
        .await
        .expect("create interactive session");

    let term_id = session.id.clone();
    assert!(session.is_running().await.expect("read process status"));

    // Send input to the waiting process
    let input_res = manager
        .send_input_to(&term_id, "interactive_message\n", Some(3000))
        .await
        .expect("send input to terminal");

    assert!(
        input_res["output"]
            .as_str()
            .unwrap_or("")
            .contains("INPUT_RECV:interactive_message")
    );

    // 3. Kill session test
    let kill_res = manager.kill_session(&term_id).await;
    // Session may have already finished after readLine or is killed
    let _ = kill_res;
    assert!(manager.get_session(&term_id).await.is_none());
    manager.stop_all().await.expect("stop remaining sessions");
}

#[tokio::test]
async fn terminal_manager_shutdown_stops_running_command_sessions() {
    use super::terminal::TerminalSessionManager;

    let directory = TestDirectory::new();
    let manager = TerminalSessionManager::isolated();
    #[cfg(windows)]
    let command = "Start-Sleep -Seconds 30";
    #[cfg(not(windows))]
    let command = "sleep 30";

    let result = manager
        .execute(command, Path::new(directory.root()), Some(60), Some(100))
        .await
        .expect("start a long-running command");
    let terminal_id = result["terminal_id"]
        .as_str()
        .expect("running command has a terminal id")
        .to_owned();
    let session = manager
        .get_session(&terminal_id)
        .await
        .expect("running session stays registered");
    assert!(session.is_running().await.expect("check running session"));

    manager
        .stop_all()
        .await
        .expect("stop sessions during shutdown");

    assert!(!session.is_running().await.expect("check stopped session"));
    assert!(manager.get_session(&terminal_id).await.is_none());
}

#[cfg(windows)]
#[tokio::test]
async fn terminal_manager_shutdown_stops_descendant_processes() {
    use super::terminal::TerminalSessionManager;
    use std::{fs, time::Duration};

    let directory = TestDirectory::new();
    let manager = TerminalSessionManager::isolated();
    let command = "$child = Start-Process -FilePath 'powershell.exe' -ArgumentList '-NoProfile -NonInteractive -Command Start-Sleep -Seconds 30' -PassThru; Set-Content -Path 'child-pid.txt' -Value $child.Id; Start-Sleep -Seconds 30";
    manager
        .execute(command, Path::new(directory.root()), Some(60), Some(100))
        .await
        .expect("start a shell with a long-running descendant");

    let pid_file = directory.0.join("child-pid.txt");
    let child_pid = tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            if let Ok(pid) = fs::read_to_string(&pid_file) {
                break pid.trim().parse::<u32>().expect("child pid is numeric");
            }
            tokio::time::sleep(Duration::from_millis(25)).await;
        }
    })
    .await
    .expect("descendant process starts");

    assert!(windows_process_is_running(child_pid));
    manager
        .stop_all()
        .await
        .expect("stop shell and descendant processes");
    assert!(!windows_process_is_running(child_pid));
}

#[cfg(windows)]
fn windows_process_is_running(pid: u32) -> bool {
    let Ok(output) = std::process::Command::new("tasklist.exe")
        .args(["/FI", &format!("PID eq {pid}"), "/FO", "CSV", "/NH"])
        .output()
    else {
        return false;
    };
    let pid = pid.to_string();
    String::from_utf8_lossy(&output.stdout).lines().any(|line| {
        line.split(',')
            .nth(1)
            .is_some_and(|field| field.trim_matches('"') == pid)
    })
}

#[tokio::test]
async fn simulate_real_world_agent_terminal_and_file_flow() {
    use super::executor::{ToolExecutor, ToolPermissionMode, execute_model_tool};
    use crate::provider_schema::ToolCall;

    let directory = TestDirectory::new();
    let executor = ToolExecutor::new(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
    );

    // Step 1: Write a workflow script file
    #[cfg(windows)]
    let script_name = "workflow.ps1";
    #[cfg(windows)]
    let script_content = "Write-Output STEP1_INIT_DONE\r\nWrite-Output ENTER_CODE:\r\n$code = [Console]::In.ReadLine()\r\nWrite-Output \"STEP2_CODE_IS:$code\"\r\n";

    #[cfg(not(windows))]
    let script_name = "workflow.sh";
    #[cfg(not(windows))]
    let script_content = "#!/bin/sh\necho \"STEP1_INIT_DONE\"\nprintf \"ENTER_CODE:\"\nread CODE\necho \"STEP2_CODE_IS:$CODE\"\n";

    let script_path = Path::new(directory.root())
        .join(script_name)
        .display()
        .to_string();

    let write_call = ToolCall {
        id: "call_write_1".to_owned(),
        name: "write_file".to_owned(),
        arguments: json!({
            "path": script_path,
            "content": script_content,
        }),
    };
    let prepared_write = executor
        .prepare_call(&write_call)
        .expect("prepare write_file");
    let write_result = execute_model_tool(&prepared_write).await;
    assert!(
        write_result.get("error").is_none(),
        "write_file failed: {write_result:?}"
    );

    // Step 2: Read file back to verify integrity
    let read_call = ToolCall {
        id: "call_read_1".to_owned(),
        name: "read_file".to_owned(),
        arguments: json!({
            "path": script_path,
        }),
    };
    let prepared_read = executor
        .prepare_call(&read_call)
        .expect("prepare read_file");
    let read_result = execute_model_tool(&prepared_read).await;
    assert!(read_result.get("error").is_none());
    assert!(read_result.to_string().contains("STEP1_INIT_DONE"));

    // Step 3: Execute the script interactively via execute_command / bash
    #[cfg(windows)]
    let run_cmd = format!(
        "& '{}'",
        Path::new(directory.root()).join(script_name).display()
    );
    #[cfg(not(windows))]
    let run_cmd = format!(
        "sh \"{}\"",
        Path::new(directory.root()).join(script_name).display()
    );

    let exec_call = ToolCall {
        id: "call_exec_1".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "command": run_cmd,
            "wait_ms": 1500,
        }),
    };
    let prepared_exec = executor
        .prepare_call(&exec_call)
        .expect("prepare execute_command");
    let exec_result = execute_model_tool(&prepared_exec).await;
    assert!(
        exec_result.get("error").is_none(),
        "exec failed: {exec_result:?}"
    );

    let term_id = exec_result["terminal_id"]
        .as_str()
        .unwrap_or_else(|| panic!("terminal_id should be returned: {exec_result}"))
        .to_owned();
    let initial_output = exec_result["output"].as_str().unwrap_or("");
    assert!(
        initial_output.contains("STEP1_INIT_DONE"),
        "initial output was: {initial_output}"
    );
    assert_eq!(
        exec_result["is_running"], true,
        "process should still be running waiting for input"
    );

    // Step 4: Send the expected input via send_terminal_input
    let input_call = ToolCall {
        id: "call_input_1".to_owned(),
        name: "send_terminal_input".to_owned(),
        arguments: json!({
            "terminal_id": term_id,
            "input": "AGENT_VERIFIED_777\n",
            "wait_ms": 2500,
        }),
    };
    let prepared_input = executor
        .prepare_call(&input_call)
        .expect("prepare send_terminal_input");
    let mut input_result = execute_model_tool(&prepared_input).await;
    assert!(
        input_result.get("error").is_none(),
        "send_terminal_input failed: {input_result:?}"
    );

    for _ in 0..60 {
        if input_result["is_running"] != true {
            break;
        }
        input_result = super::terminal::TerminalSessionManager::global()
            .read_output_of(&term_id, Some(500))
            .await
            .expect("interactive workflow output should remain readable");
    }
    if input_result["is_running"] == true {
        super::terminal::TerminalSessionManager::global()
            .kill_session(&term_id)
            .await
            .expect("timed-out interactive workflow should be terminated");
    }

    let after_input_output = input_result["output"].as_str().unwrap_or("");
    assert!(
        after_input_output.contains("STEP2_CODE_IS:AGENT_VERIFIED_777"),
        "expected code in output, got: {after_input_output}"
    );
    assert_eq!(
        input_result["is_running"], false,
        "process should have completed after receiving input"
    );
    assert_eq!(input_result["exit_code"], 0);

    // Step 5: Test edit_file on the script
    let edit_call = ToolCall {
        id: "call_edit_1".to_owned(),
        name: "edit_file".to_owned(),
        arguments: json!({
            "path": script_path,
            "old_string": "STEP1_INIT_DONE",
            "new_string": "STEP1_INITIALIZED_REVISED",
        }),
    };
    let prepared_edit = executor
        .prepare_call(&edit_call)
        .expect("prepare edit_file");
    let edit_result = execute_model_tool(&prepared_edit).await;
    assert!(
        edit_result.get("error").is_none(),
        "edit_file failed: {edit_result:?}"
    );

    // Verify revision by reading
    let read_revised = execute_model_tool(&prepared_read).await;
    assert!(
        read_revised
            .to_string()
            .contains("STEP1_INITIALIZED_REVISED")
    );

    // Step 6: Test long-running process and kill action via send_terminal_input
    #[cfg(windows)]
    let long_running_cmd = "Start-Sleep -Seconds 120";
    #[cfg(not(windows))]
    let long_running_cmd = "sleep 120";

    let long_exec_call = ToolCall {
        id: "call_long_exec".to_owned(),
        name: "bash".to_owned(),
        arguments: json!({
            "command": long_running_cmd,
            "wait_ms": 500,
        }),
    };
    let prepared_long_exec = executor
        .prepare_call(&long_exec_call)
        .expect("prepare long bash call");
    let long_result = execute_model_tool(&prepared_long_exec).await;
    assert_eq!(long_result["is_running"], true);
    let long_term_id = long_result["terminal_id"]
        .as_str()
        .expect("long term id")
        .to_owned();

    // Kill the long-running session
    let kill_call = ToolCall {
        id: "call_kill".to_owned(),
        name: "send_terminal_input".to_owned(),
        arguments: json!({
            "terminal_id": long_term_id,
            "action": "kill",
        }),
    };
    let prepared_kill = executor
        .prepare_call(&kill_call)
        .expect("prepare kill call");
    let kill_result = execute_model_tool(&prepared_kill).await;
    assert_eq!(kill_result["status"], "terminated");
}

#[tokio::test]
async fn simulate_terminal_edge_cases_and_exit_codes() {
    use super::executor::{ToolExecutor, ToolPermissionMode, execute_model_tool};
    use crate::provider_schema::ToolCall;

    let directory = TestDirectory::new();
    let executor = ToolExecutor::new(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::FullAccess,
    );

    // 1. Non-zero exit code simulation
    let non_zero_cmd = "exit 77";

    let exit_call = ToolCall {
        id: "call_exit_code".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "command": non_zero_cmd,
        }),
    };
    let prepared_exit = executor
        .prepare_call(&exit_call)
        .expect("prepare exit call");
    let exit_result = execute_model_tool(&prepared_exit).await;
    assert_eq!(exit_result["exit_code"], 77);
    assert_eq!(exit_result["is_running"], false);

    // 2. Reading output from an existing active terminal session
    #[cfg(windows)]
    let pause_cmd = "Write-Output READY_FOR_READ; [void][Console]::In.ReadLine()";
    #[cfg(not(windows))]
    let pause_cmd = "sh -c \"echo READY_FOR_READ; read -r dummy\"";

    let start_call = ToolCall {
        id: "call_start_read".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "command": pause_cmd,
            "wait_ms": 1000,
        }),
    };
    let prepared_start = executor
        .prepare_call(&start_call)
        .expect("prepare pause call");
    let start_result = execute_model_tool(&prepared_start).await;
    let tid = start_result["terminal_id"]
        .as_str()
        .expect("tid")
        .to_owned();
    assert_eq!(start_result["is_running"], true);
    assert!(
        start_result["output"]
            .as_str()
            .unwrap_or("")
            .contains("READY_FOR_READ")
    );

    // Call execute_command with terminal_id and action: "read"
    let read_call = ToolCall {
        id: "call_read_term".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "terminal_id": tid,
            "action": "read",
            "wait_ms": 200,
        }),
    };
    let prepared_read = executor
        .prepare_call(&read_call)
        .expect("prepare read call");
    let read_result = execute_model_tool(&prepared_read).await;
    assert_eq!(read_result["terminal_id"], tid);
    assert_eq!(read_result["is_running"], true);

    // 3. Send enter key to finish pause via execute_command with terminal_id and input
    let send_finish = ToolCall {
        id: "call_finish".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "terminal_id": tid,
            "input": "\n",
            "wait_ms": 2000,
        }),
    };
    let prepared_finish = executor.prepare_call(&send_finish).expect("prepare finish");
    let finish_result = execute_model_tool(&prepared_finish).await;
    assert_eq!(finish_result["is_running"], false);
    assert_eq!(
        finish_result["exit_code"], 0,
        "finish result: {finish_result}"
    );

    // 4. Invalid terminal session id error handling
    let stale_call = ToolCall {
        id: "call_stale".to_owned(),
        name: "send_terminal_input".to_owned(),
        arguments: json!({
            "terminal_id": "non_existent_terminal_9999",
            "input": "test\n",
        }),
    };
    let prepared_stale = executor
        .prepare_call(&stale_call)
        .expect("prepare stale call");
    let stale_result = execute_model_tool(&prepared_stale).await;
    assert!(stale_result.get("error").is_some());
    assert_eq!(stale_result["error"]["code"], "terminal_error");
}

#[tokio::test]
async fn simulate_named_project_task_and_terminal_permission_approval_flow() {
    use super::executor::{ToolExecutor, ToolPermissionMode};
    use crate::{
        permissions::ToolPermissionBroker,
        protocol::EventSink,
        provider_schema::{ChatStreamSnapshot, ToolCall},
        storage::AppStorage,
        user_question_broker::UserQuestionBroker,
    };
    use std::sync::Arc;
    use tokio::sync::watch;

    let directory = TestDirectory::new();
    let storage = Arc::new(
        AppStorage::open_at(PathBuf::from(directory.root())).expect("open isolated test storage"),
    );
    storage
        .connect()
        .expect("connect isolated test storage")
        .execute_batch(
            "CREATE TABLE messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER,
                tool_activities TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL,
                UNIQUE(conversation_id, id)
            );",
        )
        .expect("create assistant message checkpoint schema");
    fs::create_dir_all(Path::new(directory.root()).join(".openchat"))
        .expect("create named project task directory");
    fs::write(
        Path::new(directory.root()).join(".openchat/tasks.json"),
        r#"{"version":1,"tasks":[{"id":"verify","command":"echo USER_APPROVED_EXECUTION","timeoutSeconds":20}]}"#,
    )
    .expect("write named project task catalog");
    let broker = ToolPermissionBroker::default();
    let events = EventSink::new();
    let request_id = json!("test_req_1");
    let snapshot = ChatStreamSnapshot::new("conv_1", "msg_1", "", 0);

    // 1. Approval Granted Case
    let call_allow = ToolCall {
        id: "call_allow_1".to_owned(),
        name: "run_project_task".to_owned(),
        arguments: json!({"task": "verify"}),
    };

    let (_cancellation_tx, mut cancellation_rx) = watch::channel(false);
    let broker_clone = broker.clone();

    let exec_task = tokio::spawn({
        let mut allowed_tools = super::definitions()
            .into_iter()
            .map(|tool| tool.name)
            .collect::<Vec<_>>();
        allowed_tools.push(super::project_tasks::tool_definition(&["verify".to_owned()]).name);
        let executor = ToolExecutor::with_allowed_tool_names(
            Some(Path::new(directory.root())),
            Path::new(directory.root()),
            ToolPermissionMode::RequireApproval,
            allowed_tools,
        );
        let request_id = request_id.clone();
        let snapshot = snapshot.clone();
        let events = events.clone();
        let storage = Arc::clone(&storage);
        let user_questions = UserQuestionBroker::default();
        async move {
            executor
                .execute_call(
                    &call_allow,
                    &broker_clone,
                    &request_id,
                    &snapshot,
                    &events,
                    &mut cancellation_rx,
                    &storage,
                    "test_run_1",
                    "opencode",
                    &user_questions,
                )
                .await
        }
    });

    // Wait until permission is requested
    let mut pending_id = None;
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        let pending = broker.pending.lock().await;
        if let Some(first_id) = pending.keys().next() {
            pending_id = Some(first_id.clone());
            break;
        }
    }
    let approval_id = pending_id.expect("approval should be requested");

    // Simulate user approving the command execution in UI
    let respond_res = broker.respond(&approval_id, true).await;
    assert!(respond_res.is_ok());

    let mut result = exec_task.await.expect("join task").expect("execute_call");
    for _ in 0..60 {
        if result.output["is_running"] != true {
            break;
        }
        let terminal_id = result.output["terminal_id"]
            .as_str()
            .expect("running command should expose its terminal session")
            .to_owned();
        result.output = super::terminal::TerminalSessionManager::global()
            .read_output_of(&terminal_id, Some(500))
            .await
            .expect("approved command output should remain readable");
    }
    if result.output["is_running"] == true {
        if let Some(terminal_id) = result.output["terminal_id"].as_str() {
            super::terminal::TerminalSessionManager::global()
                .kill_session(terminal_id)
                .await
                .expect("timed-out approved command should be terminated");
        }
    }
    assert!(
        result.output["output"]
            .as_str()
            .unwrap_or("")
            .contains("USER_APPROVED_EXECUTION")
    );
    assert_eq!(result.output["exit_code"], 0);

    // 2. Approval Denied Case
    let call_deny = ToolCall {
        id: "call_deny_1".to_owned(),
        name: "execute_command".to_owned(),
        arguments: json!({
            "command": "echo USER_DENIED_EXECUTION",
        }),
    };

    let (_cancellation_tx2, mut cancellation_rx2) = watch::channel(false);
    let broker_clone2 = broker.clone();

    let deny_task = tokio::spawn({
        let executor = ToolExecutor::new(
            Some(Path::new(directory.root())),
            Path::new(directory.root()),
            ToolPermissionMode::RequireApproval,
        );
        let request_id = request_id.clone();
        let snapshot = snapshot.clone();
        let events = events.clone();
        let storage = Arc::clone(&storage);
        let user_questions = UserQuestionBroker::default();
        async move {
            executor
                .execute_call(
                    &call_deny,
                    &broker_clone2,
                    &request_id,
                    &snapshot,
                    &events,
                    &mut cancellation_rx2,
                    &storage,
                    "test_run_2",
                    "opencode",
                    &user_questions,
                )
                .await
        }
    });

    let mut deny_pending_id = None;
    for _ in 0..50 {
        tokio::time::sleep(tokio::time::Duration::from_millis(20)).await;
        let pending = broker.pending.lock().await;
        if let Some(first_id) = pending.keys().next() {
            deny_pending_id = Some(first_id.clone());
            break;
        }
    }
    let deny_approval_id = deny_pending_id.expect("deny approval should be requested");

    // Simulate user clicking "Deny" in UI
    let deny_res = broker.respond(&deny_approval_id, false).await;
    assert!(deny_res.is_ok());

    let deny_result = deny_task.await.expect("join task").expect("execute_call");
    assert_eq!(deny_result.output["error"]["code"], "permission_denied");
}

#[tokio::test]
async fn project_deny_rule_blocks_file_write_without_requesting_approval() {
    use super::executor::{ToolExecutor, ToolPermissionMode, ToolPermissionRule};
    use crate::{
        permissions::ToolPermissionBroker,
        protocol::EventSink,
        provider_schema::{ChatStreamSnapshot, ToolCall},
        storage::AppStorage,
        user_question_broker::UserQuestionBroker,
    };
    use std::{collections::HashMap, sync::Arc};
    use tokio::sync::watch;

    let directory = TestDirectory::new();
    let storage = Arc::new(
        AppStorage::open_at(PathBuf::from(directory.root())).expect("open isolated test storage"),
    );
    storage
        .connect()
        .expect("connect isolated test storage")
        .execute_batch(
            "CREATE TABLE messages (
                id TEXT NOT NULL,
                conversation_id TEXT NOT NULL,
                role TEXT NOT NULL,
                content TEXT NOT NULL,
                created_at INTEGER,
                tool_activities TEXT NOT NULL DEFAULT '[]',
                status TEXT NOT NULL,
                UNIQUE(conversation_id, id)
            );",
        )
        .expect("create assistant message activity schema");

    let mut permission_rules = HashMap::new();
    permission_rules.insert("write_file".to_owned(), ToolPermissionRule::Deny);
    let executor = ToolExecutor::with_permission_rules(
        Some(Path::new(directory.root())),
        Path::new(directory.root()),
        ToolPermissionMode::RequireApproval,
        ["write_file".to_owned()],
        permission_rules,
    );
    let call = ToolCall {
        id: "write-blocked".to_owned(),
        name: "write_file".to_owned(),
        arguments: json!({"path": "blocked.txt", "content": "must not be written"}),
    };
    let broker = ToolPermissionBroker::default();
    let events = EventSink::new();
    let request_id = json!("test_request");
    let snapshot = ChatStreamSnapshot::new("conversation", "message", "", 0);
    let user_questions = UserQuestionBroker::default();
    let (_cancellation_sender, mut cancellation) = watch::channel(false);

    let result = executor
        .execute_call(
            &call,
            &broker,
            &request_id,
            &snapshot,
            &events,
            &mut cancellation,
            &storage,
            "run",
            "test-provider",
            &user_questions,
        )
        .await
        .expect("denied tool call returns a tool result");

    assert_eq!(result.output["error"]["code"], "permission_denied");
    assert!(!Path::new(directory.root()).join("blocked.txt").exists());
    assert!(broker.pending.lock().await.is_empty());
}

#[test]
fn web_search_clean_html_tags_and_ddg_html_parsing() {
    use super::web_search::{clean_html_tags, parse_duckduckgo_html};

    let messy_text =
        "<b>Hello &amp; Welcome</b> to the &quot;OpenChat&quot; test! &#39;Fast&#39; &lt;&gt;";
    assert_eq!(
        clean_html_tags(messy_text),
        "Hello & Welcome to the \"OpenChat\" test! 'Fast' <>"
    );

    let sample_ddg_html = r#"
        <div class="result results_links">
            <h2 class="result__title">
                <a class="result__a" href="/l/?kh=-1&uddg=https%3A%2F%2Fexample.com%2Ftarget">Example <b>Title</b></a>
            </h2>
            <a class="result__snippet" href="/l/?kh=-1&uddg=https%3A%2F%2Fexample.com%2Ftarget">This is the <b>snippet</b> description.</a>
        </div>
    "#;

    let parsed = parse_duckduckgo_html(sample_ddg_html, 5);
    assert_eq!(parsed.len(), 1);
    assert_eq!(parsed[0].title, "Example Title");
    assert_eq!(parsed[0].url, "https://example.com/target");
    assert_eq!(parsed[0].snippet, "This is the snippet description.");
    assert_eq!(parsed[0].engine, "duckduckgo");
}

#[test]
fn web_search_tool_wire_and_internal_names() {
    use super::{internal_tool_name, opencode_wire_name};

    assert_eq!(opencode_wire_name("web_search"), "web_search");
    assert_eq!(opencode_wire_name("read_url_content"), "read_url_content");
    assert_eq!(opencode_wire_name("read_url"), "read_url_content");

    assert_eq!(internal_tool_name(true, "web_search"), "web_search");
    assert_eq!(
        internal_tool_name(true, "read_url_content"),
        "read_url_content"
    );
    assert_eq!(internal_tool_name(true, "read_url"), "read_url_content");
}

#[test]
fn tool_executor_prepares_web_search_and_read_url() {
    use super::executor::{ToolExecutor, ToolOperation, ToolPermissionMode};
    use crate::provider_schema::ToolCall;

    let temp_dir = std::env::temp_dir();
    let executor = ToolExecutor::new(
        Some(&temp_dir),
        &temp_dir,
        ToolPermissionMode::RequireApproval,
    );

    // 1. Valid web_search
    let call = ToolCall {
        id: "call-ws-1".to_owned(),
        name: "web_search".to_owned(),
        arguments: json!({"query": "rust async tokio", "limit": 3}),
    };
    let prepared = executor.prepare_call(&call).expect("prepare web_search");
    assert_eq!(prepared.requested_path, "rust async tokio");
    if let ToolOperation::WebSearch { query, limit } = prepared.operation {
        assert_eq!(query, "rust async tokio");
        assert_eq!(limit, Some(3));
    } else {
        panic!("expected WebSearch operation");
    }

    // 2. Empty query error
    let invalid_call = ToolCall {
        id: "call-ws-2".to_owned(),
        name: "web_search".to_owned(),
        arguments: json!({"query": "   "}),
    };
    assert!(executor.prepare_call(&invalid_call).is_err());

    // 3. Valid read_url_content
    let read_call = ToolCall {
        id: "call-ru-1".to_owned(),
        name: "read_url_content".to_owned(),
        arguments: json!({"url": "https://example.com/article", "max_chars": 2000}),
    };
    let prepared_read = executor
        .prepare_call(&read_call)
        .expect("prepare read_url_content");
    assert_eq!(prepared_read.requested_path, "https://example.com/article");
    if let ToolOperation::ReadUrlContent { url, max_chars } = prepared_read.operation {
        assert_eq!(url, "https://example.com/article");
        assert_eq!(max_chars, Some(2000));
    } else {
        panic!("expected ReadUrlContent operation");
    }
}

#[tokio::test]
#[ignore = "manual live web smoke test requires public network access"]
async fn live_manual_benchmark_web_search_and_read_url() {
    use super::web_search::{execute_read_url, execute_web_search};
    use std::time::Instant;

    println!("\n========== [CANLI TEST BASLANGICI] ==========");

    // 1. Web Search Testi (Limit: 10)
    let query = "Rust tokio tutorial";
    let start_search = Instant::now();
    let search_res = execute_web_search(query, Some(10)).await;
    let search_elapsed = start_search.elapsed();

    println!("1. WEB SEARCH SONUCU (Limit: 10):");
    println!("Sorgu: \"{}\"", query);
    println!("Gecikme: {} ms", search_elapsed.as_millis());

    let mut first_url = None;
    match search_res {
        Ok(val) => {
            println!("Durum: Basarili");
            let total = val["total_results"].as_i64().unwrap_or(0);
            println!("Toplam sonuc sayisi: {}", total);
            if let Some(results) = val["results"].as_array() {
                for (i, item) in results.iter().enumerate() {
                    let title = item["title"].as_str().unwrap_or("");
                    let url = item["url"].as_str().unwrap_or("");
                    let engine = item["engine"].as_str().unwrap_or("");
                    let snippet = item["snippet"].as_str().unwrap_or("");
                    println!("  [{}] Motor: {} | Baslik: {}", i + 1, engine, title);
                    println!("      URL: {}", url);
                    println!("      Ozet: {}", snippet);
                    if first_url.is_none() && !url.is_empty() {
                        first_url = Some(url.to_owned());
                    }
                }
            }
        }
        Err(err) => {
            println!("Hata: {}", err);
        }
    }

    println!("\n--------------------------------------------");

    // 2. Read URL Content Testi
    let target_url = first_url.unwrap_or_else(|| "https://example.com".to_owned());
    let start_read = Instant::now();
    let read_res = execute_read_url(&target_url, Some(3000)).await;
    let read_elapsed = start_read.elapsed();

    println!("2. READ URL CONTENT SONUCU:");
    println!("Hedef URL: {}", target_url);
    println!("Gecikme: {} ms", read_elapsed.as_millis());

    match read_res {
        Ok(val) => {
            println!("Durum: Basarili");
            println!("Baslik: {}", val["title"].as_str().unwrap_or(""));
            println!("Karakter Uzunlugu: {}", val["length"]);
            println!("Kesildi mi (truncated): {}", val["truncated"]);
            let content = val["content"].as_str().unwrap_or("");
            let preview: String = content.chars().take(500).collect();
            println!(
                "\nIcerik Onizleme (Ilk 500 karakter):\n---\n{}\n---",
                preview
            );
        }
        Err(err) => {
            println!("Hata: {}", err);
        }
    }

    // 3. Turkce Arama ve Sayfa Okuma Testi (Limit: 10)
    println!("\n--------------------------------------------");
    let tr_query = "Turkiye yapay zeka ekosistemi";
    let start_tr = Instant::now();
    let tr_res = execute_web_search(tr_query, Some(10)).await;
    let tr_elapsed = start_tr.elapsed();

    println!("3. TURKCE ARAMA SONUCU (Limit: 10):");
    println!("Sorgu: \"{}\"", tr_query);
    println!("Gecikme: {} ms", tr_elapsed.as_millis());
    let mut tr_url = None;
    if let Ok(val) = tr_res
        && let Some(results) = val["results"].as_array()
    {
        for (i, item) in results.iter().enumerate() {
            let title = item["title"].as_str().unwrap_or("");
            let url = item["url"].as_str().unwrap_or("");
            let engine = item["engine"].as_str().unwrap_or("");
            println!("  [{}] Motor: {} | Baslik: {}", i + 1, engine, title);
            println!("      URL: {}", url);
            if tr_url.is_none() && !url.is_empty() {
                tr_url = Some(url.to_owned());
            }
        }
    }

    if let Some(url) = tr_url {
        let start_tr_read = Instant::now();
        let tr_read_res = execute_read_url(&url, Some(1500)).await;
        let tr_read_elapsed = start_tr_read.elapsed();
        println!(
            "\nTURKCE SAYFA OKUMA Gecikmesi: {} ms",
            tr_read_elapsed.as_millis()
        );
        if let Ok(val) = tr_read_res {
            println!("Sayfa Basligi: {}", val["title"].as_str().unwrap_or(""));
            println!("Okunan Karakter: {}", val["length"]);
        }
    }

    // 4. Cache Testi
    println!("\n--------------------------------------------");
    let start_cache = Instant::now();
    let cache_res = execute_web_search(query, Some(5)).await;
    let cache_elapsed = start_cache.elapsed();
    println!("4. ONBELLEK (CACHE) HIZ TESTI:");
    println!("Ayni sorgu tekrar calistirildi.");
    println!(
        "Onbellek Gecikmesi: {} mikro-saniye ({} ms)",
        cache_elapsed.as_micros(),
        cache_elapsed.as_millis()
    );
    assert!(cache_res.is_ok());

    println!("========== [CANLI TEST BITTI] ==========\n");
}
