use {
    crate::{
        genesis::config::GenesisConfig,
        types::transactions::{ExtendedTxEnvelope, TransactionExecutionOutcome},
    },
    aptos_framework::natives::event::NativeEventContext,
    aptos_gas_schedule::{MiscGasParameters, NativeGasParameters, LATEST_GAS_FEATURE_VERSION},
    aptos_table_natives::{NativeTableContext, TableResolver},
    aptos_types::on_chain_config::{Features, TimedFeaturesBuilder},
    aptos_vm::natives::aptos_natives,
    canonical::execute_canonical_transaction,
    deposited::execute_deposited_transaction,
    move_binary_format::errors::PartialVMError,
    move_core_types::{account_address::AccountAddress, resolver::MoveResolver},
    move_vm_runtime::{
        move_vm::MoveVM, native_extensions::NativeContextExtensions, session::Session,
    },
};

mod canonical;
mod deposited;
mod eth_token;
mod execute;
mod gas;
mod nonces;
mod tag_validation;

#[cfg(test)]
mod tests;

pub fn create_move_vm() -> crate::Result<MoveVM> {
    let natives = aptos_natives(
        LATEST_GAS_FEATURE_VERSION,
        NativeGasParameters::zeros(),
        MiscGasParameters::zeros(),
        TimedFeaturesBuilder::enable_all().build(),
        Features::default(),
    );
    let vm = MoveVM::new(natives)?;
    Ok(vm)
}

pub fn create_vm_session<'l, 'r, S>(vm: &'l MoveVM, state: &'r S) -> Session<'r, 'l>
where
    S: MoveResolver<PartialVMError> + TableResolver,
{
    let mut native_extensions = NativeContextExtensions::default();

    // Events are used in `eth_token` because it depends on `fungible_asset`.
    native_extensions.add(NativeEventContext::default());

    // Tables can be used
    // TODO: what is the right value for txn_hash?
    native_extensions.add(NativeTableContext::new([0; 32], state));

    vm.new_session_with_extensions(state, native_extensions)
}

pub fn execute_transaction(
    tx: &ExtendedTxEnvelope,
    state: &(impl MoveResolver<PartialVMError> + TableResolver),
    genesis_config: &GenesisConfig,
) -> crate::Result<TransactionExecutionOutcome> {
    match tx {
        ExtendedTxEnvelope::DepositedTx(tx) => {
            execute_deposited_transaction(tx, state, genesis_config)
        }
        ExtendedTxEnvelope::Canonical(tx) => {
            execute_canonical_transaction(tx, state, genesis_config)
        }
    }
}

// TODO: is there a way to make Move use 32-byte addresses?
fn evm_address_to_move_address(address: &alloy_primitives::Address) -> AccountAddress {
    let mut bytes = [0; 32];
    bytes[12..32].copy_from_slice(address.as_slice());
    AccountAddress::new(bytes)
}