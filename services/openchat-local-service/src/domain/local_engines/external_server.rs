//! Discovers externally launched llama.cpp server processes on Windows.
//!
//! Process output and candidate counts are bounded, and detected listener ports
//! are accepted only when they resolve to loopback endpoints.

use std::collections::BTreeSet;

#[cfg(windows)]
use std::process::Stdio;
use tokio::io::{AsyncRead, AsyncReadExt};

use crate::protocol::ServiceError;

const MAX_CANDIDATES: usize = 16;
const MAX_PROCESS_OUTPUT_BYTES: usize = 1024 * 1024;
#[cfg(windows)]
const PROCESS_SCAN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
/// A running external llama-server process and its loopback listening port.
pub(crate) struct ExternalLlamaServerCandidate {
    /// Operating-system process identifier.
    pub(crate) process_id: u32,
    /// Local TCP port associated with the detected server process.
    pub(crate) port: u16,
}

/// Finds bounded candidates by matching known server processes to loopback listeners.
pub(crate) async fn find_running_servers() -> Result<Vec<ExternalLlamaServerCandidate>, ServiceError>
{
    #[cfg(windows)]
    {
        let (processes, listeners) = tokio::join!(
            windows_command_output(
                "tasklist",
                &["/FI", "IMAGENAME eq llama-server.exe", "/FO", "CSV", "/NH"]
            ),
            windows_command_output("netstat", &["-ano", "-p", "tcp"]),
        );
        let process_ids = parse_llama_server_process_ids(&processes?)?;
        let listeners = parse_loopback_listeners(&listeners?)?;
        let candidates = listeners
            .into_iter()
            .filter(|candidate| process_ids.contains(&candidate.process_id))
            .take(MAX_CANDIDATES)
            .collect();
        Ok(candidates)
    }
    #[cfg(not(windows))]
    {
        Ok(Vec::new())
    }
}

/// Rechecks whether a previously discovered process and port are still present.
pub(crate) async fn candidate_is_running(
    candidate: ExternalLlamaServerCandidate,
) -> Result<bool, ServiceError> {
    Ok(find_running_servers().await?.contains(&candidate))
}

#[cfg(windows)]
async fn windows_command_output(command: &str, arguments: &[&str]) -> Result<String, ServiceError> {
    let executable = windows_system_command(command)?;
    let mut process = tokio::process::Command::new(executable)
        .args(arguments)
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .kill_on_drop(true)
        .spawn()
        .map_err(|_| process_scan_error())?;
    let Some(mut stdout) = process.stdout.take() else {
        terminate_process(&mut process).await?;
        return Err(process_scan_error());
    };

    let result = tokio::time::timeout(PROCESS_SCAN_TIMEOUT, async {
        let output = read_bounded_process_output(&mut stdout).await?;
        let status = process.wait().await.map_err(|_| process_scan_error())?;
        if !status.success() {
            return Err(process_scan_error());
        }
        String::from_utf8(output).map_err(|_| process_scan_error())
    })
    .await;
    match result {
        Ok(Ok(output)) => Ok(output),
        Ok(Err(_)) | Err(_) => {
            terminate_process(&mut process).await?;
            Err(process_scan_error())
        }
    }
}

async fn read_bounded_process_output<R>(reader: &mut R) -> Result<Vec<u8>, ServiceError>
where
    R: AsyncRead + Unpin,
{
    let mut output = Vec::new();
    let mut buffer = [0_u8; 8192];
    loop {
        let remaining_with_detection_byte = MAX_PROCESS_OUTPUT_BYTES
            .saturating_sub(output.len())
            .saturating_add(1);
        let read_limit = remaining_with_detection_byte.min(buffer.len());
        let read = reader
            .read(&mut buffer[..read_limit])
            .await
            .map_err(|_| process_scan_error())?;
        if read == 0 {
            return Ok(output);
        }
        output.extend_from_slice(&buffer[..read]);
        if output.len() > MAX_PROCESS_OUTPUT_BYTES {
            return Err(process_scan_error());
        }
    }
}

#[cfg(windows)]
async fn terminate_process(process: &mut tokio::process::Child) -> Result<(), ServiceError> {
    if process
        .try_wait()
        .map_err(|_| process_scan_error())?
        .is_none()
    {
        match process.start_kill() {
            Ok(()) => {}
            Err(_)
                if process
                    .try_wait()
                    .map_err(|_| process_scan_error())?
                    .is_some() => {}
            Err(_) => return Err(process_scan_error()),
        }
    }
    process.wait().await.map_err(|_| process_scan_error())?;
    Ok(())
}

#[cfg(windows)]
fn windows_system_command(command: &str) -> Result<std::path::PathBuf, ServiceError> {
    if !matches!(command, "tasklist" | "netstat") {
        return Err(process_scan_error());
    }
    let system_root = std::env::var_os("SystemRoot").ok_or_else(process_scan_error)?;
    let executable = std::path::PathBuf::from(system_root)
        .join("System32")
        .join(format!("{command}.exe"));
    if executable.is_file() {
        Ok(executable)
    } else {
        Err(process_scan_error())
    }
}

fn parse_llama_server_process_ids(output: &str) -> Result<BTreeSet<u32>, ServiceError> {
    if output.len() > MAX_PROCESS_OUTPUT_BYTES {
        return Err(process_scan_error());
    }
    Ok(output
        .lines()
        .filter_map(|line| {
            let mut fields = line.split(',');
            let image_name = fields.next()?.trim().trim_matches('"');
            if !image_name.eq_ignore_ascii_case("llama-server.exe") {
                return None;
            }
            fields
                .next()?
                .trim()
                .trim_matches('"')
                .parse::<u32>()
                .ok()
                .filter(|process_id| *process_id > 0)
        })
        .collect())
}

fn parse_loopback_listeners(
    output: &str,
) -> Result<BTreeSet<ExternalLlamaServerCandidate>, ServiceError> {
    if output.len() > MAX_PROCESS_OUTPUT_BYTES {
        return Err(process_scan_error());
    }
    Ok(output
        .lines()
        .filter_map(|line| {
            let columns = line.split_whitespace().collect::<Vec<_>>();
            if columns.len() < 5
                || !columns[0].eq_ignore_ascii_case("TCP")
                || !columns[3].eq_ignore_ascii_case("LISTENING")
            {
                return None;
            }
            let (address, port) = columns[1].rsplit_once(':')?;
            if !matches!(address, "127.0.0.1" | "0.0.0.0") {
                return None;
            }
            let process_id = columns[4].parse::<u32>().ok()?;
            let port = port.parse::<u16>().ok()?;
            (process_id > 0 && port > 0)
                .then_some(ExternalLlamaServerCandidate { process_id, port })
        })
        .collect())
}

fn process_scan_error() -> ServiceError {
    ServiceError::new(
        "local_engine_process_scan_failed",
        "Running llama-server processes could not be checked.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::{
        ExternalLlamaServerCandidate, MAX_PROCESS_OUTPUT_BYTES, parse_llama_server_process_ids,
        parse_loopback_listeners, read_bounded_process_output,
    };

    #[tokio::test]
    async fn process_output_reader_accepts_the_limit_and_rejects_the_next_byte() {
        let mut within_limit = std::io::Cursor::new(vec![b'x'; MAX_PROCESS_OUTPUT_BYTES]);
        assert_eq!(
            read_bounded_process_output(&mut within_limit)
                .await
                .expect("output at the limit should be accepted")
                .len(),
            MAX_PROCESS_OUTPUT_BYTES
        );

        let mut over_limit = std::io::Cursor::new(vec![b'x'; MAX_PROCESS_OUTPUT_BYTES + 1]);
        assert!(read_bounded_process_output(&mut over_limit).await.is_err());
    }

    #[test]
    fn parses_only_llama_server_process_ids_from_tasklist_csv() {
        let output = concat!(
            "\"llama-server.exe\",\"4216\",\"Console\",\"1\",\"23,456 K\"\r\n",
            "\"python.exe\",\"5732\",\"Console\",\"1\",\"10,000 K\"\r\n",
            "INFO: No tasks are running which match the specified criteria.\r\n",
        );

        assert_eq!(
            parse_llama_server_process_ids(output).expect("tasklist should parse"),
            [4216].into_iter().collect()
        );
    }

    #[test]
    fn keeps_only_ipv4_loopback_or_wildcard_tcp_listeners() {
        let output = concat!(
            "  TCP    127.0.0.1:8080    0.0.0.0:0    LISTENING    4216\r\n",
            "  TCP    0.0.0.0:8081      0.0.0.0:0    LISTENING    4216\r\n",
            "  TCP    192.168.1.10:8082 0.0.0.0:0    LISTENING    4216\r\n",
            "  TCP    127.0.0.1:8083    0.0.0.0:0    ESTABLISHED  4216\r\n",
            "  UDP    127.0.0.1:8084    *:*                      4216\r\n",
        );

        assert_eq!(
            parse_loopback_listeners(output).expect("netstat should parse"),
            [
                ExternalLlamaServerCandidate {
                    process_id: 4216,
                    port: 8080,
                },
                ExternalLlamaServerCandidate {
                    process_id: 4216,
                    port: 8081,
                },
            ]
            .into_iter()
            .collect()
        );
    }
}
