module L1Block::l1_block {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun depositor_account(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[229, 145, 178, 130];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun base_fee_scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[197, 152, 89, 24];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun basefee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[92, 242, 73, 105];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun batcher_hash(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[232, 27, 44, 109];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun blob_base_fee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[248, 32, 97, 64];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun blob_base_fee_scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[104, 213, 220, 166];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun gas_paying_token(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[67, 151, 223, 239];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun gas_paying_token_name(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[216, 68, 71, 21];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun gas_paying_token_symbol(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[85, 15, 205, 201];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun hash(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[9, 189, 90, 96];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun is_custom_gas_token(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[33, 50, 104, 73];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun l1_fee_overhead(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[139, 35, 159, 115];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun l1_fee_scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[158, 140, 73, 102];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun number(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[131, 129, 245, 138];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun sequence_number(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[100, 202, 35, 239];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetGasPayingTokenArgs {
        token: address,
        decimals: u8,
        name: Evm::evm::SolidityFixedBytes<Evm::evm::B32>,
        symbol: Evm::evm::SolidityFixedBytes<Evm::evm::B32>,
    }

    public fun set_gas_paying_token(
        caller: &signer,
        token: address,
        decimals: u8,
        name: Evm::evm::SolidityFixedBytes<Evm::evm::B32>,
        symbol: Evm::evm::SolidityFixedBytes<Evm::evm::B32>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetGasPayingTokenArgs {
            token,
            decimals,
            name,
            symbol,
        };

        let data = abi_encode_params(
            vector[113, 207, 170, 63],
            arg_struct,
        );

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetL1BlockValuesArgs {
        number: u64,
        timestamp: u64,
        basefee: u256,
        hash: Evm::evm::SolidityFixedBytes<Evm::evm::B32>,
        sequence_number: u64,
        batcher_hash: Evm::evm::SolidityFixedBytes<Evm::evm::B32>,
        l1_fee_overhead: u256,
        l1_fee_scalar: u256,
    }

    public fun set_l1_block_values(
        caller: &signer,
        number: u64,
        timestamp: u64,
        basefee: u256,
        hash: Evm::evm::SolidityFixedBytes<Evm::evm::B32>,
        sequence_number: u64,
        batcher_hash: Evm::evm::SolidityFixedBytes<Evm::evm::B32>,
        l1_fee_overhead: u256,
        l1_fee_scalar: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetL1BlockValuesArgs {
            number,
            timestamp,
            basefee,
            hash,
            sequence_number,
            batcher_hash,
            l1_fee_overhead,
            l1_fee_scalar,
        };

        let data = abi_encode_params(
            vector[1, 93, 142, 185],
            arg_struct,
        );

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun set_l1_block_values_ecotone(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[68, 10, 94, 32];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun timestamp(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[184, 7, 119, 234];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[84, 253, 77, 80];

        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
