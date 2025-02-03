module ERC20::xADE00C28244d5CE17D72E40330B1c318cD12B7c3 {
    use aptos_framework::object::{Self, Object};
    use aptos_framework::fungible_asset_u256::{Self, MintRef, TransferRef, BurnRef, Metadata};
    use aptos_framework::primary_fungible_store_u256;
    use std::error;
    use std::option;
    use std::signer;
    use std::string::utf8;

    /// Only fungible asset metadata owner can make changes.
    const ENOT_OWNER: u64 = 1;

    const ASSET_SYMBOL: vector<u8> = b"ADX";
    const ASSET_NAME: vector<u8> = b"AdEx Network";

    /// Hold refs to control the minting, transfer and burning of fungible assets.
    struct ManagedFungibleAsset has key {
        mint_ref: MintRef,
        transfer_ref: TransferRef,
        burn_ref: BurnRef,
    }

    fun init_module(admin: &signer) {
        let constructor_ref = &object::create_named_object(admin, ASSET_SYMBOL);
        primary_fungible_store_u256::create_primary_store_enabled_fungible_asset(
            constructor_ref,
            option::none(),
            utf8(ASSET_NAME), /* name */
            utf8(ASSET_SYMBOL), /* symbol */
            18, /* decimals */
            utf8(b"https://ethereum-optimism.github.io/data/ADEX/logo.svg"), /* icon */
            utf8(b"https://www.adex.network"), /* project */
        );

        // Create mint/burn/transfer refs to allow creator to manage the fungible asset.
        let mint_ref = fungible_asset_u256::generate_mint_ref(constructor_ref);
        let burn_ref = fungible_asset_u256::generate_burn_ref(constructor_ref);
        let transfer_ref = fungible_asset_u256::generate_transfer_ref(constructor_ref);
        let metadata_object_signer = object::generate_signer(constructor_ref);
        move_to(
            &metadata_object_signer,
            ManagedFungibleAsset { mint_ref, transfer_ref, burn_ref }
        );
    }

    /// Return the address of the managed fungible asset that's created when this module is deployed.
    public fun get_metadata(): Object<Metadata> {
        let asset_address = object::create_object_address(&@ERC20, ASSET_SYMBOL);
        object::address_to_object<Metadata>(asset_address)
    }

    public fun get_balance(account: address): u256 {
        let asset = get_metadata();
        primary_fungible_store_u256::balance(account, asset)
    }

    /// Mint as the owner of metadata object.
    /// but on Ethereum it would be `U256`. Maybe need to use a different token definition after all?
    public entry fun mint(admin: &signer, to: address, amount: u256) acquires ManagedFungibleAsset {
        let asset = get_metadata();
        let managed_fungible_asset = authorized_borrow_refs(admin, asset);
        let to_wallet = primary_fungible_store_u256::ensure_primary_store_exists(to, asset);
        let fa = fungible_asset_u256::mint(&managed_fungible_asset.mint_ref, amount);
        fungible_asset_u256::deposit_with_ref(&managed_fungible_asset.transfer_ref, to_wallet, fa);
    }

    /// Transfer as the owner of metadata object ignoring `frozen` field.
    public entry fun transfer(admin: &signer, from: address, to: address, amount: u256) acquires ManagedFungibleAsset {
        let asset = get_metadata();
        let transfer_ref = &authorized_borrow_refs(admin, asset).transfer_ref;
        let from_wallet = primary_fungible_store_u256::primary_store(from, asset);
        let to_wallet = primary_fungible_store_u256::ensure_primary_store_exists(to, asset);
        let fa = fungible_asset_u256::withdraw_with_ref(transfer_ref, from_wallet, amount);
        fungible_asset_u256::deposit_with_ref(transfer_ref, to_wallet, fa);
    }

    /// Burn fungible assets as the owner of metadata object.
    public entry fun burn(admin: &signer, from: address, amount: u256) acquires ManagedFungibleAsset {
        let asset = get_metadata();
        let burn_ref = &authorized_borrow_refs(admin, asset).burn_ref;
        let from_wallet = primary_fungible_store_u256::primary_store(from, asset);
        fungible_asset_u256::burn_from(burn_ref, from_wallet, amount);
    }

    inline fun authorized_borrow_refs(
        owner: &signer,
        asset: Object<Metadata>,
    ): &ManagedFungibleAsset acquires ManagedFungibleAsset {
        assert!(object::is_owner(asset, signer::address_of(owner)), error::permission_denied(ENOT_OWNER));
        borrow_global<ManagedFungibleAsset>(object::object_address(&asset))
    }

    #[test(admin = @ERC20, alice = @0xa11ce, bob = @0xb0b)]
    fun test_eth_token(admin: &signer, alice: address, bob: address) acquires ManagedFungibleAsset {
        init_module(admin);
        assert!(get_balance(std::signer::address_of(admin)) == 0, 0);

        let mint_amount = 0x1000000000000000200000000000000030000000000000004;
        mint(admin, alice, mint_amount);
        assert!(get_balance(alice) == mint_amount, 1);

        let transfer_amount = 0x1000000000000000100000000000000010000000000000001;
        transfer(admin, alice, bob, transfer_amount);
        assert!(get_balance(alice) == mint_amount - transfer_amount, 2);
        assert!(get_balance(bob) == transfer_amount, 3);

        let burn_amount = 0x100000000000000010000000000000001;
        burn(admin, alice, burn_amount);
        assert!(get_balance(alice) == mint_amount - transfer_amount - burn_amount, 4);
    }
}
