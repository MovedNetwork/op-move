use {
    super::{EvmNativeOutcome, EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE},
    crate::primitives::{ToEthAddress, ToMoveAddress, ToMoveU256, ToU256},
    alloy::hex::ToHexExt,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        account_address::AccountAddress, identifier::Identifier, language_storage::StructTag,
    },
    move_vm_runtime::session::SerializedReturnValues,
    move_vm_types::values::{Struct, VMValueCast, Value, Vector},
    revm::primitives::{
        utilities::KECCAK_EMPTY, AccountInfo, Address, ExecutionResult, Log, B256, U256,
    },
};

pub const ACCOUNT_INFO_PREFIX: &str = "Account_";

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

pub fn account_storage_struct_tag(address: &Address, index: &U256) -> StructTag {
    let name = format!("Storage_{}_{:x}", address.encode_hex(), index);
    let name = Identifier::new(name).expect("Account storage name is valid");
    StructTag {
        address: EVM_NATIVE_ADDRESS,
        module: EVM_NATIVE_MODULE.into(),
        name,
        type_args: Vec::new(),
    }
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

pub fn account_info_to_move_value(info: &AccountInfo, code_hash: B256) -> Value {
    let fields = [
        Value::u256(info.balance.to_move_u256()),
        Value::u64(info.nonce),
        Value::vector_u8(code_hash),
    ];
    Value::struct_(Struct::pack(fields))
}

pub fn move_value_to_account_info(value: Value) -> Result<AccountInfo, PartialVMError> {
    let s: Struct = value.cast()?;
    let mut fields = s.unpack()?;
    // Safety: Unwrap is safe because AccountInfo has 3 fields (see `account_info_to_move_value`)
    let balance: move_core_types::u256::U256 = fields.next().unwrap().cast()?;
    let nonce: u64 = fields.next().unwrap().cast()?;
    let code_hash: Vec<u8> = fields.next().unwrap().cast()?;
    let code_hash: B256 = B256::from_slice(&code_hash);
    Ok(AccountInfo {
        balance: balance.to_u256(),
        nonce,
        code_hash,
        code: None,
    })
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
    let fields = [
        Value::bool(result.is_success()),
        Value::vector_u8(result.output().cloned().unwrap_or_default()),
        // TODO: this method says it's for testing only, but it seems
        // to be the only way to make a Vector of Structs.
        Value::vector_for_testing_only(result.into_logs().into_iter().map(evm_log_to_move_value)),
    ];
    Value::struct_(Struct::pack(fields))
}

// Safety: This function has a TON of unwraps in it. It should only be called on
// results that actually came from calling the EVM native.
pub fn extract_evm_result(outcome: SerializedReturnValues) -> EvmNativeOutcome {
    let mut return_values = outcome
        .return_values
        .into_iter()
        .map(|(bytes, layout)| Value::simple_deserialize(&bytes, &layout).unwrap());

    let mut evm_result_fields = return_values
        .next()
        .unwrap()
        .value_as::<Struct>()
        .unwrap()
        .unpack()
        .unwrap();

    assert!(
        return_values.next().is_none(),
        "EVM native has only one return value."
    );

    let is_success: bool = evm_result_fields.next().unwrap().value_as().unwrap();
    let output: Vec<u8> = evm_result_fields.next().unwrap().value_as().unwrap();
    let logs: Vec<Value> = evm_result_fields.next().unwrap().value_as().unwrap();
    let logs = logs
        .into_iter()
        .map(|value| {
            let mut fields = value.value_as::<Struct>().unwrap().unpack().unwrap();

            let address = fields.next().unwrap().value_as::<AccountAddress>().unwrap();
            let topics = fields
                .next()
                .unwrap()
                .value_as::<Vector>()
                .unwrap()
                .unpack_unchecked()
                .unwrap();
            let data = fields.next().unwrap().value_as::<Vec<u8>>().unwrap();

            Log::new(
                address.to_eth_address(),
                topics
                    .into_iter()
                    .map(|value| {
                        value
                            .value_as::<move_core_types::u256::U256>()
                            .unwrap()
                            .to_le_bytes()
                            .into()
                    })
                    .collect(),
                data.into(),
            )
            .unwrap()
        })
        .collect();

    assert!(
        evm_result_fields.next().is_none(),
        "There are only 3 field in EVM return value."
    );

    EvmNativeOutcome {
        is_success,
        output,
        logs,
    }
}

#[test]
fn test_account_info_round_trip() {
    let bytecode = revm::primitives::Bytecode::new();
    let account_info = AccountInfo {
        balance: U256::from(1234),
        nonce: 7,
        code_hash: bytecode.hash_slow(),
        code: None,
    };
    let value = account_info_to_move_value(&account_info, account_info.code_hash);
    let info_rt = move_value_to_account_info(value).unwrap();
    assert_eq!(account_info, info_rt);
}
