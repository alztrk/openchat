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

        tokio::select! {
            decision = receiver => {
                self.pending.lock().await.remove(&approval_request_id);
                decision.map_err(|_| permission_response_error())
            }
            _ = cancellation.changed() => {
                self.pending.lock().await.remove(&approval_request_id);
                Err(operation_cancelled_error())
            }
        }
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

fn protocol_error() -> ServiceError {
    ServiceError::new(
        "local_service_protocol_failed",
        "The local service could not send the tool permission request.",
        true,
    )
}
