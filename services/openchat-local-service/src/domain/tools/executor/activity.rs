//! Tool activity preparation, recording, and emission helpers.

use super::*;

impl ToolExecutor {
    pub(super) fn prepare_activity(
        &self,
        mut activity: ToolActivity,
        snapshot: &ChatStreamSnapshot,
    ) -> ToolActivity {
        activity.round_id = self
            .turn_state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .current_round_id
            .clone();
        activity.assistant_text_before_byte_offset = Some(snapshot.content.len());
        activity
    }

    #[cfg(test)]
    pub(crate) async fn recorded_activities(&self) -> Vec<ToolActivity> {
        self.tool_activities.lock().await.clone()
    }

    pub(super) async fn record_activity(&self, activity: &ToolActivity) -> Vec<ToolActivity> {
        let mut activities = self.tool_activities.lock().await;
        if let Some(existing) = activities
            .iter_mut()
            .find(|existing| existing.call_id == activity.call_id)
        {
            *existing = activity.clone();
        } else {
            activities.push(activity.clone());
        }
        activities.clone()
    }

    pub(super) async fn emit_prepared_activity(
        &self,
        activity: ToolActivity,
        request_id: &Value,
        snapshot: &ChatStreamSnapshot,
        events: &EventSink,
    ) -> Result<(), ServiceError> {
        let event = ChatStreamEvent::ToolActivityUpdated {
            snapshot: snapshot.clone(),
            activity,
        }
        .into_rpc(request_id.clone());
        events.send(&event).await.map_err(|_| protocol_error())
    }
}
fn protocol_error() -> ServiceError {
    ServiceError::new(
        "local_service_protocol_failed",
        "The local service could not send the tool activity update.",
        true,
    )
}
