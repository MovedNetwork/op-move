use {
    alloy::{
        consensus,
        primitives::{B256, U256},
        rlp,
    },
    revm::primitives::AccountInfo,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Account {
    pub inner: consensus::Account,
}

impl Account {
    pub fn new(nonce: u64, balance: U256, code_hash: B256, storage_root: B256) -> Self {
        Self {
            inner: consensus::Account {
                nonce,
                balance,
                storage_root,
                code_hash,
            },
        }
    }

    pub fn serialize(&self) -> Vec<u8> {
        rlp::encode(self.inner)
    }

    pub fn try_deserialize(bytes: &[u8]) -> Option<Self> {
        let inner = rlp::decode_exact(bytes).ok()?;
        Some(Self { inner })
    }
}

impl From<Account> for AccountInfo {
    fn from(value: Account) -> Self {
        Self {
            balance: value.inner.balance,
            nonce: value.inner.nonce,
            code_hash: value.inner.code_hash,
            code: None,
        }
    }
}
