module BaseFeeVault::base_fee_vault {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct MIN_WITHDRAWAL_AMOUNTArgs {}

    public fun MIN_WITHDRAWAL_AMOUNT(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MIN_WITHDRAWAL_AMOUNTArgs {};

        let data = abi_encode_params(
            vector[211, 229, 121, 43],
            arg_struct,
        );
        let result = evm_call(caller, @BaseFeeVault, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RECIPIENTArgs {}

    public fun RECIPIENT(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = RECIPIENTArgs {};

        let data = abi_encode_params(
            vector[13, 144, 25, 225],
            arg_struct,
        );
        let result = evm_call(caller, @BaseFeeVault, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct WITHDRAWAL_NETWORKArgs {}

    public fun WITHDRAWAL_NETWORK(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = WITHDRAWAL_NETWORKArgs {};

        let data = abi_encode_params(
            vector[208, 225, 47, 144],
            arg_struct,
        );
        let result = evm_call(caller, @BaseFeeVault, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct TotalProcessedArgs {}

    public fun total_processed(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = TotalProcessedArgs {};

        let data = abi_encode_params(
            vector[132, 65, 29, 101],
            arg_struct,
        );
        let result = evm_call(caller, @BaseFeeVault, _value, data);
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
        let result = evm_call(caller, @BaseFeeVault, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct WithdrawArgs {}

    public fun withdraw(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = WithdrawArgs {};

        let data = abi_encode_params(
            vector[60, 207, 214, 11],
            arg_struct,
        );
        let result = evm_call(caller, @BaseFeeVault, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
