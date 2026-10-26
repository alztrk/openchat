//! Loads bounded, glob-scoped project rules only for matching workspace files.

use std::{
    collections::BTreeMap,
    fs,
    path::{Component, Path, PathBuf},
};

use ignore::{WalkBuilder, overrides::OverrideBuilder};
use serde::Serialize;

use crate::{protocol::ServiceError, storage::tool_index_redaction};

const RULES_DIRECTORY: &str = ".openchat/rules";
const MAX_RULE_FILES: usize = 64;
const MAX_RULE_SCAN_ENTRIES: usize = 2_048;
const MAX_RULE_FILE_BYTES: usize = 32 * 1024;
const MAX_RULE_CONTEXT_BYTES: usize = 12 * 1024;
const MAX_RULE_RESPONSE_BYTES: usize = 16 * 1024;
const MAX_GLOBS_PER_RULE: usize = 32;
const MAX_GLOB_BYTES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ScopedProjectRule {
    pub(crate) workspace_root: String,
    pub(crate) path: String,
    pub(crate) globs: Vec<String>,
    pub(crate) instructions: String,
}

/// Finds rules attached to the workspace root containing one canonical file path.
pub(crate) fn matching_rules_for_target(
    workspace_roots: &[PathBuf],
    target: &Path,
) -> Result<Vec<ScopedProjectRule>, ServiceError> {
    matching_rules_for_targets(workspace_roots, &[target.to_path_buf()])
}

/// Finds the union of rules matching a bounded set of files, parsing each workspace once.
pub(crate) fn matching_rules_for_targets(
    workspace_roots: &[PathBuf],
    targets: &[PathBuf],
) -> Result<Vec<ScopedProjectRule>, ServiceError> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let roots = workspace_roots
        .iter()
        .map(|root| fs::canonicalize(root).map_err(|_| project_rules_unavailable()))
        .collect::<Result<Vec<_>, _>>()?;
    let mut targets_by_root = BTreeMap::<PathBuf, Vec<PathBuf>>::new();
    for target in targets {
        let Some(target) = canonicalize_target(target)? else {
            continue;
        };
        if let Some(root) = roots.iter().find(|root| target.starts_with(root)) {
            let relative = target
                .strip_prefix(root)
                .map_err(|_| project_rules_unavailable())?
                .to_path_buf();
            targets_by_root
                .entry(root.clone())
                .or_default()
                .push(relative);
        }
    }
    let mut matching = Vec::new();
    let mut instruction_bytes = 0usize;
    let mut response_bytes = 2usize;
    for (root, relative_targets) in targets_by_root {
        let root_rules = load_matching_rules(&root, &relative_targets)?;
        for rule in root_rules {
            instruction_bytes = instruction_bytes.saturating_add(rule.instructions.len());
            if instruction_bytes > MAX_RULE_CONTEXT_BYTES {
                return Err(project_rules_limit_error());
            }
            let serialized_rule =
                serde_json::to_vec(&rule).map_err(|_| project_rules_invalid_error())?;
            response_bytes = response_bytes
                .saturating_add(serialized_rule.len())
                .saturating_add(1);
            if response_bytes > MAX_RULE_RESPONSE_BYTES {
                return Err(project_rules_limit_error());
            }
            matching.push(rule);
        }
    }
    Ok(matching)
}

fn canonicalize_target(target: &Path) -> Result<Option<PathBuf>, ServiceError> {
    match fs::canonicalize(target) {
        Ok(target) => Ok(Some(target)),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => {
            let Some(parent) = target.parent() else {
                return Ok(None);
            };
            let Some(file_name) = target.file_name() else {
                return Ok(None);
            };
            match fs::canonicalize(parent) {
                Ok(parent) => Ok(Some(parent.join(file_name))),
                Err(parent_error) if parent_error.kind() == std::io::ErrorKind::NotFound => {
                    Ok(None)
                }
                Err(_) => Err(project_rules_unavailable()),
            }
        }
        Err(_) => Err(project_rules_unavailable()),
    }
}

fn load_matching_rules(
    project_root: &Path,
    relative_targets: &[PathBuf],
) -> Result<Vec<ScopedProjectRule>, ServiceError> {
    let project_root = fs::canonicalize(project_root).map_err(|_| project_rules_unavailable())?;
    if relative_targets.is_empty() {
        return Ok(Vec::new());
    }
    let settings_directory = project_root.join(".openchat");
    match fs::symlink_metadata(&settings_directory) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(project_rules_unavailable()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(project_rules_unavailable()),
    }
    let rules_directory = project_root.join(RULES_DIRECTORY);
    match fs::symlink_metadata(&rules_directory) {
        Ok(metadata) if metadata.is_dir() && !metadata.file_type().is_symlink() => {}
        Ok(_) => return Err(project_rules_unavailable()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(project_rules_unavailable()),
    }
    let canonical_rules_directory =
        fs::canonicalize(&rules_directory).map_err(|_| project_rules_unavailable())?;
    if !canonical_rules_directory.starts_with(&project_root) {
        return Err(project_rules_unavailable());
    }

    let mut walker = WalkBuilder::new(&canonical_rules_directory);
    walker
        .follow_links(false)
        .hidden(false)
        .parents(false)
        .ignore(false)
        .git_ignore(false)
        .git_global(false)
        .git_exclude(false)
        .require_git(false)
        .sort_by_file_name(|left, right| left.cmp(right));

    let mut scanned_entries = 0usize;
    let mut rule_count = 0usize;
    let mut matching = Vec::new();
    for entry in walker.build() {
        scanned_entries = scanned_entries.saturating_add(1);
        if scanned_entries > MAX_RULE_SCAN_ENTRIES {
            return Err(project_rules_limit_error());
        }
        let entry = entry.map_err(|_| project_rules_unavailable())?;
        if entry.depth() == 0 || entry.file_type().is_none_or(|kind| !kind.is_file()) {
            continue;
        }
        if entry.path_is_symlink()
            || entry
                .path()
                .extension()
                .and_then(|extension| extension.to_str())
                != Some("mdc")
        {
            continue;
        }
        rule_count = rule_count.saturating_add(1);
        if rule_count > MAX_RULE_FILES {
            return Err(project_rules_limit_error());
        }
        let canonical_file =
            fs::canonicalize(entry.path()).map_err(|_| project_rules_unavailable())?;
        if !canonical_file.starts_with(&canonical_rules_directory) {
            return Err(project_rules_unavailable());
        }
        let relative_file = canonical_file
            .strip_prefix(&project_root)
            .map_err(|_| project_rules_unavailable())?;
        let bytes = crate::filesystem::read_bounded_file_under_root(
            &project_root,
            relative_file,
            MAX_RULE_FILE_BYTES + 1,
        )
        .map_err(|_| project_rules_unavailable())?;
        if bytes.len() > MAX_RULE_FILE_BYTES {
            return Err(project_rules_limit_error());
        }
        let source = String::from_utf8(bytes).map_err(|_| project_rules_invalid_error())?;
        let Some((globs, instructions)) = parse_rule(&source)? else {
            continue;
        };
        let mut matcher = OverrideBuilder::new(&project_root);
        for glob in &globs {
            matcher
                .add(glob)
                .map_err(|_| project_rules_invalid_error())?;
        }
        let matcher = matcher.build().map_err(|_| project_rules_invalid_error())?;
        if !relative_targets
            .iter()
            .any(|target| matcher.matched(target, false).is_whitelist())
        {
            continue;
        }
        let instructions = tool_index_redaction::redact_tool_index_text(&instructions);
        matching.push(ScopedProjectRule {
            workspace_root: project_root.to_string_lossy().into_owned(),
            path: relative_file.to_string_lossy().replace('\\', "/"),
            globs,
            instructions,
        });
    }
    Ok(matching)
}

fn parse_rule(source: &str) -> Result<Option<(Vec<String>, String)>, ServiceError> {
    let mut lines = source.lines();
    if lines.next().map(str::trim) != Some("---") {
        return Ok(None);
    }
    let mut header = Vec::new();
    let mut found_closing_delimiter = false;
    for line in lines.by_ref() {
        if line.trim() == "---" {
            found_closing_delimiter = true;
            break;
        }
        header.push(line);
    }
    if !found_closing_delimiter {
        return Err(project_rules_invalid_error());
    }
    let body = lines.collect::<Vec<_>>().join("\n").trim().to_owned();
    let mut globs = None;
    let mut in_glob_list = false;
    for line in header {
        let trimmed = line.trim();
        if let Some(value) = trimmed.strip_prefix("globs:") {
            if globs.is_some() {
                return Err(project_rules_invalid_error());
            }
            let value = value.trim();
            if value.is_empty() {
                globs = Some(Vec::new());
                in_glob_list = true;
                continue;
            }
            globs = Some(parse_inline_globs(value)?);
            in_glob_list = false;
            continue;
        }
        if in_glob_list {
            if let Some(value) = trimmed.strip_prefix('-') {
                globs
                    .as_mut()
                    .ok_or_else(project_rules_invalid_error)?
                    .push(clean_glob(value.trim())?);
                continue;
            }
            if !trimmed.is_empty() && !line.chars().next().is_some_and(char::is_whitespace) {
                in_glob_list = false;
            }
        }
    }
    let Some(globs) = globs else {
        return Ok(None);
    };
    if globs.is_empty() || globs.len() > MAX_GLOBS_PER_RULE || body.is_empty() {
        return Err(project_rules_invalid_error());
    }
    Ok(Some((globs, body)))
}

fn parse_inline_globs(value: &str) -> Result<Vec<String>, ServiceError> {
    let value = value
        .strip_prefix('[')
        .and_then(|value| value.strip_suffix(']'))
        .unwrap_or(value);
    value
        .split(',')
        .map(|glob| clean_glob(glob.trim()))
        .collect()
}

fn clean_glob(value: &str) -> Result<String, ServiceError> {
    let value = value
        .strip_prefix('"')
        .and_then(|value| value.strip_suffix('"'))
        .or_else(|| {
            value
                .strip_prefix('\'')
                .and_then(|value| value.strip_suffix('\''))
        })
        .unwrap_or(value)
        .replace('\\', "/");
    let path = Path::new(&value);
    if value.is_empty()
        || value.len() > MAX_GLOB_BYTES
        || value.chars().any(char::is_control)
        || value.starts_with('!')
        || path.is_absolute()
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(project_rules_invalid_error());
    }
    Ok(value)
}

fn project_rules_unavailable() -> ServiceError {
    ServiceError::new(
        "project_rules_unavailable",
        "The matching project rules could not be read safely.",
        false,
    )
}

fn project_rules_invalid_error() -> ServiceError {
    ServiceError::new(
        "project_rules_invalid",
        "A project rule must have valid UTF-8 front matter, scoped globs, and non-empty instructions.",
        false,
    )
}

fn project_rules_limit_error() -> ServiceError {
    ServiceError::new(
        "project_rules_limit",
        "The matching project rules exceed the supported file or context limits.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::{matching_rules_for_target, parse_rule};
    use std::{fs, path::PathBuf};

    struct TestDirectory(PathBuf);

    impl TestDirectory {
        fn new() -> Self {
            let root = std::env::temp_dir()
                .join(format!("openchat-project-rules-{}", uuid::Uuid::new_v4()));
            fs::create_dir_all(&root).expect("create project rule fixture");
            Self(root)
        }
    }

    impl Drop for TestDirectory {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove project rule fixture");
        }
    }

    #[test]
    fn activates_only_rules_matching_the_selected_file_path() {
        let project = TestDirectory::new();
        let rules = project.0.join(".openchat/rules");
        fs::create_dir_all(&rules).expect("create project rules folder");
        fs::create_dir_all(project.0.join("src")).expect("create source folder");
        fs::create_dir_all(project.0.join("web")).expect("create web folder");
        fs::write(
            rules.join("backend.mdc"),
            "---\nglobs:\n  - \"src/**/*.rs\"\n---\nUse the Rust formatter.\n",
        )
        .expect("write backend rule");
        fs::write(
            rules.join("frontend.mdc"),
            "---\nglobs: \"web/**/*.ts\"\n---\nKeep TypeScript strict.\n",
        )
        .expect("write frontend rule");
        let source = project.0.join("src/lib.rs");
        fs::write(&source, "pub fn run() {}\n").expect("write Rust source");
        let web_source = project.0.join("web/index.ts");
        fs::write(&web_source, "export const value = 1;\n").expect("write TypeScript source");
        let root = fs::canonicalize(&project.0).expect("canonicalize project root");

        let backend = matching_rules_for_target(std::slice::from_ref(&root), &source)
            .expect("load backend rule");
        assert_eq!(backend.len(), 1);
        assert_eq!(backend[0].path, ".openchat/rules/backend.mdc");
        assert_eq!(backend[0].instructions, "Use the Rust formatter.");
        let frontend = matching_rules_for_target(std::slice::from_ref(&root), &web_source)
            .expect("load frontend rule");
        assert_eq!(frontend.len(), 1);
        assert_eq!(frontend[0].path, ".openchat/rules/frontend.mdc");
    }

    #[test]
    fn rejects_malformed_or_unbounded_scoped_rule_front_matter() {
        assert!(parse_rule("---\nglobs: ../**/*\n---\nDo something.\n").is_err());
        assert!(parse_rule("---\nglobs:\n---\nDo something.\n").is_err());
        assert!(parse_rule("---\nglobs: **/*.rs\n").is_err());
        assert_eq!(
            parse_rule("# not an MDC rule\n").expect("ignore non-rule"),
            None
        );
    }

    #[test]
    fn activates_rules_for_a_new_file_with_an_existing_parent() {
        let project = TestDirectory::new();
        let rules = project.0.join(".openchat/rules");
        fs::create_dir_all(&rules).expect("create project rules folder");
        fs::create_dir_all(project.0.join("src")).expect("create source folder");
        fs::write(
            rules.join("source.mdc"),
            "---\nglobs: \"src/**/*.rs\"\n---\nUse Rust style.\n",
        )
        .expect("write source rule");
        let target = project.0.join("src/new_module.rs");
        let root = fs::canonicalize(&project.0).expect("canonicalize project root");

        let matched = matching_rules_for_target(std::slice::from_ref(&root), &target)
            .expect("load rule for new file");

        assert_eq!(matched.len(), 1);
        assert_eq!(matched[0].instructions, "Use Rust style.");
    }
}
