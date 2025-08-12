use {
    super::tag_validation::{validate_entry_type_tag, validate_entry_value},
    crate::{ADDRESS_LAYOUT, SIGNER_LAYOUT, U256_LAYOUT, layout::has_value_invariants},
    alloy::primitives::Address,
    aptos_types::{
        transaction::{EntryFunction, ModuleBundle, Script},
        vm_status::StatusCode,
    },
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        effects::{ChangeSet, Op},
        language_storage::{ModuleId, TypeTag},
        value::MoveValue,
    },
    move_vm_runtime::{
        CodeStorage, ModuleStorage, StagingModuleStorage, module_traversal::TraversalContext,
        session::Session,
    },
    move_vm_types::{
        gas::GasMeter, loaded_data::runtime_types::Type, value_serde::ValueSerDeContext,
        values::Value,
    },
    umi_evm_ext::{
        CODE_LAYOUT, EVM_CALL_FN_NAME, EVM_CREATE_FN_NAME, EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE,
        EvmNativeOutcome, extract_evm_result,
    },
    umi_shared::{
        error::{
            Error::{self, User},
            InvalidTransactionCause, ScriptTransaction, UserError,
        },
        primitives::{ToMoveU256, U256},
    },
};

pub(super) fn execute_entry_function<G: GasMeter, MS: ModuleStorage>(
    entry_fn: EntryFunction,
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &MS,
) -> umi_shared::error::Result<()> {
    let (module_id, function_name, ty_args, args) = entry_fn.into_inner();

    // Validate signer params match the actual signer
    let function = session.load_function(module_storage, &module_id, &function_name, &ty_args)?;
    if function.param_tys().len() != args.len() {
        Err(InvalidTransactionCause::MismatchedArgumentCount)?;
    }
    for (ty, bytes) in function.param_tys().iter().zip(&args) {
        // References are ignored in entry function signatures because the
        // values are actualized in the serialized arguments.
        let ty = strip_reference(ty)?;
        // Note: the function is safe even though the `get_type_tag` implementation
        // has unbounded recursion in it because the recursion depth is limited at
        // the time a module is deployed. If a module has been successfully deployed
        // then we know the recursion is bounded to a reasonable degree (less than depth 255).
        // See `test_deeply_nested_type`.
        let tag = session.get_type_tag(ty, module_storage)?;
        validate_entry_type_tag(&tag)?;
        let layout = session.get_type_layout_from_ty(ty, module_storage)?;
        // Check layout for value-based invariants and only deserialize if necessary.
        if has_value_invariants(&layout) {
            let arg = ValueSerDeContext::new()
                .deserialize(bytes, &layout)
                .ok_or(InvalidTransactionCause::FailedArgumentDeserialization)?
                .as_move_value(&layout);
            // Note: no recursion limit is needed in this function because we have already
            // constructed the recursive types `Type`, `TypeTag`, `MoveTypeLayout` and `MoveValue` so
            // the values must have respected whatever recursion limit is present in MoveVM.
            validate_entry_value(&tag, &arg, signer, session, module_storage)?;
        }
    }

    let function = session.load_function(module_storage, &module_id, &function_name, &ty_args)?;
    session.execute_entry_function(function, args, gas_meter, traversal_context, module_storage)?;
    Ok(())
}

pub(super) fn execute_script<G: GasMeter, CS: CodeStorage>(
    script: Script,
    signer: &AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    code_storage: &CS,
) -> umi_shared::error::Result<()> {
    let function = session.load_script(code_storage, script.code(), script.ty_args())?;
    let serialized_signer = MoveValue::Signer(*signer).simple_serialize().ok_or(
        Error::script_tx_invariant_violation(ScriptTransaction::ArgsMustSerialize),
    )?;
    let args = {
        let mut result = Vec::with_capacity(function.param_tys().len());
        let mut given_args = script.args().iter();
        for ty in function.param_tys() {
            let ty = strip_reference(ty)?;
            let tag = session.get_type_tag(ty, code_storage)?;

            // Script arguments cannot encode signers so we implicitly
            // insert the known signer to all script parameters that take
            // a Signer type.
            if let TypeTag::Signer = tag {
                result.push(serialized_signer.clone());
                continue;
            }

            let arg = given_args
                .next()
                .ok_or(InvalidTransactionCause::MismatchedArgumentCount)?;
            let serialized_value = MoveValue::from(arg.clone()).simple_serialize().ok_or(
                Error::script_tx_invariant_violation(ScriptTransaction::ArgsMustSerialize),
            )?;
            result.push(serialized_value);
        }

        // All the args should have been used up.
        if given_args.next().is_some() {
            return Err(InvalidTransactionCause::MismatchedArgumentCount.into());
        }

        result
    };
    session.execute_script(
        script.code(),
        script.ty_args().to_vec(),
        args,
        gas_meter,
        traversal_context,
        code_storage,
    )?;
    Ok(())
}

pub(super) fn deploy_evm_contract<G: GasMeter, MS: ModuleStorage>(
    bytecode: Vec<u8>,
    value: U256,
    signer: AccountAddress,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &MS,
) -> umi_shared::error::Result<Address> {
    let module = ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into());
    let function_name = EVM_CREATE_FN_NAME;
    let args = vec![
        MoveValue::Signer(signer)
            .simple_serialize()
            .unwrap_or_default(),
        MoveValue::U256(value.to_move_u256())
            .simple_serialize()
            .unwrap_or_default(),
        MoveValue::vector_u8(bytecode)
            .simple_serialize()
            .unwrap_or_default(),
    ];
    let outcome = session
        .execute_function_bypass_visibility(
            &module,
            function_name,
            Vec::new(),
            args,
            gas_meter,
            traversal_context,
            module_storage,
        )
        .map_err(|e| User(UserError::Vm(e)))?;

    let evm_outcome = extract_evm_result(outcome)?;

    if !evm_outcome.is_success {
        return Err(User(UserError::EvmContractCreationFailure));
    }

    // Safety: this call does not panic because the EVM output
    // is set equal to the created address.
    let address = Address::from_slice(&evm_outcome.output);
    Ok(address)
}

// TODO(#329): group MoveVM elements (session, traversal_context,
// gas_meter, module_storage) together.
#[allow(clippy::too_many_arguments)]
pub(super) fn execute_evm_contract<G: GasMeter, MS: ModuleStorage>(
    signer: &AccountAddress,
    contract: &AccountAddress,
    value: U256,
    data: Vec<u8>,
    session: &mut Session,
    traversal_context: &mut TraversalContext,
    gas_meter: &mut G,
    module_storage: &MS,
) -> umi_shared::error::Result<EvmNativeOutcome> {
    let module = ModuleId::new(EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE.into());
    let function_name = EVM_CALL_FN_NAME;
    let args: Result<Vec<Vec<u8>>, Error> = [
        (Value::master_signer(*signer), &SIGNER_LAYOUT),
        (Value::address(*contract), &ADDRESS_LAYOUT),
        (Value::u256(value.to_move_u256()), &U256_LAYOUT),
        (Value::vector_u8(data), &CODE_LAYOUT),
    ]
    .into_iter()
    .map(|(value, layout)| {
        Ok(ValueSerDeContext::new()
            .serialize(&value, layout)?
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR)
                    .with_message("Failed to serialize EVM contract call args".into())
            })?)
    })
    .collect();
    let outcome = session
        .execute_function_bypass_visibility(
            &module,
            function_name,
            Vec::new(),
            args?,
            gas_meter,
            traversal_context,
            module_storage,
        )
        .map_err(|e| User(UserError::Vm(e)))?;

    let evm_outcome = extract_evm_result(outcome)?;

    Ok(evm_outcome)
}

// If `t` is wrapped in `Type::Reference` or `Type::MutableReference`,
// return the inner type
fn strip_reference(t: &Type) -> umi_shared::error::Result<&Type> {
    match t {
        Type::Reference(inner) | Type::MutableReference(inner) => {
            match inner.as_ref() {
                Type::Reference(_) | Type::MutableReference(_) => {
                    // References to references are not allowed and will not compile
                    // https://move-language.github.io/move/references.html#reference-operators
                    Err(InvalidTransactionCause::UnsupportedNestedReference)?
                }
                other => Ok(other),
            }
        }
        other => Ok(other),
    }
}

pub(super) fn deploy_module(
    bundle: ModuleBundle,
    address: AccountAddress,
    module_storage: &impl ModuleStorage,
) -> umi_shared::error::Result<ChangeSet> {
    let staged_module_storage =
        StagingModuleStorage::create(&address, module_storage, bundle.into_bytes())?;
    let bundle = staged_module_storage.release_verified_module_bundle();

    let mut writes = ChangeSet::new();
    for (module_id, bytes) in bundle.into_iter() {
        let addr = module_id.address();
        let name = module_id.name();

        let module_exists = module_storage.check_module_exists(addr, name)?;
        let op = if module_exists {
            Op::Modify(bytes)
        } else {
            Op::New(bytes)
        };
        writes
            .add_module_op(module_id, op)
            .expect("No duplicate module IDs in `VerifiedModuleBundle`");
    }

    Ok(writes)
}
