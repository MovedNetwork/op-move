macro_rules! impl_shared {
    () => {
        type BlockHash = umi_blockchain::block::UmiBlockHash;
        type BaseTokenAccounts = umi_execution::UmiBaseTokenAccounts;
        type BaseGasFee = umi_blockchain::block::Eip1559GasFee;
        type CreateL1GasFee = umi_execution::CreateEcotoneL1GasFee;
        type CreateL2GasFee = umi_execution::CreateUmiL2GasFee;

        fn block_hash() -> Self::BlockHash {
            umi_blockchain::block::UmiBlockHash
        }

        fn base_gas_fee() -> Self::BaseGasFee {
            umi_blockchain::block::Eip1559GasFee::new(
                crate::EIP1559_ELASTICITY_MULTIPLIER,
                crate::EIP1559_BASE_FEE_MAX_CHANGE_DENOMINATOR,
            )
        }

        fn create_l1_gas_fee() -> Self::CreateL1GasFee {
            umi_execution::CreateEcotoneL1GasFee
        }

        fn create_l2_gas_fee() -> Self::CreateL2GasFee {
            umi_execution::CreateUmiL2GasFee
        }

        fn base_token_accounts(genesis_config: &GenesisConfig) -> Self::BaseTokenAccounts {
            umi_execution::UmiBaseTokenAccounts::new(genesis_config.treasury)
        }
    };
}

pub(crate) use impl_shared;
