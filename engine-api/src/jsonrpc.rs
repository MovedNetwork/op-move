use {
    std::fmt,
    tokio::sync::{mpsc::error::SendError, oneshot::error::RecvError},
};

#[derive(Debug, serde::Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub data: serde_json::Value,
    pub message: String,
}

impl JsonRpcError {
    pub fn without_data(code: i64, message: impl Into<String>) -> Self {
        Self {
            code,
            message: message.into(),
            data: serde_json::Value::Null,
        }
    }

    pub fn parse_error(request: serde_json::Value, message: impl Into<String>) -> Self {
        Self {
            code: -32602,
            message: message.into(),
            data: request,
        }
    }

    pub fn access_state_error<E: fmt::Debug>(e: E) -> JsonRpcError {
        Self::without_data(-1, format!("Failed to access state: {e:?}"))
    }
}

#[derive(Debug, serde::Serialize)]
pub struct JsonRpcResponse {
    pub id: serde_json::Value,
    pub jsonrpc: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}

impl<T> From<SendError<T>> for JsonRpcError {
    fn from(value: SendError<T>) -> Self {
        Self::access_state_error(value)
    }
}

impl From<RecvError> for JsonRpcError {
    fn from(value: RecvError) -> Self {
        Self::access_state_error(value)
    }
}
