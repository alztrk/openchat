#![windows_subsystem = "windows"]

use std::{
    env,
    ffi::c_void,
    fs::{self, File, OpenOptions},
    io::{self, Cursor, Write, copy},
    path::{Path, PathBuf},
    process::Command,
};

use sha2::{Digest, Sha256};
use uuid::Uuid;
use zip::ZipArchive;

include!(concat!(env!("OUT_DIR"), "/embedded_bundle_hash.rs"));

const EMBEDDED_BUNDLE: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../build/zihora-portable-payload.zip"
));
const REQUIRED_FILES: [&str; 3] = [
    "zihora.exe",
    "zihora_service.exe",
    "data/flutter_assets/AssetManifest.bin",
];
const BUNDLE_MARKER: &str = ".zihora-bundle-id";
const MB_OK: u32 = 0x0000;
const MB_ICONERROR: u32 = 0x0010;

#[link(name = "user32")]
unsafe extern "system" {
    fn MessageBoxW(
        window: *mut c_void,
        text: *const u16,
        caption: *const u16,
        message_type: u32,
    ) -> i32;
}

#[link(name = "kernel32")]
unsafe extern "system" {
    fn GetUserDefaultUILanguage() -> u16;
}

fn main() {
    if let Err(error) = launch() {
        show_error(&error);
        std::process::exit(1);
    }
}

fn launch() -> Result<(), LaunchError> {
    let local_app_data = env::var_os("LOCALAPPDATA").ok_or(LaunchError::LocalDataUnavailable)?;
    let cache_root = PathBuf::from(local_app_data)
        .join("Zihora")
        .join("cache")
        .join("bundles");
    fs::create_dir_all(&cache_root).map_err(LaunchError::CacheAccess)?;

    let bundle_id = digest_hex(EMBEDDED_BUNDLE);
    if bundle_id != EMBEDDED_BUNDLE_SHA256 {
        return Err(LaunchError::EmbeddedBundleInvalid);
    }

    let bundle_directory = cache_root.join(format!("bundle-{bundle_id}"));
    prepare_bundle(&cache_root, &bundle_directory, &bundle_id)?;

    let application = bundle_directory.join("zihora.exe");
    let mut command = Command::new(application);
    command.current_dir(&bundle_directory);
    command.args(env::args_os().skip(1));
    command.spawn().map_err(LaunchError::ApplicationStart)?;
    Ok(())
}

fn prepare_bundle(
    cache_root: &Path,
    bundle_directory: &Path,
    bundle_id: &str,
) -> Result<(), LaunchError> {
    if is_complete_bundle(bundle_directory, bundle_id) {
        return Ok(());
    }
    if bundle_directory.exists() {
        return Err(LaunchError::IncompleteCachedBundle(bundle_id.to_owned()));
    }

    let staging_directory =
        cache_root.join(format!(".extract-{}-{}", &bundle_id[..16], Uuid::new_v4()));
    fs::create_dir(&staging_directory).map_err(LaunchError::CacheAccess)?;

    if let Err(error) = extract_bundle(&staging_directory, bundle_id) {
        return Err(cleanup_staging_directory(&staging_directory, error));
    }

    match fs::rename(&staging_directory, bundle_directory) {
        Ok(()) => Ok(()),
        Err(_) if is_complete_bundle(bundle_directory, bundle_id) => {
            fs::remove_dir_all(&staging_directory).map_err(LaunchError::CacheAccess)
        }
        Err(error) => Err(cleanup_staging_directory(
            &staging_directory,
            LaunchError::CacheAccess(error),
        )),
    }
}

fn cleanup_staging_directory(staging_directory: &Path, original_error: LaunchError) -> LaunchError {
    match fs::remove_dir_all(staging_directory) {
        Ok(()) => original_error,
        Err(cleanup_error) => LaunchError::CacheCleanup {
            original: Box::new(original_error),
            cleanup: cleanup_error,
        },
    }
}

fn extract_bundle(destination: &Path, bundle_id: &str) -> Result<(), LaunchError> {
    let mut archive = ZipArchive::new(Cursor::new(EMBEDDED_BUNDLE))
        .map_err(|_| LaunchError::EmbeddedBundleInvalid)?;
    if archive.is_empty() {
        return Err(LaunchError::EmbeddedBundleInvalid);
    }

    for index in 0..archive.len() {
        let mut entry = archive
            .by_index(index)
            .map_err(|_| LaunchError::EmbeddedBundleInvalid)?;
        let relative_path = entry
            .enclosed_name()
            .ok_or(LaunchError::EmbeddedBundleInvalid)?;
        if entry.is_symlink() {
            return Err(LaunchError::EmbeddedBundleInvalid);
        }

        let output_path = destination.join(relative_path);
        if entry.is_dir() {
            fs::create_dir_all(&output_path).map_err(LaunchError::CacheAccess)?;
            continue;
        }

        let parent = output_path
            .parent()
            .ok_or(LaunchError::EmbeddedBundleInvalid)?;
        fs::create_dir_all(parent).map_err(LaunchError::CacheAccess)?;
        let mut output = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&output_path)
            .map_err(LaunchError::CacheAccess)?;
        let expected_size = entry.size();
        let copied_size = copy(&mut entry, &mut output).map_err(LaunchError::CacheAccess)?;
        if copied_size != expected_size {
            return Err(LaunchError::EmbeddedBundleInvalid);
        }
        output.sync_all().map_err(LaunchError::CacheAccess)?;
    }

    for required_file in REQUIRED_FILES {
        if !destination.join(required_file).is_file() {
            return Err(LaunchError::RequiredFileMissing(required_file));
        }
    }

    let marker_path = destination.join(BUNDLE_MARKER);
    let mut marker = File::create(marker_path).map_err(LaunchError::CacheAccess)?;
    marker
        .write_all(bundle_id.as_bytes())
        .map_err(LaunchError::CacheAccess)?;
    marker.sync_all().map_err(LaunchError::CacheAccess)?;
    Ok(())
}

fn is_complete_bundle(bundle_directory: &Path, bundle_id: &str) -> bool {
    let marker_path = bundle_directory.join(BUNDLE_MARKER);
    let marker_matches =
        fs::read_to_string(marker_path).is_ok_and(|stored_id| stored_id == bundle_id);
    marker_matches
        && REQUIRED_FILES
            .iter()
            .all(|required_file| bundle_directory.join(required_file).is_file())
}

fn digest_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

fn show_error(error: &LaunchError) {
    let is_turkish = unsafe { GetUserDefaultUILanguage() & 0x03ff == 0x001f };
    let (caption, message) = error.localized_message(is_turkish);
    let caption = caption.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let message = message.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    unsafe {
        MessageBoxW(
            std::ptr::null_mut(),
            message.as_ptr(),
            caption.as_ptr(),
            MB_OK | MB_ICONERROR,
        );
    }
}

enum LaunchError {
    LocalDataUnavailable,
    CacheAccess(io::Error),
    CacheCleanup {
        original: Box<LaunchError>,
        cleanup: io::Error,
    },
    EmbeddedBundleInvalid,
    IncompleteCachedBundle(String),
    RequiredFileMissing(&'static str),
    ApplicationStart(io::Error),
}

impl LaunchError {
    fn localized_message(&self, is_turkish: bool) -> (&'static str, String) {
        if is_turkish {
            (
                "Zihora başlatılamadı",
                match self {
                    Self::LocalDataUnavailable => {
                        { "Windows yerel uygulama verileri klasörü bulunamadı." }.to_owned()
                    }
                    Self::CacheAccess(error) => format!(
                        "Uygulama dosyaları hazırlanamadı ({:?}). %LOCALAPPDATA%\\Zihora\\cache için boş alanı ve yazma iznini kontrol edip tekrar deneyin.",
                        error.kind()
                    ),
                    Self::CacheCleanup { original, cleanup } => format!(
                        "Uygulama dosyaları hazırlanamadı. Geçici dosyalar da temizlenemedi ({:?}). %LOCALAPPDATA%\\Zihora\\cache altındaki .extract-* klasörünü silip tekrar deneyin. İlk hata: {}",
                        cleanup.kind(),
                        original.summary()
                    ),
                    Self::EmbeddedBundleInvalid => {
                        "Uygulama paketi geçersiz. Zihora dosyasını yeniden indirip tekrar deneyin."
                            .to_owned()
                    }
                    Self::IncompleteCachedBundle(bundle_id) => format!(
                        "Önbellekteki uygulama dosyaları eksik. %LOCALAPPDATA%\\Zihora\\cache\\bundles\\bundle-{bundle_id} klasörünü silip tekrar deneyin."
                    ),
                    Self::RequiredFileMissing(file) => format!(
                        "Uygulama paketinde gerekli `{file}` dosyası bulunamadı. Zihora dosyasını yeniden indirin."
                    ),
                    Self::ApplicationStart(error) => format!(
                        "Uygulama açılamadı ({:?}). Önbellek klasörüne erişimi kontrol edip tekrar deneyin.",
                        error.kind()
                    ),
                },
            )
        } else {
            (
                "Zihora could not start",
                match self {
                    Self::LocalDataUnavailable => {
                        "The Windows local application data folder could not be found.".to_owned()
                    }
                    Self::CacheAccess(error) => format!(
                        "Application files could not be prepared ({:?}). Check free space and write access to %LOCALAPPDATA%\\Zihora\\cache, then try again.",
                        error.kind()
                    ),
                    Self::CacheCleanup { original, cleanup } => format!(
                        "Application files could not be prepared. Temporary files could not be cleaned up ({:?}). Delete the .extract-* folder in %LOCALAPPDATA%\\Zihora\\cache and try again. Initial error: {}",
                        cleanup.kind(),
                        original.summary()
                    ),
                    Self::EmbeddedBundleInvalid => {
                        "The application package is invalid. Download Zihora again and try again."
                            .to_owned()
                    }
                    Self::IncompleteCachedBundle(bundle_id) => format!(
                        "The cached application files are incomplete. Delete %LOCALAPPDATA%\\Zihora\\cache\\bundles\\bundle-{bundle_id} and try again."
                    ),
                    Self::RequiredFileMissing(file) => format!(
                        "The required `{file}` file is missing from the application package. Download Zihora again."
                    ),
                    Self::ApplicationStart(error) => format!(
                        "The application could not be opened ({:?}). Check access to the cache folder and try again.",
                        error.kind()
                    ),
                },
            )
        }
    }

    fn summary(&self) -> String {
        match self {
            Self::LocalDataUnavailable => "local data folder unavailable".to_owned(),
            Self::CacheAccess(error) => format!("cache access failed ({:?})", error.kind()),
            Self::CacheCleanup { cleanup, .. } => {
                format!("cache cleanup failed ({:?})", cleanup.kind())
            }
            Self::EmbeddedBundleInvalid => "embedded package invalid".to_owned(),
            Self::IncompleteCachedBundle(_) => "cached package incomplete".to_owned(),
            Self::RequiredFileMissing(file) => format!("required file missing: {file}"),
            Self::ApplicationStart(error) => {
                format!("application start failed ({:?})", error.kind())
            }
        }
    }
}
