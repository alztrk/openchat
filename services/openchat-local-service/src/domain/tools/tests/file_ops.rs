//! Split from tests.rs (15 items).
use super::*;

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
fn reading_returns_requested_lines_and_continuation() {
    let directory = TestDirectory::new();
    directory.write("notes.txt", "one\ntwo\nthree\n");

    let result = read_file(directory.root(), "notes.txt", 2, 1).expect("read line range");
    assert_eq!(result["d"], json!([[2, "two"]]));
    assert_eq!(result["n"], json!(3));
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

    assert_eq!(result["d"], json!([[2, "requested line"]]));
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
#[cfg(windows)]
fn write_file_rejects_parent_symlink_escape() {
    use std::os::windows::fs::symlink_dir;

    let directory = TestDirectory::new();
    let root = directory.0.join("root");
    let outside = directory.0.join("outside");
    fs::create_dir(&root).expect("create active tool root");
    fs::create_dir(&outside).expect("create outside directory");
    symlink_dir(&outside, root.join("redirect")).expect("create directory symlink");

    let result = super::super::write_file(&root, "redirect/escape.txt", "must stay inside root");

    assert!(
        result.is_err(),
        "writes through an escaping symlink must fail"
    );
    assert!(
        !outside.join("escape.txt").exists(),
        "a rejected write must not create a file outside the active root"
    );
}

#[cfg(windows)]
#[test]
fn file_tools_reject_an_external_root_symlink() {
    use std::os::windows::fs::symlink_dir;

    let directory = TestDirectory::new();
    let external = TestDirectory::new();
    let external_file = external.0.join("secret.txt");
    fs::write(&external_file, "keep private").expect("write external file");
    let root_link = directory.0.join("linked-root");
    symlink_dir(&external.0, &root_link).expect("create external root symlink");

    assert!(super::super::read_file(&root_link, "secret.txt", 1, 10).is_err());
    assert!(super::super::write_file(&root_link, "secret.txt", "replace").is_err());
    assert!(super::super::edit_file(&root_link, "secret.txt", "keep", "replace").is_err());
    assert_eq!(
        fs::read_to_string(external_file).expect("read external file after rejected operations"),
        "keep private"
    );
}

#[test]
#[cfg(windows)]
fn read_and_edit_reject_file_symlink_escape() {
    use std::os::windows::fs::symlink_file;

    let directory = TestDirectory::new();
    let root = directory.0.join("root");
    let outside_file = directory.0.join("outside.txt");
    fs::create_dir(&root).expect("create active tool root");
    fs::write(&outside_file, "outside content").expect("create outside file");
    symlink_file(&outside_file, root.join("redirect.txt")).expect("create file symlink");

    assert!(
        super::super::read_file(&root, "redirect.txt", 1, 10).is_err(),
        "reads through an escaping file symlink must fail"
    );
    assert!(
        super::super::edit_file(&root, "redirect.txt", "outside", "changed").is_err(),
        "edits through an escaping file symlink must fail"
    );
    assert_eq!(
        fs::read_to_string(outside_file).expect("read outside file after rejected operations"),
        "outside content",
        "rejected operations must leave the outside file unchanged"
    );
}

#[test]
fn write_file_and_edit_file_flow_and_guards() {
    let directory = TestDirectory::new();
    let root = directory.root();

    // 1. Write new file
    let write_res = super::super::write_file(root, "nested/hello.txt", "Hello OpenChat World!");
    assert!(
        write_res.is_ok(),
        "write_file should succeed for new file in subfolder"
    );
    let val = write_res.unwrap();
    assert_eq!(val["bytesWritten"], 21);
    assert_eq!(val["success"], true);

    // Verify content
    let read_res = super::super::read_file(root, "nested/hello.txt", 1, 10).unwrap();
    assert_eq!(read_res["d"][0][1], "Hello OpenChat World!");

    // 2. Edit file (single replacement)
    let edit_res = super::super::edit_file(root, "nested/hello.txt", "World", "Alican");
    assert!(
        edit_res.is_ok(),
        "edit_file should succeed when target matches exactly once"
    );
    let edit_val = edit_res.unwrap();
    assert_eq!(edit_val["replacements"], 1);

    // Verify edited content
    let read_edited = super::super::read_file(root, "nested/hello.txt", 1, 10).unwrap();
    assert_eq!(read_edited["d"][0][1], "Hello OpenChat Alican!");

    // 3. Ambiguity guard: when oldString matches multiple times
    super::super::write_file(root, "ambiguous.txt", "test foo bar foo baz").unwrap();
    let amb_res = super::super::edit_file(root, "ambiguous.txt", "foo", "qux");
    assert!(
        amb_res.is_err(),
        "edit_file must reject multiple occurrences to avoid ambiguous edits"
    );

    // 4. Not found guard
    let not_found_res = super::super::edit_file(root, "ambiguous.txt", "nonexistent", "qux");
    assert!(
        not_found_res.is_err(),
        "edit_file must reject when target string is not found"
    );

    // 5. Empty target string guard
    let empty_target = super::super::edit_file(root, "ambiguous.txt", "", "qux");
    assert!(
        empty_target.is_err(),
        "edit_file must reject empty old_string"
    );
}

#[test]
fn direct_file_write_stays_bound_to_its_opened_root() {
    let directory = TestDirectory::new();
    let root = directory.0.join("workspace");
    let moved_root = directory.0.join("moved-workspace");
    fs::create_dir(&root).expect("create selected workspace");
    fs::write(root.join("note.txt"), "before").expect("create workspace file");
    let root_capability = crate::filesystem::open_absolute_directory_no_follow(&root)
        .expect("open selected workspace capability");

    #[cfg(unix)]
    {
        fs::rename(&root, &moved_root).expect("move original workspace root");
        fs::create_dir(&root).expect("create replacement workspace root");
        fs::write(root.join("note.txt"), "replacement").expect("create replacement workspace file");
    }

    #[cfg(windows)]
    assert!(
        fs::rename(&root, &moved_root).is_err(),
        "an open workspace capability must prevent root replacement"
    );

    let _file_change_guard = crate::file_changes::lock_file_changes()
        .expect("hold file-change lock for the direct tool mutation");
    super::super::write_file_with_file_change_lock_held(&root_capability, "note.txt", "written")
        .expect("write through the selected workspace capability");
    super::super::edit_file_with_file_change_lock_held(
        &root_capability,
        "note.txt",
        "written",
        "edited",
    )
    .expect("edit through the selected workspace capability");

    #[cfg(unix)]
    {
        assert_eq!(
            fs::read_to_string(moved_root.join("note.txt")).expect("read original workspace"),
            "edited"
        );
        assert_eq!(
            fs::read_to_string(root.join("note.txt")).expect("read replacement workspace"),
            "replacement"
        );
    }

    #[cfg(windows)]
    assert_eq!(
        fs::read_to_string(root.join("note.txt")).expect("read selected workspace"),
        "edited"
    );
}

#[cfg(windows)]
#[test]
fn direct_file_mutation_handle_blocks_same_path_writes_and_replacement() {
    use std::fs::OpenOptions as StandardOpenOptions;

    let directory = TestDirectory::new();
    directory.write("note.txt", "before");
    let root = crate::filesystem::open_absolute_directory_no_follow(&directory.0)
        .expect("open workspace capability");
    let path = directory.0.join("note.txt");
    let moved_path = directory.0.join("moved-note.txt");

    let writer = StandardOpenOptions::new()
        .write(true)
        .open(&path)
        .expect("open competing writer before the mutation handle");
    let mut options = cap_std::fs::OpenOptions::new();
    options.read(true).write(true);
    assert!(
        crate::filesystem::open_mutation_file_from_directory(
            &root,
            Path::new("note.txt"),
            &options,
        )
        .is_err(),
        "the mutation handle must fail while a competing writer is open"
    );
    drop(writer);

    let (mutation_handle, _) = crate::filesystem::open_mutation_file_from_directory(
        &root,
        Path::new("note.txt"),
        &options,
    )
    .expect("open restrictive mutation handle");
    assert!(
        StandardOpenOptions::new().write(true).open(&path).is_err(),
        "a second writer must not open while the mutation handle is retained"
    );
    assert!(
        fs::rename(&path, &moved_path).is_err(),
        "the file path must not be replaceable while the mutation handle is retained"
    );
    assert!(
        crate::filesystem::opened_file_matches_directory_entry(
            &root,
            Path::new("note.txt"),
            &mutation_handle,
        )
        .expect("check the opened file identity"),
        "the retained handle must still match its workspace entry"
    );

    drop(mutation_handle);
    fs::rename(&path, &moved_path).expect("rename succeeds after releasing the handle");
}

#[test]
fn read_file_bounds_large_single_lines_to_the_output_limit() {
    let directory = TestDirectory::new();
    let large_line = "x".repeat(128 * 1024);
    fs::write(directory.0.join("large-line.txt"), large_line)
        .expect("create a file with one long line");

    let result = read_file(directory.root(), "large-line.txt", 1, 10)
        .expect("read should return a bounded truncated response");

    assert_eq!(result["c"], true);
    assert!(result["d"].as_array().is_some_and(Vec::is_empty));
}

#[test]
fn read_file_applies_output_limit_after_skipping_requested_lines() {
    let directory = TestDirectory::new();
    let path = directory.0.join("late-line.txt");
    let mut content = "skip\n".repeat(20_000);
    content.push_str("requested line\n");
    fs::write(path, content).expect("create a file with a late requested line");

    let result = read_file(directory.root(), "late-line.txt", 20_001, 1)
        .expect("read the requested line after a bounded skip");

    assert_eq!(result["c"], false);
    assert_eq!(result["d"][0][0], 20_001);
    assert_eq!(result["d"][0][1], "requested line");
}

#[test]
fn read_file_returns_a_requested_range_and_can_return_a_small_file_in_full() {
    let directory = TestDirectory::new();
    directory.write("sample.txt", "first\nsecond\nthird\n");

    let range = read_file(directory.root(), "sample.txt", 2, 1).expect("read one line");
    assert_eq!(range["d"], json!([[2, "second"]]));
    assert_eq!(range["c"], true);

    let complete = read_file(directory.root(), "sample.txt", 1, 500).expect("read small file");
    assert_eq!(complete["d"].as_array().map(Vec::len), Some(3));
    assert_eq!(complete["c"], false);
}

#[test]
fn edit_file_rejects_oversized_files_without_changing_them() {
    let directory = TestDirectory::new();
    let path = directory.0.join("oversized.txt");
    fs::File::create(&path)
        .and_then(|file| file.set_len(10 * 1024 * 1024 + 1))
        .expect("create a sparse file beyond the edit limit");

    assert!(super::super::edit_file(directory.root(), "oversized.txt", "x", "y").is_err());
    assert_eq!(
        fs::metadata(&path)
            .expect("read original file metadata")
            .len(),
        10 * 1024 * 1024 + 1
    );
}
