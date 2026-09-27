use std::{error::Error, fs, path::PathBuf};

use sha2::{Digest, Sha256};

fn main() -> Result<(), Box<dyn Error>> {
    let payload =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../build/zihora-portable-payload.zip");
    let payload_bytes = fs::read(&payload)?;
    let payload_hash = Sha256::digest(payload_bytes);
    let payload_hash = payload_hash
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect::<String>();
    let output_directory = std::env::var_os("OUT_DIR")
        .ok_or_else(|| std::io::Error::other("Cargo did not provide OUT_DIR"))?;
    let generated_hash = PathBuf::from(output_directory).join("embedded_bundle_hash.rs");
    fs::write(
        generated_hash,
        format!("const EMBEDDED_BUNDLE_SHA256: &str = \"{payload_hash}\";\n"),
    )?;

    let icon = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../windows/runner/resources/app_icon.ico");
    let icon_path = icon
        .to_str()
        .ok_or("The Zihora application icon path is not valid UTF-8.")?;

    let mut resources = winres::WindowsResource::new();
    resources.set_icon(icon_path);
    resources.compile()?;
    println!("cargo:rerun-if-changed={}", icon.display());
    println!("cargo:rerun-if-changed={}", payload.display());
    Ok(())
}
