pub use gas::{CreateEcotoneL1GasFee, CreateL1GasFee, EcotoneL1GasFee, L1GasFee, L1GasFeeInput};

use {
    crate::{
        genesis::config::GenesisConfig,
        primitives::B256,
        types::{
            session_id::SessionId,
            transactions::{NormalizedExtendedTxEnvelope, ToLog, TransactionExecutionOutcome},
        },
    },
    alloy::primitives::{Bloom, Log},
    aptos_framework::natives::{
        event::NativeEventContext, object::NativeObjectContext,
        transaction_context::NativeTransactionContext,
    },
    aptos_gas_schedule::{MiscGasParameters, NativeGasParameters, LATEST_GAS_FEATURE_VERSION},
    aptos_native_interface::SafeNativeBuilder,
    aptos_table_natives::{NativeTableContext, TableResolver},
    aptos_types::on_chain_config::{Features, TimedFeaturesBuilder},
    aptos_vm::natives::aptos_natives_with_builder,
    canonical::execute_canonical_transaction,
    deposited::execute_deposited_transaction,
    move_binary_format::errors::PartialVMError,
    move_core_types::resolver::MoveResolver,
    move_vm_runtime::{
        move_vm::MoveVM, native_extensions::NativeContextExtensions, session::Session,
    },
};

mod canonical;
mod deposited;
mod eth_token;
mod evm_native;
mod execute;
mod gas;
mod nonces;
mod tag_validation;

#[cfg(test)]
mod tests;

pub fn create_move_vm() -> crate::Result<MoveVM> {
    let mut builder = SafeNativeBuilder::new(
        LATEST_GAS_FEATURE_VERSION,
        NativeGasParameters::zeros(),
        MiscGasParameters::zeros(),
        TimedFeaturesBuilder::enable_all().build(),
        Features::default(),
    );
    let mut natives = aptos_natives_with_builder(&mut builder);
    evm_native::append_evm_natives(&mut natives, &builder);
    let vm = MoveVM::new(natives)?;
    Ok(vm)
}

pub fn create_vm_session<'l, 'r, S>(
    vm: &'l MoveVM,
    state: &'r S,
    session_id: SessionId,
) -> Session<'r, 'l>
where
    S: MoveResolver<PartialVMError> + TableResolver,
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
    native_extensions.add(evm_native::NativeEVMContext::new(state));

    vm.new_session_with_extensions(state, native_extensions)
}

pub fn execute_transaction(
    tx: &NormalizedExtendedTxEnvelope,
    tx_hash: &B256,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
    l1_cost: u64,
) -> crate::Result<TransactionExecutionOutcome> {
    match tx {
        NormalizedExtendedTxEnvelope::DepositedTx(tx) => {
            execute_deposited_transaction(tx, tx_hash, state, genesis_config)
        }
        NormalizedExtendedTxEnvelope::Canonical(tx) => {
            execute_canonical_transaction(tx, tx_hash, state, genesis_config, l1_cost)
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
    fn logs(&mut self) -> impl Iterator<Item = Log>;
}

impl Logs for NativeContextExtensions<'_> {
    fn logs(&mut self) -> impl Iterator<Item = Log> {
        self.remove::<NativeEventContext>()
            .into_events()
            .into_iter()
            .map(|(event, ..)| event.to_log())
    }
}
