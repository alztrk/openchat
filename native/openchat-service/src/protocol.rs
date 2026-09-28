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
}

impl EventSink {
    pub fn new() -> Self {
        Self {
            output: Arc::new(Mutex::new(tokio::io::stdout())),
        }
    }

    pub async fn send(&self, response: &Response) -> io::Result<()> {
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

#[derive(Debug, Serialize)]
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

#[derive(Debug, Serialize)]
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
