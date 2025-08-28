use {crate::jsonrpc::JsonRpcError, serde::de::DeserializeOwned, std::any};

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

pub fn parse_params_0(request: serde_json::Value) -> Result<(), JsonRpcError> {
    let params = get_params_list(&request);
    match params {
        [] => Ok(()),
        _ => Err(JsonRpcError::too_many_params_error(request)),
    }
}

pub fn parse_params_1<T: DeserializeOwned>(request: serde_json::Value) -> Result<T, JsonRpcError> {
    let params = get_params_list(&request);
    match params {
        [] => Err(JsonRpcError::not_enough_params_error(request)),
        [x] => Ok(deserialize(x)?),
        _ => Err(JsonRpcError::too_many_params_error(request)),
    }
}

pub fn parse_params_2<T1, T2>(request: serde_json::Value) -> Result<(T1, T2), JsonRpcError>
where
    T1: DeserializeOwned,
    T2: DeserializeOwned,
{
    let params = get_params_list(&request);
    match params {
        [] | [_] => Err(JsonRpcError::not_enough_params_error(request)),
        [a, b] => Ok((deserialize(a)?, deserialize(b)?)),
        _ => Err(JsonRpcError::too_many_params_error(request)),
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
        [] | [_] | [_, _] => Err(JsonRpcError::not_enough_params_error(request)),
        [a, b, c] => Ok((deserialize(a)?, deserialize(b)?, deserialize(c)?)),
        _ => Err(JsonRpcError::too_many_params_error(request)),
    }
}

pub fn parse_params_4<T1, T2, T3, T4>(
    request: serde_json::Value,
) -> Result<(T1, T2, T3, T4), JsonRpcError>
where
    T1: DeserializeOwned,
    T2: DeserializeOwned,
    T3: DeserializeOwned,
    T4: DeserializeOwned,
{
    let params = get_params_list(&request);
    match params {
        [] | [_] | [_, _] | [_, _, _] => Err(JsonRpcError::not_enough_params_error(request)),
        [a, b, c, d] => Ok((
            deserialize(a)?,
            deserialize(b)?,
            deserialize(c)?,
            deserialize(d)?,
        )),
        _ => Err(JsonRpcError::too_many_params_error(request)),
    }
}
