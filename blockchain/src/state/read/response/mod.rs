pub use abi::{
    MoveAbility, MoveFunction, MoveFunctionGenericTypeParam, MoveFunctionVisibility, MoveModule,
    MoveModuleId, MoveStruct, MoveStructField, MoveStructGenericTypeParam, MoveStructTag, MoveType,
};
use umi_shared::primitives::Bytes;

mod abi;
mod conversions;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MoveModuleResponse {
    /// The Move module bytecode.
    pub bytecode: Bytes,
    /// Parsed Move module bytecode into an ABI declaration, or `None` if the bytecode is invalid.
    ///
    /// A transaction module payload can contain invalid bytecode.
    pub abi: Option<MoveModule>,
}
