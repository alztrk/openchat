#![cfg(windows)]

//! Synchronous Windows standard-handle transport for the isolated storage broker.
//!
//! This module only owns transport concerns. It deliberately does not expose SQL error text,
//! filesystem paths, or a `rusqlite::Connection`-compatible API. A later storage adapter can use
//! [`RemoteStorageConnection`] as its serialized request channel.

use std::{
    fs::File,
    io::{BufRead, BufReader, BufWriter, Read, Write},
    os::windows::io::AsRawHandle,
    sync::{
        Arc, Mutex, OnceLock,
        atomic::{AtomicBool, Ordering},
    },
    thread,
    time::{Duration, Instant},
};
use windows_sys::Win32::{
    Foundation::{ERROR_BROKEN_PIPE, ERROR_PIPE_NOT_CONNECTED, GetLastError, HANDLE},
    System::Pipes::PeekNamedPipe,
};
use zeroize::Zeroizing;

use crate::sandbox::SandboxedProcess;

use super::ipc_protocol::{
    MAX_STORAGE_FRAME_BYTES, STORAGE_PROTOCOL_VERSION, StorageRequest, StorageResponse,
};

static STORAGE_BROKER_RUNTIME: OnceLock<Mutex<Option<StorageBrokerRuntime>>> = OnceLock::new();
const BROKER_RESPONSE_TIMEOUT: Duration = Duration::from_secs(8);
const BROKER_PIPE_POLL_INTERVAL: Duration = Duration::from_millis(10);

/// A transport or protocol failure that is safe to expose to callers.
///
/// The error intentionally stores no operating-system error text, SQL, request payload, or path.
/// The server's numeric SQLite code is retained because callers can use it for retry classification
/// without receiving the server's diagnostic message.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub(crate) enum RemoteStorageError {
    /// The pipe could not be opened, read, or written.
    Transport,
    /// The requested operation exceeded the bounded frame size.
    FrameTooLarge,
    /// The peer closed the pipe before a complete frame was received.
    UnexpectedEndOfStream,
    /// A frame was not valid JSON for the storage protocol.
    InvalidFrame,
    /// The peer returned a response that does not match the expected protocol shape.
    InvalidResponse,
    /// The peer advertised a different protocol version.
    ProtocolVersionMismatch,
    /// The broker rejected a request. The code is safe numeric protocol metadata only.
    ServerRejected { code: i32 },
    /// The connection mutex was poisoned after a transport worker panicked.
    Poisoned,
    /// The broker did not respond before its bounded request deadline.
    TimedOut,
}

impl std::fmt::Display for RemoteStorageError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Transport => formatter.write_str("The storage broker transport is unavailable."),
            Self::FrameTooLarge => {
                formatter.write_str("The storage broker frame exceeded its size limit.")
            }
            Self::UnexpectedEndOfStream => {
                formatter.write_str("The storage broker closed the connection unexpectedly.")
            }
            Self::InvalidFrame => formatter.write_str("The storage broker frame is invalid."),
            Self::InvalidResponse => formatter.write_str("The storage broker response is invalid."),
            Self::ProtocolVersionMismatch => {
                formatter.write_str("The storage broker protocol version is unsupported.")
            }
            Self::ServerRejected { .. } => {
                formatter.write_str("The storage broker rejected the request.")
            }
            Self::Poisoned => formatter.write_str("The storage broker connection is unavailable."),
            Self::TimedOut => formatter.write_str("The storage broker did not respond in time."),
        }
    }
}

impl std::error::Error for RemoteStorageError {}

/// A serialized, mutex-owned standard-handle connection to the storage broker.
///
/// Requests and responses are strictly paired. The mutex prevents concurrent callers from
/// interleaving frames on the byte stream. The type is cloneable so a later storage queue can
/// share one broker connection while preserving this serialization guarantee.
#[derive(Clone)]
pub(crate) struct RemoteStorageConnection {
    pipe: Arc<Mutex<RemotePipeStream>>,
    poisoned: Arc<AtomicBool>,
}

impl RemoteStorageConnection {
    /// Starts or reuses the current process's isolated storage broker.
    pub(crate) fn connect() -> Result<Self, RemoteStorageError> {
        let runtime = STORAGE_BROKER_RUNTIME.get_or_init(|| Mutex::new(None));
        let mut runtime = runtime
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        if runtime.is_none() {
            *runtime = Some(StorageBrokerRuntime::launch()?);
        }
        runtime
            .as_ref()
            .map(|runtime| runtime.connection.clone())
            .ok_or(RemoteStorageError::Transport)
    }

    /// Sends one request and waits for its paired response.
    ///
    /// Server error messages are discarded at the transport boundary. Only the safe numeric
    /// server code is returned in [`RemoteStorageError::ServerRejected`].
    pub(crate) fn request(
        &self,
        request: StorageRequest,
    ) -> Result<StorageResponse, RemoteStorageError> {
        let connection = if self.poisoned.load(Ordering::Acquire) {
            Self::connect()?
        } else {
            self.clone()
        };
        connection.request_with_recovery(request, true)
    }

    fn request_with_recovery(
        &self,
        request: StorageRequest,
        invalidate_on_failure: bool,
    ) -> Result<StorageResponse, RemoteStorageError> {
        let mut request_written = false;
        let result = (|| {
            if self.poisoned.load(Ordering::Acquire) {
                return Err(RemoteStorageError::Transport);
            }
            let mut pipe = self.pipe.lock().map_err(|_| RemoteStorageError::Poisoned)?;
            if self.poisoned.load(Ordering::Acquire) {
                return Err(RemoteStorageError::Transport);
            }
            write_frame(&mut pipe.writer, &request)?;
            request_written = true;
            let frame = Zeroizing::new(
                read_frame(&mut pipe.reader, BROKER_RESPONSE_TIMEOUT)?
                    .ok_or(RemoteStorageError::UnexpectedEndOfStream)?,
            );
            let response = serde_json::from_slice::<StorageResponse>(&frame)
                .map_err(|_| RemoteStorageError::InvalidFrame)?;
            match response {
                StorageResponse::Error { code, .. } => {
                    Err(RemoteStorageError::ServerRejected { code })
                }
                response => Ok(response),
            }
        })();

        if result.as_ref().is_err_and(|error| {
            !matches!(error, RemoteStorageError::ServerRejected { .. })
                && !(matches!(error, RemoteStorageError::FrameTooLarge) && !request_written)
        }) {
            self.poisoned.store(true, Ordering::Release);
            if invalidate_on_failure {
                self.invalidate_runtime();
            }
        }

        result
    }

    /// Drops only the broker runtime that owns this failed connection.
    fn invalidate_runtime(&self) {
        let Some(runtime) = STORAGE_BROKER_RUNTIME.get() else {
            return;
        };
        let invalidated = {
            let mut runtime = runtime
                .lock()
                .unwrap_or_else(std::sync::PoisonError::into_inner);
            if runtime
                .as_ref()
                .is_some_and(|current| Arc::ptr_eq(&current.connection.pipe, &self.pipe))
            {
                runtime.take()
            } else {
                None
            }
        };
        drop(invalidated);
    }

    fn handshake(&self) -> Result<(), RemoteStorageError> {
        let response = self.request_with_recovery(
            StorageRequest::Hello {
                version: STORAGE_PROTOCOL_VERSION,
            },
            false,
        )?;
        match response {
            StorageResponse::Ready { version } if version == STORAGE_PROTOCOL_VERSION => Ok(()),
            StorageResponse::Ready { .. } => Err(RemoteStorageError::ProtocolVersionMismatch),
            _ => Err(RemoteStorageError::InvalidResponse),
        }
    }
}

struct StorageBrokerRuntime {
    process: Option<SandboxedProcess>,
    connection: RemoteStorageConnection,
    stderr_worker: Option<thread::JoinHandle<()>>,
}

impl StorageBrokerRuntime {
    fn launch() -> Result<Self, RemoteStorageError> {
        let executable = std::env::current_exe().map_err(|_| RemoteStorageError::Transport)?;
        let mut process = SandboxedProcess::spawn_storage_broker(&executable).map_err(|error| {
            eprintln!("storage_broker_launch_failed stage=spawn error={error}");
            RemoteStorageError::Transport
        })?;
        let input = process
            .take_stdin_blocking()
            .map_err(|_| RemoteStorageError::Transport)?;
        let output = process
            .take_stdout_blocking()
            .map_err(|_| RemoteStorageError::Transport)?;
        let stderr = process
            .take_stderr_blocking()
            .map_err(|_| RemoteStorageError::Transport)?;
        let connection = RemoteStorageConnection {
            pipe: Arc::new(Mutex::new(RemotePipeStream::new_split(output, input))),
            poisoned: Arc::new(AtomicBool::new(false)),
        };
        let stderr_worker = thread::Builder::new()
            .name("openchat-storage-broker-stderr".to_owned())
            .spawn(move || drain_broker_stderr(stderr))
            .map_err(|_| RemoteStorageError::Transport)?;
        if let Err(error) = connection.handshake() {
            drop(process);
            let _ = stderr_worker.join();
            return Err(error);
        }
        Ok(Self {
            process: Some(process),
            connection,
            stderr_worker: Some(stderr_worker),
        })
    }
}

impl Drop for StorageBrokerRuntime {
    fn drop(&mut self) {
        self.process.take();
        if let Some(worker) = self.stderr_worker.take()
            && worker.join().is_err()
        {
            eprintln!("storage_broker_stderr_worker_failed");
        }
    }
}

fn drain_broker_stderr(mut stderr: File) {
    let mut buffer = [0; 4096];
    let mut line = Vec::with_capacity(256);
    loop {
        match stderr.read(&mut buffer) {
            Ok(0) => break,
            Ok(bytes_read) => {
                for byte in &buffer[..bytes_read] {
                    if *byte == b'\n' {
                        report_broker_startup_error(&line);
                        line.clear();
                    } else if line.len() < 512 {
                        line.push(*byte);
                    }
                }
            }
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => {}
            Err(_) => {
                eprintln!("storage_broker_stderr_drain_failed");
                break;
            }
        }
    }
    report_broker_startup_error(&line);
}

fn report_broker_startup_error(line: &[u8]) {
    let Some(message) = line.strip_prefix(b"openchat-storage-broker-error: ") else {
        return;
    };
    let message = String::from_utf8_lossy(message)
        .chars()
        .filter(|character| !character.is_control())
        .take(256)
        .collect::<String>();
    eprintln!("storage_broker_child_startup_error={message}");
}

/// Owns the buffered reader and writer ends of the broker's inherited standard handles.
///
/// The server uses newline-delimited JSON. `BufRead::fill_buf` is consumed in bounded chunks so a
/// peer cannot force an unbounded allocation by omitting the newline terminator.
pub(crate) struct RemotePipeStream {
    reader: BufReader<File>,
    writer: BufWriter<File>,
}

impl RemotePipeStream {
    fn new_split(reader: File, writer: File) -> Self {
        Self {
            reader: BufReader::new(reader),
            writer: BufWriter::new(writer),
        }
    }
}

fn write_frame(
    writer: &mut BufWriter<File>,
    value: &impl serde::Serialize,
) -> Result<(), RemoteStorageError> {
    let frame =
        Zeroizing::new(serde_json::to_vec(value).map_err(|_| RemoteStorageError::InvalidFrame)?);
    if frame.len() > MAX_STORAGE_FRAME_BYTES {
        return Err(RemoteStorageError::FrameTooLarge);
    }
    writer
        .write_all(&frame)
        .and_then(|()| writer.write_all(b"\n"))
        .and_then(|()| writer.flush())
        .map_err(|_| RemoteStorageError::Transport)
}

fn read_frame(
    reader: &mut BufReader<File>,
    timeout: Duration,
) -> Result<Option<Vec<u8>>, RemoteStorageError> {
    let deadline = Instant::now() + timeout;
    let mut frame = Vec::new();
    loop {
        if Instant::now() >= deadline {
            return Err(RemoteStorageError::TimedOut);
        }
        if reader.buffer().is_empty() {
            let mut available = 0u32;
            // SAFETY: The reader owns a live anonymous-pipe handle and `available` is writable.
            let peeked = unsafe {
                PeekNamedPipe(
                    reader.get_ref().as_raw_handle() as HANDLE,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if peeked == 0 {
                // SAFETY: GetLastError reads the calling thread's error from PeekNamedPipe.
                let error = unsafe { GetLastError() };
                if matches!(error, ERROR_BROKEN_PIPE | ERROR_PIPE_NOT_CONNECTED) {
                    return Ok(None);
                }
                return Err(RemoteStorageError::Transport);
            }
            if available == 0 {
                let remaining = deadline.saturating_duration_since(Instant::now());
                thread::sleep(remaining.min(BROKER_PIPE_POLL_INTERVAL));
                continue;
            }
        }
        let buffer = reader
            .fill_buf()
            .map_err(|_| RemoteStorageError::Transport)?;
        if buffer.is_empty() {
            return if frame.is_empty() {
                Ok(None)
            } else {
                Err(RemoteStorageError::UnexpectedEndOfStream)
            };
        }

        if let Some(newline) = buffer.iter().position(|byte| *byte == b'\n') {
            if frame.len().saturating_add(newline) > MAX_STORAGE_FRAME_BYTES {
                return Err(RemoteStorageError::FrameTooLarge);
            }
            frame.extend_from_slice(&buffer[..newline]);
            reader.consume(newline.saturating_add(1));
            if frame.last() == Some(&b'\r') {
                frame.pop();
            }
            return Ok(Some(frame));
        }

        if frame.len().saturating_add(buffer.len()) > MAX_STORAGE_FRAME_BYTES {
            return Err(RemoteStorageError::FrameTooLarge);
        }
        let consumed = buffer.len();
        frame.extend_from_slice(buffer);
        reader.consume(consumed);
    }
}

#[cfg(test)]
mod tests {
    use std::{
        fs::File,
        io::{BufReader, Write},
        os::windows::io::FromRawHandle,
        ptr::null_mut,
        time::{Duration, Instant},
    };

    use windows_sys::Win32::{Foundation::HANDLE, System::Pipes::CreatePipe};

    use super::{RemoteStorageError, read_frame};

    fn anonymous_pipe() -> (File, File) {
        let mut read = null_mut();
        let mut write = null_mut();
        // SAFETY: Both output pointers are writable and null security attributes request defaults.
        assert_ne!(
            unsafe { CreatePipe(&mut read, &mut write, null_mut(), 0) },
            0
        );
        // SAFETY: CreatePipe returned two distinct owned handles transferred exactly once.
        unsafe {
            (
                File::from_raw_handle(read as HANDLE),
                File::from_raw_handle(write as HANDLE),
            )
        }
    }

    #[test]
    fn broker_frame_read_times_out_while_pipe_stays_open() {
        let (read, _write) = anonymous_pipe();
        let mut reader = BufReader::new(read);
        let started = Instant::now();

        assert_eq!(
            read_frame(&mut reader, Duration::from_millis(30)),
            Err(RemoteStorageError::TimedOut)
        );
        assert!(started.elapsed() < Duration::from_secs(1));
    }

    #[test]
    fn broker_frame_read_returns_complete_pipe_frame() {
        let (read, mut write) = anonymous_pipe();
        write
            .write_all(b"broker-ready\n")
            .expect("write pipe frame");
        let mut reader = BufReader::new(read);

        assert_eq!(
            read_frame(&mut reader, Duration::from_secs(1)).expect("read pipe frame"),
            Some(b"broker-ready".to_vec())
        );
    }

    #[test]
    fn closed_broker_pipe_reports_end_of_stream() {
        let (read, write) = anonymous_pipe();
        drop(write);
        let mut reader = BufReader::new(read);

        assert_eq!(
            read_frame(&mut reader, Duration::from_secs(1)).expect("read closed pipe"),
            None
        );
    }
}
