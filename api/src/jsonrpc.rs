use std::fmt;

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
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

    pub fn block_not_found<T: fmt::Display>(block_number: T) -> Self {
        // code as defined in geth's internal/ethapi/errors.go
        Self::without_data(-38020, format!("Block not found: {block_number}"))
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
