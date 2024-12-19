module GovernanceToken::governance_token {
    use aptos_framework::fungible_asset_u256::zero;
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct DOMAIN_SEPARATORArgs {}

    public fun DOMAIN_SEPARATOR(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DOMAIN_SEPARATORArgs {};

        let data = abi_encode_params(
            vector[54, 68, 229, 21],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct AllowanceArgs {
        owner: address,
        spender: address,
    }

    public fun allowance(
        caller: &signer,
        owner: address,
        spender: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = AllowanceArgs {
            owner,
            spender,
        };

        let data = abi_encode_params(
            vector[221, 98, 237, 62],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ApproveArgs {
        spender: address,
        amount: u256,
    }

    public fun approve(
        caller: &signer,
        spender: address,
        amount: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = ApproveArgs {
            spender,
            amount,
        };

        let data = abi_encode_params(
            vector[9, 94, 167, 179],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BalanceOfArgs {
        account: address,
    }

    public fun balance_of(
        caller: &signer,
        account: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BalanceOfArgs {
            account,
        };

        let data = abi_encode_params(
            vector[112, 160, 130, 49],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BurnArgs {
        amount: u256,
    }

    public fun burn(
        caller: &signer,
        amount: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BurnArgs {
            amount,
        };

        let data = abi_encode_params(
            vector[66, 150, 108, 104],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct BurnFromArgs {
        account: address,
        amount: u256,
    }

    public fun burn_from(
        caller: &signer,
        account: address,
        amount: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = BurnFromArgs {
            account,
            amount,
        };

        let data = abi_encode_params(
            vector[121, 204, 103, 144],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct CheckpointsArgs {
        account: address,
        pos: u32,
    }

    public fun checkpoints(
        caller: &signer,
        account: address,
        pos: u32,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = CheckpointsArgs {
            account,
            pos,
        };

        let data = abi_encode_params(
            vector[241, 18, 126, 216],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct DecimalsArgs {}

    public fun decimals(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DecimalsArgs {};

        let data = abi_encode_params(
            vector[49, 60, 229, 103],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct DecreaseAllowanceArgs {
        spender: address,
        subtracted_value: u256,
    }

    public fun decrease_allowance(
        caller: &signer,
        spender: address,
        subtracted_value: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DecreaseAllowanceArgs {
            spender,
            subtracted_value,
        };

        let data = abi_encode_params(
            vector[164, 87, 194, 215],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct DelegateArgs {
        delegatee: address,
    }

    public fun delegate(
        caller: &signer,
        delegatee: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DelegateArgs {
            delegatee,
        };

        let data = abi_encode_params(
            vector[92, 25, 169, 92],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct DelegateBySigArgs {
        delegatee: address,
        nonce: u256,
        expiry: u256,
        v: u8,
        r: vector<u8>,
        s: vector<u8>,
    }

    public fun delegate_by_sig(
        caller: &signer,
        delegatee: address,
        nonce: u256,
        expiry: u256,
        v: u8,
        r: vector<u8>,
        s: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DelegateBySigArgs {
            delegatee,
            nonce,
            expiry,
            v,
            r,
            s,
        };

        let data = abi_encode_params(
            vector[195, 205, 165, 32],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct DelegatesArgs {
        account: address,
    }

    public fun delegates(
        caller: &signer,
        account: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = DelegatesArgs {
            account,
        };

        let data = abi_encode_params(
            vector[88, 124, 222, 30],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetPastTotalSupplyArgs {
        block_number: u256,
    }

    public fun get_past_total_supply(
        caller: &signer,
        block_number: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetPastTotalSupplyArgs {
            block_number,
        };

        let data = abi_encode_params(
            vector[142, 83, 158, 140],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetPastVotesArgs {
        account: address,
        block_number: u256,
    }

    public fun get_past_votes(
        caller: &signer,
        account: address,
        block_number: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetPastVotesArgs {
            account,
            block_number,
        };

        let data = abi_encode_params(
            vector[58, 70, 177, 168],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetVotesArgs {
        account: address,
    }

    public fun get_votes(
        caller: &signer,
        account: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetVotesArgs {
            account,
        };

        let data = abi_encode_params(
            vector[154, 178, 78, 176],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct IncreaseAllowanceArgs {
        spender: address,
        added_value: u256,
    }

    public fun increase_allowance(
        caller: &signer,
        spender: address,
        added_value: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = IncreaseAllowanceArgs {
            spender,
            added_value,
        };

        let data = abi_encode_params(
            vector[57, 80, 147, 81],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct MintArgs {
        account: address,
        amount: u256,
    }

    public fun mint(
        caller: &signer,
        account: address,
        amount: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = MintArgs {
            account,
            amount,
        };

        let data = abi_encode_params(
            vector[64, 193, 15, 25],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct NameArgs {}

    public fun name(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = NameArgs {};

        let data = abi_encode_params(
            vector[6, 253, 222, 3],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct NoncesArgs {
        owner: address,
    }

    public fun nonces(
        caller: &signer,
        owner: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = NoncesArgs {
            owner,
        };

        let data = abi_encode_params(
            vector[126, 206, 190, 0],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct NumCheckpointsArgs {
        account: address,
    }

    public fun num_checkpoints(
        caller: &signer,
        account: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = NumCheckpointsArgs {
            account,
        };

        let data = abi_encode_params(
            vector[111, 207, 255, 69],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct OwnerArgs {}

    public fun owner(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = OwnerArgs {};

        let data = abi_encode_params(
            vector[141, 165, 203, 91],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct PermitArgs {
        owner: address,
        spender: address,
        value: u256,
        deadline: u256,
        v: u8,
        r: vector<u8>,
        s: vector<u8>,
    }

    public fun permit(
        caller: &signer,
        owner: address,
        spender: address,
        value: u256,
        deadline: u256,
        v: u8,
        r: vector<u8>,
        s: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = PermitArgs {
            owner,
            spender,
            value,
            deadline,
            v,
            r,
            s,
        };

        let data = abi_encode_params(
            vector[213, 5, 172, 207],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct RenounceOwnershipArgs {}

    public fun renounce_ownership(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = RenounceOwnershipArgs {};

        let data = abi_encode_params(
            vector[113, 80, 24, 166],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SymbolArgs {}

    public fun symbol(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SymbolArgs {};

        let data = abi_encode_params(
            vector[149, 216, 155, 65],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct TotalSupplyArgs {}

    public fun total_supply(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = TotalSupplyArgs {};

        let data = abi_encode_params(
            vector[24, 22, 13, 221],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct TransferArgs {
        to: address,
        amount: u256,
    }

    public fun transfer(
        caller: &signer,
        to: address,
        amount: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = TransferArgs {
            to,
            amount,
        };

        let data = abi_encode_params(
            vector[169, 5, 156, 187],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct TransferFromArgs {
        from: address,
        to: address,
        amount: u256,
    }

    public fun transfer_from(
        caller: &signer,
        from: address,
        to: address,
        amount: u256,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = TransferFromArgs {
            from,
            to,
            amount,
        };

        let data = abi_encode_params(
            vector[35, 184, 114, 221],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct TransferOwnershipArgs {
        new_owner: address,
    }

    public fun transfer_ownership(
        caller: &signer,
        new_owner: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = TransferOwnershipArgs {
            new_owner,
        };

        let data = abi_encode_params(
            vector[242, 253, 227, 139],
            arg_struct,
        );
        let result = evm_call(caller, @GovernanceToken, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
