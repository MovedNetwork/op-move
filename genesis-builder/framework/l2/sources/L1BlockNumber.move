module L1BlockNumber::l1_block_number {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct GetL1BlockNumberArgs {}

    public fun get_l1_block_number(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetL1BlockNumberArgs {};

        let data = abi_encode_params(
            vector[185, 179, 239, 233],
            arg_struct,
        );
        let result = evm_call(caller, @L1BlockNumber, _value, data);
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
        let result = evm_call(caller, @L1BlockNumber, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
