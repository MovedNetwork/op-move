use {
    crate::{block::BlockMemory, primitives::B256, types::state::BlockResponse},
    std::sync::Arc,
};

pub trait Queries {
    fn block_by_hash(&self, hash: B256) -> Option<BlockResponse>;
    fn block_by_number(&self, number: u64) -> Option<BlockResponse>;
}

pub struct InMemoryQueries {
    mem: Arc<BlockMemory>,
}

impl InMemoryQueries {
    pub fn new(mem: Arc<BlockMemory>) -> Self {
        Self { mem }
    }
}

impl Queries for InMemoryQueries {
    fn block_by_hash(&self, hash: B256) -> Option<BlockResponse> {
        self.mem.by_hash(hash).map(Into::into)
    }

    fn block_by_number(&self, number: u64) -> Option<BlockResponse> {
        self.mem.by_number(number).map(Into::into)
    }
}
