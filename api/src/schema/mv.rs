use {alloy::primitives::Address, serde::Deserialize};

pub use {
    aptos_api_types::{IdentifierWrapper, MoveStruct, MoveStructTag, MoveType, TableItemRequest},
    move_table_extension::TableHandle,
};

#[derive(Debug, Deserialize)]
pub struct ListingArgs<T> {
    pub address: Address,
    pub after: Option<T>,
    #[serde(default)]
    pub limit: Option<u32>,
}
