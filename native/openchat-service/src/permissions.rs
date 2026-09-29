use std::{collections::HashMap, sync::Arc};

use serde_json::{Value, json};
use tokio::sync::{Mutex, oneshot, watch};
use uuid::Uuid;

use crate::protocol::{EventSink, Response, ServiceError};

const MAX_PENDING_APPROVALS: usize = 32;

#[derive(Clone, Default)]
pub struct ToolPermissionBroker {
    pending: Arc<Mutex<HashMap<String, oneshot::Sender<bool>>>>,
}

impl ToolPermissionBroker {
    pub async fn request_approval(
        &self,
        request_id: &Value,
        tool_name: &str,
        target_path: &str,
        arguments: &Value,
        events: &EventSink,
        cancellation: &mut watch::Receiver<bool>,
    ) -> Result<bool, ServiceError> {
        if *cancellation.borrow() {
            return Err(operation_cancelled_error());
        }

        let approval_request_id = Uuid::new_v4().simple().to_string();
        let (sender, receiver) = oneshot::channel();
        {
            let mut pending = self.pending.lock().await;
            if pending.len() >= MAX_PENDING_APPROVALS {
                return Err(ServiceError::new(
                    "tool_permission_capacity_reached",
                    "Too many tool permission requests are waiting for approval.",
                    true,
                ));
            }
            pending.insert(approval_request_id.clone(), sender);
        }

        let event = Response::event(
            request_id.clone(),
            "chat.tool.permission.requested",
            json!({
                "approvalRequestId": approval_request_id,
                "toolName": tool_name,
                "targetPath": target_path,
                "arguments": arguments,
            }),
        );
        if events.send(&event).await.is_err() {
            self.pending.lock().await.remove(&approval_request_id);
            return Err(protocol_error());
        }

        let decision = wait_for_approval(receiver, cancellation).await;
        self.pending.lock().await.remove(&approval_request_id);
        decision
    }

    pub async fn respond(
        &self,
        approval_request_id: &str,
        approved: bool,
    ) -> Result<Value, ServiceError> {
        let sender = self
            .pending
            .lock()
            .await
            .remove(approval_request_id)
            .ok_or_else(permission_request_unavailable)?;
        sender
            .send(approved)
            .map_err(|_| permission_request_unavailable())?;
        Ok(json!({"accepted": true}))
    }
}

pub(crate) fn operation_cancelled_error() -> ServiceError {
    ServiceError::new(
        "operation_cancelled",
        "The chat response was stopped.",
        false,
    )
}

fn permission_request_unavailable() -> ServiceError {
    ServiceError::new(
        "tool_permission_request_unavailable",
        "This tool permission request is no longer active. Send the message again if needed.",
        false,
    )
}

fn permission_response_error() -> ServiceError {
    ServiceError::new(
        "tool_permission_response_unavailable",
        "The tool permission response could not be received.",
        false,
    )
}

async fn wait_for_approval(
    receiver: oneshot::Receiver<bool>,
    cancellation: &mut watch::Receiver<bool>,
) -> Result<bool, ServiceError> {
    tokio::select! {
        decision = receiver => decision.map_err(|_| permission_response_error()),
        _ = cancellation.changed() => Err(operation_cancelled_error()),
    }
}

fn protocol_error() -> ServiceError {
    ServiceError::new(
        "local_service_protocol_failed",
        "The local service could not send the tool permission request.",
        true,
    )
}

#[cfg(test)]
mod tests {
    use super::{ToolPermissionBroker, wait_for_approval};
    use tokio::sync::{oneshot, watch};

    #[tokio::test]
    async fn approval_decision_resumes_the_pending_tool_without_cancelling_chat() {
        for approved in [true, false] {
            let broker = ToolPermissionBroker::default();
            let (sender, receiver) = oneshot::channel();
            broker
                .pending
                .lock()
                .await
                .insert("approval-1".to_owned(), sender);
            let (cancellation_sender, mut cancellation) = watch::channel(false);
            let waiting =
                tokio::spawn(async move { wait_for_approval(receiver, &mut cancellation).await });
            tokio::task::yield_now().await;

            assert!(!waiting.is_finished(), "the tool waits for the choice");
            let response = broker
                .respond("approval-1", approved)
                .await
                .expect("the permission response is accepted");
            assert_eq!(response["accepted"].as_bool(), Some(true));
            assert_eq!(
                waiting
                    .await
                    .expect("the waiting chat operation does not panic")
                    .expect("the tool receives the decision"),
                approved
            );
            assert!(!*cancellation_sender.borrow(), "the chat remains active");
        }
    }
}
