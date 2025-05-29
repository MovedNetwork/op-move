use {
    alloy::{
        primitives::keccak256,
        rpc::types::{EIP1186AccountProofResponse, EIP1186StorageProof},
    },
    eth_trie::{DB, EthTrie, Trie},
    move_core_types::account_address::AccountAddress,
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    moved_evm_ext::{
        ResolverBackedDB,
        state::{self, StorageTrieRepository},
    },
    moved_execution::{quick_get_eth_balance, quick_get_nonce},
    moved_shared::primitives::{Address, B256, KeyHashable, U256},
    moved_state::nodes::TreeKey,
    std::error,
};

pub type ProofResponse = EIP1186AccountProofResponse;
pub type StorageProof = EIP1186StorageProof;

/// A non-negative integer for indicating the amount of base token on an account.
pub type Balance = U256;

/// A non-negative integer for indicating the nonce used for sending transactions by an account.
pub type Nonce = u64;

/// A non-negative integer for indicating the order of a block in the blockchain, used as a tag for
/// [`Version`].
pub type BlockHeight = u64;

/// A non-negative integer for versioning a set of changes in a historical order.
///
/// Typically, each version matches one transaction, but there is an exception for changes generated
/// on genesis.
pub type Version = u64;

/// Accesses blockchain state in any particular point in history to fetch some account values.
///
/// It is defined by these operations:
/// * [`Self::balance_at`] - To fetch an amount of base token in an account read in its smallest
///   denomination at given block height.
/// * [`Self::nonce_at`] - To fetch the nonce value set for an account at given block height.
pub trait StateQueries {
    /// Queries the blockchain state version corresponding with block `height` for the amount of
    /// base token associated with `account`.
    fn balance_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Balance> {
        let resolver = self.resolver_at(height);

        Some(quick_get_eth_balance(&account, &resolver, evm_storage))
    }

    /// Queries the blockchain state version corresponding with block `height` for the nonce value
    /// associated with `account`.
    fn nonce_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Option<Nonce> {
        let resolver = self.resolver_at(height);

        Some(quick_get_nonce(&account, &resolver, evm_storage))
    }

    fn proof_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Option<ProofResponse>;

    fn resolver_at(&self, height: BlockHeight) -> impl MoveResolver + TableResolver + '_;
}

pub trait HeightToStateRootIndex {
    type Err: error::Error;
    fn root_by_height(&self, height: BlockHeight) -> Result<Option<B256>, Self::Err>;
    fn height(&self) -> Result<BlockHeight, Self::Err>;
    fn push_state_root(&self, state_root: B256) -> Result<(), Self::Err>;
}

pub fn proof_from_trie_and_resolver(
    address: Address,
    storage_slots: &[U256],
    tree: &mut EthTrie<impl DB>,
    resolver: &impl MoveResolver,
    storage_trie: &impl StorageTrieRepository,
) -> Option<ProofResponse> {
    let evm_db = ResolverBackedDB::new(storage_trie, resolver, &(), 0);

    // All L2 contract account data is part of the EVM state
    let account_info = evm_db.get_account(&address).ok()??;

    let account_key = TreeKey::Evm(address);
    let account_proof = tree
        .get_proof(account_key.key_hash().0.as_slice())
        .ok()?
        .into_iter()
        .map(Into::into)
        .collect();

    let storage_proof = if storage_slots.is_empty() {
        Vec::new()
    } else {
        let mut storage = storage_trie
            .for_account_with_root(&address, &account_info.inner.storage_root)
            .ok()?;

        storage_slots
            .iter()
            .filter_map(|index| {
                let key = keccak256::<[u8; 32]>(index.to_be_bytes());
                storage.proof(key.as_slice()).ok().map(|proof| {
                    let value = storage.get(index)?.unwrap_or_default();

                    Ok::<StorageProof, state::Error>(StorageProof {
                        key: (*index).into(),
                        value,
                        proof: proof.into_iter().map(Into::into).collect(),
                    })
                })
            })
            .collect::<Result<_, _>>()
            .unwrap()
    };

    Some(ProofResponse {
        address,
        balance: account_info.inner.balance,
        code_hash: account_info.inner.code_hash,
        nonce: account_info.inner.nonce,
        storage_hash: account_info.inner.storage_root,
        account_proof,
        storage_proof,
    })
}
