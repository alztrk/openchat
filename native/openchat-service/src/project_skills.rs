use serde::Serialize;
use serde_json::Value;
use std::{
    fs::{self, File},
    io::Read,
    path::{Path, PathBuf},
};

use crate::protocol::ServiceError;

pub(crate) const MAX_PROJECT_SKILLS: usize = 32;
pub(crate) const MAX_PROJECT_SKILL_BYTES: usize = 16 * 1024;
pub(crate) const MAX_PROJECT_SKILLS_BYTES: usize = 64 * 1024;
const SKILLS_DIRECTORY: &str = ".openchat/skills";
const MAX_SKILL_DIRECTORY_ENTRIES: usize = 256;

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub(crate) struct ProjectSkill {
    pub(crate) id: String,
    pub(crate) content: String,
}

pub(crate) fn list(project_root: &Path) -> Result<Vec<ProjectSkill>, ServiceError> {
    let root = canonical_project_root(project_root)?;
    let skills_dir = root.join(SKILLS_DIRECTORY);
    let metadata = match fs::symlink_metadata(&skills_dir) {
        Ok(metadata) => metadata,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(_) => return Err(unavailable_error()),
    };
    if !metadata.is_dir() || metadata.file_type().is_symlink() {
        return Err(unavailable_error());
    }
    let canonical_dir = fs::canonicalize(&skills_dir).map_err(|_| unavailable_error())?;
    ensure_project_path(&root, &canonical_dir)?;

    let mut skill_dirs = Vec::new();
    let mut scanned_entries = 0usize;
    for entry in fs::read_dir(&canonical_dir).map_err(|_| unavailable_error())? {
        scanned_entries = scanned_entries.saturating_add(1);
        if scanned_entries > MAX_SKILL_DIRECTORY_ENTRIES {
            return Err(too_many_error());
        }
        let entry = entry.map_err(|_| unavailable_error())?;
        let entry_type = entry.file_type().map_err(|_| unavailable_error())?;
        if entry_type.is_symlink() {
            return Err(unavailable_error());
        }
        if !entry_type.is_dir() {
            continue;
        }
        let id = entry
            .file_name()
            .into_string()
            .map_err(|_| invalid_id_error())?;
        if !valid_skill_id(&id) {
            return Err(invalid_id_error());
        }
        skill_dirs.push((id, entry.path()));
        if skill_dirs.len() > MAX_PROJECT_SKILLS {
            return Err(too_many_error());
        }
    }
    skill_dirs.sort_by(|left, right| left.0.cmp(&right.0));

    let mut skills = Vec::new();
    let mut total_bytes = 0usize;
    for (id, directory) in skill_dirs {
        let skill_path = directory.join("SKILL.md");
        match load_skill(&root, &skill_path, &id) {
            Ok(skill) => {
                total_bytes = total_bytes.saturating_add(skill.content.len());
                if total_bytes > MAX_PROJECT_SKILLS_BYTES {
                    return Err(too_large_error());
                }
                skills.push(skill);
            }
            Err(error) if error.code == "project_skill_missing" => {}
            Err(error) => return Err(error),
        }
    }
    Ok(skills)
}

pub(crate) fn load_selected(
    project_root: &Path,
    selected_ids: &[String],
) -> Result<Vec<ProjectSkill>, ServiceError> {
    validate_selected_ids(selected_ids)?;
    if selected_ids.is_empty() {
        return Ok(Vec::new());
    }
    let root = canonical_project_root(project_root)?;
    let mut skills = Vec::with_capacity(selected_ids.len());
    let mut total_bytes = 0usize;
    for id in selected_ids {
        let skill = load_skill(
            &root,
            &root.join(SKILLS_DIRECTORY).join(id).join("SKILL.md"),
            id,
        )?;
        total_bytes = total_bytes.saturating_add(skill.content.len());
        if total_bytes > MAX_PROJECT_SKILLS_BYTES {
            return Err(too_large_error());
        }
        skills.push(skill);
    }
    Ok(skills)
}

pub(crate) fn validate_selected_ids(ids: &[String]) -> Result<(), ServiceError> {
    if ids.len() > MAX_PROJECT_SKILLS || ids.iter().any(|id| !valid_skill_id(id)) {
        return Err(invalid_selection_error());
    }
    let unique = ids.iter().collect::<std::collections::HashSet<_>>();
    if unique.len() != ids.len() {
        return Err(invalid_selection_error());
    }
    Ok(())
}

pub(crate) fn from_checkpoint(value: Option<&Value>) -> Result<Vec<ProjectSkill>, ServiceError> {
    let Some(value) = value else {
        return Ok(Vec::new());
    };
    let Some(values) = value.as_array() else {
        return Err(invalid_checkpoint_error());
    };
    if values.len() > MAX_PROJECT_SKILLS {
        return Err(invalid_checkpoint_error());
    }
    let mut skills = Vec::with_capacity(values.len());
    let mut total_bytes = 0usize;
    for value in values {
        let Some(id) = value.get("id").and_then(Value::as_str) else {
            return Err(invalid_checkpoint_error());
        };
        let Some(content) = value.get("content").and_then(Value::as_str) else {
            return Err(invalid_checkpoint_error());
        };
        if !valid_skill_id(id)
            || content.trim().is_empty()
            || content.len() > MAX_PROJECT_SKILL_BYTES
        {
            return Err(invalid_checkpoint_error());
        }
        total_bytes = total_bytes.saturating_add(content.len());
        if total_bytes > MAX_PROJECT_SKILLS_BYTES {
            return Err(invalid_checkpoint_error());
        }
        skills.push(ProjectSkill {
            id: id.to_owned(),
            content: content.to_owned(),
        });
    }
    validate_selected_ids(
        &skills
            .iter()
            .map(|skill| skill.id.clone())
            .collect::<Vec<_>>(),
    )
    .map_err(|_| invalid_checkpoint_error())?;
    Ok(skills)
}

fn load_skill(root: &Path, path: &Path, id: &str) -> Result<ProjectSkill, ServiceError> {
    let config_dir = root.join(".openchat");
    let skills_dir = config_dir.join("skills");
    let skill_dir = skills_dir.join(id);
    for directory in [&config_dir, &skills_dir, &skill_dir] {
        let metadata = fs::symlink_metadata(directory).map_err(|error| {
            if error.kind() == std::io::ErrorKind::NotFound {
                ServiceError::new(
                    "project_skill_missing",
                    "The selected project Skill is unavailable.",
                    false,
                )
            } else {
                unavailable_error()
            }
        })?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(unavailable_error());
        }
    }
    let canonical_skills_dir = fs::canonicalize(&skills_dir).map_err(|_| unavailable_error())?;
    ensure_project_path(root, &canonical_skills_dir)?;
    let canonical_skill_dir = fs::canonicalize(&skill_dir).map_err(|_| unavailable_error())?;
    ensure_project_path(&canonical_skills_dir, &canonical_skill_dir)?;
    let metadata = fs::symlink_metadata(path).map_err(|error| {
        if error.kind() == std::io::ErrorKind::NotFound {
            ServiceError::new(
                "project_skill_missing",
                "The selected project Skill is unavailable.",
                false,
            )
        } else {
            unavailable_error()
        }
    })?;
    if metadata.file_type().is_symlink() || !metadata.is_file() {
        return Err(unavailable_error());
    }
    let canonical_path = fs::canonicalize(path).map_err(|_| unavailable_error())?;
    ensure_project_path(root, &canonical_path)?;
    let mut bytes = Vec::with_capacity(MAX_PROJECT_SKILL_BYTES.min(4096));
    File::open(canonical_path)
        .map_err(|_| unavailable_error())?
        .take((MAX_PROJECT_SKILL_BYTES + 1) as u64)
        .read_to_end(&mut bytes)
        .map_err(|_| unavailable_error())?;
    if bytes.len() > MAX_PROJECT_SKILL_BYTES {
        return Err(too_large_error());
    }
    let content = String::from_utf8(bytes).map_err(|_| invalid_encoding_error())?;
    if content.trim().is_empty() {
        return Err(ServiceError::new(
            "project_skill_empty",
            "The selected project Skill file is empty.",
            false,
        ));
    }
    Ok(ProjectSkill {
        id: id.to_owned(),
        content,
    })
}

fn canonical_project_root(project_root: &Path) -> Result<PathBuf, ServiceError> {
    fs::canonicalize(project_root).map_err(|_| unavailable_error())
}

fn valid_skill_id(id: &str) -> bool {
    !id.is_empty()
        && id.len() <= 64
        && id
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        && id.as_bytes().first().is_some_and(u8::is_ascii_lowercase)
}

fn ensure_project_path(root: &Path, path: &Path) -> Result<(), ServiceError> {
    if path.starts_with(root) {
        Ok(())
    } else {
        Err(ServiceError::new(
            "project_skill_outside_project",
            "The project Skill resolves outside the attached project.",
            false,
        ))
    }
}

fn invalid_id_error() -> ServiceError {
    ServiceError::new(
        "project_skill_invalid_id",
        "A project Skill has an invalid identifier.",
        false,
    )
}

fn invalid_selection_error() -> ServiceError {
    ServiceError::new(
        "project_skill_invalid_selection",
        "The selected project Skills are invalid.",
        false,
    )
}

fn too_many_error() -> ServiceError {
    ServiceError::new(
        "project_skills_too_many",
        "A project can contain at most 32 Skills.",
        false,
    )
}

fn too_large_error() -> ServiceError {
    ServiceError::new(
        "project_skills_too_large",
        "Project Skill content exceeds the 64 KiB total limit.",
        false,
    )
}

fn invalid_encoding_error() -> ServiceError {
    ServiceError::new(
        "project_skill_invalid_encoding",
        "Project Skill files must use UTF-8 encoding.",
        false,
    )
}

fn invalid_checkpoint_error() -> ServiceError {
    ServiceError::new(
        "question_run_checkpoint_invalid",
        "The saved project Skills are invalid.",
        false,
    )
}

fn unavailable_error() -> ServiceError {
    ServiceError::new(
        "project_skills_unavailable",
        "Project Skills could not be read safely.",
        false,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use uuid::Uuid;

    struct TestProject(PathBuf);

    impl TestProject {
        fn new() -> Self {
            let path =
                std::env::temp_dir().join(format!("openchat-project-skills-{}", Uuid::new_v4()));
            fs::create_dir(&path).expect("create project directory");
            Self(path)
        }

        fn skill(&self, id: &str, content: &str) {
            let directory = self.0.join(SKILLS_DIRECTORY).join(id);
            fs::create_dir_all(&directory).expect("create Skill directory");
            fs::write(directory.join("SKILL.md"), content).expect("write Skill file");
        }
    }

    impl Drop for TestProject {
        fn drop(&mut self) {
            fs::remove_dir_all(&self.0).expect("remove test project");
        }
    }

    #[test]
    fn missing_catalog_is_empty_and_selected_skills_are_loaded_in_requested_order() {
        let project = TestProject::new();
        assert!(list(&project.0).expect("list missing skills").is_empty());
        project.skill("rust-style", "Use rustfmt.");
        project.skill("api-contract", "Keep API errors stable.");

        let skills = load_selected(
            &project.0,
            &["api-contract".to_owned(), "rust-style".to_owned()],
        )
        .expect("load selected skills");
        assert_eq!(skills[0].id, "api-contract");
        assert_eq!(skills[1].content, "Use rustfmt.");
        assert_eq!(list(&project.0).expect("list skills").len(), 2);
    }

    #[test]
    fn rejects_stale_selected_skills_instead_of_ignoring_them() {
        let project = TestProject::new();
        assert_eq!(
            load_selected(&project.0, &["removed-skill".to_owned()])
                .expect_err("missing selected skill must fail")
                .code,
            "project_skill_missing"
        );
    }

    #[test]
    fn rejects_invalid_duplicate_and_oversized_skill_selections() {
        assert_eq!(
            validate_selected_ids(&["../escape".to_owned()])
                .expect_err("invalid id must fail")
                .code,
            "project_skill_invalid_selection"
        );
        assert!(validate_selected_ids(&["shared".to_owned(), "shared".to_owned()]).is_err());
        assert!(validate_selected_ids(&vec!["shared".to_owned(); MAX_PROJECT_SKILLS + 1]).is_err());
    }

    #[test]
    fn validates_skill_checkpoint_shape_and_total_size_before_resume() {
        let valid = serde_json::json!([{"id": "rust-style", "content": "Run rustfmt."}]);
        assert_eq!(
            from_checkpoint(Some(&valid)).expect("valid checkpoint")[0].id,
            "rust-style"
        );
        assert_eq!(
            from_checkpoint(None).expect("legacy checkpoint"),
            Vec::<ProjectSkill>::new()
        );

        let too_many = serde_json::json!(
            (0..=MAX_PROJECT_SKILLS)
                .map(|index| serde_json::json!({"id": format!("skill-{index}"), "content": "A"}))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            from_checkpoint(Some(&too_many))
                .expect_err("too many Skills must fail")
                .code,
            "question_run_checkpoint_invalid"
        );

        let too_large = serde_json::json!(
            (0..5)
                .map(|index| serde_json::json!({
                    "id": format!("skill-{index}"),
                    "content": "x".repeat(MAX_PROJECT_SKILL_BYTES)
                }))
                .collect::<Vec<_>>()
        );
        assert_eq!(
            from_checkpoint(Some(&too_large))
                .expect_err("excessive total content must fail")
                .code,
            "question_run_checkpoint_invalid"
        );
    }

    #[test]
    fn rejects_invalid_encoding_and_oversized_skill_content() {
        let project = TestProject::new();
        let invalid = project.0.join(SKILLS_DIRECTORY).join("invalid");
        fs::create_dir_all(&invalid).expect("create invalid Skill");
        fs::write(invalid.join("SKILL.md"), [0xff]).expect("write invalid UTF-8");
        assert_eq!(
            list(&project.0).expect_err("invalid UTF-8 must fail").code,
            "project_skill_invalid_encoding"
        );

        fs::write(
            invalid.join("SKILL.md"),
            vec![b'x'; MAX_PROJECT_SKILL_BYTES + 1],
        )
        .expect("write oversized Skill");
        assert_eq!(
            list(&project.0)
                .expect_err("oversized Skill must fail")
                .code,
            "project_skills_too_large"
        );
    }
}
