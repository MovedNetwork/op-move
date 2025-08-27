use {
    crate::{json_utils::parse_params_3, jsonrpc::JsonRpcError},
    move_table_extension::TableHandle,
    umi_app::{ApplicationReader, Dependencies},
};

/// Fetches `resource` by name that belongs to the `account` in the blockchain state corresponding
/// with the block `number`.
///
/// # Arguments
/// * `table_address`: A "0x" prefixed, 32-byte long, hex encoded number that represents a table
///   handle. Example: `0x34785afd47bed68427de0c6e15d7159bf121d6b7079baf718ad8ea330670cca9`
/// * `request`: A JSON object with the following fields:
///   - key_type: a JSON value describing the type of keys in the table
///   - value_type: a JSON value describing the type of values in the table
///   - key: a JSON value representing the key to lookup in the table
///
///   Example: `{ "key_type": "bool", "value_type": "u8", "key": true }`
/// * `number`: A string that represents a tagged block height, or a "0x" prefixed, hex encoded
///   number that represents the exact block height to read from. Example: `latest`
pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (table_address, request, number) = parse_params_3(request)?;

    let handle = TableHandle(table_address);

    let response = app.move_table_item_by_height(&handle, request, number)?;

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}
