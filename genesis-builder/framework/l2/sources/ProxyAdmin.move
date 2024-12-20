module ProxyAdmin::proxy_admin {
    use aptos_framework::fungible_asset_u256::{FungibleAsset, zero};
    use EthToken::eth_token::get_metadata;
    use Evm::evm::{abi_encode_params, emit_evm_logs, evm_call, is_result_success, EvmResult};
    use std::error;

    const ENOT_SUCCESS: u64 = 1;

    struct AddressManagerArgs {}

    public fun address_manager(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = AddressManagerArgs {};

        let data = abi_encode_params(
            vector[58, 183, 110, 159],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ChangeProxyAdminArgs {
        proxy: address,
        new_admin: address,
    }

    public fun change_proxy_admin(
        caller: &signer,
        proxy: address,
        new_admin: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = ChangeProxyAdminArgs {
            proxy,
            new_admin,
        };

        let data = abi_encode_params(
            vector[126, 255, 39, 94],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetProxyAdminArgs {
        proxy: address,
    }

    public fun get_proxy_admin(
        caller: &signer,
        proxy: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetProxyAdminArgs {
            proxy,
        };

        let data = abi_encode_params(
            vector[243, 183, 222, 173],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct GetProxyImplementationArgs {
        proxy: address,
    }

    public fun get_proxy_implementation(
        caller: &signer,
        proxy: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = GetProxyImplementationArgs {
            proxy,
        };

        let data = abi_encode_params(
            vector[32, 78, 28, 122],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ImplementationNameArgs {
        key: address,
    }

    public fun implementation_name(
        caller: &signer,
        key: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = ImplementationNameArgs {
            key,
        };

        let data = abi_encode_params(
            vector[35, 129, 129, 174],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct IsUpgradingArgs {}

    public fun is_upgrading(
        caller: &signer,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = IsUpgradingArgs {};

        let data = abi_encode_params(
            vector[183, 148, 114, 98],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
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
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct ProxyTypeArgs {
        key: address,
    }

    public fun proxy_type(
        caller: &signer,
        key: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = ProxyTypeArgs {
            key,
        };

        let data = abi_encode_params(
            vector[107, 217, 245, 22],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
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
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetAddressArgs {
        name: vector<u8>,
        address: address,
    }

    public fun set_address(
        caller: &signer,
        name: vector<u8>,
        address: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetAddressArgs {
            name,
            address,
        };

        let data = abi_encode_params(
            vector[155, 46, 164, 189],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetAddressManagerArgs {
        address: address,
    }

    public fun set_address_manager(
        caller: &signer,
        address: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetAddressManagerArgs {
            address,
        };

        let data = abi_encode_params(
            vector[6, 82, 181, 122],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetImplementationNameArgs {
        address: address,
        name: vector<u8>,
    }

    public fun set_implementation_name(
        caller: &signer,
        address: address,
        name: vector<u8>,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetImplementationNameArgs {
            address,
            name,
        };

        let data = abi_encode_params(
            vector[134, 15, 124, 218],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetProxyTypeArgs {
        address: address,
        type: u8,
    }

    public fun set_proxy_type(
        caller: &signer,
        address: address,
        type: u8,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetProxyTypeArgs {
            address,
            type,
        };

        let data = abi_encode_params(
            vector[141, 82, 212, 160],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct SetUpgradingArgs {
        upgrading: bool,
    }

    public fun set_upgrading(
        caller: &signer,
        upgrading: bool,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = SetUpgradingArgs {
            upgrading,
        };

        let data = abi_encode_params(
            vector[7, 200, 247, 176],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
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
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct UpgradeArgs {
        proxy: address,
        implementation: address,
    }

    public fun upgrade(
        caller: &signer,
        proxy: address,
        implementation: address,
    ): EvmResult {
        let _value = zero(get_metadata());
        let arg_struct = UpgradeArgs {
            proxy,
            implementation,
        };

        let data = abi_encode_params(
            vector[153, 168, 142, 196],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }

    struct UpgradeAndCallArgs {
        proxy: address,
        implementation: address,
        data: vector<u8>,
    }

    public fun upgrade_and_call(
        caller: &signer,
        proxy: address,
        implementation: address,
        data: vector<u8>,
        _value: FungibleAsset,
    ): EvmResult {
        let arg_struct = UpgradeAndCallArgs {
            proxy,
            implementation,
            data,
        };

        let data = abi_encode_params(
            vector[150, 35, 96, 157],
            arg_struct,
        );
        let result = evm_call(caller, @ProxyAdmin, _value, data);
        assert!(is_result_success(&result), error::aborted(ENOT_SUCCESS));
        emit_evm_logs(&result);
        result
    }
}
