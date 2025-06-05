use alloy::signers::local::PrivateKeySigner;

#[derive(Debug, Clone)]
pub struct Signer {
    pub inner: PrivateKeySigner,
    pub nonce: u64,
}

impl Signer {
    pub fn new(key_bytes: &[u8; 32]) -> Self {
        Self {
            inner: PrivateKeySigner::from_bytes(&key_bytes.into()).unwrap(),
            nonce: 0,
        }
    }
}
