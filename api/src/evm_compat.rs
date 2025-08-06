use {
    alloy::primitives::TxKind,
    umi_execution::transaction::{
        L2_HIGHEST_ADDRESS, L2_LOWEST_ADDRESS, ScriptOrDeployment, TransactionData,
    },
};

/// Wrap EVM input in the right structs needed for processing by our system then
/// serialize using `bcs` so that it will be parsed properly downstream.
pub fn bcs_serialize_evm_input(input: Option<&[u8]>, to: Option<TxKind>) -> Option<Vec<u8>> {
    match (input, to) {
        (None, _) => None,
        (Some([]), _) => None,
        (Some(bytes), None | Some(TxKind::Create)) => {
            // Encode EVM data for contract creation
            let deployment = ScriptOrDeployment::EvmContract(bytes.to_vec());
            Some(bcs::to_bytes(&deployment).expect("Must serialize EVM data"))
        }
        (Some(bytes), Some(TxKind::Call(address))) => {
            // Encode EVM data for contract call
            // (ignoring L2 address space since no re-encoding is needed there).
            if address < L2_LOWEST_ADDRESS || L2_HIGHEST_ADDRESS < address {
                let tx_data = TransactionData::EvmContract {
                    address,
                    data: bytes.to_vec(),
                };
                Some(tx_data.to_bytes().expect("Must serialize EVM data"))
            } else {
                None
            }
        }
    }
}
