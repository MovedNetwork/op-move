use {
    crate::jsonrpc::JsonRpcError,
    serde::de::DeserializeOwned,
    std::{any, fmt},
};

pub fn get_field(x: &serde_json::Value, name: &str) -> serde_json::Value {
    x.as_object()
        .and_then(|o| o.get(name))
        .cloned()
        .unwrap_or(serde_json::Value::Null)
}

pub fn get_params_list(x: &serde_json::Value) -> &[serde_json::Value] {
    x.as_object()
        .and_then(|o| o.get("params"))
        .and_then(|v| v.as_array())
        .map(|a| a.as_slice())
        .unwrap_or(&[])
}

pub fn deserialize<T: DeserializeOwned>(x: &serde_json::Value) -> Result<T, JsonRpcError> {
    serde_json::from_value(x.clone()).map_err(|e| JsonRpcError {
        code: -32602,
        data: x.clone(),
        message: format!("Failed to parse type {}: {:?}", any::type_name::<T>(), e),
    })
}

pub fn access_state_error<E: fmt::Debug>(e: E) -> JsonRpcError {
    JsonRpcError::access_state_error(e)
}
