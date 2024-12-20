module L1Block::l1_block {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct DepositorAccountArgs {}

    public fun depositor_account(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DepositorAccountArgs {};

        let data = abi_encode_params(
            vector[229, 145, 178, 130],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BaseFeeScalarArgs {}

    public fun base_fee_scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BaseFeeScalarArgs {};

        let data = abi_encode_params(
            vector[197, 152, 89, 24],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BasefeeArgs {}

    public fun basefee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BasefeeArgs {};

        let data = abi_encode_params(
            vector[92, 242, 73, 105],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BatcherHashArgs {}

    public fun batcher_hash(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BatcherHashArgs {};

        let data = abi_encode_params(
            vector[232, 27, 44, 109],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BlobBaseFeeArgs {}

    public fun blob_base_fee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BlobBaseFeeArgs {};

        let data = abi_encode_params(
            vector[248, 32, 97, 64],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BlobBaseFeeScalarArgs {}

    public fun blob_base_fee_scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BlobBaseFeeScalarArgs {};

        let data = abi_encode_params(
            vector[104, 213, 220, 166],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GasPayingTokenArgs {}

    public fun gas_paying_token(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GasPayingTokenArgs {};

        let data = abi_encode_params(
            vector[67, 151, 223, 239],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GasPayingTokenNameArgs {}

    public fun gas_paying_token_name(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GasPayingTokenNameArgs {};

        let data = abi_encode_params(
            vector[216, 68, 71, 21],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GasPayingTokenSymbolArgs {}

    public fun gas_paying_token_symbol(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GasPayingTokenSymbolArgs {};

        let data = abi_encode_params(
            vector[85, 15, 205, 201],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct HashArgs {}

    public fun hash(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = HashArgs {};

        let data = abi_encode_params(
            vector[9, 189, 90, 96],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct IsCustomGasTokenArgs {}

    public fun is_custom_gas_token(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = IsCustomGasTokenArgs {};

        let data = abi_encode_params(
            vector[33, 50, 104, 73],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct L1FeeOverheadArgs {}

    public fun l1_fee_overhead(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = L1FeeOverheadArgs {};

        let data = abi_encode_params(
            vector[139, 35, 159, 115],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct L1FeeScalarArgs {}

    public fun l1_fee_scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = L1FeeScalarArgs {};

        let data = abi_encode_params(
            vector[158, 140, 73, 102],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct NumberArgs {}

    public fun number(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = NumberArgs {};

        let data = abi_encode_params(
            vector[131, 129, 245, 138],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SequenceNumberArgs {}

    public fun sequence_number(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SequenceNumberArgs {};

        let data = abi_encode_params(
            vector[100, 202, 35, 239],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetGasPayingTokenArgs {
        token: address,
        decimals: u8,
        name: vector<u8>,
        symbol: vector<u8>,
    }

    public fun set_gas_paying_token(
        caller: &signer,
        token: address,
        decimals: u8,
        name: vector<u8>,
        symbol: vector<u8>,
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
        hash: vector<u8>,
        sequence_number: u64,
        batcher_hash: vector<u8>,
        l1_fee_overhead: u256,
        l1_fee_scalar: u256,
    }

    public fun set_l1_block_values(
        caller: &signer,
        number: u64,
        timestamp: u64,
        basefee: u256,
        hash: vector<u8>,
        sequence_number: u64,
        batcher_hash: vector<u8>,
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

    struct SetL1BlockValuesEcotoneArgs {}

    public fun set_l1_block_values_ecotone(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetL1BlockValuesEcotoneArgs {};

        let data = abi_encode_params(
            vector[68, 10, 94, 32],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct TimestampArgs {}

    public fun timestamp(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = TimestampArgs {};

        let data = abi_encode_params(
            vector[184, 7, 119, 234],
            arg_struct,
        );
        let result = evm_call(caller, @L1Block, _value, data);
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
        let result = evm_call(caller, @L1Block, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
