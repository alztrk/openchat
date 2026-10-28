//! Native-owned, localized copy for security confirmation dialogs.

use std::{collections::HashMap, sync::OnceLock};

use serde::Deserialize;
use serde_json::Value;

#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq)]
#[serde(rename_all = "lowercase")]
pub(crate) enum NativeDialogLocale {
    De,
    En,
    Es,
    Fr,
    Tr,
}

impl NativeDialogLocale {
    pub(crate) fn message(self, key: &str) -> Result<String, &'static str> {
        native_dialog_catalog()?
            .get(&self)
            .and_then(|messages| messages.get(key))
            .and_then(Value::as_str)
            .map(str::to_owned)
            .ok_or("native_dialog_catalog_invalid")
    }
}

type NativeDialogCatalog = HashMap<NativeDialogLocale, Value>;

static NATIVE_DIALOG_CATALOG: OnceLock<Result<NativeDialogCatalog, &'static str>> = OnceLock::new();

fn native_dialog_catalog() -> Result<&'static NativeDialogCatalog, &'static str> {
    NATIVE_DIALOG_CATALOG
        .get_or_init(|| {
            let mut catalog = HashMap::new();
            catalog.insert(
                NativeDialogLocale::De,
                serde_json::from_str(include_str!(
                    "../../../frontend/src/localization/locales/de.json"
                ))
                .map_err(|_| "native_dialog_catalog_invalid")?,
            );
            catalog.insert(
                NativeDialogLocale::En,
                serde_json::from_str(include_str!(
                    "../../../frontend/src/localization/locales/en.json"
                ))
                .map_err(|_| "native_dialog_catalog_invalid")?,
            );
            catalog.insert(
                NativeDialogLocale::Es,
                serde_json::from_str(include_str!(
                    "../../../frontend/src/localization/locales/es.json"
                ))
                .map_err(|_| "native_dialog_catalog_invalid")?,
            );
            catalog.insert(
                NativeDialogLocale::Fr,
                serde_json::from_str(include_str!(
                    "../../../frontend/src/localization/locales/fr.json"
                ))
                .map_err(|_| "native_dialog_catalog_invalid")?,
            );
            catalog.insert(
                NativeDialogLocale::Tr,
                serde_json::from_str(include_str!(
                    "../../../frontend/src/localization/locales/tr.json"
                ))
                .map_err(|_| "native_dialog_catalog_invalid")?,
            );
            Ok(catalog)
        })
        .as_ref()
        .map_err(|error| *error)
}

/// Copy selected from the host's embedded locale catalog for one native permission dialog.
#[derive(Debug)]
pub(crate) struct PermissionDialogCopy {
    pub(crate) title: String,
    pub(crate) intro: String,
    pub(crate) full_access_intro: String,
    pub(crate) full_access_warning: String,
    pub(crate) project_allow_intro: String,
    pub(crate) project_allow_warning: String,
    pub(crate) project_allow_more: String,
    pub(crate) tool_label: String,
    pub(crate) target_label: String,
    pub(crate) arguments_label: String,
    pub(crate) warning: String,
    pub(crate) allow_once: String,
    pub(crate) enable_full_access: String,
    pub(crate) save_project_allow: String,
    pub(crate) deny: String,
    pub(crate) too_large: String,
    pub(crate) close: String,
}

impl PermissionDialogCopy {
    pub(crate) fn for_locale(locale: NativeDialogLocale) -> Result<Self, &'static str> {
        Ok(Self {
            title: locale.message("nativePermissionTitle")?,
            intro: locale.message("nativePermissionIntro")?,
            full_access_intro: locale.message("nativePermissionFullAccessIntro")?,
            full_access_warning: locale.message("nativePermissionFullAccessWarning")?,
            project_allow_intro: locale.message("nativePermissionProjectAllowIntro")?,
            project_allow_warning: locale.message("nativePermissionProjectAllowWarning")?,
            project_allow_more: locale.message("nativePermissionProjectAllowMore")?,
            tool_label: locale.message("nativePermissionToolLabel")?,
            target_label: locale.message("nativePermissionTargetLabel")?,
            arguments_label: locale.message("nativePermissionArgumentsLabel")?,
            warning: locale.message("nativePermissionWarning")?,
            allow_once: locale.message("nativePermissionAllowOnce")?,
            enable_full_access: locale.message("nativePermissionEnableFullAccess")?,
            save_project_allow: locale.message("nativePermissionSaveProjectAllow")?,
            deny: locale.message("nativePermissionDeny")?,
            too_large: locale.message("nativePermissionTooLarge")?,
            close: locale.message("nativePermissionClose")?,
        })
    }

    /// Validates embedded strings before they are formatted into an OS-owned dialog.
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

#[cfg(test)]
mod tests {
    use super::{NativeDialogLocale, PermissionDialogCopy};

    #[test]
    fn every_supported_locale_has_host_owned_permission_and_http_copy() {
        for locale in [
            NativeDialogLocale::De,
            NativeDialogLocale::En,
            NativeDialogLocale::Es,
            NativeDialogLocale::Fr,
            NativeDialogLocale::Tr,
        ] {
            let copy = PermissionDialogCopy::for_locale(locale)
                .expect("permission translations should be present");
            copy.validate()
                .expect("permission translations should be valid");
            for key in [
                "nativeProviderHttpTitle",
                "nativeProviderHttpIntro",
                "nativeProviderHttpWarning",
                "nativeProviderHttpAllow",
                "nativeProviderHttpCancel",
            ] {
                let message = locale
                    .message(key)
                    .expect("HTTP translations should be present");
                assert!(!message.is_empty());
                assert!(!message.chars().any(char::is_control));
            }
        }
    }

    #[test]
    fn native_dialog_locale_rejects_arbitrary_renderer_text() {
        assert!(serde_json::from_str::<NativeDialogLocale>("\"ja\"").is_err());
        assert!(serde_json::from_str::<NativeDialogLocale>("{\"title\":\"Approve\"}").is_err());
    }
}
