//! Renderer-supplied copy used by native permission confirmation dialogs.

use serde::Deserialize;

use crate::request_bounds::deserialize_bounded_text;

const MAXIMUM_DIALOG_COPY_FIELD_BYTES: usize = 1024;

fn deserialize_dialog_copy_text<'de, D>(deserializer: D) -> Result<String, D::Error>
where
    D: serde::Deserializer<'de>,
{
    deserialize_bounded_text::<D, MAXIMUM_DIALOG_COPY_FIELD_BYTES>(deserializer)
}

/// Localized copy resolved by the renderer's active i18next language.
///
/// Native code deliberately does not own translations. The renderer passes the copy for each
/// confirmation request so a language change is reflected immediately and all user-facing text
/// continues to come from the same locale resources as the rest of the application.
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "camelCase")]
pub(crate) struct PermissionDialogCopy {
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) title: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) intro: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) full_access_intro: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) full_access_warning: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) project_allow_intro: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) project_allow_warning: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) project_allow_more: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) tool_label: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) target_label: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) arguments_label: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) warning: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) allow_once: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) enable_full_access: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) save_project_allow: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) deny: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) too_large: String,
    #[serde(deserialize_with = "deserialize_dialog_copy_text")]
    pub(crate) close: String,
}

impl PermissionDialogCopy {
    /// Rejects empty or control-character-only text before it reaches an OS dialog.
    pub(crate) fn validate(&self) -> Result<(), &'static str> {
        let fields = [
            &self.title,
            &self.intro,
            &self.full_access_intro,
            &self.full_access_warning,
            &self.project_allow_intro,
            &self.project_allow_warning,
            &self.project_allow_more,
            &self.tool_label,
            &self.target_label,
            &self.arguments_label,
            &self.warning,
            &self.allow_once,
            &self.enable_full_access,
            &self.save_project_allow,
            &self.deny,
            &self.too_large,
            &self.close,
        ];
        if fields
            .iter()
            .any(|field| field.is_empty() || field.chars().any(char::is_control))
        {
            return Err("native_permission_copy_invalid");
        }
        Ok(())
    }
}
