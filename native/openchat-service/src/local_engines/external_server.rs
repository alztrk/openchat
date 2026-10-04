use std::collections::BTreeSet;

use crate::protocol::ServiceError;

const MAX_CANDIDATES: usize = 16;
const MAX_PROCESS_OUTPUT_BYTES: usize = 1024 * 1024;
#[cfg(windows)]
const PROCESS_SCAN_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct ExternalLlamaServerCandidate {
    pub(crate) process_id: u32,
    pub(crate) port: u16,
}

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

pub(crate) async fn candidate_is_running(
    candidate: ExternalLlamaServerCandidate,
) -> Result<bool, ServiceError> {
    Ok(find_running_servers().await?.contains(&candidate))
}

#[cfg(windows)]
async fn windows_command_output(command: &str, arguments: &[&str]) -> Result<String, ServiceError> {
    let executable = windows_system_command(command)?;
    let output = tokio::time::timeout(
        PROCESS_SCAN_TIMEOUT,
        tokio::process::Command::new(executable)
            .args(arguments)
            .kill_on_drop(true)
            .output(),
    )
    .await
    .map_err(|_| process_scan_error())?
    .map_err(|_| process_scan_error())?;
    if !output.status.success() || output.stdout.len() > MAX_PROCESS_OUTPUT_BYTES {
        return Err(process_scan_error());
    }
    String::from_utf8(output.stdout).map_err(|_| process_scan_error())
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
        ExternalLlamaServerCandidate, parse_llama_server_process_ids, parse_loopback_listeners,
    };

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
