pub use {
    alloy::primitives::U256,
    eth_token::{BaseTokenAccounts, UmiBaseTokenAccounts, mint_eth, quick_get_eth_balance},
    gas::{
        CreateEcotoneL1GasFee, CreateL1GasFee, CreateL2GasFee, CreateUmiL2GasFee, EcotoneGasFee,
        L1GasFee, L1GasFeeInput, L2GasFee, L2GasFeeInput, UmiGasFee,
    },
    nonces::{check_nonce, increment_account_nonce, quick_get_nonce},
};

use {
    crate::resolver_cache::ResolverCache,
    alloy::primitives::{Bloom, Keccak256, Log, LogData},
    aptos_framework::natives::{
        event::NativeEventContext, object::NativeObjectContext,
        transaction_context::NativeTransactionContext,
    },
    aptos_table_natives::{NativeTableContext, TableResolver},
    aptos_types::contract_event::ContractEvent,
    canonical::execute_canonical_transaction,
    deposited::execute_deposited_transaction,
    move_core_types::{
        language_storage::TypeTag,
        value::{MoveTypeLayout, MoveValue},
    },
    move_vm_runtime::{
        move_vm::MoveVM, native_extensions::NativeContextExtensions, session::Session,
    },
    move_vm_types::resolver::MoveResolver,
    op_alloy::consensus::TxDeposit,
    session_id::SessionId,
    std::ops::Deref,
    transaction::{NormalizedEthTransaction, TransactionExecutionOutcome},
    umi_evm_ext::{
        HeaderForExecution,
        events::{
            EVM_LOGS_EVENT_LAYOUT, EVM_LOGS_EVENT_TAG, EthTransferLog, evm_logs_event_to_log,
        },
        state::{BlockHashLookup, StorageTrieRepository},
    },
    umi_genesis::config::GenesisConfig,
    umi_shared::primitives::{B256, ToEthAddress},
};

pub mod resolver_cache;
pub mod session_id;
pub mod simulate;
pub mod transaction;

mod canonical;
mod deposited;
mod eth_token;
mod execute;
mod gas;
mod layout;
mod nonces;
mod table_changes;
mod tag_validation;
#[cfg(test)]
mod tests;

const ADDRESS_LAYOUT: MoveTypeLayout = MoveTypeLayout::Address;
const SIGNER_LAYOUT: MoveTypeLayout = MoveTypeLayout::Signer;
const U256_LAYOUT: MoveTypeLayout = MoveTypeLayout::U256;

pub fn create_vm_session<'l, 'r, S, L, B>(
    vm: &'l MoveVM,
    state: &'r S,
    session_id: SessionId,
    storage_trie: &'r impl StorageTrieRepository,
    eth_transfers_log: &'r L,
    block_hash_lookup: &'r B,
) -> Session<'r, 'l>
where
    S: MoveResolver + TableResolver,
    L: EthTransferLog,
    B: BlockHashLookup,
{
    let txn_hash = session_id.txn_hash;
    let mut native_extensions = NativeContextExtensions::default();

    // Events are used in `eth_token` because it depends on `fungible_asset`.
    native_extensions.add(NativeEventContext::default());

    // Objects are part of the standard library
    native_extensions.add(NativeObjectContext::default());

    // Objects require transaction_context to work
    native_extensions.add(NativeTransactionContext::new(
        txn_hash.to_vec(),
        session_id
            .script_hash
            .map(|h| h.to_vec())
            .unwrap_or_default(),
        session_id.chain_id,
        session_id.user_txn_context,
    ));

    // Tables can be used
    native_extensions.add(NativeTableContext::new(txn_hash, state));

    // EVM native extension
    native_extensions.add(umi_evm_ext::NativeEVMContext::new(
        state,
        storage_trie,
        eth_transfers_log,
        session_id.block_header,
        block_hash_lookup,
    ));

    vm.new_session_with_extensions(state, native_extensions)
}

#[derive(Debug)]
pub enum TransactionExecutionInput<'input, S, ST, F, B, H> {
    Deposit(DepositExecutionInput<'input, S, ST, H>),
    Canonical(CanonicalExecutionInput<'input, S, ST, F, B, H>),
}

#[derive(Debug)]
pub struct DepositExecutionInput<'input, S, ST, H> {
    pub tx: &'input TxDeposit,
    pub tx_hash: &'input B256,
    pub state: &'input S,
    pub storage_trie: &'input ST,
    pub genesis_config: &'input GenesisConfig,
    pub block_header: HeaderForExecution,
    pub block_hash_lookup: &'input H,
}

impl<'input, S, ST, F, B, H> From<DepositExecutionInput<'input, S, ST, H>>
    for TransactionExecutionInput<'input, S, ST, F, B, H>
{
    fn from(value: DepositExecutionInput<'input, S, ST, H>) -> Self {
        Self::Deposit(value)
    }
}

#[derive(Debug)]
pub struct CanonicalExecutionInput<'input, S, ST, F, B, H> {
    pub tx: &'input NormalizedEthTransaction,
    pub tx_hash: &'input B256,
    pub state: &'input S,
    pub storage_trie: &'input ST,
    pub genesis_config: &'input GenesisConfig,
    pub l1_cost: U256,
    pub l2_fee: F,
    pub l2_input: L2GasFeeInput,
    pub base_token: &'input B,
    pub block_header: HeaderForExecution,
    pub block_hash_lookup: &'input H,
}

impl<'input, S, ST, F, B, H> From<CanonicalExecutionInput<'input, S, ST, F, B, H>>
    for TransactionExecutionInput<'input, S, ST, F, B, H>
{
    fn from(value: CanonicalExecutionInput<'input, S, ST, F, B, H>) -> Self {
        Self::Canonical(value)
    }
}

pub fn execute_transaction<
    S: MoveResolver + TableResolver,
    ST: StorageTrieRepository,
    F: L2GasFee,
    B: BaseTokenAccounts,
    H: BlockHashLookup,
>(
    input: TransactionExecutionInput<S, ST, F, B, H>,
    resolver_cache: &mut ResolverCache,
) -> umi_shared::error::Result<TransactionExecutionOutcome> {
    match input {
        TransactionExecutionInput::Deposit(input) => execute_deposited_transaction(input),
        TransactionExecutionInput::Canonical(input) => {
            execute_canonical_transaction(input, resolver_cache)
        }
    }
}

pub trait LogsBloom {
    fn logs_bloom(&mut self) -> Bloom;
}

impl<'a, I: Iterator<Item = &'a Log>> LogsBloom for I {
    fn logs_bloom(&mut self) -> Bloom {
        self.fold(Bloom::ZERO, |mut bloom, log| {
            bloom.accrue_log(log);
            bloom
        })
    }
}

trait Logs {
    fn logs(self) -> Vec<Log>;
}

impl<T> Logs for T
where
    T: IntoIterator<Item = (ContractEvent, Option<MoveTypeLayout>)>,
{
    fn logs(self) -> Vec<Log> {
        let mut result = Vec::new();
        for (event, _) in self.into_iter() {
            push_logs(&event, &mut result);
        }
        result
    }
}

fn push_logs(event: &ContractEvent, dest: &mut Vec<Log<LogData>>) {
    let (type_tag, event_data) = match event {
        ContractEvent::V1(v1) => (v1.type_tag(), v1.event_data()),
        ContractEvent::V2(v2) => (v2.type_tag(), v2.event_data()),
    };

    let struct_tag = match type_tag {
        TypeTag::Struct(struct_tag) => struct_tag,
        _ => unreachable!("This would break move event extension invariant"),
    };

    // Special case for events coming from EVM native
    if struct_tag.as_ref() == EVM_LOGS_EVENT_TAG.deref() {
        return MoveValue::simple_deserialize(event_data, &EVM_LOGS_EVENT_LAYOUT)
            .ok()
            .and_then(|value| evm_logs_event_to_log(value, dest))
            .expect("EVM logs must deserialize correctly");
    }

    let address = struct_tag.address.to_eth_address();

    let mut hasher = Keccak256::new();
    let type_string = type_tag.to_canonical_string();
    hasher.update(type_string.as_bytes());
    let type_hash = hasher.finalize();

    let topics = vec![type_hash];

    let data = event_data.to_vec();
    let data = data.into();

    let log = Log::new_unchecked(address, topics, data);
    dest.push(log);
}
