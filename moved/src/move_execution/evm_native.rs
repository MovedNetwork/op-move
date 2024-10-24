use {
    crate::primitives::ToEthAddress,
    aptos_native_interface::{
        safely_pop_arg, safely_pop_vec_arg, SafeNativeBuilder, SafeNativeContext, SafeNativeResult,
    },
    better_any::{Tid, TidAble},
    move_binary_format::errors::PartialVMError,
    move_core_types::{account_address::AccountAddress, ident_str, resolver::MoveResolver},
    move_vm_runtime::native_functions::NativeFunctionTable,
    move_vm_types::{loaded_data::runtime_types::Type, values::Value},
    revm::{
        db::{CacheDB, DatabaseRef},
        primitives::{AccountInfo, Address, Bytecode, TxEnv, TxKind, B256, U256},
        Evm,
    },
    smallvec::SmallVec,
    std::collections::VecDeque,
};

const EVM_NATIVE_ADDRESS: AccountAddress = AccountAddress::ONE;

#[derive(Tid)]
pub struct NativeEVMContext<'a> {
    db: CacheDB<ResolverBackedDB<'a>>,
}

impl<'a> NativeEVMContext<'a> {
    pub fn new(state: &'a impl MoveResolver<PartialVMError>) -> Self {
        let inner_db = ResolverBackedDB { resolver: state };
        let db = CacheDB::new(inner_db);
        Self { db }
    }
}

pub fn append_evm_natives(natives: &mut NativeFunctionTable, builder: &SafeNativeBuilder) {
    let native = builder.make_native(evm_call);
    natives.push((
        EVM_NATIVE_ADDRESS,
        ident_str!("evm").into(),
        ident_str!("evm_call").into(),
        native,
    ));
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

    let caller = safely_pop_arg!(args, AccountAddress);
    let transact_to = safely_pop_arg!(args, AccountAddress);
    let value = safely_pop_arg!(args, u64); // TODO: handle U256 values
    let data = safely_pop_vec_arg!(args, u8);

    // TODO: does it make sense for EVM gas to be 1:1 with MoveVM gas?
    let gas_limit: u64 = context.gas_balance().into();

    let evm_native_ctx = context.extensions_mut().get_mut::<NativeEVMContext>();
    // TODO: also need to set block env context
    let mut evm = Evm::builder()
        .with_db(&mut evm_native_ctx.db)
        .with_tx_env(TxEnv {
            caller: caller.to_eth_address(),
            gas_limit,
            // Gas price can be zero here because fee is charged in the MoveVM
            gas_price: U256::ZERO,
            transact_to: TxKind::Call(transact_to.to_eth_address()),
            value: U256::from(value),
            data: data.into(),
            // Nonce and chain id can be None because replay attacks
            // are prevented at the MoveVM level. I.e. replay will
            // never occur because the MoveVM will not accept a duplicate
            // transaction
            nonce: None,
            chain_id: None,
            // TODO: could maybe construct something based on the values that
            // have already been accessed in `context.traversal_context()`.
            access_list: Vec::new(),
            gas_priority_fee: None,
            blob_hashes: Vec::new(),
            max_fee_per_blob_gas: None,
            authorization_list: None,
        })
        .build();

    let result = evm.transact();

    todo!()
}

struct ResolverBackedDB<'a> {
    resolver: &'a dyn MoveResolver<PartialVMError>,
}

impl<'a> DatabaseRef for ResolverBackedDB<'a> {
    type Error = PartialVMError;

    fn basic_ref(&self, address: Address) -> Result<Option<AccountInfo>, Self::Error> {
        todo!()
    }

    fn code_by_hash_ref(&self, code_hash: B256) -> Result<Bytecode, Self::Error> {
        todo!()
    }

    fn storage_ref(&self, address: Address, index: U256) -> Result<U256, Self::Error> {
        todo!()
    }

    fn block_hash_ref(&self, number: u64) -> Result<B256, Self::Error> {
        todo!()
    }
}
