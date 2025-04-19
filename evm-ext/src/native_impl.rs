use {
    super::{
        EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE,
        events::EthTransfer,
        native_evm_context::NativeEVMContext,
        solidity_abi::{abi_decode_params, abi_encode_params},
        type_utils::evm_result_to_move_value,
    },
    crate::{ResolverBackedDB, events::EthTransferLog, native_evm_context::DbError},
    alloy::eips::eip2930::AccessList,
    aptos_gas_algebra::{GasExpression, GasQuantity, InternalGasUnit},
    aptos_native_interface::{
        SafeNativeBuilder, SafeNativeContext, SafeNativeError, SafeNativeResult, safely_pop_arg,
    },
    aptos_types::vm_status::StatusCode,
    move_binary_format::errors::PartialVMError,
    move_core_types::{account_address::AccountAddress, ident_str, identifier::IdentStr},
    move_vm_runtime::native_functions::NativeFunctionTable,
    move_vm_types::{loaded_data::runtime_types::Type, values::Value},
    moved_shared::primitives::{ToEthAddress, ToMoveAddress, ToU256},
    revm::{
        Journal, JournalEntry, MainBuilder, MainContext,
        context::{BlockEnv, CfgEnv, Context, Evm, TxEnv, result::ResultAndState},
        context_interface::{block::BlobExcessGasAndPrice, result::EVMError},
        database::in_memory_db::CacheDB,
        handler::{
            EthFrame, EthPrecompiles, FrameResult, Handler, MainnetHandler,
            instructions::EthInstructions,
        },
        interpreter::{InitialAndFloorGas, interpreter::EthInterpreter},
        primitives::{Address, TxKind, U256},
    },
    smallvec::SmallVec,
    std::collections::VecDeque,
};

pub const EVM_CALL_FN_NAME: &IdentStr = ident_str!("system_evm_call");

// Scale factor relating EVM gas units to MoveVM internal gas units.
// We make the following assumptions:
// 1s CPU time ~ 30M EVM gas (based on
// https://ethereum.stackexchange.com/questions/127852/what-is-the-maximum-amount-of-gas-the-ethereum-virtual-machine-can-handle)
// 1s CPU time ~ 10M Move Internal Gas units (based on comments in the Aptos code:
// https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.27.2/aptos-move/aptos-gas-schedule/src/gas_schedule/transaction.rs#L212)
// This implies 1 IG ~ 3 E.
// But this seems much too optimistic given that the scale factor needed to make the
// MoveVM gas values look like EVM ones (i.e. a transfer transaction costs around 21_000)
// is 600. EVM gas should be 1:1 with this external gas number since the external
// gas is meant to look like EVM numbers. So we'll instead use the conversion that
// 600 IG ~ 1 E (which is 1800x different from the timing-based conversion).
const EVM_SCALE_FACTOR: u64 = 600;
// Amount of EVM gas charged on all EVM transactions.
// This amount is ignored when converting to/from MoveVM internal gas units
// because we do not need to do any signature or nonce checks to start an EVM
// transaction in our case; that was already done by Move.
const EVM_BASE_GAS: u64 = 21_000;

pub fn append_evm_natives(natives: &mut NativeFunctionTable, builder: &SafeNativeBuilder) {
    type NativeFn = fn(
        &mut SafeNativeContext,
        Vec<Type>,
        VecDeque<Value>,
    ) -> SafeNativeResult<SmallVec<[Value; 1]>>;
    let mut push_native = |name, f: NativeFn| {
        let native = builder.make_native(f);
        natives.push((EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into(), name, native));
    };

    push_native(ident_str!("native_evm_call").into(), evm_call);
    push_native(ident_str!("native_evm_create").into(), evm_create);
    push_native(ident_str!("abi_encode_params").into(), abi_encode_params);
    push_native(ident_str!("abi_decode_params").into(), abi_decode_params);
}

fn evm_call(
    context: &mut SafeNativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    debug_assert!(ty_args.is_empty(), "No ty_args in EVM native");
    debug_assert_eq!(
        args.len(),
        4,
        "EVM native args should be from, to, value, data"
    );

    // Safety: unwrap is safe because of the length check above
    // Note: the `safely_pop_vec_arg` macro does not work well for `Vec<u8>`
    // because it has a special runtime representation.
    let data = args.pop_back().unwrap().value_as::<Vec<u8>>()?;
    let value = safely_pop_arg!(args, move_core_types::u256::U256);
    let transact_to = safely_pop_arg!(args, AccountAddress);
    let caller = safely_pop_arg!(args, AccountAddress);

    evm_transact_inner(
        context,
        caller.to_eth_address(),
        TxKind::Call(transact_to.to_eth_address()),
        value.to_u256(),
        data,
    )
}

fn evm_create(
    context: &mut SafeNativeContext,
    ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    debug_assert!(ty_args.is_empty(), "No ty_args in EVM native");
    debug_assert_eq!(args.len(), 3, "EVM native args should be from, value, data");

    // Safety: unwrap is safe because of the length check above
    // Note: the `safely_pop_vec_arg` macro does not work well for `Vec<u8>`
    // because it has a special runtime representation.
    let data = args.pop_back().unwrap().value_as::<Vec<u8>>()?;
    let value = safely_pop_arg!(args, move_core_types::u256::U256);
    let caller = safely_pop_arg!(args, AccountAddress);

    evm_transact_inner(
        context,
        caller.to_eth_address(),
        TxKind::Create,
        value.to_u256(),
        data,
    )
}

// Clippy is warning us that using `Arc` is not necessary when the inner type
// is not `Send`, but we don't have a choice because the library chose to use `Arc`.
// We are choosing to make the inner type `!Send` because this function is only
// executed on a single thread anyway.
#[allow(clippy::arc_with_non_send_sync)]
fn evm_transact_inner(
    context: &mut SafeNativeContext,
    caller: Address,
    transact_to: TxKind,
    value: U256,
    data: Vec<u8>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    let gas_limit: u64 = {
        let internal_units: u64 = context.gas_balance().into();
        internal_units
            .saturating_div(EVM_SCALE_FACTOR)
            .saturating_add(EVM_BASE_GAS)
    };

    let evm_native_ctx = context.extensions_mut().get_mut::<NativeEVMContext>();

    let outcome =
        evm_transact_with_native(evm_native_ctx, caller, transact_to, value, data, gas_limit)?;

    let gas_used = EvmGasUsed::new(outcome.result.gas_used());
    context.charge(gas_used)?;

    let result = outcome.result;
    Ok(smallvec::smallvec![evm_result_to_move_value(result)])
}

pub fn evm_transact_with_native(
    evm_native_ctx: &mut NativeEVMContext,
    caller: Address,
    transact_to: TxKind,
    value: U256,
    data: Vec<u8>,
    gas_limit: u64,
) -> SafeNativeResult<ResultAndState> {
    evm_native_ctx
        .transfer_logs
        .add_tx_origin(caller.to_move_address(), value);
    let db = &mut evm_native_ctx.db;

    let mut evm = Context::mainnet()
        .with_db(db)
        .with_tx(TxEnv {
            caller,
            gas_limit,
            // Gas price can be zero here because fee is charged in the MoveVM
            gas_price: 0,
            tx_type: 0,
            kind: transact_to,
            value,
            data: data.into(),
            // Nonce and chain id can be ignored because replay attacks
            // are prevented at the MoveVM level. I.e. replay will
            // never occur because the MoveVM will not accept a duplicate
            // transaction
            nonce: 0,
            chain_id: None,
            // TODO: could maybe construct something based on the values that
            // have already been accessed in `context.traversal_context()`.
            access_list: AccessList::default(),
            gas_priority_fee: None,
            blob_hashes: Vec::new(),
            max_fee_per_blob_gas: 0,
            authorization_list: Vec::new(),
        })
        .with_block(BlockEnv {
            number: evm_native_ctx.block_header.number,
            beneficiary: Address::ZERO,
            timestamp: evm_native_ctx.block_header.timestamp,
            gas_limit: u64::MAX,
            basefee: 0,
            difficulty: U256::ZERO,
            prevrandao: Some(evm_native_ctx.block_header.prev_randao),
            blob_excess_gas_and_price: Some(BlobExcessGasAndPrice {
                excess_blob_gas: 0,
                blob_gasprice: 0,
            }),
        })
        .modify_cfg_chained(|env| {
            // We can safely disable the transaction-level check because
            // the Move side ensures the funds for `value` were present.
            env.disable_balance_check = true;
            // Nonce can be ignored because replay attacks are prevented by MoveVM.
            env.disable_nonce_check = true;
        })
        .build_mainnet();

    let mut handler = WrappedMainnetHandler {
        inner: InnerMainnetHandler::default(),
        transfer_logs: evm_native_ctx.transfer_logs,
    };
    let outcome = handler.run(&mut evm).map_err(|e| match e {
        EVMError::Database(e) => SafeNativeError::InvariantViolation(e.inner),
        other => SafeNativeError::InvariantViolation(
            PartialVMError::new(StatusCode::ABORTED).with_message(format!("EVM Error: {other:?}")),
        ),
    })?;

    // Capture changes in native context so that they can be
    // converted into Move changes when the session is finalized
    evm_native_ctx.state_changes.push(outcome.state.clone());

    Ok(outcome)
}

// Type aliases to make the `revm` types more tractable
type EvmDB<'a, 'b> = &'a mut CacheDB<ResolverBackedDB<'b>>;
type EvmCtx<'a, 'b> =
    Context<BlockEnv, TxEnv, CfgEnv, EvmDB<'a, 'b>, Journal<EvmDB<'a, 'b>, JournalEntry>>;
type InnerMainnetHandler<'a, 'b> = MainnetHandler<
    Evm<EvmCtx<'a, 'b>, (), EthInstructions<EthInterpreter, EvmCtx<'a, 'b>>, EthPrecompiles>,
    EVMError<DbError>,
    EthFrame<
        Evm<EvmCtx<'a, 'b>, (), EthInstructions<EthInterpreter, EvmCtx<'a, 'b>>, EthPrecompiles>,
        EVMError<DbError>,
        EthInterpreter,
    >,
>;

/// Custom handler to allow extracting transfer events.
struct WrappedMainnetHandler<'a, 'b> {
    inner: InnerMainnetHandler<'a, 'b>,
    transfer_logs: &'a dyn EthTransferLog,
}

impl<'a, 'b> Handler for WrappedMainnetHandler<'a, 'b> {
    type Evm = <InnerMainnetHandler<'a, 'b> as Handler>::Evm;
    type Error = <InnerMainnetHandler<'a, 'b> as Handler>::Error;
    type Frame = <InnerMainnetHandler<'a, 'b> as Handler>::Frame;
    type HaltReason = <InnerMainnetHandler<'a, 'b> as Handler>::HaltReason;

    // Modify the post-execution handler to extract transfer events.
    fn post_execution(
        &self,
        evm: &mut Self::Evm,
        exec_result: FrameResult,
        init_and_floor_gas: InitialAndFloorGas,
        eip7702_gas_refund: i64,
    ) -> Result<ResultAndState<Self::HaltReason>, Self::Error> {
        let transfers = evm.journaled_state.journal.iter().flat_map(|entries| {
            entries.iter().filter_map(|entry| {
                if let revm::JournalEntry::BalanceTransfer { from, to, balance } = entry {
                    Some(EthTransfer {
                        from: from.to_move_address(),
                        to: to.to_move_address(),
                        amount: *balance,
                    })
                } else {
                    None
                }
            })
        });
        for t in transfers {
            self.transfer_logs.push_transfer(t);
        }
        self.inner
            .post_execution(evm, exec_result, init_and_floor_gas, eip7702_gas_refund)
    }
}

struct EvmGasUsed {
    amount: u64,
}

impl EvmGasUsed {
    fn new(amount: u64) -> Self {
        Self { amount }
    }
}

impl<Env> GasExpression<Env> for EvmGasUsed {
    type Unit = InternalGasUnit;

    fn evaluate(&self, _feature_version: u64, _env: &Env) -> GasQuantity<Self::Unit> {
        GasQuantity::new(
            self.amount
                .saturating_sub(EVM_BASE_GAS)
                .saturating_mul(EVM_SCALE_FACTOR),
        )
    }

    fn visit(&self, visitor: &mut impl aptos_gas_algebra::GasExpressionVisitor) {
        visitor.quantity::<Self::Unit>(GasQuantity::new(self.amount));
    }
}
