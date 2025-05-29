use {
    aptos_types::state_store::{state_key::StateKey, state_value::StateValue},
    std::borrow::Cow,
    umi_shared::primitives::{Address, KeyHash, KeyHashable},
};

/// Type representing the keys used in the state trie.
///
/// The EVM native is designed such that all EVM state is represented
/// as Move resources and thus all keys can be represented by an Aptos
/// `StateKey`. However, we must still special-case the EVM addresses
/// here because the "real" key of the trie (the one who's nibbles are
/// actually used to navigate from node to node) is the hash of the
/// "semantic" key given here. Therefore, Ethereum clients expect to be
/// able to verify the Merkle proof for an account using the "real" key
/// given by `keccack256(address)`, and this does not work if our key is
/// instead derived as `keccak256(state_key)`. Thus for compatibility with
/// Ethereum tooling that is checking the state of accounts in our EVM,
/// we must explicitly separate those keys in the trie instead of using
/// the uniform `StateKey` which reflects the actual way the state is stored.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum TreeKey {
    StateKey(StateKey),
    Evm(Address),
}

impl KeyHashable for TreeKey {
    fn key_hash(&self) -> KeyHash {
        match self {
            Self::StateKey(key) => {
                let bytes = key.encoded();
                KeyHash(alloy::primitives::keccak256(bytes))
            }
            Self::Evm(address) => KeyHash(alloy::primitives::keccak256(address)),
        }
    }
}

/// Type representing the values used in the state trie.
///
/// As with the keys, EVM values are treated separately.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TreeValue {
    Deleted,
    StateValue(StateValue),
    Evm(bytes::Bytes),
}

impl TreeValue {
    pub fn serialize(&self) -> Cow<[u8]> {
        match self {
            Self::Deleted => Cow::Borrowed(&[]),
            Self::Evm(bytes) => Cow::Borrowed(bytes),
            Self::StateValue(value) => {
                Cow::Owned(bcs::to_bytes(value).expect("StateValue must serialize"))
            }
        }
    }
}
