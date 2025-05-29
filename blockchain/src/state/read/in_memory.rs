use {
    crate::{
        block::ReadBlockMemory,
        in_memory::SharedMemoryReader,
        state::{BlockHeight, EthTrieStateQueries, read::model::HeightToStateRootIndex},
    },
    moved_shared::primitives::B256,
};

pub type InMemoryStateQueries<R = SharedMemoryReader, D = moved_state::InMemoryTrieDb> =
    EthTrieStateQueries<R, D>;

impl HeightToStateRootIndex for SharedMemoryReader {
    fn root_by_height(&self, height: BlockHeight) -> Option<B256> {
        self.block_memory
            .map_by_height(height, |v| v.block.header.state_root)
    }

    fn height(&self) -> BlockHeight {
        self.block_memory
            .height()
            .expect("Genesis should not be missing")
    }
}
