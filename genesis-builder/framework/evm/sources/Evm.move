module Evm::evm {
    use aptos_framework::event;
    use aptos_framework::fungible_asset_u256::{Self, FungibleAsset};
    use aptos_framework::primary_fungible_store_u256::ensure_primary_store_exists;
    use EthToken::eth_token::get_metadata;
    use std::error;
    use std::signer;

    /// For now deploying EVM contracts is restricted to an admin account.
    /// This restriction may be lifted in the future.
    const ENOT_OWNER: u64 = 1;

    /// Solidity FixedBytes must have length between 1 and 32 (inclusive).
    const EINVALID_FIXED_BYTES_SIZE: u64 = 2;

    const OWNER: address = @evm_admin;

    struct EvmLog has copy, store, drop {
        addr: address,
        topics: vector<u256>,
        data: vector<u8>
    }

    #[event]
    struct EvmLogsEvent has store, drop {
        logs: vector<EvmLog>
    }

    /// TODO: what capabilities should this have?
    struct EvmResult has drop {
        is_success: bool,
        output: vector<u8>,
        logs: vector<EvmLog>
    }

    // Marker byte size structs
    struct B1 {}
    struct B2 {}
    struct B4 {}
    struct B8 {}
    struct B16 {}
    struct B20 {}
    struct B32 {}

    /// Mark a byte array as being of fixed length for the purpose
    /// of encoding it into the Solidity ABI.
    struct SolidityFixedBytes<phantom S> has drop {
        data: vector<u8>,
    }

    /// Mark a collection of values as being of fixed length for the purpose
    /// of encoding it into the Solidity ABI.
    struct SolidityFixedArray<T> has drop {
        elements: vector<T>
    }

    public fun as_fixed_bytes<S>(data: vector<u8>): SolidityFixedBytes<S> {
        let actual_size = std::vector::length(&data);

        // Solidity ABI always pads fixed bytes to 32 bytes
        assert!(actual_size == 32, error::invalid_argument(EINVALID_FIXED_BYTES_SIZE));
    
        SolidityFixedBytes<S> { data }
    }

    public fun as_fixed_array<T>(elements: vector<T>): SolidityFixedArray<T> {
        SolidityFixedArray { elements }
    }

    /// Same as `evm_call`, but with the type signature modified to follow the rules of
    /// entry functions (namely: `value` must be zero because `FungibleAsset` cannot
    /// be exernally constructed, and there cannot be a return value).
    public entry fun entry_evm_call(
        caller: &signer, to: address, data: vector<u8>
    ) {
        let eth_metadata = get_metadata();
        let value = fungible_asset_u256::zero(eth_metadata);
        evm_call(caller, to, value, data);
    }

    public fun evm_call(
        caller: &signer,
        to: address,
        value: FungibleAsset,
        data: vector<u8>
    ): EvmResult {
        native_evm_call(
            signer::address_of(caller),
            to,
            get_asset_value(value),
            data
        )
    }

    public fun evm_create(
        caller: &signer, value: FungibleAsset, data: vector<u8>
    ): EvmResult {
        let caller_addr = signer::address_of(caller);
        assert!(caller_addr == OWNER, error::permission_denied(ENOT_OWNER));

        native_evm_create(caller_addr, get_asset_value(value), data)
    }

    /// Encode the move value into bytes using the Solidity ABI
    /// such that it would be suitable for passing to a Solidity contract's function.
    /// The prefix can be used to prepend the output with a Solidity 4-byte function
    /// selector if needed.
    public native fun abi_encode_params<T>(prefix: vector<u8>, value: T): vector<u8>;

    /// Decode the Solidity ABI bytes into move value
    /// such that it would be suitable for using Solidity contract's return value.
    public native fun abi_decode_params<T>(value: vector<u8>): T;

    /// View function for checking if EVM execution was successful.
    public fun is_result_success(result: &EvmResult): bool {
        result.is_success
    }

    /// View function to retrieve EVM execution output.
    public fun evm_output(result: &EvmResult): vector<u8> {
        result.output
    }

    /// Emit the EVM logs to MoveVM logging system
    public fun emit_evm_logs(result: &EvmResult) {
        event::emit(EvmLogsEvent { logs: result.logs });
    }

    fun get_asset_value(f: FungibleAsset): u256 {
        let amount = fungible_asset_u256::amount(&f);
        if (amount == 0) {
            fungible_asset_u256::destroy_zero(f);
            return 0
        };
        let eth_metadata = get_metadata();
        let store = ensure_primary_store_exists(OWNER, eth_metadata);
        fungible_asset_u256::deposit(store, f);
        amount
    }

    // A private function used by the system to call the EVM native.
    // (For some reason we cannot call the native function directly)
    fun system_evm_call(
        caller: address, to: address, value: u256, data: vector<u8>
    ): EvmResult {
        native_evm_call(caller, to, value, data)
    }

    native fun native_evm_call(
        caller: address, to: address, value: u256, data: vector<u8>
    ): EvmResult;
    native fun native_evm_create(
        caller: address, value: u256, data: vector<u8>
    ): EvmResult;
}
