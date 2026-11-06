//! Split from tests.rs (8 items).
use super::*;

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
    let root = super::super::canonical_root(directory.root()).expect("canonical workspace root");
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

    let all = super::super::search::search_files_direct(
        &root,
        &root,
        "parallel marker",
        false,
        false,
        0,
        40,
    )
    .expect("search parallel candidates");
    assert_eq!(
        all["matches"],
        json!([
            {"path": first_match_path.strip_prefix(&root).expect("relative path").to_string_lossy().replace('\\', "/"), "line": 1, "text": "parallel marker first"},
            {"path": second_match_path.strip_prefix(&root).expect("relative path").to_string_lossy().replace('\\', "/"), "line": 1, "text": "parallel marker second"},
        ])
    );

    let first = super::super::search::search_files_direct(
        &root,
        &root,
        "parallel marker",
        false,
        false,
        0,
        1,
    )
    .expect("search first page");
    let second = super::super::search::search_files_direct(
        &root,
        &root,
        "parallel marker",
        false,
        false,
        1,
        1,
    )
    .expect("search second page");
    assert_eq!(first["matches"][0], all["matches"][0]);
    assert_eq!(first["nextOffset"], json!(1));
    assert_eq!(second["matches"][0], all["matches"][1]);
    assert!(second["nextOffset"].is_null());
}

#[test]
fn direct_search_keeps_pagination_order_across_candidate_batches() {
    let directory = TestDirectory::new();
    let file_count = super::super::search::DIRECT_SEARCH_INITIAL_BATCH_FILES + 4;
    for file_index in 0..file_count {
        directory.write(&format!("file_{file_index:05}.txt"), "no match here\n");
    }
    directory.write(
        &format!(
            "file_{:05}.txt",
            super::super::search::DIRECT_SEARCH_INITIAL_BATCH_FILES + 1
        ),
        "boundary marker first\n",
    );
    directory.write(
        &format!(
            "file_{:05}.txt",
            super::super::search::DIRECT_SEARCH_INITIAL_BATCH_FILES + 2
        ),
        "boundary marker second\n",
    );

    let root = super::super::canonical_root(directory.root()).expect("canonical workspace root");
    let first = super::super::search::search_files_direct(
        &root,
        &root,
        "boundary marker",
        false,
        false,
        0,
        1,
    )
    .expect("search first page across candidate batches");
    let second = super::super::search::search_files_direct(
        &root,
        &root,
        "boundary marker",
        false,
        false,
        1,
        1,
    )
    .expect("search second page across candidate batches");

    assert_eq!(first["matches"][0]["text"], json!("boundary marker first"));
    assert_eq!(
        first["matches"][0]["path"],
        json!(format!(
            "file_{:05}.txt",
            super::super::search::DIRECT_SEARCH_INITIAL_BATCH_FILES + 1
        ))
    );
    assert_eq!(first["nextOffset"], json!(1));
    assert_eq!(
        second["matches"][0]["text"],
        json!("boundary marker second")
    );
    assert_eq!(
        second["matches"][0]["path"],
        json!(format!(
            "file_{:05}.txt",
            super::super::search::DIRECT_SEARCH_INITIAL_BATCH_FILES + 2
        ))
    );
    assert!(second["nextOffset"].is_null());
}

#[test]
fn direct_search_enforces_the_total_byte_budget_across_serial_and_parallel_reads() {
    let directory = TestDirectory::new();
    directory.write("file_000.txt", "aa");
    directory.write("file_001.txt", "bb");
    for file_index in 2..(super::super::search::DIRECT_SEARCH_INITIAL_BATCH_FILES + 2) {
        directory.write(&format!("file_{file_index:03}.txt"), "12345678");
    }

    let root = super::super::canonical_root(directory.root()).expect("canonical workspace root");
    for byte_budget in [3, 20] {
        let (result, bytes_read) = super::super::search::search_files_direct_with_byte_budget(
            &root,
            &root,
            "needle",
            false,
            false,
            0,
            1,
            byte_budget,
        )
        .expect("search within the supplied byte budget");

        assert_eq!(bytes_read, byte_budget);
        assert_eq!(result["matches"], json!([]));
        assert_eq!(result["truncated"], true);
    }
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
fn read_search_write_and_edit_reject_hard_linked_files() {
    let directory = TestDirectory::new();
    let root = directory.0.join("root");
    let outside_file = directory.0.join("outside.txt");
    fs::create_dir(&root).expect("create active tool root");
    fs::write(&outside_file, "original content").expect("create outside file");
    fs::hard_link(&outside_file, root.join("write_alias.txt")).expect("create write hard link");
    fs::hard_link(&outside_file, root.join("edit_alias.txt")).expect("create edit hard link");
    fs::hard_link(&outside_file, root.join("read_alias.txt")).expect("create read hard link");
    fs::hard_link(&outside_file, root.join("search_alias.txt")).expect("create search hard link");

    assert!(
        super::super::read_file(&root, "read_alias.txt", 1, 10).is_err(),
        "reading a hard-linked file must fail"
    );
    let search = super::super::search_files(&root, "", "original content", false, 0, 40)
        .expect("search safely skips hard-linked files");
    assert!(
        search["matches"]
            .as_array()
            .is_some_and(|matches| matches.is_empty())
    );
    assert_eq!(search["truncated"], true);

    assert!(
        super::super::write_file(&root, "write_alias.txt", "replacement content").is_err(),
        "overwriting a hard-linked file must fail"
    );
    assert!(
        super::super::edit_file(&root, "edit_alias.txt", "original", "replacement").is_err(),
        "editing a hard-linked file must fail"
    );
    assert_eq!(
        fs::read_to_string(outside_file).expect("read outside file after rejected operations"),
        "original content",
        "rejected operations must leave the linked file unchanged"
    );
}
