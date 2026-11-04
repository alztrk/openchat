use std::sync::{
    atomic::{AtomicU8, Ordering},
    Arc,
};

const MAX_AUTOMATIC_RELOADS: u8 = 2;

#[derive(Clone, Default)]
pub(super) struct RendererRecoveryTracker(Arc<AtomicU8>);

#[derive(Debug, PartialEq, Eq)]
pub(super) enum RendererRecoveryAction {
    Reload { attempt: u8 },
    ShowRecovery,
}

impl RendererRecoveryTracker {
    pub(super) fn record_failure(&self) -> RendererRecoveryAction {
        let previous_failure_count = self
            .0
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |count| {
                Some(count.saturating_add(1))
            })
            .unwrap_or_else(|count| count);

        if previous_failure_count < MAX_AUTOMATIC_RELOADS {
            RendererRecoveryAction::Reload {
                attempt: previous_failure_count + 1,
            }
        } else {
            RendererRecoveryAction::ShowRecovery
        }
    }

    pub(super) fn renderer_ready(&self) {
        self.0.store(0, Ordering::SeqCst);
    }
}

#[cfg(test)]
mod tests {
    use super::{RendererRecoveryAction, RendererRecoveryTracker};

    #[test]
    fn allows_two_automatic_reloads_before_recovery() {
        let tracker = RendererRecoveryTracker::default();

        assert_eq!(
            tracker.record_failure(),
            RendererRecoveryAction::Reload { attempt: 1 }
        );
        assert_eq!(
            tracker.record_failure(),
            RendererRecoveryAction::Reload { attempt: 2 }
        );
        assert_eq!(
            tracker.record_failure(),
            RendererRecoveryAction::ShowRecovery
        );
    }

    #[test]
    fn a_ready_renderer_starts_a_fresh_failure_sequence() {
        let tracker = RendererRecoveryTracker::default();
        tracker.record_failure();
        tracker.record_failure();
        tracker.renderer_ready();

        assert_eq!(
            tracker.record_failure(),
            RendererRecoveryAction::Reload { attempt: 1 }
        );
    }

    #[test]
    fn failure_count_saturates_instead_of_wrapping() {
        let tracker = RendererRecoveryTracker::default();
        tracker.record_failure();
        tracker.record_failure();

        for _ in 0..=u8::MAX {
            assert_eq!(
                tracker.record_failure(),
                RendererRecoveryAction::ShowRecovery
            );
        }
    }
}
