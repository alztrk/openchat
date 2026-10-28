#![cfg(all(windows, feature = "storage-broker-test-support"))]

use std::{fs, io, path::PathBuf, time::Duration};

use base64::{Engine as _, engine::general_purpose::STANDARD};
use openchat_local_service::{sandbox::StorageBrokerTestProfile, storage_broker_test_support};
use sha2::{Digest, Sha256};
use tokio::{
    io::{AsyncBufReadExt, AsyncReadExt, AsyncWriteExt, BufReader},
    time::timeout,
};
use zeroize::{Zeroize, Zeroizing};

const BROKER_PROTOCOL_VERSION: u32 = 3;
const MAX_TEST_SNAPSHOT_BYTES: u64 = 32 * 1024 * 1024;

#[tokio::test]
async fn broker_runs_sqlcipher_protocol_in_a_temporary_appcontainer() {
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_openchat-local-service"));
    let profile = StorageBrokerTestProfile::create(&executable)
        .expect("create a unique temporary AppContainer profile");
    let mut broker = profile
        .spawn_broker()
        .expect("start the storage broker in the temporary AppContainer profile");

    let mut stdin = broker.take_stdin().expect("take broker stdin");
    let stdout = broker.take_stdout().expect("take broker stdout");
    let mut stdout = BufReader::new(stdout);
    let mut stderr = broker.take_stderr().expect("take broker stderr");
    let mut key = Zeroizing::new([0x5a; 32]);

    let protocol_result = timeout(Duration::from_secs(15), async {
        let mut bootstrap_status = [0; 5];
        stdin.write_all(b"OCK1\0").await?;
        stdout.read_exact(&mut bootstrap_status).await?;
        if bootstrap_status != *b"OCK1\x01" {
            return Err(io::Error::other(
                "broker rejected temporary key initialization",
            ));
        }

        stdin.write_all(&*key).await?;
        stdout.read_exact(&mut bootstrap_status).await?;
        if bootstrap_status != *b"OCK1\x02" {
            return Err(io::Error::other(
                "broker did not finish SQLCipher initialization",
            ));
        }

        write_request(
            &mut stdin,
            &serde_json::json!({
                "operation": "hello",
                "version": BROKER_PROTOCOL_VERSION,
            }),
        )
        .await?;
        let hello = read_response(&mut stdout).await?;
        if hello["result"] != "ready" || hello["version"] != BROKER_PROTOCOL_VERSION {
            return Err(io::Error::other(
                "broker rejected the storage protocol handshake",
            ));
        }

        write_request(
            &mut stdin,
            &serde_json::json!({
                "operation": "query",
                "sql": "PRAGMA cipher_version",
                "parameters": [],
            }),
        )
        .await?;
        let cipher_version = read_response(&mut stdout).await?;
        let version = cipher_version["rows"][0][0]["value"]
            .as_str()
            .filter(|version| !version.trim().is_empty());
        if cipher_version["result"] != "rows" || version.is_none() {
            return Err(io::Error::other(
                "broker database did not report a SQLCipher version",
            ));
        }

        write_request(
            &mut stdin,
            &serde_json::json!({
                "operation": "execute_batch",
                "sql": "CREATE TABLE broker_restart_probe (value TEXT NOT NULL); INSERT INTO broker_restart_probe VALUES ('persisted-across-restart');",
            }),
        )
        .await?;
        let inserted_value = read_response(&mut stdout).await?;
        if inserted_value["result"] != "unit" {
            return Err(io::Error::other(
                "broker did not persist the SQLCipher restart probe",
            ));
        }

        write_request(
            &mut stdin,
            &serde_json::json!({"operation": "begin_snapshot"}),
        )
        .await?;
        let snapshot = read_response(&mut stdout).await?;
        let snapshot_id = snapshot["snapshot_id"]
            .as_str()
            .filter(|id| !id.is_empty())
            .ok_or_else(|| io::Error::other("broker did not start a database snapshot"))?;
        let total_bytes = snapshot["total_bytes"]
            .as_u64()
            .filter(|size| *size > 0 && *size <= MAX_TEST_SNAPSHOT_BYTES)
            .ok_or_else(|| io::Error::other("broker snapshot size is invalid"))?;
        let expected_sha256 = snapshot["sha256"]
            .as_str()
            .filter(|hash| hash.len() == 64)
            .ok_or_else(|| io::Error::other("broker snapshot digest is invalid"))?;
        if snapshot["result"] != "snapshot_started" {
            return Err(io::Error::other(
                "broker returned an invalid snapshot response",
            ));
        }

        let mut snapshot_bytes = Vec::with_capacity(total_bytes as usize);
        let mut offset = 0u64;
        while offset < total_bytes {
            write_request(
                &mut stdin,
                &serde_json::json!({
                    "operation": "read_snapshot_chunk",
                    "snapshot_id": snapshot_id,
                    "offset": offset,
                }),
            )
            .await?;
            let chunk = read_response(&mut stdout).await?;
            if chunk["result"] != "snapshot_chunk" || chunk["offset"] != offset {
                return Err(io::Error::other(
                    "broker returned an invalid snapshot chunk",
                ));
            }
            let encoded = chunk["data"]
                .as_str()
                .ok_or_else(|| io::Error::other("broker snapshot chunk data is missing"))?;
            let decoded = STANDARD.decode(encoded).map_err(io::Error::other)?;
            if decoded.is_empty() {
                return Err(io::Error::other("broker returned an empty snapshot chunk"));
            }
            offset = offset
                .checked_add(decoded.len() as u64)
                .filter(|next| *next <= total_bytes)
                .ok_or_else(|| io::Error::other("broker snapshot exceeds its declared size"))?;
            if chunk["complete"] != (offset == total_bytes) {
                return Err(io::Error::other(
                    "broker snapshot completion state is invalid",
                ));
            }
            snapshot_bytes.extend_from_slice(&decoded);
        }

        let actual_sha256 = format!("{:x}", Sha256::digest(&snapshot_bytes));
        if actual_sha256 != expected_sha256 {
            return Err(io::Error::other(
                "broker snapshot digest does not match its bytes",
            ));
        }
        if snapshot_bytes.starts_with(b"SQLite format 3\0") {
            return Err(io::Error::other("broker snapshot is plaintext SQLite"));
        }

        write_request(
            &mut stdin,
            &serde_json::json!({
                "operation": "end_snapshot",
                "snapshot_id": snapshot_id,
            }),
        )
        .await?;
        let snapshot_end = read_response(&mut stdout).await?;
        if snapshot_end["result"] != "snapshot_ended" {
            return Err(io::Error::other(
                "broker did not finish the database snapshot",
            ));
        }

        write_request(
            &mut stdin,
            &serde_json::json!({"operation": "commit_pending_profile_restore"}),
        )
        .await?;
        let restore_without_stage = read_response(&mut stdout).await?;
        if restore_without_stage["result"] != "error" {
            return Err(io::Error::other(
                "broker accepted a restore commit without staged restore data",
            ));
        }

        Ok::<(), io::Error>(())
    })
    .await;

    if let Err(error) = protocol_result {
        drop(stdin);
        drop(stdout);
        let mut diagnostic = Vec::new();
        let _ = timeout(Duration::from_secs(2), stderr.read_to_end(&mut diagnostic)).await;
        panic!(
            "temporary AppContainer broker protocol failed: {error}; stderr={}",
            String::from_utf8_lossy(&diagnostic)
        );
    }

    drop(stdin);
    let exit_status = timeout(Duration::from_secs(5), broker.wait())
        .await
        .expect("temporary AppContainer broker exits after stdin closes")
        .expect("wait for temporary AppContainer broker");
    assert!(exit_status.success(), "broker exited with {exit_status}");
    drop(stdout);
    drop(stderr);
    drop(broker);

    let mut broker = profile
        .spawn_broker()
        .expect("restart the storage broker with the same temporary profile");
    let mut stdin = broker.take_stdin().expect("take restarted broker stdin");
    let stdout = broker.take_stdout().expect("take restarted broker stdout");
    let mut stdout = BufReader::new(stdout);
    let mut stderr = broker.take_stderr().expect("take restarted broker stderr");

    let restart_result = timeout(Duration::from_secs(15), async {
        stdin.write_all(b"OCK1\x01").await?;
        stdin.write_all(&*key).await?;
        let mut bootstrap_status = [0; 5];
        stdout.read_exact(&mut bootstrap_status).await?;
        if bootstrap_status != *b"OCK1\x02" {
            return Err(io::Error::other(
                "restarted broker did not reopen the SQLCipher database",
            ));
        }

        write_request(
            &mut stdin,
            &serde_json::json!({
                "operation": "hello",
                "version": BROKER_PROTOCOL_VERSION,
            }),
        )
        .await?;
        let hello = read_response(&mut stdout).await?;
        if hello["result"] != "ready" || hello["version"] != BROKER_PROTOCOL_VERSION {
            return Err(io::Error::other(
                "restarted broker rejected the storage protocol handshake",
            ));
        }

        write_request(
            &mut stdin,
            &serde_json::json!({
                "operation": "query",
                "sql": "SELECT value FROM broker_restart_probe",
                "parameters": [],
            }),
        )
        .await?;
        let persisted_value = read_response(&mut stdout).await?;
        if persisted_value["result"] != "rows"
            || persisted_value["rows"][0][0]["value"] != "persisted-across-restart"
        {
            return Err(io::Error::other(
                "restarted broker could not read the value persisted by the previous process",
            ));
        }
        Ok::<(), io::Error>(())
    })
    .await;
    key.zeroize();

    if let Err(error) = restart_result {
        drop(stdin);
        drop(stdout);
        let mut diagnostic = Vec::new();
        let _ = timeout(Duration::from_secs(2), stderr.read_to_end(&mut diagnostic)).await;
        panic!(
            "restarted AppContainer broker protocol failed: {error}; stderr={}",
            String::from_utf8_lossy(&diagnostic)
        );
    }

    drop(stdin);
    let exit_status = timeout(Duration::from_secs(5), broker.wait())
        .await
        .expect("restarted AppContainer broker exits after stdin closes")
        .expect("wait for restarted AppContainer broker");
    assert!(
        exit_status.success(),
        "restarted broker exited with {exit_status}"
    );
}

#[tokio::test]
async fn broker_activates_an_encrypted_profile_archive_and_persists_it_after_restart() {
    const CONVERSATION_ID: &str = "broker-restored-chat";
    const PASSPHRASE: &str = "temporary-profile-archive-passphrase";
    const ATTACHMENT_BYTES: &[u8] = b"broker archive attachment";

    let executable = PathBuf::from(env!("CARGO_BIN_EXE_openchat-local-service"));
    let temporary_directory = TestDirectory::new();
    let archive_path = temporary_directory
        .path()
        .join("profile.openchatprofilebackup");
    let source_root = temporary_directory.path().join("source-profile");
    let profile = StorageBrokerTestProfile::create(&executable)
        .expect("create a unique temporary AppContainer profile");
    let storage_root = profile
        .storage_root()
        .expect("resolve the disposable AppContainer storage root");
    let key = Zeroizing::new([0x6b; 32]);

    storage_broker_test_support::create_profile_archive(
        &source_root,
        &archive_path,
        CONVERSATION_ID,
        Zeroizing::new(PASSPHRASE.to_owned()),
    )
    .expect("create a passphrase-encrypted profile archive in a temporary directory");
    storage_broker_test_support::stage_profile_restore(
        &storage_root,
        &archive_path,
        &key,
        Zeroizing::new(PASSPHRASE.to_owned()),
    )
    .expect("validate and stage the archive under the destination SQLCipher key");

    let mut broker = profile
        .spawn_broker()
        .expect("start the storage broker in the temporary AppContainer profile");
    let mut stdin = broker.take_stdin().expect("take broker stdin");
    let stdout = broker.take_stdout().expect("take broker stdout");
    let mut stdout = BufReader::new(stdout);
    let mut stderr = broker.take_stderr().expect("take broker stderr");

    let first_start = timeout(Duration::from_secs(15), async {
        bootstrap_existing_database(&mut stdin, &mut stdout, &key).await?;
        assert_restored_conversation(&mut stdin, &mut stdout, CONVERSATION_ID).await?;

        write_request(
            &mut stdin,
            &serde_json::json!({"operation": "commit_pending_profile_restore"}),
        )
        .await?;
        let commit = read_response(&mut stdout).await?;
        if commit["result"] != "unit" {
            return Err(io::Error::other(
                "broker did not commit the validated profile restore",
            ));
        }
        Ok::<(), io::Error>(())
    })
    .await
    .unwrap_or_else(|error| Err(io::Error::other(error)));

    if let Err(error) = first_start {
        drop(stdin);
        drop(stdout);
        let mut diagnostic = Vec::new();
        let _ = timeout(Duration::from_secs(2), stderr.read_to_end(&mut diagnostic)).await;
        panic!(
            "temporary AppContainer restore activation failed: {error}; stderr={}",
            String::from_utf8_lossy(&diagnostic)
        );
    }

    drop(stdin);
    drop(stdout);
    drop(stderr);
    let exit_status = timeout(Duration::from_secs(5), broker.wait())
        .await
        .expect("restored broker exits after stdin closes")
        .expect("wait for restored broker");
    assert!(
        exit_status.success(),
        "restored broker exited with {exit_status}"
    );
    drop(broker);

    let attachment_path = storage_root
        .join("attachments")
        .join(CONVERSATION_ID)
        .join("broker-archive-message")
        .join("broker-archive-attachment.data");
    assert_eq!(
        fs::read(&attachment_path).expect("read the restored attachment from the test profile"),
        ATTACHMENT_BYTES
    );

    let mut broker = profile
        .spawn_broker()
        .expect("restart the storage broker with the same temporary profile");
    let mut stdin = broker.take_stdin().expect("take restarted broker stdin");
    let stdout = broker.take_stdout().expect("take restarted broker stdout");
    let mut stdout = BufReader::new(stdout);
    let mut stderr = broker.take_stderr().expect("take restarted broker stderr");

    let restart = timeout(Duration::from_secs(15), async {
        bootstrap_existing_database(&mut stdin, &mut stdout, &key).await?;
        assert_restored_conversation(&mut stdin, &mut stdout, CONVERSATION_ID).await
    })
    .await
    .unwrap_or_else(|error| Err(io::Error::other(error)));

    if let Err(error) = restart {
        drop(stdin);
        drop(stdout);
        let mut diagnostic = Vec::new();
        let _ = timeout(Duration::from_secs(2), stderr.read_to_end(&mut diagnostic)).await;
        panic!(
            "restarted AppContainer restore verification failed: {error}; stderr={}",
            String::from_utf8_lossy(&diagnostic)
        );
    }

    drop(stdin);
    drop(stdout);
    drop(stderr);
    let exit_status = timeout(Duration::from_secs(5), broker.wait())
        .await
        .expect("restarted broker exits after stdin closes")
        .expect("wait for restarted broker");
    assert!(
        exit_status.success(),
        "restarted broker exited with {exit_status}"
    );
}

async fn bootstrap_existing_database(
    stdin: &mut (impl AsyncWriteExt + Unpin),
    stdout: &mut (impl AsyncBufReadExt + Unpin),
    key: &[u8; 32],
) -> io::Result<()> {
    let mut bootstrap_status = [0; 5];
    stdin.write_all(b"OCK1\x01").await?;
    stdin.write_all(key).await?;
    stdout.read_exact(&mut bootstrap_status).await?;
    if bootstrap_status != *b"OCK1\x02" {
        return Err(io::Error::other(
            "broker did not activate the staged SQLCipher profile archive",
        ));
    }

    write_request(
        stdin,
        &serde_json::json!({
            "operation": "hello",
            "version": BROKER_PROTOCOL_VERSION,
        }),
    )
    .await?;
    let hello = read_response(stdout).await?;
    if hello["result"] != "ready" || hello["version"] != BROKER_PROTOCOL_VERSION {
        return Err(io::Error::other(
            "broker rejected the storage protocol handshake",
        ));
    }

    write_request(
        stdin,
        &serde_json::json!({
            "operation": "query",
            "sql": "PRAGMA cipher_version",
            "parameters": [],
        }),
    )
    .await?;
    let cipher_version = read_response(stdout).await?;
    let version = cipher_version["rows"][0][0]["value"]
        .as_str()
        .filter(|version| !version.trim().is_empty());
    if cipher_version["result"] != "rows" || version.is_none() {
        return Err(io::Error::other(
            "restored broker database did not report a SQLCipher version",
        ));
    }
    Ok(())
}

async fn assert_restored_conversation(
    stdin: &mut (impl AsyncWriteExt + Unpin),
    stdout: &mut (impl AsyncBufReadExt + Unpin),
    conversation_id: &str,
) -> io::Result<()> {
    write_request(
        stdin,
        &serde_json::json!({
            "operation": "query",
            "sql": "SELECT title FROM conversations WHERE id = ?1",
            "parameters": [{"type": "text", "value": conversation_id}],
        }),
    )
    .await?;
    let conversation = read_response(stdout).await?;
    if conversation["result"] != "rows"
        || conversation["rows"][0][0]["value"] != "Restored broker profile"
    {
        return Err(io::Error::other(format!(
            "broker could not read the restored conversation: {conversation}"
        )));
    }
    Ok(())
}

struct TestDirectory(PathBuf);

impl TestDirectory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!(
            "openchat-storage-broker-archive-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir(&path).expect("create an isolated storage broker test directory");
        Self(path)
    }

    fn path(&self) -> &std::path::Path {
        &self.0
    }
}

impl Drop for TestDirectory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

async fn write_request(
    writer: &mut (impl AsyncWriteExt + Unpin),
    request: &serde_json::Value,
) -> io::Result<()> {
    let mut frame = serde_json::to_vec(request).map_err(io::Error::other)?;
    frame.push(b'\n');
    writer.write_all(&frame).await
}

async fn read_response(
    reader: &mut (impl AsyncBufReadExt + Unpin),
) -> io::Result<serde_json::Value> {
    let mut frame = Vec::new();
    reader.read_until(b'\n', &mut frame).await?;
    serde_json::from_slice(&frame).map_err(io::Error::other)
}
