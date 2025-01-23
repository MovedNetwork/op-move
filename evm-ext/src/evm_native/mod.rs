pub use self::{
    native_evm_context::{NativeEVMContext, ResolverBackedDB},
    native_impl::{append_evm_natives, EVM_CALL_FN_NAME},
    state_changes::{extract_evm_changes, genesis_state_changes},
    type_utils::extract_evm_result,
};

use {
    move_core_types::{
        account_address::AccountAddress, ident_str, identifier::IdentStr, value::MoveTypeLayout,
    },
    revm::primitives::Log,
    std::sync::LazyLock,
};

pub mod events;
mod native_evm_context;
mod native_impl;
mod solidity_abi;
mod state_changes;
pub mod trie_types;
pub mod type_utils;

/// Address where the EVM native is stored
pub const EVM_NATIVE_ADDRESS: AccountAddress = AccountAddress::ONE;

/// Module name to access the EVM native
pub const EVM_NATIVE_MODULE: &IdentStr = ident_str!("evm");

/// Layout for EVM byte code. It is simply a byte vector because we store the raw bytes directly.
pub static CODE_LAYOUT: LazyLock<MoveTypeLayout> =
    LazyLock::new(|| MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)));

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvmNativeOutcome {
    pub is_success: bool,
    pub output: Vec<u8>,
    pub logs: Vec<Log>,
}
