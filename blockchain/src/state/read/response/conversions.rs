use {
    crate::state::read::response::abi::{
        MoveAbility, MoveFunction, MoveFunctionGenericTypeParam, MoveModule, MoveStruct,
        MoveStructField, MoveStructGenericTypeParam, MoveStructTag, MoveType,
    },
    move_binary_format::{
        CompiledModule,
        access::ModuleAccess,
        file_format::{
            AddressIdentifierIndex, FieldDefinition, FunctionDefinition, FunctionHandle,
            FunctionHandleIndex, IdentifierIndex, ModuleHandle, ModuleHandleIndex, Signature,
            SignatureIndex, SignatureToken, StructDefinition, StructFieldInformation, StructHandle,
            StructHandleIndex, Visibility,
        },
    },
    move_core_types::{
        account_address::AccountAddress,
        identifier::{IdentStr, Identifier},
        metadata::Metadata,
    },
    std::borrow::Borrow,
};

impl From<CompiledModule> for MoveModule {
    fn from(value: CompiledModule) -> Self {
        let (address, name) = <(AccountAddress, Identifier)>::from(value.self_id());
        Self {
            address: address.into(),
            name: name.into_string().into(),
            friends: value
                .immediate_friends()
                .into_iter()
                .map(Into::into)
                .collect(),
            exposed_functions: value
                .function_defs
                .iter()
                // Return all entry or public functions.
                // Private entry functions are still callable by entry function transactions so
                // they should be included.
                .filter(|def| {
                    def.is_entry
                        || matches!(def.visibility, Visibility::Public | Visibility::Friend)
                })
                .map(|def| value.new_move_function(def))
                .collect(),
            structs: value
                .struct_defs
                .iter()
                .map(|def| value.new_move_struct(def))
                .collect(),
        }
    }
}

pub trait Bytecode {
    fn module_handle_at(&self, idx: ModuleHandleIndex) -> &ModuleHandle;

    fn struct_handle_at(&self, idx: StructHandleIndex) -> &StructHandle;

    fn function_handle_at(&self, idx: FunctionHandleIndex) -> &FunctionHandle;

    fn signature_at(&self, idx: SignatureIndex) -> &Signature;

    fn identifier_at(&self, idx: IdentifierIndex) -> &IdentStr;

    fn address_identifier_at(&self, idx: AddressIdentifierIndex) -> &AccountAddress;

    fn find_entry_function(&self, name: &IdentStr) -> Option<MoveFunction>;

    fn find_function(&self, name: &IdentStr) -> Option<MoveFunction>;

    fn function_is_view(&self, name: &IdentStr) -> bool;

    fn struct_is_event(&self, name: &IdentStr) -> bool;

    fn new_move_struct_field(&self, def: &FieldDefinition) -> MoveStructField {
        MoveStructField {
            name: self.identifier_at(def.name).as_str().into(),
            r#type: self.new_move_type(&def.signature.0),
        }
    }

    fn new_move_struct_tag(
        &self,
        index: &StructHandleIndex,
        type_params: &[SignatureToken],
    ) -> MoveStructTag {
        let s_handle = self.struct_handle_at(*index);
        let m_handle = self.module_handle_at(s_handle.module);
        MoveStructTag {
            address: (*self.address_identifier_at(m_handle.address)).into(),
            module: self.identifier_at(m_handle.name).as_str().into(),
            name: self.identifier_at(s_handle.name).as_str().into(),
            generic_type_params: type_params.iter().map(|t| self.new_move_type(t)).collect(),
        }
    }

    fn new_move_type(&self, token: &SignatureToken) -> MoveType {
        match token {
            SignatureToken::Bool => MoveType::Bool,
            SignatureToken::U8 => MoveType::U8,
            SignatureToken::U16 => MoveType::U16,
            SignatureToken::U32 => MoveType::U32,
            SignatureToken::U64 => MoveType::U64,
            SignatureToken::U128 => MoveType::U128,
            SignatureToken::U256 => MoveType::U256,
            SignatureToken::Address => MoveType::Address,
            SignatureToken::Signer => MoveType::Signer,
            SignatureToken::Vector(t) => MoveType::Vector {
                items: Box::new(self.new_move_type(t.borrow())),
            },
            SignatureToken::Struct(v) => MoveType::Struct(self.new_move_struct_tag(v, &[])),
            SignatureToken::StructInstantiation(shi, type_params) => {
                MoveType::Struct(self.new_move_struct_tag(shi, type_params))
            }
            SignatureToken::TypeParameter(i) => MoveType::GenericTypeParam { index: *i },
            SignatureToken::Reference(t) => MoveType::Reference {
                mutable: false,
                to: Box::new(self.new_move_type(t.borrow())),
            },
            SignatureToken::MutableReference(t) => MoveType::Reference {
                mutable: true,
                to: Box::new(self.new_move_type(t.borrow())),
            },
            SignatureToken::Function(args, result, abilities) => {
                let new_vec = |toks: &[SignatureToken]| {
                    toks.iter()
                        .map(|t| self.new_move_type(t))
                        .collect::<Vec<_>>()
                };
                MoveType::Function {
                    args: new_vec(args),
                    results: new_vec(result),
                    abilities: *abilities,
                }
            }
        }
    }

    fn new_move_struct(&self, def: &StructDefinition) -> MoveStruct {
        let handle = self.struct_handle_at(def.struct_handle);
        let (is_native, fields) = match &def.field_information {
            StructFieldInformation::Native => (true, vec![]),
            StructFieldInformation::Declared(fields) => (
                false,
                fields
                    .iter()
                    .map(|f| self.new_move_struct_field(f))
                    .collect(),
            ),
            StructFieldInformation::DeclaredVariants(..) => {
                // TODO(#13806): implement for enums. Currently we pretend they don't have fields
                (false, vec![])
            }
        };
        let name = self.identifier_at(handle.name);
        let is_event = self.struct_is_event(&name);
        let abilities = handle
            .abilities
            .into_iter()
            .map(MoveAbility::from)
            .collect();
        let generic_type_params = handle
            .type_parameters
            .iter()
            .map(MoveStructGenericTypeParam::from)
            .collect();
        MoveStruct {
            name: name.as_str().into(),
            is_native,
            is_event,
            abilities,
            generic_type_params,
            fields,
        }
    }

    fn new_move_function(&self, def: &FunctionDefinition) -> MoveFunction {
        let fhandle = self.function_handle_at(def.function);
        let name = self.identifier_at(fhandle.name);
        let is_view = self.function_is_view(&name);
        MoveFunction {
            name: name.as_str().into(),
            visibility: def.visibility.into(),
            is_entry: def.is_entry,
            is_view,
            generic_type_params: fhandle
                .type_parameters
                .iter()
                .map(MoveFunctionGenericTypeParam::from)
                .collect(),
            params: self
                .signature_at(fhandle.parameters)
                .0
                .iter()
                .map(|s| self.new_move_type(s))
                .collect(),
            r#return: self
                .signature_at(fhandle.return_)
                .0
                .iter()
                .map(|s| self.new_move_type(s))
                .collect(),
        }
    }
}

impl Bytecode for CompiledModule {
    fn module_handle_at(&self, idx: ModuleHandleIndex) -> &ModuleHandle {
        ModuleAccess::module_handle_at(self, idx)
    }

    fn struct_handle_at(&self, idx: StructHandleIndex) -> &StructHandle {
        ModuleAccess::struct_handle_at(self, idx)
    }

    fn function_handle_at(&self, idx: FunctionHandleIndex) -> &FunctionHandle {
        ModuleAccess::function_handle_at(self, idx)
    }

    fn signature_at(&self, idx: SignatureIndex) -> &Signature {
        ModuleAccess::signature_at(self, idx)
    }

    fn identifier_at(&self, idx: IdentifierIndex) -> &IdentStr {
        ModuleAccess::identifier_at(self, idx)
    }

    fn address_identifier_at(&self, idx: AddressIdentifierIndex) -> &AccountAddress {
        ModuleAccess::address_identifier_at(self, idx)
    }

    fn find_entry_function(&self, name: &IdentStr) -> Option<MoveFunction> {
        self.function_defs
            .iter()
            .filter(|def| def.is_entry)
            .find(|def| {
                let fhandle = ModuleAccess::function_handle_at(self, def.function);
                ModuleAccess::identifier_at(self, fhandle.name) == name
            })
            .map(|def| self.new_move_function(def))
    }

    fn find_function(&self, name: &IdentStr) -> Option<MoveFunction> {
        self.function_defs
            .iter()
            .find(|def| {
                let fhandle = ModuleAccess::function_handle_at(self, def.function);
                ModuleAccess::identifier_at(self, fhandle.name) == name
            })
            .map(|def| self.new_move_function(def))
    }

    fn function_is_view(&self, _name: &IdentStr) -> bool {
        // TODO: Determine if function is view, possibly by using module metadata
        false
    }

    fn struct_is_event(&self, _name: &IdentStr) -> bool {
        // TODO: Determine if struct is event, possibly by using module metadata
        false
    }
}

/// Trait to unify accesses to [CompiledModule] and [CompiledScript] for extracting metadata.
pub trait CompiledCodeMetadata {
    /// Returns the binary version.
    fn version(&self) -> u32;
    /// Returns the [Metadata] stored in this module or script.
    fn metadata(&self) -> &[Metadata];
}

impl CompiledCodeMetadata for CompiledModule {
    fn version(&self) -> u32 {
        self.version
    }

    fn metadata(&self) -> &[Metadata] {
        &self.metadata
    }
}
