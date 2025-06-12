#[cfg(feature = "storage-lmdb")]
pub use heed::*;
#[cfg(all(not(feature = "storage-rocksdb"), not(feature = "storage-lmdb")))]
pub use in_memory::*;
#[cfg(all(feature = "storage-rocksdb", not(feature = "storage-lmdb")))]
pub use rocksdb::*;
pub use shared::create;

#[cfg(feature = "storage-lmdb")]
mod heed;
#[cfg(all(not(feature = "storage-rocksdb"), not(feature = "storage-lmdb")))]
mod in_memory;
#[cfg(all(feature = "storage-rocksdb", not(feature = "storage-lmdb")))]
mod rocksdb;
mod shared;
