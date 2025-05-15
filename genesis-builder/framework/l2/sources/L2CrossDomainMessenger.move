module L2CrossDomainMessenger::l2_cross_domain_messenger {
    use aptos_framework::fungible_asset_u256::{FungibleAsset, zero};
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, evm_view, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun message_version(
    ): EvmResult {
        let data = vector[63, 130, 122, 90];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun min_gas_calldata_overhead(
    ): EvmResult {
        let data = vector[2, 143, 133, 247];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun min_gas_dynamic_overhead_denominator(
    ): EvmResult {
        let data = vector[12, 86, 132, 152];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun min_gas_dynamic_overhead_numerator(
    ): EvmResult {
        let data = vector[40, 40, 215, 232];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun other_messenger(
    ): EvmResult {
        let data = vector[159, 206, 129, 44];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun relay_call_overhead(
    ): EvmResult {
        let data = vector[76, 29, 106, 105];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun relay_constant_overhead(
    ): EvmResult {
        let data = vector[131, 167, 64, 116];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun relay_gas_check_buffer(
    ): EvmResult {
        let data = vector[86, 68, 207, 223];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun relay_reserved_gas(
    ): EvmResult {
        let data = vector[140, 190, 238, 242];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BaseGasArgs {
        message: vector<u8>,
        min_gas_limit: u32,
    }

    public fun base_gas(
        message: vector<u8>,
        min_gas_limit: u32,
    ): EvmResult {
        let arg_struct = BaseGasArgs {
            message,
            min_gas_limit,
        };

        let data = abi_encode_params(
            vector[178, 138, 222, 37],
            arg_struct,
        );

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct FailedMessagesArgs {
        key: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    }

    public fun failed_messages(
        key: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    ): EvmResult {
        let arg_struct = FailedMessagesArgs {
            key,
        };

        let data = abi_encode_params(
            vector[164, 231, 248, 189],
            arg_struct,
        );

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
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


    public fun l1_cross_domain_messenger(
    ): EvmResult {
        let data = vector[167, 17, 152, 105];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun message_nonce(
    ): EvmResult {
        let data = vector[236, 199, 4, 40];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun paused(
    ): EvmResult {
        let data = vector[92, 151, 90, 187];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
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
        key: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    }

    public fun successful_messages(
        key: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    ): EvmResult {
        let arg_struct = SuccessfulMessagesArgs {
            key,
        };

        let data = abi_encode_params(
            vector[177, 177, 178, 9],
            arg_struct,
        );

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
    ): EvmResult {
        let data = vector[84, 253, 77, 80];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun x_domain_message_sender(
    ): EvmResult {
        let data = vector[110, 41, 110, 69];

        let result = evm_view(@0x0, @L2CrossDomainMessenger, 0, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
