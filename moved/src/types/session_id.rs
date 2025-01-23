use {
    super::transactions::DepositedTx,
    crate::{
        genesis::config::{GenesisConfig, CHAIN_ID},
        primitives::{ToMoveAddress, B256},
        types::transactions::NormalizedEthTransaction,
    },
    alloy::primitives::U256,
    aptos_types::transaction::EntryFunction,
    aptos_vm::move_vm_ext::UserTransactionContext,
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
    pub block_header: moved_evm_ext::native_evm_context::HeaderForExecution,
}

impl SessionId {
    pub fn new_from_canonical(
        tx: &NormalizedEthTransaction,
        maybe_entry_fn: Option<&EntryFunction>,
        tx_hash: &B256,
        genesis_config: &GenesisConfig,
        block_header: moved_evm_ext::native_evm_context::HeaderForExecution,
        script_hash: Option<B256>,
    ) -> Self {
        let chain_id = u8_chain_id(genesis_config);
        let sender = tx.signer.to_move_address();
        let user_context = UserTransactionContext::new(
            sender,
            Vec::new(),
            sender,
            tx.gas_limit(),
            u64_gas_price(&tx.max_fee_per_gas),
            chain_id,
            maybe_entry_fn.map(EntryFunction::as_entry_function_payload),
            None,
        );
        Self {
            txn_hash: tx_hash.0,
            script_hash: script_hash.map(|x| x.0),
            chain_id,
            user_txn_context: Some(user_context),
            block_header,
        }
    }

    pub fn new_from_deposited(
        tx: &DepositedTx,
        tx_hash: &B256,
        genesis_config: &GenesisConfig,
        block_header: moved_evm_ext::native_evm_context::HeaderForExecution,
    ) -> Self {
        let chain_id = u8_chain_id(genesis_config);
        let sender = tx.from.to_move_address();
        let user_context = UserTransactionContext::new(
            sender,
            Vec::new(),
            sender,
            tx.gas.into_limbs()[0],
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

// TODO: Should we make it an invariant that the gas price is always less than u64::MAX?
fn u64_gas_price(u256_gas_price: &U256) -> u64 {
    match u256_gas_price.as_limbs() {
        [value, 0, 0, 0] => *value,
        _ => u64::MAX,
    }
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
