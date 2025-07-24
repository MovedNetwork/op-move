use {
    crate::state::read::response::abi::MoveModule,
    move_binary_format::{CompiledModule, access::ModuleAccess, file_format::Visibility},
    move_core_types::{account_address::AccountAddress, identifier::Identifier},
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
