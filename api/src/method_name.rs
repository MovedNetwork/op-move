use {crate::jsonrpc::JsonRpcError, std::str::FromStr};

#[derive(Debug)]
pub enum MethodName {
    ForkChoiceUpdatedV3,
    GetPayloadV3,
    NewPayloadV3,
    SendRawTransaction,
    ChainId,
    GetBalance,
    GetCode,
    GetBlockByHash,
    GetBlockByNumber,
    GetStorageAt,
    GetTransactionByHash,
    GetNonce,
    BlockNumber,
    FeeHistory,
    EstimateGas,
    Call,
    TransactionReceipt,
    GetProof,
    GasPrice,
    MaxPriorityFeePerGas,
    ClientVersion,
}

impl MethodName {
    pub fn is_non_engine_api(&self) -> bool {
        !self.is_engine_api()
    }

    pub fn is_engine_api(&self) -> bool {
        matches!(
            self,
            Self::ForkChoiceUpdatedV3 | Self::GetPayloadV3 | Self::NewPayloadV3
        )
    }
}

impl FromStr for MethodName {
    type Err = JsonRpcError;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        Ok(match s {
            "engine_forkchoiceUpdatedV3" => Self::ForkChoiceUpdatedV3,
            "engine_getPayloadV3" => Self::GetPayloadV3,
            "engine_newPayloadV3" => Self::NewPayloadV3,

            "eth_chainId" => Self::ChainId,
            "eth_getBalance" => Self::GetBalance,
            "eth_getCode" => Self::GetCode,
            "eth_getStorageAt" => Self::GetStorageAt,
            "eth_getTransactionCount" => Self::GetNonce,
            "eth_getTransactionByHash" => Self::GetTransactionByHash,
            "eth_getBlockByHash" => Self::GetBlockByHash,
            "eth_getBlockByNumber" => Self::GetBlockByNumber,
            "eth_feeHistory" => Self::FeeHistory,
            "eth_blockNumber" => Self::BlockNumber,
            "eth_sendRawTransaction" => Self::SendRawTransaction,
            "eth_estimateGas" => Self::EstimateGas,
            "eth_call" => Self::Call,
            "eth_getTransactionReceipt" => Self::TransactionReceipt,
            "eth_getProof" => Self::GetProof,
            "eth_gasPrice" => Self::GasPrice,
            "eth_maxPriorityFeePerGas" => Self::MaxPriorityFeePerGas,

            "web3_clientVersion" => Self::ClientVersion,

            other => {
                return Err(JsonRpcError::invalid_method(other));
            }
        })
    }
}
