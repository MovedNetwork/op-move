module ERC20::base {
    use aptos_framework::object::{Self, Object};
    use aptos_framework::fungible_asset_u256::{Self, MintRef, TransferRef, BurnRef, Metadata};
    use aptos_framework::primary_fungible_store_u256;
    use std::error;
    use std::option;
    use std::signer;
    use std::string::utf8;

    /// Only fungible asset metadata owner can make changes.
    const ENOT_OWNER: u64 = 1;

    /// Hold refs to control the minting, transfer and burning of fungible assets.
    struct ManagedFungibleAsset has key {
        mint_ref: MintRef,
        transfer_ref: TransferRef,
        burn_ref: BurnRef,
    }

    struct TokenMetadata has drop {
        symbol: vector<u8>,
        name: vector<u8>,
        decimals: u8,
        icon_uri: vector<u8>,
        project_uri: vector<u8>,
    }

    public fun create_token_metadata(
        symbol: vector<u8>,
        name: vector<u8>,
        decimals: u8,
        icon_uri: vector<u8>,
        project_uri: vector<u8>,
    ): TokenMetadata {
        TokenMetadata { 
            symbol,
            name,
            decimals,
            icon_uri,
            project_uri,
        }
    }

    public fun init_token(
        admin: &signer,
        metadata: TokenMetadata
    ) {
        let constructor_ref = &object::create_named_object(admin, metadata.symbol);
        primary_fungible_store_u256::create_primary_store_enabled_fungible_asset(
            constructor_ref,
            option::none(),
            utf8(metadata.name),
            utf8(metadata.symbol),
            metadata.decimals,
            utf8(metadata.icon_uri),
            utf8(metadata.project_uri),
        );

        let mint_ref = fungible_asset_u256::generate_mint_ref(constructor_ref);
        let burn_ref = fungible_asset_u256::generate_burn_ref(constructor_ref);
        let transfer_ref = fungible_asset_u256::generate_transfer_ref(constructor_ref);
        let metadata_object_signer = object::generate_signer(constructor_ref);
        move_to(
            &metadata_object_signer,
            ManagedFungibleAsset { mint_ref, transfer_ref, burn_ref }
        );
    }

    public fun get_metadata(creator: address, symbol: vector<u8>): Object<Metadata> {
        let asset_address = object::create_object_address(&creator, symbol);
        object::address_to_object<Metadata>(asset_address)
    }

    public fun get_balance(account: address, asset: Object<Metadata>): u256 {
        primary_fungible_store_u256::balance(account, asset)
    }

    public fun mint(
        admin: &signer,
        to: address,
        amount: u256,
        asset: Object<Metadata>
    ) acquires ManagedFungibleAsset {
        let managed_fungible_asset = authorized_borrow_refs(admin, asset);
        let to_wallet = primary_fungible_store_u256::ensure_primary_store_exists(to, asset);
        let fa = fungible_asset_u256::mint(&managed_fungible_asset.mint_ref, amount);
        fungible_asset_u256::deposit_with_ref(&managed_fungible_asset.transfer_ref, to_wallet, fa);
    }

    public fun transfer(
        admin: &signer,
        from: address,
        to: address,
        amount: u256,
        asset: Object<Metadata>
    ) acquires ManagedFungibleAsset {
        let transfer_ref = &authorized_borrow_refs(admin, asset).transfer_ref;
        let from_wallet = primary_fungible_store_u256::primary_store(from, asset);
        let to_wallet = primary_fungible_store_u256::ensure_primary_store_exists(to, asset);
        let fa = fungible_asset_u256::withdraw_with_ref(transfer_ref, from_wallet, amount);
        fungible_asset_u256::deposit_with_ref(transfer_ref, to_wallet, fa);
    }

    public fun burn(
        admin: &signer,
        from: address,
        amount: u256,
        asset: Object<Metadata>
    ) acquires ManagedFungibleAsset {
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
    fun test_generic_token(admin: &signer, alice: address, bob: address) acquires ManagedFungibleAsset {
        // Initialize with test metadata
        let metadata = create_token_metadata(
            b"TEST",
            b"Test Token",
            18,
            b"",
            b"",
        );
        init_token(admin, metadata);
        
        let asset = get_metadata(@ERC20, b"TEST");
        assert!(get_balance(std::signer::address_of(admin), asset) == 0, 0);

        let mint_amount = 0x1000000000000000200000000000000030000000000000004;
        mint(admin, alice, mint_amount, asset);
        assert!(get_balance(alice, asset) == mint_amount, 1);

        let transfer_amount = 0x1000000000000000100000000000000010000000000000001;
        transfer(admin, alice, bob, transfer_amount, asset);
        assert!(get_balance(alice, asset) == mint_amount - transfer_amount, 2);
        assert!(get_balance(bob, asset) == transfer_amount, 3);

        let burn_amount = 0x100000000000000010000000000000001;
        burn(admin, alice, burn_amount, asset);
        assert!(get_balance(alice, asset) == mint_amount - transfer_amount - burn_amount, 4);
    }
} 
