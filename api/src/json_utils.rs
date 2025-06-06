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

pub fn transaction_error<E: fmt::Debug>(e: E) -> JsonRpcError {
    // code as defined in geth's internal/ethapi/errors.go
    JsonRpcError::without_data(-32000, format!("Execution reverted: {e:?}"))
}

pub fn parse_params_0(request: serde_json::Value) -> Result<(), JsonRpcError> {
    let params = get_params_list(&request);
    match params {
        [] => Ok(()),
        _ => Err(JsonRpcError::parse_error(request, "Too many params")),
    }
}

pub fn parse_params_1<T: DeserializeOwned>(request: serde_json::Value) -> Result<T, JsonRpcError> {
    let params = get_params_list(&request);
    match params {
        [] => Err(JsonRpcError::parse_error(request, "Not enough params")),
        [x] => Ok(deserialize(x)?),
        _ => Err(JsonRpcError::parse_error(request, "Too many params")),
    }
}

pub fn parse_params_2<T1, T2>(request: serde_json::Value) -> Result<(T1, T2), JsonRpcError>
where
    T1: DeserializeOwned,
    T2: DeserializeOwned,
{
    let params = get_params_list(&request);
    match params {
        [] | [_] => Err(JsonRpcError::parse_error(request, "Not enough params")),
        [a, b] => Ok((deserialize(a)?, deserialize(b)?)),
        _ => Err(JsonRpcError::parse_error(request, "Too many params")),
    }
}

pub fn parse_params_3<T1, T2, T3>(request: serde_json::Value) -> Result<(T1, T2, T3), JsonRpcError>
where
    T1: DeserializeOwned,
    T2: DeserializeOwned,
    T3: DeserializeOwned,
{
    let params = get_params_list(&request);
    match params {
        [] | [_] | [_, _] => Err(JsonRpcError::parse_error(request, "Not enough params")),
        [a, b, c] => Ok((deserialize(a)?, deserialize(b)?, deserialize(c)?)),
        _ => Err(JsonRpcError::parse_error(request, "Too many params")),
    }
}
