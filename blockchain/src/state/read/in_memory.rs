use {
    crate::{
        block::ReadBlockMemory,
        in_memory::SharedMemoryReader,
        state::{BlockHeight, EthTrieStateQueries, read::model::HeightToStateRootIndex},
    },
    moved_shared::primitives::B256,
    std::convert::Infallible,
};

pub type InMemoryStateQueries<R = SharedMemoryReader, D = moved_state::InMemoryTrieDb> =
    EthTrieStateQueries<R, D>;

impl HeightToStateRootIndex for SharedMemoryReader {
    type Err = Infallible;

    fn root_by_height(&self, height: BlockHeight) -> Result<Option<B256>, Self::Err> {
        Ok(self
            .block_memory
            .map_by_height(height, |v| v.block.header.state_root))
    }

    fn height(&self) -> Result<BlockHeight, Self::Err> {
        Ok(self
            .block_memory
            .height()
            .expect("Genesis should not be missing"))
    }

    fn push_state_root(&self, _state_root: B256) -> Result<(), Self::Err> {
        Ok(())
    }
}
