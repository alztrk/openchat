use std::{
    ffi::OsStr,
    path::{Path, PathBuf},
    process::Stdio,
    time::Duration,
};

use tokio::{process::Command, time::timeout};

pub(crate) fn command(executable: &Path, use_wsl: bool) -> Result<Command, ()> {
    if use_wsl {
        #[cfg(windows)]
        {
            let mut command = Command::new(wsl_executable()?);
            command.arg("--exec").arg(executable);
            command.kill_on_drop(true);
            return Ok(command);
        }
        #[cfg(not(windows))]
        {
            let _ = executable;
            return Err(());
        }
    }

    let mut command = Command::new(executable);
    command.kill_on_drop(true);
    Ok(command)
}

pub(crate) fn command_with_python_install_dir(
    executable: &Path,
    use_wsl: bool,
    install_dir: &Path,
) -> Result<Command, ()> {
    if use_wsl {
        #[cfg(windows)]
        {
            let install_dir = install_dir.to_str().ok_or(())?;
            if !install_dir.starts_with('/') || install_dir.contains('\0') {
                return Err(());
            }
            let mut command = Command::new(wsl_executable()?);
            // uv needs this scoped value on later calls to discover the runtime-local Python install.
            command
                .args([OsStr::new("--exec"), OsStr::new("env")])
                .arg(format!("UV_PYTHON_INSTALL_DIR={install_dir}"))
                .arg(executable)
                .kill_on_drop(true);
            return Ok(command);
        }
        #[cfg(not(windows))]
        {
            let _ = (executable, install_dir);
            return Err(());
        }
    }

    let mut command = Command::new(executable);
    command.env("UV_PYTHON_INSTALL_DIR", install_dir);
    command.kill_on_drop(true);
    Ok(command)
}

pub(crate) async fn path(path: &Path, use_wsl: bool) -> Result<PathBuf, ()> {
    if !use_wsl {
        return Ok(path.to_path_buf());
    }

    #[cfg(windows)]
    {
        let path = path.to_str().ok_or(())?;
        let mut command = Command::new(wsl_executable()?);
        command
            .args([
                OsStr::new("--exec"),
                OsStr::new("wslpath"),
                OsStr::new("-u"),
            ])
            .arg(path)
            .stdin(Stdio::null())
            .kill_on_drop(true);
        let output = timeout(Duration::from_secs(5), command.output())
            .await
            .map_err(|_| ())?
            .map_err(|_| ())?;
        if !output.status.success() {
            return Err(());
        }
        linux_path_from_output(&output.stdout).map(PathBuf::from)
    }
    #[cfg(not(windows))]
    {
        let _ = path;
        Err(())
    }
}

pub(crate) async fn make_executable(path: &Path) -> Result<(), ()> {
    let mut command = command(Path::new("chmod"), true)?;
    command
        .arg("u+x")
        .arg(path)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    let status = timeout(Duration::from_secs(10), command.status())
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    status.success().then_some(()).ok_or(())
}

#[cfg(windows)]
pub(crate) async fn nvidia_driver_output() -> Result<Vec<u8>, ()> {
    let mut command = Command::new(wsl_executable()?);
    command
        .args([
            OsStr::new("--exec"),
            OsStr::new("/usr/lib/wsl/lib/nvidia-smi"),
            OsStr::new("--query-gpu=driver_version"),
            OsStr::new("--format=csv,noheader,nounits"),
        ])
        .stdin(Stdio::null())
        .kill_on_drop(true);
    let output = timeout(Duration::from_secs(3), command.output())
        .await
        .map_err(|_| ())?
        .map_err(|_| ())?;
    if output.status.success() {
        Ok(output.stdout)
    } else {
        Err(())
    }
}

pub(crate) fn linux_path_from_output(stdout: &[u8]) -> Result<String, ()> {
    let path = std::str::from_utf8(stdout).map_err(|_| ())?.trim();
    if !path.starts_with('/')
        || path.contains('\\')
        || path.contains('\0')
        || path.lines().count() != 1
        || path
            .split('/')
            .any(|component| matches!(component, "." | ".."))
    {
        return Err(());
    }
    Ok(path.to_owned())
}

pub(crate) fn linux_path_is_within(path: &str, root: &str) -> bool {
    if !path.starts_with('/') || !root.starts_with('/') || !path.starts_with(root) {
        return false;
    }

    path.len() == root.len()
        || root.ends_with('/')
        || path.as_bytes().get(root.len()).copied() == Some(b'/')
}

#[cfg(windows)]
fn wsl_executable() -> Result<PathBuf, ()> {
    let system_root = std::env::var_os("WINDIR").ok_or(())?;
    let executable = PathBuf::from(system_root).join("System32").join("wsl.exe");
    executable.is_file().then_some(executable).ok_or(())
}

#[cfg(test)]
mod tests {
    use super::{linux_path_from_output, linux_path_is_within};
    use std::path::PathBuf;

    #[test]
    fn accepts_only_one_absolute_linux_path() {
        let path = linux_path_from_output(b"/mnt/c/OpenChat/models\r\n")
            .expect("a WSL path should be accepted");
        assert_eq!(
            PathBuf::from(&path),
            PathBuf::from("/mnt/c/OpenChat/models")
        );
        assert_eq!(path, "/mnt/c/OpenChat/models");
        assert!(linux_path_from_output(b"C:\\OpenChat").is_err());
        assert!(linux_path_from_output(b"/mnt/c/one\n/mnt/c/two").is_err());
        assert!(linux_path_from_output(b"/mnt/c/../etc").is_err());
    }

    #[test]
    fn checks_linux_path_boundaries_without_host_path_semantics() {
        assert!(linux_path_is_within(
            "/root/.local/share/uv/python/cpython-3.12.13/bin/python3.12",
            "/root/.local/share/uv/python"
        ));
        assert!(linux_path_is_within(
            "/root/.local/share/uv/python",
            "/root/.local/share/uv/python"
        ));
        assert!(!linux_path_is_within(
            "/root/.local/share/uv/python-malicious/python3.12",
            "/root/.local/share/uv/python"
        ));
    }
}
