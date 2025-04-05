pub use utils::*;

use {
    super::*,
    crate::{
        eth_token::quick_get_eth_balance,
        tests::signer::Signer,
        transaction::{
            DepositedTx, ExtendedTxEnvelope, NormalizedExtendedTxEnvelope, ScriptOrModule,
            TransactionData,
        },
    },
    alloy::{
        consensus::{SignableTransaction, TxEnvelope, transaction::TxEip1559},
        network::TxSignerSync,
        primitives::{Address, Bytes, FixedBytes, TxKind, address, hex, keccak256},
        rlp::Encodable,
    },
    anyhow::Context,
    aptos_types::{
        contract_event::ContractEventV2,
        transaction::{EntryFunction, Module, Script, TransactionArgument},
    },
    move_binary_format::{
        CompiledModule,
        file_format::{
            FieldDefinition, IdentifierIndex, ModuleHandleIndex, SignatureToken, StructDefinition,
            StructFieldInformation, StructHandle, StructHandleIndex, TypeSignature,
        },
    },
    move_core_types::{
        ability::AbilitySet,
        account_address::AccountAddress,
        identifier::Identifier,
        language_storage::{ModuleId, StructTag},
        value::{MoveStruct, MoveValue},
    },
    move_vm_runtime::module_traversal::{TraversalContext, TraversalStorage},
    move_vm_types::{gas::UnmeteredGasMeter, resolver::ModuleResolver},
    moved_shared::primitives::{B256, ToMoveAddress, ToMoveU256, U64, U256},
    moved_state::{InMemoryState, State},
    serde::de::DeserializeOwned,
    std::path::Path,
};

mod counter;
mod data_type;
mod erc20;
mod evm_native;
mod framework;
mod gas_cost;
mod marketplace;
mod natives;
mod signer;
mod transaction;
mod transfer;
mod utils;

pub const EVM_ADDRESS: Address = address!("8fd379246834eac74b8419ffda202cf8051f7a03");

/// The address corresponding to this private key is 0x8fd379246834eac74B8419FfdA202CF8051F7A03
pub const PRIVATE_KEY: [u8; 32] = [0xaa; 32];

pub const ALT_EVM_ADDRESS: Address = address!("88f9b82462f6c4bf4a0fb15e5c3971559a316e7f");

/// The address corresponding to this private key is 0x88f9b82462f6c4bf4a0fb15e5c3971559a316e7f
pub const ALT_PRIVATE_KEY: [u8; 32] = [0xbb; 32];
