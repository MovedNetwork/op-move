use {
    crate::{
        json_utils,
        jsonrpc::{JsonRpcError, JsonRpcResponse},
        method_name::MethodName,
    },
    umi_app::{ApplicationReader, CommandQueue, Dependencies},
    umi_blockchain::payload::NewPayloadId,
};

/// Dependency injection that can change how requests are handled.
pub struct RequestModifiers<'a, A, P> {
    is_allowed: A,
    payload_id: &'a P,
}

impl<'a, A, P> RequestModifiers<'a, A, P>
where
    A: Fn(&MethodName) -> bool,
    P: NewPayloadId,
{
    pub fn new(is_allowed: A, payload_id: &'a P) -> Self {
        Self {
            is_allowed,
            payload_id,
        }
    }
}

#[tracing::instrument(level = "debug", skip(queue, modifiers, app))]
pub async fn handle<'reader, A, P>(
    request: serde_json::Value,
    queue: CommandQueue,
    modifiers: RequestModifiers<'_, A, P>,
    app: ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> JsonRpcResponse
where
    A: Fn(&MethodName) -> bool,
    P: NewPayloadId,
{
    let id = json_utils::get_field(&request, "id");
    let jsonrpc = json_utils::get_field(&request, "jsonrpc");

    match inner_handle_request(request, queue, modifiers, &app).await {
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

async fn inner_handle_request<'reader, A, P>(
    request: serde_json::Value,
    queue: CommandQueue,
    modifiers: RequestModifiers<'_, A, P>,
    app: &ApplicationReader<'reader, impl Dependencies<'reader>>,
) -> Result<serde_json::Value, JsonRpcError>
where
    A: Fn(&MethodName) -> bool,
    P: NewPayloadId,
{
    use {crate::methods::*, MethodName::*};

    let RequestModifiers {
        is_allowed,
        payload_id,
    } = modifiers;
    let method: MethodName = json_utils::get_field(&request, "method")
        .as_str()
        .ok_or(JsonRpcError::missing_method(request.clone()))?
        .parse()?;

    if !is_allowed(&method) {
        return Err(JsonRpcError::missing_method(request));
    }

    match method {
        ForkChoiceUpdatedV3 => forkchoice_updated::execute_v3(request, queue, payload_id).await,
        GetPayloadV3 => get_payload::execute_v3(request, app).await,
        NewPayloadV3 => new_payload::execute_v3(request, app).await,
        SendRawTransaction => send_raw_transaction::execute(request, queue).await,
        ChainId => chain_id::execute(request, app).await,
        GetBalance => get_balance::execute(request, app).await,
        GetCode => get_code::execute(request, app).await,
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
        GasPrice => gas_price::execute(request, app).await,
        MaxPriorityFeePerGas => max_priority_fee_per_gas::execute(request, app).await,
    }
}
