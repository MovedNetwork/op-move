module GasPriceOracle::gas_price_oracle {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct DecimalsArgs {}

    public fun decimals(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DecimalsArgs {};

        let data = abi_encode_params(
            vector[46, 15, 38, 37],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BaseFeeArgs {}

    public fun base_fee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BaseFeeArgs {};

        let data = abi_encode_params(
            vector[110, 242, 92, 58],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
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
        let result = evm_call(caller, @GasPriceOracle, _value, data);
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
        let result = evm_call(caller, @GasPriceOracle, _value, data);
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
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GasPriceArgs {}

    public fun gas_price(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GasPriceArgs {};

        let data = abi_encode_params(
            vector[254, 23, 59, 151],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetL1FeeArgs {
        data: vector<u8>,
    }

    public fun get_l1_fee(
        caller: &signer,
        data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetL1FeeArgs {
            data,
        };

        let data = abi_encode_params(
            vector[73, 148, 142, 14],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetL1FeeUpperBoundArgs {
        unsigned_tx_size: u256,
    }

    public fun get_l1_fee_upper_bound(
        caller: &signer,
        unsigned_tx_size: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetL1FeeUpperBoundArgs {
            unsigned_tx_size,
        };

        let data = abi_encode_params(
            vector[241, 199, 165, 139],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetL1GasUsedArgs {
        data: vector<u8>,
    }

    public fun get_l1_gas_used(
        caller: &signer,
        data: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetL1GasUsedArgs {
            data,
        };

        let data = abi_encode_params(
            vector[222, 38, 196, 161],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct IsEcotoneArgs {}

    public fun is_ecotone(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = IsEcotoneArgs {};

        let data = abi_encode_params(
            vector[78, 246, 226, 36],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct IsFjordArgs {}

    public fun is_fjord(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = IsFjordArgs {};

        let data = abi_encode_params(
            vector[150, 14, 58, 35],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct L1BaseFeeArgs {}

    public fun l1_base_fee(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = L1BaseFeeArgs {};

        let data = abi_encode_params(
            vector[81, 155, 75, 211],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct OverheadArgs {}

    public fun overhead(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = OverheadArgs {};

        let data = abi_encode_params(
            vector[12, 24, 193, 98],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ScalarArgs {}

    public fun scalar(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = ScalarArgs {};

        let data = abi_encode_params(
            vector[244, 94, 101, 216],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetEcotoneArgs {}

    public fun set_ecotone(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetEcotoneArgs {};

        let data = abi_encode_params(
            vector[34, 185, 10, 179],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetFjordArgs {}

    public fun set_fjord(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetFjordArgs {};

        let data = abi_encode_params(
            vector[142, 152, 177, 6],
            arg_struct,
        );
        let result = evm_call(caller, @GasPriceOracle, _value, data);
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
        let result = evm_call(caller, @GasPriceOracle, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
