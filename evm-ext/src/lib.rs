pub use self::{
    native_evm_context::{
        FRAMEWORK_ADDRESS, HeaderForExecution, NativeEVMContext, ResolverBackedDB,
    },
    native_impl::{
        EVM_CALL_FN_NAME, EVM_CREATE_FN_NAME, EVM_DEPOSIT_FN_NAME, append_evm_natives,
        evm_transact_with_native,
    },
    state_changes::{
        Changes, extract_evm_changes, extract_evm_changes_from_native, genesis_state_changes,
    },
    type_utils::extract_evm_result,
};

use {
    crate::events::EVM_LOG_LAYOUT,
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        identifier::IdentStr,
        value::{MoveStructLayout, MoveTypeLayout},
    },
    revm::primitives::Log,
    std::sync::LazyLock,
};

pub mod erc20;
pub mod events;
mod native_evm_context;
mod native_impl;
mod solidity_abi;
pub mod state;
mod state_changes;
pub mod type_utils;

/// Address where the EVM native is stored
pub const EVM_NATIVE_ADDRESS: AccountAddress = AccountAddress::ONE;

/// Module name to access the EVM native
pub const EVM_NATIVE_MODULE: &IdentStr = ident_str!("evm");

/// Layout for EVM byte code. It is simply a byte vector because we store the raw bytes directly.
pub static CODE_LAYOUT: LazyLock<MoveTypeLayout> =
    LazyLock::new(|| MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)));

pub static EVM_NATIVE_OUTCOME_LAYOUT: LazyLock<MoveTypeLayout> = LazyLock::new(|| {
    MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
        MoveTypeLayout::Bool,
        CODE_LAYOUT.clone(),
        MoveTypeLayout::Vector(Box::new(EVM_LOG_LAYOUT.clone())),
    ]))
});

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmNativeOutcome {
    pub is_success: bool,
    pub output: Vec<u8>,
    pub logs: Vec<Log>,
}
