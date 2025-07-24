use {
    move_core_types::{
        ability::AbilitySet, account_address::AccountAddress, identifier::Identifier,
        language_storage::ModuleId,
    },
    std::fmt,
};

/// A Move module
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveModule {
    /// Address of the account the module is associated to.
    pub address: AccountAddress,
    /// String identifier of the module within the `address`.
    pub name: Box<str>,
    /// Friends of the module
    pub friends: Vec<MoveModuleId>,
    /// Public functions of the module
    pub exposed_functions: Vec<MoveFunction>,
    /// Structs of the module
    pub structs: Vec<MoveStruct>,
}

/// A Move module Id
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct MoveModuleId {
    pub address: AccountAddress,
    pub name: Box<str>,
}

impl From<ModuleId> for MoveModuleId {
    fn from(id: ModuleId) -> Self {
        let (address, name) = <(AccountAddress, Identifier)>::from(id);
        Self {
            address: address.into(),
            name: name.into(),
        }
    }
}

impl fmt::Display for MoveModuleId {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}::{}", self.address, self.name)
    }
}

/// Move function
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveFunction {
    pub name: Box<str>,
    pub visibility: MoveFunctionVisibility,
    /// Whether the function can be called as an entry function directly in a transaction
    pub is_entry: bool,
    /// Whether the function is a view function or not
    pub is_view: bool,
    /// Generic type params associated with the Move function
    pub generic_type_params: Vec<MoveFunctionGenericTypeParam>,
    /// Parameters associated with the move function
    pub params: Vec<MoveType>,
    /// Return type of the function
    pub r#return: Vec<MoveType>,
}

/// A move struct
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveStruct {
    pub name: Box<str>,
    /// Whether the struct is a native struct of Move
    pub is_native: bool,
    /// Whether the struct is marked with the #[event] annotation
    pub is_event: bool,
    /// Abilities associated with the struct
    pub abilities: Vec<MoveAbility>,
    /// Generic types associated with the struct
    pub generic_type_params: Vec<MoveStructGenericTypeParam>,
    /// Fields associated with the struct
    pub fields: Vec<MoveStructField>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveAbility {
    /// Allows values of types with this ability to be copied, via CopyLoc or ReadRef
    Copy,
    /// Allows values of types with this ability to be dropped, via Pop, WriteRef, StLoc, Eq, Neq,
    /// or if left in a local when Ret is invoked
    /// Technically also needed for numeric operations (Add, BitAnd, Shift, etc), but all
    /// of the types that can be used with those operations have Drop
    Drop,
    /// Allows values of types with this ability to exist inside a struct in global storage
    Store,
    /// Allows the type to serve as a key for global storage operations: MoveTo, MoveFrom, etc.
    Key,
}

/// Move generic type param
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveStructGenericTypeParam {
    /// Move abilities tied to the generic type param and associated with the type that uses it
    pub constraints: Vec<MoveAbility>,
}

/// Move struct field
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveStructField {
    pub name: Box<str>,
    pub r#type: MoveType,
}

/// An enum of Move's possible types on-chain
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveType {
    /// A bool type
    Bool,
    /// An 8-bit unsigned int
    U8,
    /// A 16-bit unsigned int
    U16,
    /// A 32-bit unsigned int
    U32,
    /// A 64-bit unsigned int
    U64,
    /// A 128-bit unsigned int
    U128,
    /// A 256-bit unsigned int
    U256,
    /// A 32-byte account address
    Address,
    /// An account signer
    Signer,
    /// A Vector of [`MoveType`]
    Vector { items: Box<MoveType> },
    /// A struct of [`MoveStructTag`]
    Struct(MoveStructTag),
    /// A function
    Function {
        args: Vec<MoveType>,
        results: Vec<MoveType>,
        abilities: AbilitySet,
    },
    /// A generic type param with index
    GenericTypeParam { index: u16 },
    /// A reference
    Reference { mutable: bool, to: Box<MoveType> },
    /// A move type that couldn't be parsed
    ///
    /// This prevents the parser from just throwing an error because one field
    /// was unparsable, and gives the value in it.
    Unparsable(String),
}

/// A Move struct tag for referencing an on-chain struct type
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveStructTag {
    pub address: AccountAddress,
    pub module: Box<str>,
    pub name: Box<str>,
    /// Generic type parameters associated with the struct
    pub generic_type_params: Vec<MoveType>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MoveFunctionVisibility {
    /// Visible only by this module
    Private,
    /// Visible by all modules
    Public,
    /// Visible by friend modules
    Friend,
}

/// Move function generic type param
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveFunctionGenericTypeParam {
    /// Move abilities tied to the generic type param and associated with the function that uses it
    pub constraints: Vec<MoveAbility>,
}
