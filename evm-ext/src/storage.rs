use {
    alloy::{primitives::keccak256, rlp},
    eth_trie::{EthTrie, Trie, TrieError, DB},
    moved_shared::primitives::{Address, B256, U256},
    std::{error, fmt::Debug, result},
    thiserror::Error,
};

/// [`result::Result`] with its `Err` variant set to [`Error`].
type Result<T> = result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    EthTrie(#[from] TrieError),
    #[error("{0}")]
    Rlp(#[from] rlp::Error),
}

pub struct StorageTrie<E: error::Error>(pub EthTrie<BoxedTrieDb<E>>);

#[auto_impl::auto_impl(Box)]
pub trait StorageTrieRepository {
    type Err: error::Error;

    fn for_account(&self, account: &Address) -> StorageTrie<Self::Err>;

    fn by_root(&self, storage_root: &B256) -> StorageTrie<Self::Err>;
}

pub struct BoxedTrieDb<T>(pub Box<dyn DB<Error = T>>);

impl<T: error::Error> DB for BoxedTrieDb<T> {
    type Error = T;

    fn get(&self, key: &[u8]) -> result::Result<Option<Vec<u8>>, Self::Error> {
        self.0.get(key)
    }

    fn insert(&self, key: &[u8], value: Vec<u8>) -> result::Result<(), Self::Error> {
        self.0.insert(key, value)
    }

    fn remove(&self, key: &[u8]) -> result::Result<(), Self::Error> {
        self.0.remove(key)
    }

    fn flush(&self) -> result::Result<(), Self::Error> {
        self.0.flush()
    }
}

impl<E: error::Error> StorageTrie<E> {
    pub fn root_hash(&mut self) -> Result<B256> {
        Ok(self.0.root_hash()?)
    }

    pub fn proof(&mut self, key: &[u8]) -> Result<Vec<Vec<u8>>> {
        Ok(self.0.get_proof(key)?)
    }

    pub fn get(&self, index: &U256) -> Result<Option<U256>> {
        let trie_key = keccak256::<[u8; 32]>(index.to_be_bytes());
        let Some(bytes) = self.0.get(trie_key.as_slice())? else {
            return Ok(None);
        };

        Ok(Some(rlp::decode_exact(&bytes)?))
    }

    pub fn insert(&mut self, index: &U256, value: &U256) -> Result<()> {
        let trie_key = keccak256::<[u8; 32]>(index.to_be_bytes());

        if value.is_zero() {
            self.0.remove(trie_key.as_slice())?;
        } else {
            let value = rlp::encode_fixed_size(value);
            self.0.insert(trie_key.as_slice(), &value)?;
        }

        Ok(())
    }
}
