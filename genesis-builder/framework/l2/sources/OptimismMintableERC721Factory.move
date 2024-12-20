module OptimismMintableERC721Factory::optimism_mintable_erc721_factory {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct BridgeArgs {}

    public fun bridge(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BridgeArgs {};

        let data = abi_encode_params(
            vector[238, 154, 49, 162],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC721Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RemoteChainIdArgs {}

    public fun remote_chain_id(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = RemoteChainIdArgs {};

        let data = abi_encode_params(
            vector[125, 29, 12, 91],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC721Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct CreateOptimismMintableErc721Args {
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
    }

    public fun create_optimism_mintable_erc721(
        caller: &signer,
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = CreateOptimismMintableErc721Args {
            remote_token,
            name,
            symbol,
        };

        let data = abi_encode_params(
            vector[217, 125, 246, 82],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC721Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct IsOptimismMintableErc721Args {
        key: address,
    }

    public fun is_optimism_mintable_erc721(
        caller: &signer,
        key: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = IsOptimismMintableErc721Args {
            key,
        };

        let data = abi_encode_params(
            vector[85, 114, 172, 174],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC721Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct VersionArgs {}

    public fun version(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = VersionArgs {};

        let data = abi_encode_params(
            vector[84, 253, 77, 80],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC721Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
