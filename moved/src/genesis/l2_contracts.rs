use {
    crate::{move_execution, storage::State},
    alloy::genesis::Genesis,
    move_binary_format::errors::PartialVMError,
    move_core_types::effects::ChangeSet,
};

pub fn init_state(genesis: Genesis, state: &impl State<Err = PartialVMError>) -> ChangeSet {
    move_execution::genesis_state_changes(genesis, state.resolver())
}
