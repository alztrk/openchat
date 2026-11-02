//! Direct file-change capture and tracking helpers.

use super::*;

pub(super) fn qualify_citation_source_ids(output: &mut Value, call_id: &str) {
    let suffix = call_id
        .chars()
        .filter(char::is_ascii_alphanumeric)
        .take(8)
        .collect::<String>();
    if suffix.is_empty() {
        return;
    }
    let qualify = |source_id: &mut Value| {
        let Some(id) = source_id.as_str() else {
            return;
        };
        *source_id = json!(format!("{id}-{suffix}"));
    };
    if let Some(results) = output.get_mut("results").and_then(Value::as_array_mut) {
        for result in results {
            if let Some(source_id) = result.get_mut("sourceId") {
                qualify(source_id);
            }
        }
    }
    if let Some(source_id) = output.get_mut("sourceId") {
        qualify(source_id);
    }
}

pub(super) fn execute_direct_file_change_with_tracking(
    storage: &crate::storage::AppStorage,
    conversation_id: &str,
    tool_name: &str,
    prepared: &PreparedToolCall,
    approved_root_directory: Option<std::sync::Arc<crate::filesystem::OpenedDirectory>>,
) -> (
    Value,
    Vec<crate::file_changes::FileChangeSummary>,
    Option<String>,
    Option<String>,
) {
    let guard = match crate::file_changes::lock_file_changes() {
        Ok(guard) => guard,
        Err(_) => {
            return (
                tool_error(
                    "storage_unavailable",
                    "File changes could not be coordinated safely.",
                ),
                Vec::new(),
                Some("storage_unavailable".to_owned()),
                None,
            );
        }
    };
    let root_directory = match approved_root_directory {
        Some(directory) => {
            match crate::file_changes::workspace_directory_matches_path(&prepared.root, &directory)
            {
                Ok(true) => {}
                Ok(false) => {
                    return (
                        tool_error(
                            "permission_target_changed",
                            "The active project folder changed after approval. Request permission again.",
                        ),
                        Vec::new(),
                        Some("workspace_unavailable".to_owned()),
                        None,
                    );
                }
                Err(_) => {
                    return (
                        tool_error(
                            "workspace_unavailable",
                            "The active project folder could not be checked safely after approval.",
                        ),
                        Vec::new(),
                        Some("workspace_unavailable".to_owned()),
                        None,
                    );
                }
            }
            directory
        }
        None => match crate::file_changes::open_workspace_directory(&prepared.root) {
            Ok((directory, _)) => std::sync::Arc::new(directory),
            Err(error) => {
                let code = tracking_error_code(error);
                return (
                    tool_error(
                        code,
                        "The active project folder could not be opened safely.",
                    ),
                    Vec::new(),
                    Some(code.to_owned()),
                    None,
                );
            }
        },
    };
    let mut capture = match capture_direct_file_change_before(prepared, root_directory.clone()) {
        Ok(capture) => capture,
        Err(error) => {
            let mut output = tool_error(error.code, &error.message);
            output["error"]["category"] = json!(error.category.as_str());
            if let Some(diagnostic_code) = error.diagnostic_code {
                output["error"]["diagnosticCode"] = json!(diagnostic_code);
            }
            drop(guard);
            return (output, Vec::new(), Some(error.code.to_owned()), None);
        }
    };
    let result = match (&mut capture, &prepared.operation) {
        (PendingFileChangeCapture::File { target, .. }, ToolOperation::Write { content }) => {
            crate::tools::write_opened_file(target, &prepared.relative_path, content)
        }
        (
            PendingFileChangeCapture::File { target, .. },
            ToolOperation::Edit {
                old_string,
                new_string,
            },
        ) => {
            crate::tools::edit_opened_file(target, &prepared.relative_path, old_string, new_string)
        }
        _ => Err(ServiceError::new(
            "tool_execution_unavailable",
            "This operation is not a direct file change.",
            false,
        )),
    };
    let output = result.unwrap_or_else(|error| {
        let mut output = tool_error(error.code, &error.message);
        output["error"]["category"] = json!(error.category.as_str());
        if let Some(diagnostic_code) = error.diagnostic_code {
            output["error"]["diagnosticCode"] = json!(diagnostic_code);
        }
        output
    });
    let (file_changes, file_changes_error, recovery_checkpoint_id) =
        finish_file_change_capture(storage, conversation_id, tool_name, prepared, capture, true);
    drop(guard);
    (
        output,
        file_changes,
        file_changes_error,
        recovery_checkpoint_id,
    )
}

pub(super) fn open_approved_file_root(
    root: &Path,
    expected_identity: crate::file_changes::DirectoryIdentity,
) -> Result<crate::filesystem::OpenedDirectory, &'static str> {
    let (directory, canonical_root) =
        crate::file_changes::open_workspace_directory(root).map_err(|_| "workspace_unavailable")?;
    if canonical_root != root {
        return Err("permission_target_changed");
    }
    match crate::file_changes::workspace_directory_identity(&directory) {
        Ok(identity) if identity == expected_identity => Ok(directory),
        Ok(_) => Err("permission_target_changed"),
        Err(_) => Err("workspace_unavailable"),
    }
}

pub(super) fn capture_direct_file_change_before(
    prepared: &PreparedToolCall,
    directory: std::sync::Arc<crate::filesystem::OpenedDirectory>,
) -> Result<PendingFileChangeCapture, ServiceError> {
    let path = prepared.relative_path.clone();
    let root_identity =
        crate::file_changes::workspace_directory_identity(&directory).map_err(|error| {
            ServiceError::new(
                tracking_error_code(error),
                "The active project folder could not be tracked safely.",
                false,
            )
        })?;
    let (mut target, created) = match &prepared.operation {
        ToolOperation::Write { content } => {
            crate::tools::open_file_for_direct_write(&directory, &path, content.len())?
        }
        ToolOperation::Edit { old_string, .. } => (
            crate::tools::open_file_for_direct_edit(&directory, &path, old_string)?,
            false,
        ),
        _ => {
            return Err(ServiceError::new(
                "tool_execution_unavailable",
                "This operation is not a direct file change.",
                false,
            ));
        }
    };
    let before = if created {
        Ok((root_identity, None))
    } else {
        crate::file_changes::snapshot_opened_file(&mut target)
            .map(|snapshot| (root_identity, Some(snapshot)))
    };
    Ok(PendingFileChangeCapture::File {
        directory,
        path,
        before,
        target,
    })
}

pub(super) fn capture_file_change_before(
    prepared: &PreparedToolCall,
    project_roots: &[PathBuf],
) -> PendingFileChangeCapture {
    match &prepared.operation {
        ToolOperation::Write { .. } | ToolOperation::Edit { .. } => PendingFileChangeCapture::None,
        ToolOperation::Bash { .. } | ToolOperation::SendTerminalInput { .. } => {
            PendingFileChangeCapture::Workspace {
                before: if project_roots.is_empty() {
                    vec![(
                        prepared.root.clone(),
                        crate::file_changes::capture_workspace(&prepared.root),
                    )]
                } else {
                    project_roots
                        .iter()
                        .map(|root| (root.clone(), crate::file_changes::capture_workspace(root)))
                        .collect()
                },
            }
        }
        #[cfg(windows)]
        ToolOperation::ProjectTool { .. } => PendingFileChangeCapture::Workspace {
            before: vec![(
                prepared.root.clone(),
                crate::file_changes::capture_workspace(&prepared.root),
            )],
        },
        _ => PendingFileChangeCapture::None,
    }
}

pub(super) fn finish_file_change_capture(
    storage: &crate::storage::AppStorage,
    conversation_id: &str,
    tool_name: &str,
    prepared: &PreparedToolCall,
    capture: PendingFileChangeCapture,
    file_change_lock_held: bool,
) -> (
    Vec<crate::file_changes::FileChangeSummary>,
    Option<String>,
    Option<String>,
) {
    let mut retained_target = None;
    let grouped_deltas = match capture {
        PendingFileChangeCapture::None => return (Vec::new(), None, None),
        PendingFileChangeCapture::File {
            directory,
            path,
            before,
            mut target,
        } => {
            let Ok((before_root_identity, before)) = before else {
                return (Vec::new(), Some("snapshot_unavailable".to_owned()), None);
            };
            let target_matches_path = crate::filesystem::opened_file_matches_directory_entry(
                directory.as_ref(),
                Path::new(&path),
                &target,
            );
            match target_matches_path {
                Ok(true) => {}
                Ok(false) => {
                    return (Vec::new(), Some("workspace_unavailable".to_owned()), None);
                }
                Err(_) => {
                    return (Vec::new(), Some("snapshot_unavailable".to_owned()), None);
                }
            }
            let after = match crate::file_changes::snapshot_opened_file(&mut target) {
                Ok(after) => Some(after),
                Err(error) => {
                    return (
                        Vec::new(),
                        Some(tracking_error_code(error).to_owned()),
                        None,
                    );
                }
            };
            match crate::filesystem::opened_file_matches_directory_entry(
                directory.as_ref(),
                Path::new(&path),
                &target,
            ) {
                Ok(true) => {}
                Ok(false) => {
                    return (Vec::new(), Some("workspace_unavailable".to_owned()), None);
                }
                Err(_) => {
                    return (Vec::new(), Some("snapshot_unavailable".to_owned()), None);
                }
            }
            if before_root_identity
                != match crate::file_changes::workspace_directory_identity(&directory) {
                    Ok(identity) => identity,
                    Err(error) => {
                        return (
                            Vec::new(),
                            Some(tracking_error_code(error).to_owned()),
                            None,
                        );
                    }
                }
            {
                return (Vec::new(), Some("workspace_unavailable".to_owned()), None);
            }
            let before_hash = before.as_ref().map(|file| file.hash.as_str());
            let after_hash = after.as_ref().map(|file| file.hash.as_str());
            retained_target = Some(target);
            if before_hash == after_hash {
                Vec::new()
            } else {
                vec![(
                    prepared.root.clone(),
                    vec![crate::file_changes::FileChangeDelta {
                        root_identity: before_root_identity,
                        path,
                        before,
                        after,
                    }],
                )]
            }
        }
        PendingFileChangeCapture::Workspace { before } => {
            let mut changes = Vec::new();
            for (root, before) in before {
                let Ok(before) = before else {
                    return (
                        Vec::new(),
                        Some("workspace_snapshot_unavailable".to_owned()),
                        None,
                    );
                };
                let after = match crate::file_changes::capture_workspace(&root) {
                    Ok(after) => after,
                    Err(error) => {
                        return (
                            Vec::new(),
                            Some(tracking_error_code(error).to_owned()),
                            None,
                        );
                    }
                };
                let deltas = match crate::file_changes::workspace_deltas(&before, &after) {
                    Ok(deltas) => deltas,
                    Err(error) => {
                        return (
                            Vec::new(),
                            Some(tracking_error_code(error).to_owned()),
                            None,
                        );
                    }
                };
                if !deltas.is_empty() {
                    changes.push((root, deltas));
                }
            }
            changes
        }
    };

    if grouped_deltas.is_empty() {
        return (Vec::new(), None, None);
    }
    let changes = match if file_change_lock_held {
        crate::file_changes::record_deltas_batch_locked(storage, conversation_id, &grouped_deltas)
    } else {
        crate::file_changes::record_deltas_batch(storage, conversation_id, &grouped_deltas)
    } {
        Ok(changes) => changes,
        Err(error) => {
            return (
                Vec::new(),
                Some(tracking_error_code(error).to_owned()),
                None,
            );
        }
    };
    let checkpoint = if file_change_lock_held {
        crate::file_changes::checkpoints::persist_checkpoint_locked(
            storage,
            conversation_id,
            tool_name,
            &grouped_deltas,
        )
    } else {
        crate::file_changes::checkpoints::persist_checkpoint(
            storage,
            conversation_id,
            tool_name,
            &grouped_deltas,
        )
    };
    let result = match checkpoint {
        Ok(checkpoint_id) => (changes, None, checkpoint_id),
        Err(error) => (
            Vec::new(),
            Some(tracking_error_code(error).to_owned()),
            None,
        ),
    };
    drop(retained_target);
    result
}

pub(super) fn tracking_error_code(error: crate::file_changes::TrackingError) -> &'static str {
    match error {
        crate::file_changes::TrackingError::WorkspaceUnavailable => "workspace_unavailable",
        crate::file_changes::TrackingError::WorkspaceTooLarge => "workspace_too_large",
        crate::file_changes::TrackingError::SnapshotUnavailable => "snapshot_unavailable",
        crate::file_changes::TrackingError::StorageUnavailable => "storage_unavailable",
        crate::file_changes::TrackingError::StorageFailure(code) => code,
        crate::file_changes::TrackingError::StorageBusy(code) => code,
        crate::file_changes::TrackingError::Cancelled => "operation_cancelled",
    }
}
