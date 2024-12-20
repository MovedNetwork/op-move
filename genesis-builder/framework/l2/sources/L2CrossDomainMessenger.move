module L2CrossDomainMessenger::l2_cross_domain_messenger {
    use aptos_framework::fungible_asset_u256::{FungibleAsset, zero};
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct MessageVersionArgs {}

    public fun message_version(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MessageVersionArgs {};

        let data = abi_encode_params(
            vector[63, 130, 122, 90],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct MinGasCalldataOverheadArgs {}

    public fun min_gas_calldata_overhead(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MinGasCalldataOverheadArgs {};

        let data = abi_encode_params(
            vector[2, 143, 133, 247],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct MinGasDynamicOverheadDenominatorArgs {}

    public fun min_gas_dynamic_overhead_denominator(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MinGasDynamicOverheadDenominatorArgs {};

        let data = abi_encode_params(
            vector[12, 86, 132, 152],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct MinGasDynamicOverheadNumeratorArgs {}

    public fun min_gas_dynamic_overhead_numerator(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MinGasDynamicOverheadNumeratorArgs {};

        let data = abi_encode_params(
            vector[40, 40, 215, 232],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct OtherMessengerArgs {}

    public fun other_messenger(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = OtherMessengerArgs {};

        let data = abi_encode_params(
            vector[159, 206, 129, 44],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RelayCallOverheadArgs {}

    public fun relay_call_overhead(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = RelayCallOverheadArgs {};

        let data = abi_encode_params(
            vector[76, 29, 106, 105],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RelayConstantOverheadArgs {}

    public fun relay_constant_overhead(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = RelayConstantOverheadArgs {};

        let data = abi_encode_params(
            vector[131, 167, 64, 116],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RelayGasCheckBufferArgs {}

    public fun relay_gas_check_buffer(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = RelayGasCheckBufferArgs {};

        let data = abi_encode_params(
            vector[86, 68, 207, 223],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RelayReservedGasArgs {}

    public fun relay_reserved_gas(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = RelayReservedGasArgs {};

        let data = abi_encode_params(
            vector[140, 190, 238, 242],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BaseGasArgs {
        message: vector<u8>,
        min_gas_limit: u32,
    }

    public fun base_gas(
        caller: &signer,
        message: vector<u8>,
        min_gas_limit: u32,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BaseGasArgs {
            message,
            min_gas_limit,
        };

        let data = abi_encode_params(
            vector[178, 138, 222, 37],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct FailedMessagesArgs {
        key: vector<u8>,
    }

    public fun failed_messages(
        caller: &signer,
        key: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = FailedMessagesArgs {
            key,
        };

        let data = abi_encode_params(
            vector[164, 231, 248, 189],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct InitializeArgs {
        l1_cross_domain_messenger: address,
    }

    public fun initialize(
        caller: &signer,
        l1_cross_domain_messenger: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = InitializeArgs {
            l1_cross_domain_messenger,
        };

        let data = abi_encode_params(
            vector[196, 214, 109, 232],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct L1CrossDomainMessengerArgs {}

    public fun l1_cross_domain_messenger(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = L1CrossDomainMessengerArgs {};

        let data = abi_encode_params(
            vector[167, 17, 152, 105],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct MessageNonceArgs {}

    public fun message_nonce(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MessageNonceArgs {};

        let data = abi_encode_params(
            vector[236, 199, 4, 40],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
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
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RelayMessageArgs {
        nonce: u256,
        sender: address,
        target: address,
        value: u256,
        min_gas_limit: u256,
        message: vector<u8>,
    }

    public fun relay_message(
        caller: &signer,
        nonce: u256,
        sender: address,
        target: address,
        value: u256,
        min_gas_limit: u256,
        message: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = RelayMessageArgs {
            nonce,
            sender,
            target,
            value,
            min_gas_limit,
            message,
        };

        let data = abi_encode_params(
            vector[215, 100, 173, 11],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SendMessageArgs {
        target: address,
        message: vector<u8>,
        min_gas_limit: u32,
    }

    public fun send_message(
        caller: &signer,
        target: address,
        message: vector<u8>,
        min_gas_limit: u32,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = SendMessageArgs {
            target,
            message,
            min_gas_limit,
        };

        let data = abi_encode_params(
            vector[61, 187, 32, 43],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SuccessfulMessagesArgs {
        key: vector<u8>,
    }

    public fun successful_messages(
        caller: &signer,
        key: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SuccessfulMessagesArgs {
            key,
        };

        let data = abi_encode_params(
            vector[177, 177, 178, 9],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
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
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct XDomainMessageSenderArgs {}

    public fun x_domain_message_sender(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = XDomainMessageSenderArgs {};

        let data = abi_encode_params(
            vector[110, 41, 110, 69],
            arg_struct,
        );
        let result = evm_call(caller, @L2CrossDomainMessenger, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
