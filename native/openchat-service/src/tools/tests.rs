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
        let path =
            std::env::temp_dir().join(format!("openchat-tools-test-{}-{id}", std::process::id()));
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
        "dataset_files=501 dataset_bytes={dataset_bytes} benchmark_iterations=25 warmup_iterations=1"
    );
    for measurement in measurements {
        eprintln!(
            "tool={} p50_ms={:.3} p95_ms={:.3} output_bytes={} output_chars={} rough_tokens_chars_div_4={}",
            measurement.name,
            measurement.p50.as_secs_f64() * 1000.0,
            measurement.p95.as_secs_f64() * 1000.0,
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
    let listing = measure_iterations("list_files", 7, || {
        list_files(directory.root(), "src", 0, 100).expect("benchmark large listing")
    });
    let cold_started = Instant::now();
    let cold = search_files(directory.root(), "", "no-such-marker", false, 0, 40)
        .expect("benchmark cold full search");
    let cold_elapsed = cold_started.elapsed();
    let canonical_root =
        super::canonical_root(directory.root()).expect("canonical benchmark workspace root");
    let source_directory = canonical_root.join("src");
    let direct_listing = measure_iterations("list_files_direct_no_app_cache", 3, || {
        let listing = super::read_directory_listing(&source_directory, false, 100)
            .expect("benchmark direct directory listing");
        super::directory_listing_response(Path::new("src"), &listing, 0, 100)
    });
    let direct_search = measure_iterations("search_files_direct_no_app_cache", 3, || {
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
    let late_match = measure_iterations("search_files_late_match_no_app_cache", 3, || {
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
    let repeated = measure_iterations("search_files_direct_no_app_cache_repeat", 7, || {
        search_files(directory.root(), "", "another-no-marker", false, 0, 40)
            .expect("benchmark repeated direct full search")
    });
    let common = measure_iterations("search_files_common_term_no_app_cache", 7, || {
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
    output_bytes: usize,
    output_chars: usize,
}

fn measure(name: &'static str, operation: impl FnMut() -> serde_json::Value) -> Measurement {
    measure_iterations(name, 25, operation)
}

fn measure_iterations(
    name: &'static str,
    iterations: usize,
    mut operation: impl FnMut() -> serde_json::Value,
) -> Measurement {
    let _ = operation();
    let mut durations = Vec::with_capacity(iterations);
    let mut output = Vec::new();
    for _ in 0..iterations {
        let started = Instant::now();
        let result = operation();
        durations.push(started.elapsed());
        output = serde_json::to_vec(&result).expect("serialize benchmark output");
    }
    durations.sort_unstable();
    let output_text = std::str::from_utf8(&output).expect("tool output is valid UTF-8");
    let p95_index = (durations.len() - 1) * 95 / 100;
    Measurement {
        name,
        p50: durations[durations.len() / 2],
        p95: durations[p95_index],
        output_bytes: output.len(),
        output_chars: output_text.chars().count(),
    }
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

    // Measure edit latency (replaces unique needle in 100KB file)
    let edit_start = Instant::now();
    let edit_res = super::edit_file(
        root,
        "bench_100kb.txt",
        "UNIQUE_NEEDLE_FOR_BENCHMARK",
        "REPLACED_SUCCESSFULLY",
    );
    let edit_duration = edit_start.elapsed();
    assert!(edit_res.is_ok());
    println!("[BENCHMARK] edit_file in 100KB: {:?}", edit_duration);

    // Verify the write duration is well under 50ms (typically < 3ms in Rust with BufWriter)
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
}
