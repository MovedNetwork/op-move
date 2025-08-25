use {
    super::{EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE, EvmNativeOutcome},
    crate::FRAMEWORK_ADDRESS,
    alloy::hex::ToHexExt,
    aptos_types::vm_status::StatusCode,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        identifier::{IdentStr, Identifier},
        language_storage::StructTag,
        value::{MoveStructLayout, MoveTypeLayout},
    },
    move_vm_runtime::session::SerializedReturnValues,
    move_vm_types::{
        resolver::MoveResolver,
        value_serde::ValueSerDeContext,
        values::{Struct, Value, Vector},
    },
    revm::{
        context::result::ExecutionResult,
        primitives::{Address, B256, KECCAK_EMPTY, Log},
        state::AccountInfo,
    },
    std::sync::LazyLock,
    umi_shared::primitives::{ToEthAddress, ToMoveAddress, ToMoveU256},
};

pub const ACCOUNT_MODULE_NAME: &IdentStr = ident_str!("account");
pub const ACCOUNT_RESOURCE_NAME: &IdentStr = ident_str!("Account");
pub const ACCOUNT_INFO_PREFIX: &str = "Account_";

static ACCOUNT_STRUCT_TAG: LazyLock<StructTag> = LazyLock::new(|| StructTag {
    address: FRAMEWORK_ADDRESS,
    module: ACCOUNT_MODULE_NAME.into(),
    name: ACCOUNT_RESOURCE_NAME.into(),
    type_args: Vec::new(),
});
static ACCOUNT_LAYOUT: LazyLock<MoveTypeLayout> = LazyLock::new(|| {
    let id_layout = MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
        MoveTypeLayout::U64,
        MoveTypeLayout::Address,
    ]));

    let guid_layout = MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![id_layout]));

    let event_handle_layout = MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
        MoveTypeLayout::U64,
        guid_layout,
    ]));

    let capability_offer_layout =
        MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![MoveTypeLayout::Struct(
            MoveStructLayout::Runtime(vec![MoveTypeLayout::Vector(Box::new(
                MoveTypeLayout::Address,
            ))]),
        )]));

    MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
        crate::CODE_LAYOUT.clone(),
        MoveTypeLayout::U64,
        MoveTypeLayout::U64,
        event_handle_layout.clone(),
        event_handle_layout,
        capability_offer_layout.clone(),
        capability_offer_layout,
    ]))
});

pub fn account_info_struct_tag(address: &Address) -> StructTag {
    let name = format!("{ACCOUNT_INFO_PREFIX}{}", address.encode_hex());
    let name = Identifier::new(name).expect("Account info name is valid");
    StructTag {
        address: EVM_NATIVE_ADDRESS,
        module: EVM_NATIVE_MODULE.into(),
        name,
        type_args: Vec::new(),
    }
}

pub fn code_hash_struct_tag(code_hash: &B256) -> StructTag {
    let name = format!("CodeHash_{}", code_hash.encode_hex());
    let name = Identifier::new(name).expect("Code hash name is valid");
    StructTag {
        address: EVM_NATIVE_ADDRESS,
        module: EVM_NATIVE_MODULE.into(),
        name,
        type_args: Vec::new(),
    }
}

pub fn get_move_account_nonce(
    address: &AccountAddress,
    resolver: &dyn MoveResolver,
) -> Option<u64> {
    let tag: &StructTag = &ACCOUNT_STRUCT_TAG;
    let metadata = resolver.get_module_metadata(&tag.module_id());
    let bytes = resolver
        .get_resource_bytes_with_metadata_and_layout(address, tag, &metadata, Some(&ACCOUNT_LAYOUT))
        .ok()?
        .0?;

    let value = ValueSerDeContext::new()
        .deserialize(&bytes, &ACCOUNT_LAYOUT)
        .expect("Account resource layout is known");

    let mut fields = value
        .value_as::<Struct>()
        .expect("Account resource is a struct")
        .unpack()
        .expect("Can get account fields");

    let nonce: u64 = fields
        .nth(1)
        .expect("Account has at least two fields")
        .value_as()
        .expect("Second field is u64");

    Some(nonce)
}

pub fn get_account_code_hash(info: &AccountInfo) -> B256 {
    if let Some(code) = &info.code {
        if code.is_empty() {
            KECCAK_EMPTY
        } else {
            code.hash_slow()
        }
    } else if info.code_hash.is_zero() {
        KECCAK_EMPTY
    } else {
        info.code_hash
    }
}

pub fn evm_log_to_move_value(log: Log) -> Value {
    let fields = [
        Value::address(log.address.to_move_address()),
        Value::vector_u256(log.data.topics().iter().map(|x| x.to_move_u256())),
        Value::vector_u8(log.data.data),
    ];
    Value::struct_(Struct::pack(fields))
}

pub fn evm_result_to_move_value(result: ExecutionResult) -> Value {
    // In the case of create, set the output equal to the address of
    // the newly deployed contract (for convenience).
    let output = result.created_address().map_or_else(
        || {
            result
                .output()
                .map(|bytes| bytes.to_vec())
                .unwrap_or_default()
        },
        |address| address.to_vec(),
    );
    let fields = [
        Value::bool(result.is_success()),
        Value::vector_u8(output),
        // TODO: this method says it's for testing only, but it seems
        // to be the only way to make a Vector of Structs.
        Value::vector_for_testing_only(result.into_logs().into_iter().map(evm_log_to_move_value)),
    ];
    Value::struct_(Struct::pack(fields))
}

pub fn extract_evm_result(
    outcome: SerializedReturnValues,
) -> Result<EvmNativeOutcome, PartialVMError> {
    let malformed = || {
        PartialVMError::new(StatusCode::ABORT_TYPE_MISMATCH_ERROR)
            .with_message("Malformed EVM native return value".into())
    };

    let mut return_values = outcome.return_values.into_iter().map(|(bytes, layout)| {
        ValueSerDeContext::new()
            .deserialize(&bytes, &layout)
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::ABORT_TYPE_MISMATCH_ERROR)
                    .with_message("Invalid bytes+layout combination given for EVM native".into())
            })
    });

    let mut evm_result_fields = return_values
        .next()
        .ok_or_else(malformed)??
        .value_as::<Struct>()?
        .unpack()?;

    if return_values.next().is_some() {
        return Err(PartialVMError::new(StatusCode::ABORT_TYPE_MISMATCH_ERROR)
            .with_message("EVM native has only one return value.".into()));
    }

    let is_success: bool = evm_result_fields.next().ok_or_else(malformed)?.value_as()?;
    let output: Vec<u8> = evm_result_fields.next().ok_or_else(malformed)?.value_as()?;
    let logs: Vec<Value> = evm_result_fields.next().ok_or_else(malformed)?.value_as()?;
    let logs = logs
        .into_iter()
        .map(|value| {
            let mut fields = value.value_as::<Struct>()?.unpack()?;

            let address = fields
                .next()
                .ok_or_else(malformed)?
                .value_as::<AccountAddress>()?;
            let topics = fields
                .next()
                .ok_or_else(malformed)?
                .value_as::<Vector>()?
                .unpack_unchecked()?;
            let data = fields.next().ok_or_else(malformed)?.value_as::<Vec<u8>>()?;

            let log = Log::new(
                address.to_eth_address(),
                topics
                    .into_iter()
                    .map(|value| {
                        let topic = value
                            .value_as::<move_core_types::u256::U256>()?
                            .to_le_bytes()
                            .into();
                        Ok(topic)
                    })
                    .collect::<Result<Vec<B256>, PartialVMError>>()?,
                data.into(),
            )
            .ok_or_else(|| {
                PartialVMError::new(StatusCode::ABORT_TYPE_MISMATCH_ERROR)
                    .with_message("Greater than 4 topics in EVM return value".into())
            })?;
            Ok(log)
        })
        .collect::<Result<Vec<Log>, PartialVMError>>()?;

    if evm_result_fields.next().is_some() {
        return Err(PartialVMError::new(StatusCode::ABORT_TYPE_MISMATCH_ERROR)
            .with_message("There are only 3 field in EVM return value.".into()));
    }

    Ok(EvmNativeOutcome {
        is_success,
        output,
        logs,
    })
}
