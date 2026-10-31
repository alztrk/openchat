//! Operation gate for managing cancellation during async operations.
//!
//! Tracks the four-phase lifecycle: `RUNNING -> CANCELLATION_REQUESTED -> COMMITTING -> COMPLETED`.

use std::sync::atomic::{AtomicU8, Ordering};

/// Manages a multi-phase operation with cancellation support.
///
/// Phases:
/// 1. `RUNNING` — operation is actively running
/// 2. `CANCELLATION_REQUESTED` — cancellation requested, waiting for commit
/// 3. `COMMITTING` — finalizing before completion
/// 4. `COMPLETED` — operation finished (terminal state)
pub struct OperationGate {
    state: AtomicU8,
}

impl OperationGate {
    const RUNNING: u8 = 0;
    const CANCELLATION_REQUESTED: u8 = 1;
    const COMMITTING: u8 = 2;
    const COMPLETED: u8 = 3;

    /// Accepts cancellation only while the operation has not claimed its commit step.
    pub fn accept_cancellation(&self) -> bool {
        self.state.compare_exchange(
            Self::RUNNING,
            Self::CANCELLATION_REQUESTED,
            Ordering::AcqRel,
            Ordering::Relaxed,
        )
        .is_ok()
    }

    /// Claims the commit phase. Returns false if the operation is already in a terminal state.
    pub fn claim_commit(&self) -> bool {
        self.state
            .compare_exchange(
                Self::CANCELLATION_REQUESTED,
                Self::COMMITTING,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    /// Try to begin commit phase. Returns false if already in a different state.
    pub fn try_begin_commit(&self) -> bool {
        self.state
            .compare_exchange(
                Self::CANCELLATION_REQUESTED,
                Self::COMMITTING,
                Ordering::AcqRel,
                Ordering::Relaxed,
            )
            .is_ok()
    }

    /// Marks the operation as completed.
    pub fn complete(&self) {
        self.state.store(Self::COMPLETED, Ordering::Release);
    }

    /// Returns true if the operation is currently in the running state.
    pub fn is_running(&self) -> bool {
        self.state.load(Ordering::Acquire) == Self::RUNNING
    }

    /// Returns true if cancellation has been requested but not yet committed.
    pub fn cancellation_requested(&self) -> bool {
        self.state.load(Ordering::Acquire) == Self::CANCELLATION_REQUESTED
    }
}

impl Default for OperationGate {
    fn default() -> Self {
        Self {
            state: AtomicU8::new(Self::RUNNING),
        }
    }
}
