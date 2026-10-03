use std::{io, sync::Arc};

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::{
    io::{AsyncWriteExt, Stdout},
    sync::Mutex,
};

#[derive(Clone)]
pub struct EventSink {
    output: Arc<Mutex<Stdout>>,
    #[cfg(test)]
    test_events: Option<tokio::sync::mpsc::UnboundedSender<Response>>,
}

impl EventSink {
    pub fn new() -> Self {
        Self {
            output: Arc::new(Mutex::new(tokio::io::stdout())),
            #[cfg(test)]
            test_events: None,
        }
    }

    #[cfg(test)]
    pub(crate) fn test_channel() -> (Self, tokio::sync::mpsc::UnboundedReceiver<Response>) {
        let (sender, receiver) = tokio::sync::mpsc::unbounded_channel();
        (
            Self {
                output: Arc::new(Mutex::new(tokio::io::stdout())),
                test_events: Some(sender),
            },
            receiver,
        )
    }

    pub async fn send(&self, response: &Response) -> io::Result<()> {
        #[cfg(test)]
        if let Some(test_events) = &self.test_events {
            return test_events.send(response.clone()).map_err(io::Error::other);
        }
        let encoded = serde_json::to_vec(response).map_err(io::Error::other)?;
        let mut output = self.output.lock().await;
        output.write_all(&encoded).await?;
        output.write_all(b"\n").await?;
        output.flush().await
    }
}

#[derive(Debug, Deserialize)]
pub struct Request {
    pub id: Value,
    pub method: String,
    #[serde(default = "empty_params")]
    pub params: Value,
}

fn empty_params() -> Value {
    Value::Object(Default::default())
}

#[derive(Clone, Debug, Serialize)]
pub struct Response {
    pub id: Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ServiceError>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub event: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<Value>,
}

impl Response {
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
            event: None,
            data: None,
        }
    }

    pub fn failure(id: Value, error: ServiceError) -> Self {
        Self {
            id,
            result: None,
            error: Some(error),
            event: None,
            data: None,
        }
    }

    pub fn event(id: Value, event: impl Into<String>, data: Value) -> Self {
        Self {
            id,
            result: None,
            error: None,
            event: Some(event.into()),
            data: Some(data),
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct ServiceError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
}

impl ServiceError {
    pub fn new(code: &'static str, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
        }
    }
}
