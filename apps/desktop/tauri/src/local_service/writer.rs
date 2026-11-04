//! Bounded, ordered writes to the local-service stdin pipe.

use std::{
    sync::{
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, RecvTimeoutError, SyncSender, TrySendError},
        Arc, Mutex,
    },
    thread::{self, JoinHandle},
    time::Duration,
};

use tauri_plugin_shell::process::CommandChild;
use zeroize::Zeroizing;

use super::process_interrupt::ProcessInterrupt;

const MAX_QUEUED_WRITES: usize = 8;
const MAX_QUEUED_WRITE_BYTES: usize = 32 * 1024 * 1024;
pub(crate) const WRITE_TIMEOUT: Duration = Duration::from_secs(10);
const WRITER_SHUTDOWN_TIMEOUT: Duration = Duration::from_secs(2);

type WriteResult = Result<(), WriteError>;

#[derive(Clone)]
pub(crate) struct ServiceWriter {
    sender: SyncSender<WriterMessage>,
    queued_bytes: Arc<AtomicUsize>,
    closed: Arc<AtomicBool>,
}

pub(crate) struct ServiceWriterTask {
    join: Option<JoinHandle<()>>,
    done: Receiver<()>,
}

pub(crate) struct WriteTicket {
    result: Receiver<WriteResult>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum EnqueueError {
    Closed,
    Backpressure,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WriteError {
    TimedOut,
    Failed,
}

enum WriterMessage {
    Write(QueuedWrite),
    Close,
}

struct QueuedWrite {
    frame: Zeroizing<Vec<u8>>,
    result: SyncSender<WriteResult>,
    _reservation: ByteReservation,
}

struct ByteReservation {
    total: Arc<AtomicUsize>,
    bytes: usize,
}

impl ServiceWriter {
    pub(crate) fn start(
        child: Arc<Mutex<Option<CommandChild>>>,
        interrupt: Arc<ProcessInterrupt>,
    ) -> Result<(Self, ServiceWriterTask), String> {
        let write_child = Arc::clone(&child);
        let interrupt_on_failure = Arc::clone(&interrupt);
        Self::spawn_worker(
            move |frame| {
                let mut child = write_child.lock().map_err(|_| WriteError::Failed)?;
                let child = child.as_mut().ok_or(WriteError::Failed)?;
                child.write(frame).map_err(|_| WriteError::Failed)
            },
            move || {
                let _ = interrupt_on_failure.interrupt();
            },
        )
    }

    fn spawn_worker(
        mut write_frame: impl FnMut(&[u8]) -> WriteResult + Send + 'static,
        on_failure: impl FnOnce() + Send + 'static,
    ) -> Result<(Self, ServiceWriterTask), String> {
        let (sender, receiver) = mpsc::sync_channel(MAX_QUEUED_WRITES);
        let (done_sender, done) = mpsc::sync_channel(1);
        let queued_bytes = Arc::new(AtomicUsize::new(0));
        let closed = Arc::new(AtomicBool::new(false));
        let worker_closed = Arc::clone(&closed);
        let join = thread::Builder::new()
            .name("openchat-local-service-writer".to_owned())
            .spawn(move || {
                run_writer(
                    receiver,
                    &worker_closed,
                    |frame| write_frame(frame),
                    on_failure,
                );
                let _ = done_sender.send(());
            })
            .map_err(|_| "The local-service writer could not be started.".to_owned())?;

        Ok((
            Self {
                sender,
                queued_bytes,
                closed,
            },
            ServiceWriterTask {
                join: Some(join),
                done,
            },
        ))
    }

    pub(crate) fn enqueue(&self, frame: Zeroizing<Vec<u8>>) -> Result<WriteTicket, EnqueueError> {
        if self.closed.load(Ordering::Acquire) {
            return Err(EnqueueError::Closed);
        }
        let bytes = frame.len();
        let reservation =
            reserve_bytes(&self.queued_bytes, bytes).ok_or(EnqueueError::Backpressure)?;
        let (result_sender, result) = mpsc::sync_channel(1);
        let message = WriterMessage::Write(QueuedWrite {
            frame,
            result: result_sender,
            _reservation: reservation,
        });
        match self.sender.try_send(message) {
            Ok(()) => Ok(WriteTicket { result }),
            Err(TrySendError::Full(_)) => Err(EnqueueError::Backpressure),
            Err(TrySendError::Disconnected(_)) => Err(EnqueueError::Closed),
        }
    }

    pub(crate) fn close(&self) {
        self.closed.store(true, Ordering::Release);
        let _ = self.sender.try_send(WriterMessage::Close);
    }
}

impl WriteTicket {
    pub(crate) async fn wait(self) -> WriteResult {
        tauri::async_runtime::spawn_blocking(move || self.wait_blocking(WRITE_TIMEOUT))
            .await
            .unwrap_or(Err(WriteError::Failed))
    }

    fn wait_blocking(self, timeout: Duration) -> WriteResult {
        match self.result.recv_timeout(timeout) {
            Ok(result) => result,
            Err(RecvTimeoutError::Timeout) => Err(WriteError::TimedOut),
            Err(RecvTimeoutError::Disconnected) => Err(WriteError::Failed),
        }
    }
}

impl ServiceWriterTask {
    pub(crate) fn wait(self) -> Result<(), String> {
        let mut task = self;
        match task.done.recv_timeout(WRITER_SHUTDOWN_TIMEOUT) {
            Ok(()) => task
                .join
                .take()
                .ok_or_else(|| "The local-service writer state is unavailable.".to_owned())?
                .join()
                .map_err(|_| "The local-service writer stopped unexpectedly.".to_owned()),
            Err(RecvTimeoutError::Timeout) => {
                Err("The local-service writer did not stop in time.".to_owned())
            }
            Err(RecvTimeoutError::Disconnected) => {
                Err("The local-service writer stopped without reporting completion.".to_owned())
            }
        }
    }
}

impl Drop for ByteReservation {
    fn drop(&mut self) {
        self.total.fetch_sub(self.bytes, Ordering::AcqRel);
    }
}

fn reserve_bytes(total: &Arc<AtomicUsize>, bytes: usize) -> Option<ByteReservation> {
    let mut current = total.load(Ordering::Acquire);
    loop {
        let next = current.checked_add(bytes)?;
        if next > MAX_QUEUED_WRITE_BYTES {
            return None;
        }
        match total.compare_exchange_weak(current, next, Ordering::AcqRel, Ordering::Acquire) {
            Ok(_) => {
                return Some(ByteReservation {
                    total: Arc::clone(total),
                    bytes,
                });
            }
            Err(observed) => current = observed,
        }
    }
}

fn run_writer(
    receiver: Receiver<WriterMessage>,
    closed: &AtomicBool,
    mut write_frame: impl FnMut(&[u8]) -> WriteResult,
    on_failure: impl FnOnce(),
) {
    let mut on_failure = Some(on_failure);
    while let Ok(message) = receiver.recv() {
        if closed.load(Ordering::Acquire) || matches!(&message, WriterMessage::Close) {
            break;
        }
        let WriterMessage::Write(request) = message else {
            break;
        };
        let result = write_frame(&request.frame);
        let failed = result.is_err();
        let _ = request.result.send(result);
        drop(request);
        if failed {
            closed.store(true, Ordering::Release);
            if let Some(on_failure) = on_failure.take() {
                on_failure();
            }
            break;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::atomic::AtomicUsize;

    #[test]
    fn write_queue_preserves_fifo_order() {
        let received = Arc::new(Mutex::new(Vec::<Vec<u8>>::new()));
        let writer_received = Arc::clone(&received);
        let (writer, task) = ServiceWriter::spawn_worker(
            move |frame| {
                writer_received
                    .lock()
                    .map_err(|_| WriteError::Failed)?
                    .push(frame.to_vec());
                Ok(())
            },
            || {},
        )
        .expect("writer thread starts");
        let tickets = [b"first".as_slice(), b"second", b"third"]
            .into_iter()
            .map(|frame| {
                writer
                    .enqueue(Zeroizing::new(frame.to_vec()))
                    .expect("frame fits bounded queue")
            })
            .collect::<Vec<_>>();

        for ticket in tickets {
            ticket
                .wait_blocking(Duration::from_secs(1))
                .expect("writer acknowledges frame");
        }
        writer.close();
        task.wait().expect("writer thread stops");

        assert_eq!(
            *received.lock().expect("recorded frames available"),
            vec![b"first".to_vec(), b"second".to_vec(), b"third".to_vec()]
        );
    }

    #[test]
    fn byte_budget_accepts_one_maximum_frame_and_rejects_overflow() {
        let total = Arc::new(AtomicUsize::new(0));
        let maximum_frame = reserve_bytes(&total, 24 * 1024 * 1024)
            .expect("the protocol maximum fits the writer budget");
        let remaining = reserve_bytes(&total, 8 * 1024 * 1024)
            .expect("the remaining writer budget is available");
        assert!(reserve_bytes(&total, 1).is_none());
        drop(maximum_frame);
        let replacement =
            reserve_bytes(&total, 24 * 1024 * 1024).expect("released capacity is reusable");
        drop(remaining);
        drop(replacement);
        assert_eq!(total.load(Ordering::Acquire), 0);
        assert!(reserve_bytes(&total, 24 * 1024 * 1024).is_some());
    }

    #[test]
    fn queue_reports_backpressure_without_waiting_for_the_writer() {
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let writer_gate = Arc::clone(&gate);
        let (started_sender, started_receiver) = mpsc::sync_channel(1);
        let mut started_sender = Some(started_sender);
        let (writer, task) = ServiceWriter::spawn_worker(
            move |_| {
                if let Some(started_sender) = started_sender.take() {
                    let _ = started_sender.send(());
                }
                let (lock, changed) = &*writer_gate;
                let mut released = lock.lock().map_err(|_| WriteError::Failed)?;
                while !*released {
                    released = changed.wait(released).map_err(|_| WriteError::Failed)?;
                }
                Ok(())
            },
            || {},
        )
        .expect("writer thread starts");
        let mut tickets = vec![writer
            .enqueue(Zeroizing::new(vec![b'x']))
            .expect("first frame starts the writer")];
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("writer enters the controlled write");
        for _ in 0..MAX_QUEUED_WRITES {
            let result = writer.enqueue(Zeroizing::new(vec![b'x']));
            match result {
                Ok(ticket) => tickets.push(ticket),
                Err(EnqueueError::Backpressure) => break,
                Err(EnqueueError::Closed) => panic!("writer closed unexpectedly"),
            }
        }
        assert_eq!(tickets.len(), MAX_QUEUED_WRITES + 1);
        assert_eq!(
            writer.enqueue(Zeroizing::new(vec![b'x'])).err(),
            Some(EnqueueError::Backpressure)
        );
        {
            let (lock, changed) = &*gate;
            *lock.lock().expect("release lock available") = true;
            changed.notify_all();
        }
        for ticket in tickets {
            ticket
                .wait_blocking(Duration::from_secs(1))
                .expect("queued frame completes");
        }
        writer.close();
        task.wait().expect("writer thread stops");
        assert_eq!(writer.queued_bytes.load(Ordering::Acquire), 0);
    }

    #[test]
    fn stalled_write_acknowledgment_times_out_without_blocking_the_caller() {
        let gate = Arc::new((Mutex::new(false), std::sync::Condvar::new()));
        let writer_gate = Arc::clone(&gate);
        let (started_sender, started_receiver) = mpsc::sync_channel(1);
        let mut started_sender = Some(started_sender);
        let (writer, task) = ServiceWriter::spawn_worker(
            move |_| {
                if let Some(started_sender) = started_sender.take() {
                    let _ = started_sender.send(());
                }
                let (lock, changed) = &*writer_gate;
                let mut released = lock.lock().map_err(|_| WriteError::Failed)?;
                while !*released {
                    released = changed.wait(released).map_err(|_| WriteError::Failed)?;
                }
                Ok(())
            },
            || {},
        )
        .expect("writer thread starts");
        let ticket = writer
            .enqueue(Zeroizing::new(vec![b'x']))
            .expect("frame fits bounded queue");
        started_receiver
            .recv_timeout(Duration::from_secs(1))
            .expect("writer enters the controlled write");

        assert_eq!(
            ticket.wait_blocking(Duration::from_millis(10)),
            Err(WriteError::TimedOut)
        );
        writer.close();
        {
            let (lock, changed) = &*gate;
            *lock.lock().expect("release lock available") = true;
            changed.notify_all();
        }
        task.wait().expect("writer thread stops after release");
    }
}
