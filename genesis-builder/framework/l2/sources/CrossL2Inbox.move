module CrossL2Inbox::cross_l2_inbox {
    use aptos_framework::fungible_asset_u256::{FungibleAsset, zero};
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct Identifier {
        origin: address,
        block_number: u256,
        log_index: u256,
        timestamp: u256,
        chain_id: u256,
    }

    struct BlockNumberArgs {}

    public fun block_number(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BlockNumberArgs {};

        let data = abi_encode_params(
            vector[87, 232, 113, 231],
            arg_struct,
        );
        let result = evm_call(caller, @CrossL2Inbox, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ChainIdArgs {}

    public fun chain_id(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = ChainIdArgs {};

        let data = abi_encode_params(
            vector[154, 138, 5, 146],
            arg_struct,
        );
        let result = evm_call(caller, @CrossL2Inbox, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ExecuteMessageArgs {
        id: Identifier,
        target: address,
        message: vector<u8>,
    }

    public fun execute_message(
        caller: &signer,
        id: Identifier,
        target: address,
        message: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = ExecuteMessageArgs {
            id,
            target,
            message,
        };

        let data = abi_encode_params(
            vector[89, 132, 197, 62],
            arg_struct,
        );
        let result = evm_call(caller, @CrossL2Inbox, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct LogIndexArgs {}

    public fun log_index(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = LogIndexArgs {};

        let data = abi_encode_params(
            vector[218, 153, 247, 41],
            arg_struct,
        );
        let result = evm_call(caller, @CrossL2Inbox, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct OriginArgs {}

    public fun origin(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = OriginArgs {};

        let data = abi_encode_params(
            vector[147, 139, 95, 50],
            arg_struct,
        );
        let result = evm_call(caller, @CrossL2Inbox, _value, data);
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
        let result = evm_call(caller, @CrossL2Inbox, _value, data);
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
        let result = evm_call(caller, @CrossL2Inbox, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
