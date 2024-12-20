module L2StandardBridge::l2_standard_bridge {
    use aptos_framework::fungible_asset_u256::{FungibleAsset, zero};
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct MessengerArgs {}

    public fun messenger(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MessengerArgs {};

        let data = abi_encode_params(
            vector[146, 126, 222, 45],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
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
            vector[127, 70, 221, 178],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BridgeErc20Args {
        local_token: address,
        remote_token: address,
        amount: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    }

    public fun bridge_erc20(
        caller: &signer,
        local_token: address,
        remote_token: address,
        amount: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BridgeErc20Args {
            local_token,
            remote_token,
            amount,
            min_gas_limit,
            extra_data,
        };

        let data = abi_encode_params(
            vector[135, 8, 118, 35],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BridgeErc20ToArgs {
        local_token: address,
        remote_token: address,
        to: address,
        amount: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    }

    public fun bridge_erc20_to(
        caller: &signer,
        local_token: address,
        remote_token: address,
        to: address,
        amount: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BridgeErc20ToArgs {
            local_token,
            remote_token,
            to,
            amount,
            min_gas_limit,
            extra_data,
        };

        let data = abi_encode_params(
            vector[84, 10, 191, 115],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BridgeEthArgs {
        min_gas_limit: u32,
        extra_data: vector<u8>,
    }

    public fun bridge_eth(
        caller: &signer,
        min_gas_limit: u32,
        extra_data: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = BridgeEthArgs {
            min_gas_limit,
            extra_data,
        };

        let data = abi_encode_params(
            vector[9, 252, 136, 67],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BridgeEthToArgs {
        to: address,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    }

    public fun bridge_eth_to(
        caller: &signer,
        to: address,
        min_gas_limit: u32,
        extra_data: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = BridgeEthToArgs {
            to,
            min_gas_limit,
            extra_data,
        };

        let data = abi_encode_params(
            vector[225, 16, 19, 221],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct DepositsArgs {
        key: address,
        key_2: address,
    }

    public fun deposits(
        caller: &signer,
        key: address,
        key_2: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DepositsArgs {
            key,
            key_2,
        };

        let data = abi_encode_params(
            vector[143, 96, 31, 102],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct FinalizeBridgeErc20Args {
        local_token: address,
        remote_token: address,
        from: address,
        to: address,
        amount: u256,
        extra_data: vector<u8>,
    }

    public fun finalize_bridge_erc20(
        caller: &signer,
        local_token: address,
        remote_token: address,
        from: address,
        to: address,
        amount: u256,
        extra_data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = FinalizeBridgeErc20Args {
            local_token,
            remote_token,
            from,
            to,
            amount,
            extra_data,
        };

        let data = abi_encode_params(
            vector[1, 102, 160, 122],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct FinalizeBridgeEthArgs {
        from: address,
        to: address,
        amount: u256,
        extra_data: vector<u8>,
    }

    public fun finalize_bridge_eth(
        caller: &signer,
        from: address,
        to: address,
        amount: u256,
        extra_data: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = FinalizeBridgeEthArgs {
            from,
            to,
            amount,
            extra_data,
        };

        let data = abi_encode_params(
            vector[22, 53, 245, 253],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct InitializeArgs {
        other_bridge: address,
    }

    public fun initialize(
        caller: &signer,
        other_bridge: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = InitializeArgs {
            other_bridge,
        };

        let data = abi_encode_params(
            vector[196, 214, 109, 232],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct L1TokenBridgeArgs {}

    public fun l1_token_bridge(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = L1TokenBridgeArgs {};

        let data = abi_encode_params(
            vector[54, 199, 23, 193],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
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
        let result = evm_call(caller, @L2StandardBridge, _value, data);
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
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct WithdrawArgs {
        l2_token: address,
        amount: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    }

    public fun withdraw(
        caller: &signer,
        l2_token: address,
        amount: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = WithdrawArgs {
            l2_token,
            amount,
            min_gas_limit,
            extra_data,
        };

        let data = abi_encode_params(
            vector[50, 183, 0, 109],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct WithdrawToArgs {
        l2_token: address,
        to: address,
        amount: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
    }

    public fun withdraw_to(
        caller: &signer,
        l2_token: address,
        to: address,
        amount: u256,
        min_gas_limit: u32,
        extra_data: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = WithdrawToArgs {
            l2_token,
            to,
            amount,
            min_gas_limit,
            extra_data,
        };

        let data = abi_encode_params(
            vector[163, 167, 149, 72],
            arg_struct,
        );
        let result = evm_call(caller, @L2StandardBridge, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
