use {
    alloy::{
        primitives::keccak256,
        rpc::types::{EIP1186AccountProofResponse, EIP1186StorageProof},
    },
    eth_trie::{DB, EthTrie, Trie},
    move_core_types::account_address::AccountAddress,
    move_table_extension::TableResolver,
    move_vm_types::resolver::MoveResolver,
    std::error,
    umi_evm_ext::{
        ResolverBackedDB,
        state::{self, StorageTrieRepository},
    },
    umi_execution::{quick_get_eth_balance, quick_get_nonce},
    umi_shared::primitives::{Address, B256, KeyHashable, U256},
    umi_state::nodes::TreeKey,
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
/// * [`Self::proof_at`] - To fetch the account and storage values of the specified account
///   including the Merkle proof.
pub trait StateQueries {
    /// Queries the blockchain state version corresponding with block `height` for the amount of
    /// base token associated with `account`.
    fn balance_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Result<Balance, state::Error> {
        let resolver = self.resolver_at(height)?;

        Ok(quick_get_eth_balance(&account, &resolver, evm_storage))
    }

    /// Queries the blockchain state version corresponding with block `height` for the nonce value
    /// associated with `account`.
    fn nonce_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        height: BlockHeight,
    ) -> Result<Nonce, state::Error> {
        let resolver = self.resolver_at(height)?;

        Ok(quick_get_nonce(&account, &resolver, evm_storage))
    }

    /// Queries the blockchain state version corresponding with block `height` for the  
    /// account and storage proofs associated with `account`.
    fn proof_at(
        &self,
        evm_storage: &impl StorageTrieRepository,
        account: AccountAddress,
        storage_slots: &[U256],
        height: BlockHeight,
    ) -> Result<ProofResponse, state::Error>;

    fn resolver_at(
        &self,
        height: BlockHeight,
    ) -> Result<impl MoveResolver + TableResolver + '_, state::Error>;
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
) -> Result<ProofResponse, state::Error> {
    let evm_db = ResolverBackedDB::new(storage_trie, resolver, &(), 0);

    // All L2 contract account data is part of the EVM state
    let account_info = evm_db
        .get_account(&address)?
        .ok_or(state::Error::AccountNotFound(address))?;

    let account_key = TreeKey::Evm(address);
    let account_proof = tree
        .get_proof(account_key.key_hash().0.as_slice())?
        .into_iter()
        .map(Into::into)
        .collect();

    let storage_proof = if storage_slots.is_empty() {
        Vec::new()
    } else {
        let mut storage =
            storage_trie.for_account_with_root(&address, &account_info.inner.storage_root)?;

        storage_slots
            .iter()
            .map(|index| {
                let key = keccak256::<[u8; 32]>(index.to_be_bytes());
                let proof = storage.proof(key.as_slice())?;
                let value = storage.get(index)?.unwrap_or_default();

                Ok(StorageProof {
                    key: (*index).into(),
                    value,
                    proof: proof.into_iter().map(Into::into).collect(),
                })
            })
            .collect::<Result<Vec<_>, state::Error>>()?
    };

    Ok(ProofResponse {
        address,
        balance: account_info.inner.balance,
        code_hash: account_info.inner.code_hash,
        nonce: account_info.inner.nonce,
        storage_hash: account_info.inner.storage_root,
        account_proof,
        storage_proof,
    })
}
