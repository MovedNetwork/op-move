use {crate::jsonrpc::JsonRpcError, std::str::FromStr};

#[derive(Debug)]
pub enum MethodName {
    ForkChoiceUpdatedV2,
    GetPayloadV2,
    NewPayloadV2,
    ForkChoiceUpdatedV3,
    GetPayloadV3,
    NewPayloadV3,
    SendRawTransaction,
    ChainId,
    GetBalance,
    GetBlockByHash,
    GetBlockByNumber,
}

impl FromStr for MethodName {
    type Err = JsonRpcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "engine_forkchoiceUpdatedV2" => Self::ForkChoiceUpdatedV2,
            "engine_forkchoiceUpdatedV3" => Self::ForkChoiceUpdatedV3,
            "engine_getPayloadV2" => Self::GetPayloadV2,
            "engine_getPayloadV3" => Self::GetPayloadV3,
            "engine_newPayloadV2" => Self::NewPayloadV2,
            "engine_newPayloadV3" => Self::NewPayloadV3,
            "eth_chainId" => Self::ChainId,
            "eth_getBalance" => Self::GetBalance,
            "eth_getBlockByHash" => Self::GetBlockByHash,
            "eth_getBlockByNumber" => Self::GetBlockByNumber,
            "eth_sendRawTransaction" => Self::SendRawTransaction,
            other => {
                return Err(JsonRpcError::without_data(
                    -32601,
                    format!("Unsupported method: {other}"),
                ))
            }
        })
    }
}
