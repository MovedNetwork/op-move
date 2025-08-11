use {alloy::primitives::Address, serde::Deserialize};

#[derive(Debug, Deserialize)]
pub struct ListingArgs<T> {
    pub address: Address,
    pub after: Option<T>,
    #[serde(default)]
    pub limit: Option<u32>,
}
