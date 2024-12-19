module L2ERC721Bridge::l2_erc721_bridge {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct MESSENGERArgs {}

    public fun MESSENGER(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MESSENGERArgs {};

        let data = abi_encode_params(
            vector[146, 126, 222, 45],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct OTHER_BRIDGEArgs {}

    public fun OTHER_BRIDGE(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = OTHER_BRIDGEArgs {};

        let data = abi_encode_params(
            vector[127, 70, 221, 178],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BridgeERC721Args {
        local_token: address,
        remote_token: address,
        token_id: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    }

    public fun bridge_erc721(
        caller: &signer,
        local_token: address,
        remote_token: address,
        token_id: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BridgeERC721Args {
            local_token,
            remote_token,
            token_id,
            min_gas_limit,
            extra_data,
        };

        let data = abi_encode_params(
            vector[54, 135, 1, 26],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BridgeERC721ToArgs {
        local_token: address,
        remote_token: address,
        to: address,
        token_id: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    }

    public fun bridge_erc721_to(
        caller: &signer,
        local_token: address,
        remote_token: address,
        to: address,
        token_id: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BridgeERC721ToArgs {
            local_token,
            remote_token,
            to,
            token_id,
            min_gas_limit,
            extra_data,
        };

        let data = abi_encode_params(
            vector[170, 85, 116, 82],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct FinalizeBridgeERC721Args {
        local_token: address,
        remote_token: address,
        from: address,
        to: address,
        token_id: u256,
        extra_data: vector<u8>,
    }

    public fun finalize_bridge_erc721(
        caller: &signer,
        local_token: address,
        remote_token: address,
        from: address,
        to: address,
        token_id: u256,
        extra_data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = FinalizeBridgeERC721Args {
            local_token,
            remote_token,
            from,
            to,
            token_id,
            extra_data,
        };

        let data = abi_encode_params(
            vector[118, 31, 68, 147],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct InitializeArgs {
        l1_erc721_bridge: address,
    }

    public fun initialize(
        caller: &signer,
        l1_erc721_bridge: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = InitializeArgs {
            l1_erc721_bridge,
        };

        let data = abi_encode_params(
            vector[196, 214, 109, 232],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct MessengerArgs {}

    public fun messenger(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MessengerArgs {};

        let data = abi_encode_params(
            vector[60, 183, 71, 191],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct OtherBridgeArgs {}

    public fun other_bridge(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = OtherBridgeArgs {};

        let data = abi_encode_params(
            vector[200, 151, 1, 162],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct PausedArgs {}

    public fun paused(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = PausedArgs {};

        let data = abi_encode_params(
            vector[92, 151, 90, 187],
            arg_struct,
        );
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
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
        let result = evm_call(caller, @L2ERC721Bridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
