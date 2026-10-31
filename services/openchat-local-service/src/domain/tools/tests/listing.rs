//! Split from tests.rs (7 items).
use super::*;

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

#[cfg(any(unix, windows))]
#[test]
fn listing_rejects_symlink_roots_and_nested_directory_targets() {
    let directory = TestDirectory::new();
    let workspace = directory.0.join("workspace");
    let external = directory.0.join("external");
    fs::create_dir(&workspace).expect("create workspace");
    fs::create_dir(&external).expect("create external directory");
    let external_file = external.join("secret.txt");
    fs::write(&external_file, "keep private").expect("write external file");

    let nested_link = workspace.join("external");
    create_directory_symlink(&external, &nested_link).expect("create nested directory link");
    let workspace_listing = list_files(workspace.to_str().expect("workspace path"), "", 0, 100)
        .expect("list workspace root");
    assert_eq!(workspace_listing["entries"], json!([]));
    assert!(
        list_files(
            workspace.to_str().expect("workspace path"),
            "external",
            0,
            100
        )
        .is_err()
    );

    let root_link = directory.0.join("linked-root");
    create_directory_symlink(&external, &root_link).expect("create root directory link");
    assert!(list_files(root_link, "", 0, 100).is_err());
    assert_eq!(
        fs::read_to_string(external_file).expect("read external file after rejected listings"),
        "keep private"
    );
}

#[cfg(any(unix, windows))]
#[test]
fn listing_rejects_symlinked_gitignore_rules() {
    let directory = TestDirectory::new();
    let workspace = directory.0.join("workspace");
    let external = directory.0.join("external");
    fs::create_dir(&workspace).expect("create workspace");
    fs::create_dir(&external).expect("create external directory");
    let external_rules = external.join("rules");
    fs::write(&external_rules, "private.txt\n").expect("write external ignore rules");
    fs::write(workspace.join("private.txt"), "workspace content").expect("write workspace file");
    create_file_symlink(&external_rules, &workspace.join(".gitignore"))
        .expect("create linked ignore rules");

    assert!(list_files(workspace, "", 0, 100).is_err());
    assert_eq!(
        fs::read_to_string(external_rules).expect("read external ignore rules"),
        "private.txt\n"
    );
}

#[test]
fn listing_rejects_hardlinked_gitignore_rules() {
    let directory = TestDirectory::new();
    let workspace = directory.0.join("workspace");
    let external_rules = directory.0.join("external-rules");
    fs::create_dir(&workspace).expect("create workspace");
    fs::write(&external_rules, "private.txt\n").expect("write external ignore rules");
    fs::hard_link(&external_rules, workspace.join(".gitignore"))
        .expect("link external ignore rules");

    assert!(list_files(workspace, "", 0, 100).is_err());
    assert_eq!(
        fs::read_to_string(external_rules).expect("read external ignore rules"),
        "private.txt\n"
    );
}

#[test]
fn listing_rejects_oversized_gitignore_rules() {
    let directory = TestDirectory::new();
    directory.write("visible.txt", "content");
    fs::write(
        directory.0.join(".gitignore"),
        vec![b'x'; super::super::MAX_GITIGNORE_FILE_BYTES + 1],
    )
    .expect("write oversized ignore rules");

    assert!(list_files(directory.root(), "", 0, 100).is_err());
}

#[test]
fn listing_preserves_nested_gitignore_precedence() {
    let directory = TestDirectory::new();
    directory.write(".gitignore", "src/*.txt\n");
    directory.write("src/.gitignore", "!keep.txt\n");
    directory.write("src/keep.txt", "keep");
    directory.write("src/other.txt", "ignore");
    directory.write("src/readme.md", "keep");

    let listing = list_files(directory.root(), "src", 0, 100).expect("list source files");
    let listed_paths = listing["entries"]
        .as_array()
        .expect("directory entries")
        .iter()
        .filter_map(|entry| entry["path"].as_str())
        .collect::<Vec<_>>();
    assert_eq!(listed_paths, ["src/keep.txt", "src/readme.md"]);
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
