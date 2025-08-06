use {
    crate::{json_utils::parse_params_2, jsonrpc::JsonRpcError},
    alloy::primitives::Address,
    move_core_types::identifier::Identifier,
    serde::Deserialize,
    umi_app::{ApplicationReader, Dependencies},
};

const DEFAULT_LIMIT: u32 = 10;

pub async fn execute<'reader>(
    request: serde_json::Value,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError> {
    let (
        ListingArgs {
            address,
            after,
            limit,
        },
        block_number,
    ) = parse_params_2(request)?;

    let response = app.move_list_modules(
        address,
        block_number,
        after.as_ref(),
        limit.unwrap_or(DEFAULT_LIMIT),
    )?;

    Ok(serde_json::to_value(response).expect("Must be able to JSON-serialize response"))
}

#[derive(Debug, Deserialize)]
struct ListingArgs {
    address: Address,
    #[serde(default)]
    after: Option<Identifier>,
    #[serde(default)]
    limit: Option<u32>,
}
