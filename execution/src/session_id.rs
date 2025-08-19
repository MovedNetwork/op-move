use {
    crate::transaction::NormalizedEthTransaction,
    aptos_types::transaction::EntryFunction,
    aptos_vm::move_vm_ext::UserTransactionContext,
    op_alloy::consensus::TxDeposit,
    umi_evm_ext::HeaderForExecution,
    umi_genesis::config::{CHAIN_ID, GenesisConfig},
    umi_shared::{
        error::{InvalidTransactionCause, Result},
        primitives::{B256, ToMoveAddress},
    },
};

/// This struct represents a unique identifier for the current session of the MoveVM.
/// It is constructed from data of the transaction that is being executed in that session.
/// his is based on the corresponding [Aptos type](https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.14.0/aptos-move/aptos-vm/src/move_vm_ext/session/session_id.rs#L16)
/// plus the extra parameter Aptos includes in its
/// [session creation function](https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.14.0/aptos-move/aptos-vm/src/move_vm_ext/vm.rs#L130).
#[derive(Default)]
pub struct SessionId {
    pub txn_hash: [u8; 32],
    pub script_hash: Option<[u8; 32]>,
    pub chain_id: u8,
    pub user_txn_context: Option<UserTransactionContext>,
    pub block_header: HeaderForExecution,
}

impl SessionId {
    pub fn new_from_canonical(
        tx: &NormalizedEthTransaction,
        maybe_entry_fn: Option<&EntryFunction>,
        tx_hash: &B256,
        genesis_config: &GenesisConfig,
        block_header: HeaderForExecution,
        script_hash: Option<B256>,
    ) -> Result<Self> {
        let chain_id = u8_chain_id(genesis_config);
        let sender = tx.signer.to_move_address();
        let user_context = UserTransactionContext::new(
            sender,
            Vec::new(),
            sender,
            tx.gas_limit(),
            u64_gas_price(tx.max_fee_per_gas)?,
            chain_id,
            maybe_entry_fn.map(EntryFunction::as_entry_function_payload),
            None,
        );
        Ok(Self {
            txn_hash: tx_hash.0,
            script_hash: script_hash.map(|x| x.0),
            chain_id,
            user_txn_context: Some(user_context),
            block_header,
        })
    }

    pub fn new_from_deposited(
        tx: &TxDeposit,
        tx_hash: &B256,
        genesis_config: &GenesisConfig,
        block_header: HeaderForExecution,
    ) -> Self {
        let chain_id = u8_chain_id(genesis_config);
        let sender = tx.from.to_move_address();
        let user_context = UserTransactionContext::new(
            sender,
            Vec::new(),
            sender,
            tx.gas_limit,
            0,
            chain_id,
            None,
            None,
        );
        Self {
            txn_hash: tx_hash.0,
            script_hash: None,
            chain_id,
            user_txn_context: Some(user_context),
            block_header,
        }
    }
}

fn u64_gas_price(u128_gas_price: u128) -> Result<u64> {
    u128_gas_price
        .try_into()
        .map_err(|_| InvalidTransactionCause::InvalidGasPrice(u128_gas_price).into())
}

/// Ethereum uses U256 (and most projects on Ethereum use u64) for chain id,
/// but Aptos requires u8. Therefore, the purpose of this function is to map
/// the u64 chain ID we have for Ethereum compatibility to a u8 chain ID we
/// need for the Aptos Move extensions. The choice of 1 here was motivated
/// by [Aptos's choice of 1 = Mainnet](https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.14.0/types/src/chain_id.rs#L18).
fn u8_chain_id(genesis_config: &GenesisConfig) -> u8 {
    if genesis_config.chain_id == CHAIN_ID {
        1
    } else {
        genesis_config.chain_id.to_le_bytes()[0]
    }
}
