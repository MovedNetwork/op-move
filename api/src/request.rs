use {
    crate::{
        json_utils,
        jsonrpc::{JsonRpcError, JsonRpcResponse},
        method_name::MethodName,
    },
    moved_app::{ApplicationReader, CommandQueue, Dependencies},
    moved_blockchain::payload::NewPayloadId,
};

pub async fn handle(
    request: serde_json::Value,
    queue: CommandQueue,
    is_allowed: impl Fn(&MethodName) -> bool,
    payload_id: &impl NewPayloadId,
    app: ApplicationReader<impl Dependencies>,
) -> JsonRpcResponse {
    let id = json_utils::get_field(&request, "id");
    let jsonrpc = json_utils::get_field(&request, "jsonrpc");

    match inner_handle_request(request, queue, is_allowed, payload_id, app).await {
        Ok(r) => JsonRpcResponse {
            id,
            jsonrpc,
            result: Some(r),
            error: None,
        },
        Err(e) => JsonRpcResponse {
            id,
            jsonrpc,
            result: None,
            error: Some(e),
        },
    }
}

async fn inner_handle_request(
    request: serde_json::Value,
    queue: CommandQueue,
    is_allowed: impl Fn(&MethodName) -> bool,
    payload_id: &impl NewPayloadId,
    app: ApplicationReader<impl Dependencies>,
) -> Result<serde_json::Value, JsonRpcError> {
    use {crate::methods::*, MethodName::*};

    let method: MethodName = json_utils::get_field(&request, "method")
        .as_str()
        .ok_or(JsonRpcError::without_data(-32601, "Invalid/missing method"))?
        .parse()?;

    if !is_allowed(&method) {
        return Err(JsonRpcError::without_data(-32601, "Invalid/missing method"));
    }

    match method {
        ForkChoiceUpdatedV3 => forkchoice_updated::execute_v3(request, queue, payload_id).await,
        GetPayloadV3 => get_payload::execute_v3(request, app).await,
        NewPayloadV3 => new_payload::execute_v3(request, app).await,
        SendRawTransaction => send_raw_transaction::execute(request, queue).await,
        ChainId => chain_id::execute(app).await,
        GetBalance => get_balance::execute(request, app).await,
        GetNonce => get_nonce::execute(request, app).await,
        GetTransactionByHash => get_transaction_by_hash::execute(request, app).await,
        GetBlockByHash => get_block_by_hash::execute(request, app).await,
        GetBlockByNumber => get_block_by_number::execute(request, app).await,
        BlockNumber => block_number::execute(request, app).await,
        FeeHistory => fee_history::execute(request, app).await,
        EstimateGas => estimate_gas::execute(request, app).await,
        Call => call::execute(request, app).await,
        TransactionReceipt => get_transaction_receipt::execute(request, app).await,
        GetProof => get_proof::execute(request, app).await,
        GasPrice => gas_price::execute().await,
    }
}
