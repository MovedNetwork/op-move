module OptimismMintableERC20Factory::optimism_mintable_erc20_factory {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct BRIDGEArgs {}

    public fun BRIDGE(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BRIDGEArgs {};

        let data = abi_encode_params(
            vector[238, 154, 49, 162],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC20Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BridgeArgs {}

    public fun bridge(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BridgeArgs {};

        let data = abi_encode_params(
            vector[231, 140, 234, 146],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC20Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct CreateOptimismMintableERC20Args {
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
    }

    public fun create_optimism_mintable_erc20(
        caller: &signer,
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = CreateOptimismMintableERC20Args {
            remote_token,
            name,
            symbol,
        };

        let data = abi_encode_params(
            vector[206, 90, 201, 15],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC20Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct CreateOptimismMintableERC20WithDecimalsArgs {
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
        decimals: u8,
    }

    public fun create_optimism_mintable_erc20_with_decimals(
        caller: &signer,
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
        decimals: u8,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = CreateOptimismMintableERC20WithDecimalsArgs {
            remote_token,
            name,
            symbol,
            decimals,
        };

        let data = abi_encode_params(
            vector[140, 240, 98, 156],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC20Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct CreateStandardL2TokenArgs {
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
    }

    public fun create_standard_l2_token(
        caller: &signer,
        remote_token: address,
        name: vector<u8>,
        symbol: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = CreateStandardL2TokenArgs {
            remote_token,
            name,
            symbol,
        };

        let data = abi_encode_params(
            vector[137, 111, 147, 209],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC20Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct InitializeArgs {
        bridge: address,
    }

    public fun initialize(
        caller: &signer,
        bridge: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = InitializeArgs {
            bridge,
        };

        let data = abi_encode_params(
            vector[196, 214, 109, 232],
            arg_struct,
        );
        let result = evm_call(caller, @OptimismMintableERC20Factory, _value, data);
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
        let result = evm_call(caller, @OptimismMintableERC20Factory, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
