//! Defines the newline-delimited JSON request and response contract for the sidecar process.
//!
//! Stdout is reserved for framed protocol responses; diagnostics belong on stderr so they cannot
//! corrupt the frontend's RPC stream.

use std::{fmt, io, sync::Arc};

use serde::{Deserialize, Serialize, de, de::DeserializeSeed};
use serde_json::{Map, Value};
use tokio::{
    io::{AsyncWriteExt, Stdout},
    sync::Mutex,
};
use zeroize::Zeroize;

#[derive(Clone)]
/// Serializes complete protocol responses to the sidecar's stdout stream.
pub struct EventSink {
    output: Arc<Mutex<Stdout>>,
    #[cfg(test)]
    test_events: Option<tokio::sync::mpsc::UnboundedSender<Response>>,
}

impl EventSink {
    /// Creates a sink backed by the process stdout handle.
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

    /// Serializes and flushes one response as a newline-delimited JSON frame.
    ///
    /// # Errors
    /// Returns an I/O error if serialization or stdout writing fails.
    pub async fn send(&self, response: &Response) -> io::Result<()> {
        #[cfg(test)]
        if let Some(test_events) = &self.test_events {
            return test_events.send(response.clone()).map_err(io::Error::other);
        }
        let mut encoded = serde_json::to_vec(response).map_err(io::Error::other)?;
        encoded.push(b'\n');
        let mut output = self.output.lock().await;
        output.write_all(&encoded).await?;
        output.flush().await
    }
}

/// One frontend RPC request decoded from a newline-delimited JSON frame.
pub struct Request {
    pub id: Value,
    pub method: String,
    pub params: Value,
}

impl fmt::Debug for Request {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Request")
            .field("id", &"[REDACTED]")
            .field("method", &self.method)
            .field("params", &"[REDACTED]")
            .finish()
    }
}

impl Drop for Request {
    fn drop(&mut self) {
        zeroize_sensitive_request_fields(&mut self.params);
    }
}

/// Clears known credential fields recursively when an RPC request leaves service ownership.
fn zeroize_sensitive_request_fields(value: &mut Value) {
    match value {
        Value::Array(items) => {
            for item in items {
                zeroize_sensitive_request_fields(item);
            }
        }
        Value::Object(fields) => {
            for (name, field_value) in fields {
                if matches!(name.as_str(), "apiKey" | "secret" | "passphrase") {
                    zeroize_request_json_strings(field_value);
                } else {
                    zeroize_sensitive_request_fields(field_value);
                }
            }
        }
        _ => {}
    }
}

/// Zeroizes every string contained in one field known to hold credentials.
fn zeroize_request_json_strings(value: &mut Value) {
    match value {
        Value::String(text) => text.zeroize(),
        Value::Array(items) => {
            for item in items {
                zeroize_request_json_strings(item);
            }
        }
        Value::Object(fields) => {
            for field_value in fields.values_mut() {
                zeroize_request_json_strings(field_value);
            }
        }
        _ => {}
    }
}

/// Maximum number of JSON values the service retains from one request's parameters.
const MAX_REQUEST_PARAMETER_NODES: usize = 100_000;

/// A decoded request plus parser accounting used by the service admission budget.
pub(crate) struct DecodedRequest {
    pub(crate) request: Request,
    pub(crate) parameter_nodes: usize,
    pub(crate) parameters_are_object: bool,
    pub(crate) too_complex: bool,
}

/// Parses an RPC frame while limiting the number of materialized parameter values.
///
/// `IgnoredAny` drains values after the node budget is reached, so a hostile large array cannot
/// force the service to allocate one `Value` per element before the frame is rejected.
pub(crate) fn decode_request(source: &str) -> Result<DecodedRequest, serde_json::Error> {
    let mut deserializer = serde_json::Deserializer::from_str(source);
    let mut budget = RequestValueBudget::default();
    let request = RequestSeed {
        budget: &mut budget,
    }
    .deserialize(&mut deserializer)?;
    deserializer.end()?;

    Ok(DecodedRequest {
        request,
        parameter_nodes: budget.nodes,
        parameters_are_object: budget.parameters_are_object,
        too_complex: budget.too_complex,
    })
}

/// Tracks retained parameter complexity and envelope validation during one decode.
struct RequestValueBudget {
    nodes: usize,
    parameters_are_object: bool,
    too_complex: bool,
}

impl Default for RequestValueBudget {
    fn default() -> Self {
        Self {
            nodes: 0,
            parameters_are_object: true,
            too_complex: false,
        }
    }
}

/// Deserializes the RPC envelope while sharing its parameter budget with the value visitor.
struct RequestSeed<'a> {
    budget: &'a mut RequestValueBudget,
}

impl<'de> de::DeserializeSeed<'de> for RequestSeed<'_> {
    type Value = Request;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        deserializer.deserialize_map(RequestVisitor {
            budget: self.budget,
        })
    }
}

/// Reads known request fields and skips unknown envelope fields without retaining them.
struct RequestVisitor<'a> {
    budget: &'a mut RequestValueBudget,
}

impl<'de> de::Visitor<'de> for RequestVisitor<'_> {
    type Value = Request;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("an RPC request object")
    }

    fn visit_map<A>(self, mut fields: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut id = None;
        let mut method = None;
        let mut params = None;

        while let Some(key) = fields.next_key::<String>()? {
            match key.as_str() {
                "id" => {
                    if id.is_some() {
                        return Err(de::Error::duplicate_field("id"));
                    }
                    id = Some(fields.next_value::<Value>()?);
                }
                "method" => {
                    if method.is_some() {
                        return Err(de::Error::duplicate_field("method"));
                    }
                    method = Some(fields.next_value::<String>()?);
                }
                "params" => {
                    if params.is_some() {
                        return Err(de::Error::duplicate_field("params"));
                    }
                    params = Some(fields.next_value_seed(ParameterValueSeed {
                        budget: self.budget,
                        root: true,
                    })?);
                }
                _ => {
                    let _: de::IgnoredAny = fields.next_value()?;
                }
            }
        }

        Ok(Request {
            id: id.ok_or_else(|| de::Error::missing_field("id"))?,
            method: method.ok_or_else(|| de::Error::missing_field("method"))?,
            params: params.unwrap_or_else(|| Value::Object(Map::new())),
        })
    }
}

/// Builds one parameter value until the request-wide node budget is exhausted.
struct ParameterValueSeed<'a> {
    budget: &'a mut RequestValueBudget,
    root: bool,
}

impl<'de> de::DeserializeSeed<'de> for ParameterValueSeed<'_> {
    type Value = Value;

    fn deserialize<D>(self, deserializer: D) -> Result<Self::Value, D::Error>
    where
        D: de::Deserializer<'de>,
    {
        if self.budget.too_complex {
            let _: de::IgnoredAny = de::IgnoredAny::deserialize(deserializer)?;
            return Ok(Value::Null);
        }
        if self.budget.nodes >= MAX_REQUEST_PARAMETER_NODES {
            self.budget.too_complex = true;
            let _: de::IgnoredAny = de::IgnoredAny::deserialize(deserializer)?;
            return Ok(Value::Null);
        }
        self.budget.nodes += 1;
        deserializer.deserialize_any(ParameterValueVisitor {
            budget: self.budget,
            root: self.root,
        })
    }
}

/// Builds bounded JSON containers and drains values that exceed the allocation budget.
struct ParameterValueVisitor<'a> {
    budget: &'a mut RequestValueBudget,
    root: bool,
}

impl<'de> de::Visitor<'de> for ParameterValueVisitor<'_> {
    type Value = Value;

    fn expecting(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str("a JSON value")
    }

    fn visit_bool<E>(self, value: bool) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if self.root {
            self.budget.parameters_are_object = false;
            Ok(Value::Null)
        } else {
            Ok(Value::Bool(value))
        }
    }

    fn visit_i64<E>(self, value: i64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if self.root {
            self.budget.parameters_are_object = false;
            Ok(Value::Null)
        } else {
            Ok(Value::from(value))
        }
    }

    fn visit_u64<E>(self, value: u64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if self.root {
            self.budget.parameters_are_object = false;
            Ok(Value::Null)
        } else {
            Ok(Value::from(value))
        }
    }

    fn visit_f64<E>(self, value: f64) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if self.root {
            self.budget.parameters_are_object = false;
            return Ok(Value::Null);
        }
        serde_json::Number::from_f64(value)
            .map(Value::Number)
            .ok_or_else(|| E::custom("invalid JSON number"))
    }

    fn visit_str<E>(self, value: &str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if self.root {
            self.budget.parameters_are_object = false;
            Ok(Value::Null)
        } else {
            Ok(Value::String(value.to_owned()))
        }
    }

    fn visit_borrowed_str<E>(self, value: &'de str) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_str(value)
    }

    fn visit_string<E>(self, value: String) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if self.root {
            self.budget.parameters_are_object = false;
            Ok(Value::Null)
        } else {
            Ok(Value::String(value))
        }
    }

    fn visit_none<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        self.visit_unit()
    }

    fn visit_unit<E>(self) -> Result<Self::Value, E>
    where
        E: de::Error,
    {
        if self.root {
            self.budget.parameters_are_object = false;
        }
        Ok(Value::Null)
    }

    fn visit_seq<A>(self, mut sequence: A) -> Result<Self::Value, A::Error>
    where
        A: de::SeqAccess<'de>,
    {
        if self.root {
            self.budget.parameters_are_object = false;
            while sequence.next_element::<de::IgnoredAny>()?.is_some() {}
            return Ok(Value::Null);
        }

        let mut values = Vec::new();
        while let Some(value) = sequence.next_element_seed(ParameterValueSeed {
            budget: self.budget,
            root: false,
        })? {
            if self.budget.too_complex {
                while sequence.next_element::<de::IgnoredAny>()?.is_some() {}
                break;
            }
            values.push(value);
        }
        Ok(Value::Array(values))
    }

    fn visit_map<A>(self, mut fields: A) -> Result<Self::Value, A::Error>
    where
        A: de::MapAccess<'de>,
    {
        let mut values = Map::new();
        while let Some(key) = fields.next_key::<String>()? {
            let value = fields.next_value_seed(ParameterValueSeed {
                budget: self.budget,
                root: false,
            })?;
            if self.budget.too_complex {
                while fields.next_key::<de::IgnoredAny>()?.is_some() {
                    let _: de::IgnoredAny = fields.next_value()?;
                }
                break;
            }
            values.insert(key, value);
        }
        Ok(Value::Object(values))
    }
}

#[cfg(test)]
mod tests {
    use super::{MAX_REQUEST_PARAMETER_NODES, decode_request, zeroize_sensitive_request_fields};

    #[test]
    fn bounded_request_parser_counts_nested_parameter_values() {
        let decoded = decode_request(
            r#"{"id":"read","method":"system.health","params":{"messages":[{"role":"user","content":"hello"}],"count":2}}"#,
        )
        .expect("valid request should decode");

        assert_eq!(decoded.parameter_nodes, 6);
        assert!(decoded.parameters_are_object);
        assert!(!decoded.too_complex);
        assert_eq!(decoded.request.params["messages"][0]["content"], "hello");
    }

    #[test]
    fn bounded_request_parser_drains_non_object_parameters_without_building_them() {
        let decoded = decode_request(
            r#"{"id":"bad-params","method":"system.health","params":[{"large":[1,2,3]}]}"#,
        )
        .expect("non-object params should remain a decoded request");

        assert_eq!(decoded.request.id, "bad-params");
        assert!(!decoded.parameters_are_object);
        assert_eq!(decoded.request.params, serde_json::Value::Null);
    }

    #[test]
    fn bounded_request_parser_drains_large_non_object_parameters_without_value_allocation() {
        let items = std::iter::repeat_n("0", MAX_REQUEST_PARAMETER_NODES).collect::<Vec<_>>();
        let source = format!(
            "{{\"id\":\"bad-params\",\"method\":\"system.health\",\"params\":[{}]}}",
            items.join(",")
        );
        let decoded = decode_request(&source).expect("wrong parameter shape should still decode");

        assert_eq!(decoded.request.id, "bad-params");
        assert!(!decoded.parameters_are_object);
        assert!(!decoded.too_complex);
        assert_eq!(decoded.parameter_nodes, 1);
    }

    #[test]
    fn bounded_request_parser_keeps_default_empty_parameters_and_large_text_values() {
        let absent = decode_request(r#"{"id":1,"method":"system.health"}"#)
            .expect("params retain their empty-object default");
        assert!(absent.parameters_are_object);
        assert_eq!(absent.request.params, serde_json::json!({}));
        assert_eq!(absent.parameter_nodes, 0);

        let text = "a".repeat(2 * 1024 * 1024);
        let source = serde_json::json!({
            "id": 2,
            "method": "system.health",
            "params": {"content": text},
        })
        .to_string();
        let decoded = decode_request(&source).expect("large strings use one bounded JSON value");

        assert!(!decoded.too_complex);
        assert_eq!(decoded.parameter_nodes, 2);
        assert_eq!(
            decoded.request.params["content"].as_str().map(str::len),
            Some(2 * 1024 * 1024)
        );
    }

    #[test]
    fn bounded_request_parser_stops_materializing_values_after_the_limit() {
        let items = std::iter::repeat_n("0", MAX_REQUEST_PARAMETER_NODES).collect::<Vec<_>>();
        let source = format!(
            "{{\"id\":\"complex\",\"method\":\"system.health\",\"params\":{{\"items\":[{}]}}}}",
            items.join(",")
        );
        let decoded = decode_request(&source).expect("over-limit request remains frame-aligned");

        assert_eq!(decoded.request.id, "complex");
        assert!(decoded.parameters_are_object);
        assert!(decoded.too_complex);
        assert_eq!(decoded.parameter_nodes, MAX_REQUEST_PARAMETER_NODES);
    }

    #[test]
    fn bounded_request_parser_stops_materializing_object_entries_after_the_limit() {
        let entries = (0..MAX_REQUEST_PARAMETER_NODES)
            .map(|index| format!("\"key-{index}\":0"))
            .collect::<Vec<_>>();
        let source = format!(
            "{{\"id\":\"complex-map\",\"method\":\"system.health\",\"params\":{{\"items\":{{{}}}}}}}",
            entries.join(",")
        );
        let decoded = decode_request(&source).expect("over-limit map remains frame-aligned");

        assert_eq!(decoded.request.id, "complex-map");
        assert!(decoded.parameters_are_object);
        assert!(decoded.too_complex);
        assert_eq!(decoded.parameter_nodes, MAX_REQUEST_PARAMETER_NODES);
        assert!(
            decoded
                .request
                .params
                .as_object()
                .is_some_and(|params| params.is_empty())
        );
    }

    #[test]
    fn bounded_request_parser_rejects_malformed_frames_and_duplicate_fields() {
        assert!(decode_request(r#"{"id":1,"method":"system.health","params":{}"#).is_err());
        assert!(decode_request(r#"{"id":1,"method":"system.health","params":{},"id":2}"#).is_err());
    }

    #[test]
    fn request_cleanup_zeroizes_nested_known_secret_fields_only() {
        let mut params = serde_json::json!({
            "apiKey": "provider-secret",
            "nested": {
                "secret": "mcp-secret",
                "archive": {"passphrase": "archive-secret"},
                "content": "ordinary request content"
            }
        });

        zeroize_sensitive_request_fields(&mut params);

        assert_eq!(params["apiKey"], "");
        assert_eq!(params["nested"]["secret"], "");
        assert_eq!(params["nested"]["archive"]["passphrase"], "");
        assert_eq!(params["nested"]["content"], "ordinary request content");
    }

    #[test]
    fn request_debug_output_redacts_untrusted_identifiers_and_parameters() {
        let request = super::Request {
            id: serde_json::json!("api-key-value"),
            method: "chat.send".to_owned(),
            params: serde_json::json!({"apiKey": "provider-secret"}),
        };

        let debug = format!("{request:?}");

        assert!(debug.contains("chat.send"));
        assert!(!debug.contains("api-key-value"));
        assert!(!debug.contains("provider-secret"));
    }
}

#[derive(Clone, Debug, Serialize)]
/// RPC result, error, or asynchronous event serialized back to the frontend.
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
    /// Builds a successful RPC response with a result payload.
    pub fn success(id: Value, result: Value) -> Self {
        Self {
            id,
            result: Some(result),
            error: None,
            event: None,
            data: None,
        }
    }

    /// Builds a failed RPC response with a stable service error.
    pub fn failure(id: Value, error: ServiceError) -> Self {
        Self {
            id,
            result: None,
            error: Some(error),
            event: None,
            data: None,
        }
    }

    /// Builds an event response associated with the originating request ID.
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
/// Machine-readable service failure with user-safe text and retry guidance.
pub struct ServiceError {
    pub code: &'static str,
    pub message: String,
    pub retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub details: Option<String>,
}

impl ServiceError {
    /// Creates an RPC error with a stable code, safe message, and retryability flag.
    pub fn new(code: &'static str, message: impl Into<String>, retryable: bool) -> Self {
        Self {
            code,
            message: message.into(),
            retryable,
            details: None,
        }
    }

    /// Adds a safe diagnostic detail from the failing subsystem without changing the stable code.
    pub fn with_details(mut self, details: impl Into<String>) -> Self {
        self.details = Some(details.into());
        self
    }
}
