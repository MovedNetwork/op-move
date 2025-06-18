use {
    super::{EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE},
    crate::native_evm_context::FRAMEWORK_ADDRESS,
    alloy::dyn_abi::{DynSolType, DynSolValue, Error},
    aptos_gas_algebra::{GasExpression, InternalGasUnit},
    aptos_gas_schedule::{
        NativeGasParameters,
        gas_params::natives::aptos_framework::{TYPE_INFO_TYPE_OF_BASE, UTIL_FROM_BYTES_PER_BYTE},
    },
    aptos_native_interface::{
        SafeNativeContext, SafeNativeError, SafeNativeResult, safely_pop_arg, safely_pop_type_arg,
    },
    aptos_types::vm_status::StatusCode,
    move_binary_format::errors::PartialVMError,
    move_core_types::{
        gas_algebra::NumBytes,
        ident_str,
        language_storage::{StructTag, TypeTag},
        value::{MoveStructLayout, MoveTypeLayout, MoveValue},
    },
    move_vm_types::{
        loaded_data::runtime_types::Type,
        values::{Struct, Value},
    },
    revm::primitives::U256,
    smallvec::{SmallVec, smallvec},
    std::{collections::VecDeque, sync::LazyLock},
    umi_shared::primitives::{ToEthAddress, ToMoveAddress, ToMoveU256, ToU256},
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

    // Charge for the lookup of the type (twice because we need to get annotated and not).
    context.charge(TYPE_INFO_TYPE_OF_BASE)?;
    context.charge(TYPE_INFO_TYPE_OF_BASE)?;

    let undecorated_layout = context.type_to_type_layout(&ty_arg)?;
    let annotated_layout = context.type_to_fully_annotated_layout(&ty_arg)?;

    // It's not possible to construct a `MoveValue` using the annotated layout
    // (the aptos code panics), so we use the undecorated layout to construct the value
    // and then pass in the annotated layout to make use of when converting to a
    // Solidity value.
    let mv = value.as_move_value(&undecorated_layout);

    // Charge gas for encoding. It is important to charge _before_ we do the work.
    context.charge(abi_encode_gas_cost(&mv)?)?;

    let encoding = inner_abi_encode_params(mv, &annotated_layout);

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

    // Charge for the lookup of the type.
    context.charge(TYPE_INFO_TYPE_OF_BASE)?;
    // Charge gas for decoding. It is important to charge _before_ we do the work.
    context.charge(abi_decode_gas_cost(&value))?;

    let annotated_layout = context.type_to_fully_annotated_layout(&ty_arg)?;
    let result = inner_abi_decode_params(&value, &annotated_layout)
        .map_err(|_| SafeNativeError::Abort { abort_code: 0 })?;
    Ok(smallvec![result])
}

fn abi_encode_gas_cost(
    mv: &MoveValue,
) -> SafeNativeResult<impl GasExpression<NativeGasParameters, Unit = InternalGasUnit>> {
    // Charge for encoding the value based on its bcs-serialized size.
    // We don't charge based on the ABI-encoded size because we must charge for gas
    // before we do the work.
    let size = bcs::serialized_size(&mv).map_err(|e| {
        PartialVMError::new(StatusCode::VALUE_SERIALIZATION_ERROR).with_message(format!(
            "failed to compute serialized size of a value: {e:?}"
        ))
    })?;
    // Assume constructing the ABI-encoding is a similar amount of work to
    // constructing a Move value from bcs-encoded bytes. Both involve recursively
    // traversing a structure.
    Ok(UTIL_FROM_BYTES_PER_BYTE * NumBytes::new(size as u64))
}

fn abi_decode_gas_cost(
    value: &[u8],
) -> impl GasExpression<NativeGasParameters, Unit = InternalGasUnit> {
    let size = value.len();
    // Assume ABI decoding is a similar cost to bcs decoding because
    // both involve constructing a Move value from bytes.
    UTIL_FROM_BYTES_PER_BYTE * NumBytes::new(size as u64)
}

/// Encode the move value into bytes using the Solidity ABI
/// such that it would be suitable for passing to a Solidity contract's function.
fn inner_abi_encode_params(mv: MoveValue, annotated_layout: &MoveTypeLayout) -> Vec<u8> {
    // A couple special cases that we can handle easily
    match &mv {
        MoveValue::U8(x) => {
            let mut result = vec![0; 32];
            result[31] = *x;
            return result;
        }
        MoveValue::U16(x) => {
            let mut result = vec![0; 32];
            let [a, b] = x.to_be_bytes();
            result[30] = a;
            result[31] = b;
            return result;
        }
        MoveValue::U32(x) => {
            let mut result = vec![0; 32];
            let [a, b, c, d] = x.to_be_bytes();
            result[28] = a;
            result[29] = b;
            result[30] = c;
            result[31] = d;
            return result;
        }
        MoveValue::Vector(inner) if inner.is_empty() => {
            let mut result = vec![0; 64];
            result[31] = 32;
            return result;
        }
        _ => (),
    }
    move_value_to_sol_value(mv, annotated_layout).abi_encode_params()
}

/// Decode the Solidity ABI bytes to native value
fn inner_abi_decode_params(value: &[u8], layout: &MoveTypeLayout) -> Result<Value, Error> {
    let sol_type = layout_to_sol_type(layout)?;
    let sol_value = sol_type.abi_decode_params(value)?;
    sol_to_value(sol_value, layout)
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
                let size_arg = type_args_to_usize(&struct_tag.type_args);
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

fn layout_to_sol_type(layout: &MoveTypeLayout) -> Result<DynSolType, Error> {
    let sol_type = match layout {
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
                return Ok(DynSolType::Bytes);
            }
            DynSolType::Array(Box::new(layout_to_sol_type(vector_layout)?))
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
                let bytes_size = type_args_to_usize(&struct_tag.type_args);
                return Ok(DynSolType::FixedBytes(bytes_size));
            }

            // Equality check like fixed bytes will not work because the type tags will not match.
            // Instead of equality, check specifically for module id and name to match.
            if FIXED_ARRAY_TAG.module_id() == struct_tag.module_id()
                && FIXED_ARRAY_TAG.name == struct_tag.name
            {
                // Fixed size vs dynamic array are defined e.g. as `uint[3]` vs `uint[]` in Solidity.
                // Move doesn't support fixed size vectors, hence we don't know the actual size beforehand.
                return Err(Error::SolTypes(alloy::sol_types::Error::custom(
                    "Fixed size arrays are not supported in move",
                )));
            }

            if STRING_TAG.eq(struct_tag) {
                return Ok(DynSolType::String);
            }

            // All other struct types are tuples in Solidity
            DynSolType::Tuple(
                field_layouts
                    .iter()
                    .map(|field_layout| layout_to_sol_type(&field_layout.layout))
                    .collect::<Result<Vec<DynSolType>, Error>>()?,
            )
        }
        MoveTypeLayout::Native(_, native_layout) => layout_to_sol_type(native_layout)?,
    };
    Ok(sol_type)
}

fn sol_to_value(sv: DynSolValue, layout: &MoveTypeLayout) -> Result<Value, Error> {
    let value = match (sv, layout) {
        (DynSolValue::Bool(b), MoveTypeLayout::Bool) => Value::bool(b),
        (DynSolValue::Uint(u, size), _) => match (size, layout) {
            (8, MoveTypeLayout::U8) => Value::u8(u.to_move_u256().unchecked_as_u8()),
            (16, MoveTypeLayout::U16) => Value::u16(u.to_move_u256().unchecked_as_u16()),
            (32, MoveTypeLayout::U32) => Value::u32(u.to_move_u256().unchecked_as_u32()),
            (64, MoveTypeLayout::U64) => Value::u64(u.to_move_u256().unchecked_as_u64()),
            (128, MoveTypeLayout::U128) => Value::u128(u.to_move_u256().unchecked_as_u128()),
            (256, MoveTypeLayout::U256) => Value::u256(u.to_move_u256()),
            _ => {
                return Err(Error::custom(
                    "Only 8, 16, 32, 64, 128 and 256 bit uints are supported by move",
                ));
            }
        },
        // Packing it back into type-erased `SolidityFixedBytes`
        (DynSolValue::FixedBytes(w, _size), MoveTypeLayout::Struct(_)) => {
            Value::struct_(Struct::pack(vec![Value::vector_u8(w.to_vec())]))
        }
        (DynSolValue::Address(a), MoveTypeLayout::Address) => Value::address(a.to_move_address()),
        (DynSolValue::Address(_), MoveTypeLayout::Signer) => {
            // The reason this is not allowed is because if it were then
            // signatures could be forged for any address.
            return Err(Error::custom("Decoding signer type not allowed"));
        }
        (DynSolValue::Bytes(b), MoveTypeLayout::Vector(inner_layout))
            if matches!(inner_layout.as_ref(), MoveTypeLayout::U8) =>
        {
            Value::vector_u8(b)
        }
        (DynSolValue::String(s), MoveTypeLayout::Struct(_)) => {
            Value::struct_(Struct::pack(vec![Value::vector_u8(s.into_bytes())]))
        }
        (DynSolValue::Array(a), MoveTypeLayout::Vector(inner_layout)) => {
            Value::vector_for_testing_only(
                a.into_iter()
                    .map(|sv| sol_to_value(sv, inner_layout))
                    .collect::<Result<Vec<_>, Error>>()?,
            )
        }
        (DynSolValue::Tuple(t), MoveTypeLayout::Struct(struct_layout)) => {
            let fields = match struct_layout {
                MoveStructLayout::Runtime(move_type_layouts) => move_type_layouts.clone(),
                MoveStructLayout::WithFields(move_field_layouts) => move_field_layouts
                    .iter()
                    .map(|f| f.layout.clone())
                    .collect(),
                MoveStructLayout::WithTypes { fields, .. } => {
                    fields.iter().map(|f| f.layout.clone()).collect()
                }
                MoveStructLayout::WithVariants(_) | MoveStructLayout::RuntimeVariants(_) => {
                    return Err(Error::custom("Variant-type layouts are unsupported"));
                }
            };
            Value::struct_(Struct::pack(
                t.into_iter()
                    .zip(fields)
                    .map(|(sv, layout)| sol_to_value(sv, &layout))
                    .collect::<Result<Vec<_>, Error>>()?,
            ))
        }
        _ => {
            return Err(Error::custom(
                "Unrecognized and/or mismatched types. Note: Int, FixedArray, Function and CustomStruct are not supported",
            ));
        }
    };
    Ok(value)
}

fn type_args_to_usize(type_args: &[TypeTag]) -> usize {
    // We treat it loosely so that even completely malformed type params
    // still return a reasonable fixed bytes size
    let u5_type_params = type_args
        .first()
        .and_then(|tag| tag.struct_tag())
        .map(|struct_tag| &struct_tag.type_args);

    let mut bytes_size: usize = 0b11111;

    if let Some(params) = u5_type_params {
        for (i, param) in params.iter().enumerate() {
            // If more type args than needed were passed, we only process the first 5
            if i >= 5 {
                break;
            }
            let bit_param_name = param
                .struct_tag()
                .map(|tag| tag.name.as_str())
                .unwrap_or_default();

            // By only doing something for B0 markers we default to a size of 32
            if bit_param_name == "B0" {
                // Clear the i-th bit at that position
                bytes_size ^= 1 << (4 - i);
            }
        }
    }

    // Add 1 to shift the return range to 1-32
    bytes_size + 1
}

fn force_to_u8(mv: MoveValue) -> u8 {
    if let MoveValue::U8(x) = mv {
        x
    } else {
        unreachable!("Only call force_to_u8 with MoveValue::U8")
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::primitives::Address,
        aptos_gas_schedule::{InitialGasSchedule, LATEST_GAS_FEATURE_VERSION},
        arbitrary::Unstructured,
        move_core_types::value::{MoveFieldLayout, MoveStruct},
        rand::{Rng, RngCore, rngs::ThreadRng, seq::SliceRandom},
        std::time::{Duration, Instant},
    };

    #[test]
    fn test_gas_costs() {
        let gas_params = NativeGasParameters::initial();
        let mut rng = rand::thread_rng();
        let values = construct_move_values(&mut rng);
        for mv in values {
            let (abi_encoding, layout) = bench_abi_encode(&gas_params, mv.clone(), &mut rng);
            match bench_abi_decode(&gas_params, &abi_encoding, layout.clone()) {
                Ok(round_trip_mv) => {
                    assert_eq!(round_trip_mv, mv);
                }
                Err(e) => {
                    assert!(format!("{e:?}").contains("signer type not allowed"));
                    assert!(layout_contains_signer(&layout));
                }
            }
        }
    }

    fn bench_abi_encode(
        gas_params: &NativeGasParameters,
        mv: MoveValue,
        rng: &mut ThreadRng,
    ) -> (Vec<u8>, MoveTypeLayout) {
        let annotated_layout = construct_annotated_layout(&mv, rng);
        let gas_cost: u64 = abi_encode_gas_cost(&mv)
            .ok()
            .unwrap()
            .evaluate(LATEST_GAS_FEATURE_VERSION, gas_params)
            .into();
        let message = format!("Took too long to encode value: {mv:#?}");
        let now = Instant::now();
        let abi_encoded = inner_abi_encode_params(mv, &annotated_layout);
        let duration = now.elapsed();

        assert_sufficient_gas(gas_cost, duration, message);

        (abi_encoded, annotated_layout)
    }

    fn bench_abi_decode(
        gas_params: &NativeGasParameters,
        value: &[u8],
        annotated_layout: MoveTypeLayout,
    ) -> Result<MoveValue, Error> {
        let gas_cost: u64 = abi_decode_gas_cost(value)
            .evaluate(LATEST_GAS_FEATURE_VERSION, gas_params)
            .into();

        let message = format!(
            "Took too long to decode value: {}",
            alloy::hex::encode(value)
        );
        let (value, duration) =
            median_duration(|| inner_abi_decode_params(value, &annotated_layout));

        assert_sufficient_gas(gas_cost, duration, message);

        let undecorated_layout = undecorate_layout(annotated_layout);
        value.map(|v| v.as_move_value(&undecorated_layout))
    }

    // Ensure enough gas was charged for the amount of computation done.
    fn assert_sufficient_gas(gas_cost: u64, duration: Duration, message: String) {
        let ns = duration.as_nanos();
        // 1 InternalGasUnit ~= 100 nanoseconds
        // (conversion from
        // https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.27.2/aptos-move/aptos-gas-schedule/src/gas_schedule/transaction.rs#L212)
        assert!(
            (gas_cost as u128) * 100 >= ns,
            "gas={gas_cost} time={ns} {message}"
        );
    }

    fn rand_bytes(rng: &mut ThreadRng, size: usize) -> Vec<u8> {
        let mut buf = vec![0; size];
        rng.fill_bytes(&mut buf);
        buf
    }

    fn median_duration<T, F: Fn() -> T>(f: F) -> (T, Duration) {
        const N_TRIALS: usize = 100;
        let mut durations = Vec::with_capacity(N_TRIALS);
        for _ in 0..N_TRIALS {
            let now = Instant::now();
            f();
            durations.push(now.elapsed());
        }
        durations.sort_unstable();
        (f(), durations[N_TRIALS / 2])
    }

    // We have this function instead of using the `Arbitrary` for `MoveValue`
    // to ensure we get a good spread of the different possible values.
    fn construct_move_values(rng: &mut ThreadRng) -> Vec<MoveValue> {
        // Include basic values
        let mut result = construct_basic_move_values(rng);
        let mut vector_values = vec![Vec::new(); result.len()];

        // Include empty vector
        result.push(MoveValue::Vector(Vec::new()));

        // Include a vector of each basic value
        for _ in 0..8 {
            for (acc, mv) in vector_values
                .iter_mut()
                .zip(construct_basic_move_values(rng))
            {
                acc.push(mv);
            }
        }
        for collection in &vector_values {
            result.push(MoveValue::Vector(collection.clone()));
        }

        // Include a nested vector of vectors for each basic value
        for collection in &vector_values {
            result.push(MoveValue::Vector(vec![
                MoveValue::Vector(collection.clone()),
                MoveValue::Vector(collection.clone()),
            ]));
        }

        // Include structs with different combinations of basic values
        for _ in 0..8 {
            result.push(construct_simple_move_struct(rng));
        }

        // Include structs with vector fields
        for _ in 0..8 {
            let mut fields = Vec::with_capacity(3);
            for _ in 0..fields.capacity() {
                fields.push(MoveValue::Vector(
                    vector_values.choose(rng).unwrap().clone(),
                ));
            }
            result.push(MoveValue::Struct(MoveStruct::Runtime(fields)));
        }

        // Include nested struct
        result.push(MoveValue::Struct(MoveStruct::Runtime(vec![
            construct_simple_move_struct(rng),
            construct_simple_move_struct(rng),
            construct_simple_move_struct(rng),
        ])));

        result
    }

    fn construct_simple_move_struct(rng: &mut ThreadRng) -> MoveValue {
        let mut fields = construct_basic_move_values(rng);
        fields.shuffle(rng);
        fields.truncate(rng.gen_range(3..7));
        MoveValue::Struct(MoveStruct::Runtime(fields))
    }

    fn construct_basic_move_values(rng: &mut ThreadRng) -> Vec<MoveValue> {
        vec![
            MoveValue::U8(rng.r#gen()),
            MoveValue::U16(rng.r#gen()),
            MoveValue::U32(rng.r#gen()),
            MoveValue::U64(rng.r#gen()),
            MoveValue::U128(rng.r#gen()),
            MoveValue::U256(rng.r#gen()),
            MoveValue::Bool(rng.r#gen()),
            MoveValue::Signer(Address::from_slice(&rand_bytes(rng, 20)).to_move_address()),
            MoveValue::Address(Address::from_slice(&rand_bytes(rng, 20)).to_move_address()),
        ]
    }

    fn construct_annotated_layout(mv: &MoveValue, rng: &mut ThreadRng) -> MoveTypeLayout {
        match mv {
            MoveValue::U8(_) => MoveTypeLayout::U8,
            MoveValue::U64(_) => MoveTypeLayout::U64,
            MoveValue::U128(_) => MoveTypeLayout::U128,
            MoveValue::Bool(_) => MoveTypeLayout::Bool,
            MoveValue::Address(_) => MoveTypeLayout::Address,
            MoveValue::Signer(_) => MoveTypeLayout::Signer,
            MoveValue::U16(_) => MoveTypeLayout::U16,
            MoveValue::U32(_) => MoveTypeLayout::U32,
            MoveValue::U256(_) => MoveTypeLayout::U256,
            MoveValue::Vector(move_values) => match move_values.first() {
                Some(mv) => MoveTypeLayout::Vector(Box::new(construct_annotated_layout(mv, rng))),
                None => {
                    let bytes = rand_bytes(rng, 8);
                    let mut unstructured = Unstructured::new(&bytes);
                    let inner: MoveTypeLayout = unstructured.arbitrary().unwrap();
                    MoveTypeLayout::Vector(Box::new(inner))
                }
            },
            MoveValue::Struct(move_struct) => {
                let (_, fields) = move_struct.clone().into_optional_variant_and_fields();
                let field_layouts = fields
                    .iter()
                    .map(|f| {
                        let layout = construct_annotated_layout(f, rng);
                        MoveFieldLayout::new(ident_str!("field_name").into(), layout)
                    })
                    .collect();
                let bytes = rand_bytes(rng, 1024);
                let mut unstructured = Unstructured::new(&bytes);
                let struct_tag: StructTag = unstructured.arbitrary().unwrap();
                MoveTypeLayout::Struct(MoveStructLayout::WithTypes {
                    type_: struct_tag,
                    fields: field_layouts,
                })
            }
        }
    }

    // Remove struct type information from the layout.
    fn undecorate_layout(layout: MoveTypeLayout) -> MoveTypeLayout {
        if let MoveTypeLayout::Struct(struct_layout) = layout {
            let fields = struct_layout.into_fields(None);
            return MoveTypeLayout::Struct(MoveStructLayout::Runtime(
                fields.into_iter().map(undecorate_layout).collect(),
            ));
        }

        layout
    }

    fn layout_contains_signer(layout: &MoveTypeLayout) -> bool {
        match layout {
            MoveTypeLayout::Signer => true,
            MoveTypeLayout::Vector(inner) => layout_contains_signer(inner),
            MoveTypeLayout::Struct(struct_layout) => match struct_layout {
                MoveStructLayout::Runtime(fields) => fields.iter().any(layout_contains_signer),
                MoveStructLayout::WithTypes { fields, .. }
                | MoveStructLayout::WithFields(fields) => {
                    fields.iter().any(|f| layout_contains_signer(&f.layout))
                }
                _ => panic!("Unexpected struct layout variant"),
            },
            _ => false,
        }
    }
}
