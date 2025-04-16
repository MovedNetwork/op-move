module L2ToL1MessagePasser::l2_to_l1_message_passer {
    use aptos_framework::fungible_asset_u256::{FungibleAsset, zero};
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;


    public fun message_version(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[63, 130, 122, 90];

        let result = evm_call(caller, @L2ToL1MessagePasser, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun burn(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[68, 223, 142, 112];

        let result = evm_call(caller, @L2ToL1MessagePasser, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct InitiateWithdrawalArgs {
        target: address,
        gas_limit: u256,
        data: vector<u8>,
    }

    public fun initiate_withdrawal(
        caller: &signer,
        target: address,
        gas_limit: u256,
        data: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = InitiateWithdrawalArgs {
            target,
            gas_limit,
            data,
        };

        let data = abi_encode_params(
            vector[194, 179, 229, 172],
            arg_struct,
        );

        let result = evm_call(caller, @L2ToL1MessagePasser, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun message_nonce(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[236, 199, 4, 40];

        let result = evm_call(caller, @L2ToL1MessagePasser, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SentMessagesArgs {
        key: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    }

    public fun sent_messages(
        caller: &signer,
        key: Evm::evm::SolidityFixedBytes<Evm::evm::U5<Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1, Evm::evm::B1>>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SentMessagesArgs {
            key,
        };

        let data = abi_encode_params(
            vector[130, 227, 112, 45],
            arg_struct,
        );

        let result = evm_call(caller, @L2ToL1MessagePasser, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }


    public fun version(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let data = vector[84, 253, 77, 80];

        let result = evm_call(caller, @L2ToL1MessagePasser, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
