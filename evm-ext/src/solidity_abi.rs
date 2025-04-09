use {
    super::{EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE},
    crate::native_evm_context::FRAMEWORK_ADDRESS,
    alloy::dyn_abi::{DynSolType, DynSolValue, Error},
    aptos_native_interface::{
        SafeNativeContext, SafeNativeError, SafeNativeResult, safely_pop_arg, safely_pop_type_arg,
    },
    move_core_types::{
        ident_str,
        language_storage::{StructTag, TypeTag},
        value::{MoveStructLayout, MoveTypeLayout, MoveValue},
    },
    move_vm_types::{
        loaded_data::runtime_types::Type,
        values::{Struct, Value},
    },
    moved_shared::primitives::{ToEthAddress, ToMoveAddress, ToMoveU256, ToU256},
    revm::primitives::U256,
    smallvec::{SmallVec, smallvec},
    std::{collections::VecDeque, sync::LazyLock},
};

/// Marker struct defined in our framework for marking data as FixedBytes in the Solidity ABI.
static FIXED_BYTES_TAG: LazyLock<StructTag> = LazyLock::new(|| StructTag {
    address: EVM_NATIVE_ADDRESS,
    module: EVM_NATIVE_MODULE.into(),
    name: ident_str!("SolidityFixedBytes").into(),
    type_args: Vec::new(),
});

/// Marker struct defined in our framework for marking data as FixedArray in the Solidity ABI.
static FIXED_ARRAY_TAG: LazyLock<StructTag> = LazyLock::new(|| StructTag {
    address: EVM_NATIVE_ADDRESS,
    module: EVM_NATIVE_MODULE.into(),
    name: ident_str!("SolidityFixedArray").into(),
    type_args: Vec::new(),
});

/// Marker struct defined in move framework for the standard string.
static STRING_TAG: LazyLock<StructTag> = LazyLock::new(|| StructTag {
    address: FRAMEWORK_ADDRESS,
    module: ident_str!("string").into(),
    name: ident_str!("String").into(),
    type_args: Vec::new(),
});

/// Implementation for `native fun abi_encode_params<T>(prefix: vector<u8>, value: T): vector<u8>;`
///
/// Encode the move value into bytes using the Solidity ABI
/// such that it would be suitable for passing to a Solidity contract's function.
pub fn abi_encode_params(
    context: &mut SafeNativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    debug_assert_eq!(
        ty_args.len(),
        1,
        "abi_encode arg includes the type of the thing to encode"
    );
    debug_assert_eq!(
        args.len(),
        2,
        "abi_encode args: prefix bytes, arg to encode"
    );

    // Safety: unwrap is safe because of the length check above
    let value = args.pop_back().unwrap();
    let prefix = args.pop_back().unwrap().value_as::<Vec<u8>>()?;
    let ty_arg = safely_pop_type_arg!(ty_args);

    // TODO: need to figure out how much gas to charge for these operations.

    let undecorated_layout = context.type_to_type_layout(&ty_arg)?;
    let annotated_layout = context.type_to_fully_annotated_layout(&ty_arg)?;
    let encoding = inner_abi_encode_params(value, &undecorated_layout, &annotated_layout);

    let result = Value::vector_u8(prefix.into_iter().chain(encoding));
    Ok(smallvec![result])
}

/// Implementation for `native fun abi_decode_params<T>(value: vector<u8>): T;`
///
/// Decode the Solidity ABI bytes into move value
/// such that it would be suitable for using Solidity contract's return value.
pub fn abi_decode_params(
    context: &mut SafeNativeContext,
    mut ty_args: Vec<Type>,
    mut args: VecDeque<Value>,
) -> SafeNativeResult<SmallVec<[Value; 1]>> {
    debug_assert_eq!(ty_args.len(), 1, "abi_decode arg type to decode into");
    debug_assert_eq!(args.len(), 1, "abi_decode arg to decode");

    let value = safely_pop_arg!(args, Vec<u8>);
    let ty_arg = safely_pop_type_arg!(ty_args);

    // TODO: need to figure out how much gas to charge for these operations.

    let annotated_layout = context.type_to_fully_annotated_layout(&ty_arg)?;
    let result = inner_abi_decode_params(&value, &annotated_layout)
        .map_err(|_| SafeNativeError::Abort { abort_code: 0 })?;
    Ok(smallvec![result])
}

/// Encode the move value into bytes using the Solidity ABI
/// such that it would be suitable for passing to a Solidity contract's function.
fn inner_abi_encode_params(
    value: Value,
    undecorated_layout: &MoveTypeLayout,
    annotated_layout: &MoveTypeLayout,
) -> Vec<u8> {
    // It's not possible to construct a `MoveValue` using the annotated layout
    // (the aptos code panics), so we use the undecorated layout to construct the value
    // and then pass in the annotated layout to make use of when converting to a
    // Solidity value.
    let mv = value.as_move_value(undecorated_layout);
    move_value_to_sol_value(mv, annotated_layout).abi_encode_params()
}

/// Decode the Solidity ABI bytes to native value
fn inner_abi_decode_params(value: &[u8], layout: &MoveTypeLayout) -> Result<Value, Error> {
    let sol_type = layout_to_sol_type(layout);
    let sol_value = sol_type.abi_decode_params(value)?;
    Ok(sol_to_value(sol_value))
}

fn move_value_to_sol_value(mv: MoveValue, annotated_layout: &MoveTypeLayout) -> DynSolValue {
    match mv {
        MoveValue::Signer(move_address) | MoveValue::Address(move_address) => {
            let evm_address = move_address.to_eth_address();
            DynSolValue::Address(evm_address)
        }
        MoveValue::Bool(b) => DynSolValue::Bool(b),
        MoveValue::U8(x) => DynSolValue::Uint(U256::from(x), 8),
        MoveValue::U16(x) => DynSolValue::Uint(U256::from(x), 16),
        MoveValue::U32(x) => DynSolValue::Uint(U256::from(x), 32),
        MoveValue::U64(x) => DynSolValue::Uint(U256::from(x), 64),
        MoveValue::U128(x) => DynSolValue::Uint(U256::from(x), 128),
        MoveValue::U256(x) => DynSolValue::Uint(x.to_u256(), 256),
        MoveValue::Vector(xs) => {
            // Special case for byte arrays
            if let Some(MoveValue::U8(_)) = xs.first() {
                return DynSolValue::Bytes(xs.into_iter().map(force_to_u8).collect());
            }

            let MoveTypeLayout::Vector(inner_layout) = annotated_layout else {
                unreachable!("The annotated layout must match the MoveValue")
            };

            DynSolValue::Array(
                xs.into_iter()
                    .map(|x| move_value_to_sol_value(x, inner_layout))
                    .collect(),
            )
        }
        MoveValue::Struct(inner) => {
            let MoveTypeLayout::Struct(struct_layout) = annotated_layout else {
                unreachable!("The annotated layout must match the MoveValue")
            };
            let (struct_tag, field_layouts) = match struct_layout {
                MoveStructLayout::WithTypes { type_, fields } => (type_, fields),
                _ => unreachable!(
                    "Must have type because layout is constructed with `type_to_fully_annotated_layout`"
                ),
            };
            let mut fields = inner.into_optional_variant_and_fields().1;

            // Special case data marked as being fixed bytes
            if FIXED_BYTES_TAG.module_id() == struct_tag.module_id()
                && FIXED_BYTES_TAG.name == struct_tag.name
            {
                let Some(MoveValue::Vector(data)) = fields.pop() else {
                    unreachable!("SolidityFixedBytes contains a vector")
                };

                // Solidity only supports fixed bytes up to 32 by storing a whole word and slicing
                // into it.
                debug_assert!(
                    data.len() == 32,
                    "Solidity pads fixed bytes length to 32. This condition is enforced by the constructor in the Evm.move module"
                );

                // Fill the fixed-sized buffer from the beginning with the given bytes
                let mut buf = [0u8; 32];
                for (b, x) in buf.iter_mut().zip(data.into_iter().map(force_to_u8)) {
                    *b = x;
                }
                let size_arg = type_arg_to_usize(&struct_tag.type_args);
                return DynSolValue::FixedBytes(buf.into(), size_arg);
            }

            // Special case data marked as being fixed-sized array.
            // We intentionally do not compare the type args because they
            // are only known at runtime.
            if FIXED_ARRAY_TAG.module_id() == struct_tag.module_id()
                && FIXED_ARRAY_TAG.name == struct_tag.name
            {
                let Some(MoveValue::Vector(data)) = fields.pop() else {
                    unreachable!("SolidityFixedArray contains a vector")
                };
                let MoveTypeLayout::Vector(inner_layout) = &field_layouts[0].layout else {
                    unreachable!("The annotated layout must match the MoveValue")
                };
                let data = data
                    .into_iter()
                    .map(|x| move_value_to_sol_value(x, inner_layout))
                    .collect();
                return DynSolValue::FixedArray(data);
            }

            // Assume all other structs are meant to be tuples.
            DynSolValue::Tuple(
                fields
                    .into_iter()
                    .zip(field_layouts)
                    .map(|(x, l)| move_value_to_sol_value(x, &l.layout))
                    .collect(),
            )
        }
    }
}

fn layout_to_sol_type(layout: &MoveTypeLayout) -> DynSolType {
    match layout {
        MoveTypeLayout::Bool => DynSolType::Bool,
        MoveTypeLayout::U8 => DynSolType::Uint(8),
        MoveTypeLayout::U16 => DynSolType::Uint(16),
        MoveTypeLayout::U32 => DynSolType::Uint(32),
        MoveTypeLayout::U64 => DynSolType::Uint(64),
        MoveTypeLayout::U128 => DynSolType::Uint(128),
        MoveTypeLayout::U256 => DynSolType::Uint(256),
        MoveTypeLayout::Signer | MoveTypeLayout::Address => DynSolType::Address,
        MoveTypeLayout::Vector(vector_layout) => {
            // Special case for byte arrays
            if **vector_layout == MoveTypeLayout::U8 {
                return DynSolType::Bytes;
            }
            DynSolType::Array(Box::new(layout_to_sol_type(vector_layout)))
        }
        MoveTypeLayout::Struct(struct_layout) => {
            let (struct_tag, field_layouts) = match struct_layout {
                MoveStructLayout::WithTypes { type_, fields } => (type_, fields),
                _ => unreachable!(
                    "Must have type because layout is constructed with `type_to_fully_annotated_layout`"
                ),
            };

            // Special case data marked as being fixed bytes
            if FIXED_BYTES_TAG.module_id() == struct_tag.module_id()
                && FIXED_BYTES_TAG.name == struct_tag.name
            {
                // The generic argument encodes actual size at the type level
                let bytes_size = type_arg_to_usize(&struct_tag.type_args);
                return DynSolType::FixedBytes(bytes_size);
            }

            // Equality check like fixed bytes will not work because the type tags will not match.
            // Instead of equality, check specifically for module id and name to match.
            if FIXED_ARRAY_TAG.module_id() == struct_tag.module_id()
                && FIXED_ARRAY_TAG.name == struct_tag.name
            {
                // Fixed size vs dynamic array are defined e.g. as `uint[3]` vs `uint[]` in Solidity.
                // Move doesn't support fixed size vectors, hence we don't know the actual size beforehand.
                unimplemented!("Fixed size arrays are not supported in move");
            }

            if STRING_TAG.eq(struct_tag) {
                return DynSolType::String;
            }

            // All other struct types are tuples in Solidity
            DynSolType::Tuple(
                field_layouts
                    .iter()
                    .map(|field_layout| layout_to_sol_type(&field_layout.layout))
                    .collect(),
            )
        }
        MoveTypeLayout::Native(_, native_layout) => layout_to_sol_type(native_layout),
    }
}

fn sol_to_value(sv: DynSolValue) -> Value {
    match sv {
        DynSolValue::Bool(b) => Value::bool(b),
        DynSolValue::Uint(u, size) => match size {
            8 => Value::u8(u.to_move_u256().unchecked_as_u8()),
            16 => Value::u16(u.to_move_u256().unchecked_as_u16()),
            32 => Value::u32(u.to_move_u256().unchecked_as_u32()),
            64 => Value::u64(u.to_move_u256().unchecked_as_u64()),
            128 => Value::u128(u.to_move_u256().unchecked_as_u128()),
            256 => Value::u256(u.to_move_u256()),
            _ => unreachable!("Only 8, 16, 32, 64, 128 and 256 bit uints are supported by move"),
        },
        // Packing it back into type-erased `SolidityFixedBytes`
        // TODO: any way to pass back the size type parameter?
        DynSolValue::FixedBytes(w, _size) => {
            Value::struct_(Struct::pack(vec![Value::vector_u8(w.to_vec())]))
        }
        DynSolValue::Address(a) => Value::address(a.to_move_address()),
        DynSolValue::Bytes(b) => Value::vector_u8(b),
        DynSolValue::String(s) => {
            Value::struct_(Struct::pack(vec![Value::vector_u8(s.into_bytes())]))
        }
        DynSolValue::Array(a) => Value::vector_for_testing_only(
            // TODO: Make sure all the vector elements are of the same type
            a.into_iter().map(sol_to_value).collect::<Vec<_>>(),
        ),
        DynSolValue::Tuple(t) => Value::struct_(Struct::pack(
            t.into_iter().map(sol_to_value).collect::<Vec<_>>(),
        )),
        _ => unreachable!("Int, FixedArray, Function and CustomStruct are not supported"),
    }
}

fn type_arg_to_usize(type_args: &[TypeTag]) -> usize {
    let type_arg = type_args
        .iter()
        .next()
        .expect("Type args should be at least 1 in length");
    let type_name = type_arg
        .struct_tag()
        .expect("Should only be called for struct-based tags")
        .name
        .as_str();

    match type_name {
        "B1" => 1,
        "B2" => 2,
        "B4" => 4,
        "B8" => 8,
        "B16" => 16,
        "B20" => 20,
        "B32" => 32,
        _ => unreachable!("Other sizes are not available and should cause a failure at Move level"),
    }
}

fn force_to_u8(mv: MoveValue) -> u8 {
    if let MoveValue::U8(x) = mv {
        x
    } else {
        unreachable!("Only call force_to_u8 with MoveValue::U8")
    }
}
