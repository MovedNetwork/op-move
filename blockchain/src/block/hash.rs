use {crate::block::Header, alloy::primitives::B256};

/// Represents an algorithm that computes the block hash.
pub trait BlockHash {
    /// Computes a block hash.
    fn block_hash(&self, header: &Header) -> B256;
}

/// Computes the block hash following the Ethereum specification.
#[derive(Debug, Clone)]
pub struct MovedBlockHash;

impl BlockHash for MovedBlockHash {
    fn block_hash(&self, header: &Header) -> B256 {
        header.hash_slow()
    }
}

#[cfg(any(feature = "test-doubles", test))]
mod tests {
    use super::*;

    impl BlockHash for B256 {
        fn block_hash(&self, _header: &Header) -> B256 {
            *self
        }
    }

    #[test]
    fn test_block_hash() {
        use alloy::{hex, primitives::address};

        let header = Header {
            parent_hash: B256::new(hex!(
                "ae28c295bedd4e905e9a39b31e2880fbeb31b980738f8566ac097f3a17b8fa60"
            )),
            ommers_hash: B256::new(hex!(
                "1dcc4de8dec75d7aab85b567b6ccd41ad312451b948a7413f0a142fd40d49347"
            )),
            beneficiary: address!("4200000000000000000000000000000000000011"),
            state_root: B256::new(hex!(
                "7f1f9b306ee78488973ff93266ef8946a62743fe3503cfbfc427c63add288e9e"
            )),
            transactions_root: B256::new(hex!(
                "90e7a8d12f001569a72bfae8ec3b108c72342f9e8aa824658b974b4f4c0cc640"
            )),
            receipts_root: B256::new(hex!(
                "3c55e3bccc48ee3ee637d8fc6936e4825d1489cbebf6057ce8025d63755ebf54"
            )),
            logs_bloom: Default::default(),
            difficulty: Default::default(),
            number: 1,
            gas_limit: 0x1c9c380,
            gas_used: 0x272a2,
            timestamp: 0x674de721,
            extra_data: Default::default(),
            mix_hash: B256::new(hex!(
                "0f5496f9f62026179c9021c646b4b1a985408b8f2eceb42408138589caa4ab6e"
            )),
            nonce: Default::default(),
            base_fee_per_gas: Some(0x3b5dc100),
            withdrawals_root: Some(B256::new(hex!(
                "56e81f171bcc55a6ff8345e692c0f86e5b48e01b996cadc001622fb5e363b421"
            ))),
            blob_gas_used: Some(0),
            excess_blob_gas: Some(0),
            parent_beacon_block_root: Some(Default::default()),
            requests_hash: None,
        };
        assert_eq!(
            MovedBlockHash.block_hash(&header),
            B256::new(hex!(
                "2fb75468d63d9e7c88f5f9b846417cdfd208db0defd1cdae2f388c92ca82a839"
            ))
        );
    }
}
