//! The block module is responsible for the concerns of blocks such that it:
//!
//! * Defines the structure of Ethereum blocks.
//! * Implements an algorithm for producing its hashes.
//! * Declares a collection of blocks in the node.

mod gas;
mod hash;
mod in_memory;
mod read;
mod write;

pub use {
    gas::{BaseGasFee, Eip1559GasFee},
    hash::{BlockHash, MovedBlockHash},
    in_memory::{BlockMemory, BlockMemoryReader},
    read::{BlockQueries, BlockResponse, in_memory::InMemoryBlockQueries},
    write::{Block, BlockRepository, ExtendedBlock, Header, in_memory::InMemoryBlockRepository},
};
