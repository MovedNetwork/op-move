use {
    core::str,
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        identifier::IdentStr,
        language_storage::{StructTag, TypeTag},
        value::{MoveStruct, MoveValue},
    },
    move_vm_runtime::session::Session,
    move_vm_types::loaded_data::runtime_types::Type,
    moved_shared::error::{EntryFunctionValue, Error, InvalidTransactionCause},
};

const ALLOWED_STRUCTS: [MoveStructInfo<'static>; 5] = [
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("string"),
        name: ident_str!("String"),
    },
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("object"),
        name: ident_str!("Object"),
    },
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("option"),
        name: ident_str!("Option"),
    },
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("fixed_point32"),
        name: ident_str!("FixedPoint32"),
    },
    MoveStructInfo {
        address: AccountAddress::ONE,
        module: ident_str!("fixed_point64"),
        name: ident_str!("FixedPoint64"),
    },
];

/// Only certain types are allowed to be used as arguments for entry functions.
/// This function ensures only allowed types are present.
/// The reason for this restriction is to respect the invariants that Move contracts
/// may have around their types. For example token types and capability types should
/// only be created under specific conditions, not directly deserialized from raw bytes.
pub fn validate_entry_type_tag(tag: &TypeTag) -> moved_shared::error::Result<()> {
    let mut current_tag = tag;
    loop {
        match current_tag {
            TypeTag::Vector(inner) => {
                // Vectors are allowed iff they hold other allowed types,
                // so we must check the inner type.
                current_tag = inner;
            }
            TypeTag::Struct(struct_tag) => {
                let info = MoveStructInfo::from_tag(struct_tag);

                if !ALLOWED_STRUCTS.contains(&info) {
                    return Err(Error::InvalidTransaction(
                        InvalidTransactionCause::DisallowedEntryFunctionType(tag.clone()),
                    ));
                }

                match struct_tag.type_args.as_slice() {
                    [] => {
                        // No type parameters => nothing more to check
                        return Ok(());
                    }
                    [inner_type] => {
                        if info.name == ALLOWED_STRUCTS[2].name {
                            // Option<T> is allowed iff T is allowed.
                            current_tag = inner_type;
                        } else {
                            // For Object<T> we do not need to validate the inner type
                            // because no value for that type is actually created (it's phantom).
                            // No other allowed types have type parameters.
                            return Ok(());
                        }
                    }
                    _ => unreachable!("No allowed type has more than 1 type parameter"),
                }
            }
            TypeTag::Address
            | TypeTag::Bool
            | TypeTag::Signer
            | TypeTag::U128
            | TypeTag::U16
            | TypeTag::U256
            | TypeTag::U32
            | TypeTag::U64
            | TypeTag::U8 => {
                // Primitive types are allowed.
                // Note that signer is an allowed type because
                // the value is checked in `crate::signer`.
                return Ok(());
            }
        }
    }
}

/// Some allowed types have restrictions on what values those types can take.
/// This function ensures the values are properly constructed for the types they
/// are meant to be.
pub fn validate_entry_value(
    tag: &TypeTag,
    value: &MoveValue,
    expected_signer: &AccountAddress,
    session: &mut Session,
) -> moved_shared::error::Result<()> {
    let mut stack = Vec::with_capacity(10);
    stack.push((tag, value));

    while let Some(tagged_value) = stack.pop() {
        match tagged_value {
            (TypeTag::Signer, MoveValue::Signer(given_signer)) => {
                // Signer must be the expected signer (you cannot forge the
                // signature of another address).
                if given_signer != expected_signer {
                    Err(InvalidTransactionCause::InvalidSigner)?
                }
            }
            (TypeTag::Vector(inner_tag), MoveValue::Vector(array)) => {
                // Each element of a vector must follow the invariant of
                // the type that vector holds.
                for elem in array {
                    stack.push((inner_tag, elem));
                }
            }
            (TypeTag::Struct(struct_tag), MoveValue::Struct(struct_value)) => {
                let info = MoveStructInfo::from_tag(struct_tag);

                if info.name == ALLOWED_STRUCTS[0].name {
                    validate_string(struct_value)?;
                    continue;
                } else if info.name == ALLOWED_STRUCTS[1].name {
                    let inner_type =
                        struct_tag
                            .type_args
                            .first()
                            .ok_or(Error::entry_fn_invariant_violation(
                                EntryFunctionValue::ObjectStructHasTypeParameter,
                            ))?;
                    validate_object(struct_value, inner_type, session)?;
                    // We don't need to push the inner type on the stack for validation
                    // because a value of it is not actually constructed.
                    continue;
                } else if info.name == ALLOWED_STRUCTS[2].name {
                    if let Some(inner_value) = validate_option(struct_value)? {
                        let inner_tag = struct_tag.type_args.first().ok_or(
                            Error::entry_fn_invariant_violation(
                                EntryFunctionValue::OptionHasInnerType,
                            ),
                        )?;
                        stack.push((inner_tag, inner_value));
                    };
                    continue;
                } else if info.name == ALLOWED_STRUCTS[3].name
                    || info.name == ALLOWED_STRUCTS[4].name
                {
                    // FixedPoint data structures have no invariants to check.
                    continue;
                }

                // No other structs are allowed per `validate_entry_type_tag`
                return Err(Error::entry_fn_invariant_violation(
                    EntryFunctionValue::OnlyAllowedStructs,
                ));
            }
            // No other types have invariants
            _ => (),
        }
    }

    Ok(())
}

/// String must be utf-8 encoded bytes.
fn validate_string(value: &MoveStruct) -> moved_shared::error::Result<()> {
    let inner = value
        .fields()
        .first()
        .ok_or(Error::entry_fn_invariant_violation(
            EntryFunctionValue::StringStructHasField,
        ))?;

    match inner {
        MoveValue::Vector(array) => {
            let mut bytes = Vec::with_capacity(array.len());
            for elem in array {
                match elem {
                    MoveValue::U8(b) => {
                        bytes.push(*b);
                    }
                    _ => {
                        return Err(Error::entry_fn_invariant_violation(
                            EntryFunctionValue::StringElementsAreU8,
                        ));
                    }
                }
            }
            if String::from_utf8(bytes).is_err() {
                Err(InvalidTransactionCause::InvalidString)?;
            }
        }
        _ => {
            return Err(Error::entry_fn_invariant_violation(
                EntryFunctionValue::StringStructFieldIsVector,
            ));
        }
    }

    Ok(())
}

/// Option must be a vector with 0 or 1 values
fn validate_option(value: &MoveStruct) -> moved_shared::error::Result<Option<&MoveValue>> {
    let inner = value
        .fields()
        .first()
        .ok_or(Error::entry_fn_invariant_violation(
            EntryFunctionValue::OptionStructHasField,
        ))?;

    match inner {
        MoveValue::Vector(array) => {
            match array.as_slice() {
                [] => Ok(None),
                [inner] => Ok(Some(inner)),
                _ => {
                    // If there is more than 1 element then it's invalid.
                    Err(InvalidTransactionCause::InvalidOption.into())
                }
            }
        }
        _ => Err(Error::entry_fn_invariant_violation(
            EntryFunctionValue::OptionStructFieldIsVector,
        )),
    }
}

// Based on
// https://github.com/aptos-labs/aptos-core/blob/aptos-node-v1.14.0/aptos-move/framework/aptos-framework/sources/object.move#L192
fn validate_object(
    value: &MoveStruct,
    inner_type: &TypeTag,
    session: &mut Session,
) -> moved_shared::error::Result<()> {
    let inner = value
        .fields()
        .first()
        .ok_or(Error::entry_fn_invariant_violation(
            EntryFunctionValue::ObjectStructHasField,
        ))?;

    match inner {
        MoveValue::Address(addr) => {
            let object_core = get_object_core_type(session)?;
            if !resource_exists(session, *addr, &object_core) {
                return Err(InvalidTransactionCause::InvalidObject.into());
            }

            let inner_type = session.load_type(inner_type).map_err(|_| {
                Error::entry_fn_invariant_violation(EntryFunctionValue::ObjectInnerTypeExists)
            })?;
            if !resource_exists(session, *addr, &inner_type) {
                return Err(InvalidTransactionCause::InvalidObject.into());
            }

            Ok(())
        }
        _ => Err(Error::entry_fn_invariant_violation(
            EntryFunctionValue::ObjectStructFieldIsAddress,
        )),
    }
}

#[inline]
fn resource_exists(session: &mut Session, addr: AccountAddress, ty: &Type) -> bool {
    session
        .load_resource(addr, ty)
        .and_then(|(value, _)| value.exists())
        .unwrap_or(false)
}

#[inline]
fn get_object_core_type(session: &mut Session) -> moved_shared::error::Result<Type> {
    let type_tag = TypeTag::Struct(Box::new(StructTag {
        address: ALLOWED_STRUCTS[1].address,
        module: ALLOWED_STRUCTS[1].module.into(),
        name: ident_str!("ObjectCore").into(),
        type_args: Vec::new(),
    }));
    session
        .load_type(&type_tag)
        .map_err(|_| Error::entry_fn_invariant_violation(EntryFunctionValue::ObjectCoreTypeExists))
}

#[derive(Debug, PartialEq, Eq, Clone, Copy)]
struct MoveStructInfo<'a> {
    pub address: AccountAddress,
    pub module: &'a IdentStr,
    pub name: &'a IdentStr,
}

impl<'a> MoveStructInfo<'a> {
    pub fn from_tag(tag: &'a StructTag) -> Self {
        Self {
            address: tag.address,
            module: &tag.module,
            name: &tag.name,
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        crate::{create_move_vm, create_vm_session, session_id::SessionId, tests::EVM_ADDRESS},
        alloy::primitives::address,
        move_core_types::value::MoveStruct,
        moved_evm_ext::state::InMemoryStorageTrieRepository,
        moved_shared::primitives::ToMoveAddress,
        moved_state::{InMemoryState, State},
    };

    #[test]
    fn test_validate_entry_type_tag() {
        struct TestCase {
            typ: TypeTag,
            is_allowed: bool,
        }

        let disallowed_struct = StructTag {
            address: AccountAddress::TWO,
            module: "thing".parse().unwrap(),
            name: "Thing".parse().unwrap(),
            type_args: Vec::new(),
        };

        let disallowed_recursive_struct = StructTag {
            address: AccountAddress::ONE,
            module: "option".parse().unwrap(),
            name: "Option".parse().unwrap(),
            type_args: vec![TypeTag::Struct(Box::new(disallowed_struct.clone()))],
        };

        let allowed_struct = StructTag {
            address: AccountAddress::ONE,
            module: "object".parse().unwrap(),
            name: "Object".parse().unwrap(),
            type_args: Vec::new(),
        };

        let allowed_recursive_struct = StructTag {
            address: AccountAddress::ONE,
            module: "option".parse().unwrap(),
            name: "Option".parse().unwrap(),
            type_args: vec![TypeTag::Struct(Box::new(allowed_struct.clone()))],
        };

        let test_cases = [
            // Primitive types are allowed
            TestCase {
                typ: TypeTag::Address,
                is_allowed: true,
            },
            TestCase {
                typ: TypeTag::Signer,
                is_allowed: true,
            },
            TestCase {
                typ: TypeTag::U64,
                is_allowed: true,
            },
            // Allowed struct
            TestCase {
                typ: TypeTag::Struct(Box::new(allowed_struct)),
                is_allowed: true,
            },
            // Disallowed struct
            TestCase {
                typ: TypeTag::Struct(Box::new(disallowed_struct)),
                is_allowed: false,
            },
            // Allowed recursive struct
            TestCase {
                typ: TypeTag::Struct(Box::new(allowed_recursive_struct)),
                is_allowed: true,
            },
            // Disallowed recursive struct
            TestCase {
                typ: TypeTag::Struct(Box::new(disallowed_recursive_struct)),
                is_allowed: false,
            },
        ];

        for t in test_cases {
            let result = validate_entry_type_tag(&t.typ);
            if t.is_allowed {
                result.unwrap_or_else(|_| panic!("{:?} should be allowed", t.typ));
            } else {
                result.expect_err(&format!("{:?} should NOT be allowed", t.typ));
            }
        }
    }

    #[test]
    fn test_check_signer() {
        let correct_signer = EVM_ADDRESS.to_move_address();
        let incorrect_signer =
            address!("c104f4840573bed437190daf5d2898c2bdf928ac").to_move_address();
        type CheckSignerOutcome = Result<(), ()>;

        let test_cases: &[(TypeTag, MoveValue, CheckSignerOutcome)] = &[
            (
                TypeTag::Address,
                MoveValue::Address(incorrect_signer),
                Ok(()),
            ),
            (TypeTag::Address, MoveValue::Address(correct_signer), Ok(())),
            (
                TypeTag::Signer,
                MoveValue::Signer(incorrect_signer),
                Err(()),
            ),
            (TypeTag::Signer, MoveValue::Signer(correct_signer), Ok(())),
            (
                TypeTag::Vector(Box::new(TypeTag::Signer)),
                MoveValue::Vector(vec![]),
                Ok(()),
            ),
            (
                TypeTag::Vector(Box::new(TypeTag::Signer)),
                MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(correct_signer),
                ]),
                Ok(()),
            ),
            (
                TypeTag::Vector(Box::new(TypeTag::Signer)),
                MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(correct_signer),
                ]),
                Ok(()),
            ),
            (
                TypeTag::Vector(Box::new(TypeTag::Signer)),
                MoveValue::Vector(vec![
                    MoveValue::Signer(incorrect_signer),
                    MoveValue::Signer(correct_signer),
                ]),
                Err(()),
            ),
            (
                TypeTag::Vector(Box::new(TypeTag::Signer)),
                MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(incorrect_signer),
                ]),
                Err(()),
            ),
            (
                TypeTag::Vector(Box::new(TypeTag::Signer)),
                MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(correct_signer),
                    MoveValue::Signer(incorrect_signer),
                ]),
                Err(()),
            ),
            (
                TypeTag::Vector(Box::new(TypeTag::U32)),
                MoveValue::Vector(vec![
                    MoveValue::U32(0),
                    MoveValue::U32(1),
                    MoveValue::U32(2),
                    MoveValue::U32(3),
                ]),
                Ok(()),
            ),
            (
                TypeTag::Struct(Box::new(StructTag {
                    address: AccountAddress::ONE,
                    module: "option".parse().unwrap(),
                    name: "Option".parse().unwrap(),
                    type_args: vec![TypeTag::Signer],
                })),
                MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(Vec::new())])),
                Ok(()),
            ),
            (
                TypeTag::Struct(Box::new(StructTag {
                    address: AccountAddress::ONE,
                    module: "option".parse().unwrap(),
                    name: "Option".parse().unwrap(),
                    type_args: vec![TypeTag::Signer],
                })),
                MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(vec![
                    MoveValue::Signer(correct_signer),
                ])])),
                Ok(()),
            ),
            (
                TypeTag::Struct(Box::new(StructTag {
                    address: AccountAddress::ONE,
                    module: "option".parse().unwrap(),
                    name: "Option".parse().unwrap(),
                    type_args: vec![TypeTag::Signer],
                })),
                MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(vec![
                    MoveValue::Signer(incorrect_signer),
                ])])),
                Err(()),
            ),
        ];

        let move_vm = create_move_vm().unwrap();
        let state = InMemoryState::new();
        let evm_storage = InMemoryStorageTrieRepository::new();
        let mut session = create_vm_session(
            &move_vm,
            state.resolver(),
            SessionId::default(),
            &evm_storage,
            &(),
        );
        for (type_tag, test_case, expected_outcome) in test_cases {
            let actual_outcome =
                validate_entry_value(type_tag, test_case, &correct_signer, &mut session)
                    .map_err(|_| ());
            assert_eq!(
                &actual_outcome,
                expected_outcome,
                "check_signer test case {test_case:?} failed. Expected={expected_outcome:?} Actual={actual_outcome:?}"
            );
        }
    }

    #[test]
    fn test_validate_option() {
        type CheckOptionOutcome = Result<(), ()>;

        let option_tag = |t| {
            TypeTag::Struct(Box::new(StructTag {
                address: AccountAddress::ONE,
                module: "option".parse().unwrap(),
                name: "Option".parse().unwrap(),
                type_args: vec![t],
            }))
        };

        let option_value = |maybe_v: Option<MoveValue>| {
            let inner = maybe_v.map_or(Vec::new(), |v| vec![v]);
            MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(inner)]))
        };

        let test_cases: &[(TypeTag, MoveValue, CheckOptionOutcome)] = &[
            // None
            (option_tag(TypeTag::U8), option_value(None), Ok(())),
            // Some
            (
                option_tag(TypeTag::U8),
                option_value(Some(MoveValue::U8(0))),
                Ok(()),
            ),
            // Invalid
            (
                option_tag(TypeTag::U8),
                MoveValue::Struct(MoveStruct::new(vec![MoveValue::Vector(vec![
                    MoveValue::U8(0),
                    MoveValue::U8(1),
                ])])),
                Err(()),
            ),
            // Some -> None Nested
            (
                option_tag(option_tag(TypeTag::U32)),
                option_value(Some(option_value(None))),
                Ok(()),
            ),
            // Some -> Some Nested
            (
                option_tag(option_tag(TypeTag::U32)),
                option_value(Some(option_value(Some(MoveValue::U32(3))))),
                Ok(()),
            ),
            // Some -> Invalid Nested
            (
                option_tag(option_tag(TypeTag::U32)),
                option_value(Some(MoveValue::Struct(MoveStruct::new(vec![
                    MoveValue::Vector(vec![MoveValue::U32(0), MoveValue::U32(1)]),
                ])))),
                Err(()),
            ),
        ];

        let move_vm = create_move_vm().unwrap();
        let state = InMemoryState::new();
        let evm_storage = InMemoryStorageTrieRepository::new();
        let mut session = create_vm_session(
            &move_vm,
            state.resolver(),
            SessionId::default(),
            &evm_storage,
            &(),
        );
        for (type_tag, test_case, expected_outcome) in test_cases {
            let actual_outcome =
                validate_entry_value(type_tag, test_case, &AccountAddress::ZERO, &mut session)
                    .map_err(|_| ());
            assert_eq!(
                &actual_outcome,
                expected_outcome,
                "validate_option test case {test_case:?} failed. Expected={expected_outcome:?} Actual={actual_outcome:?}"
            );
        }
    }
}
