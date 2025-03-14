module ERC20::xa0b86991c6218b36c1d19d4a2e9eb0ce3606eb48 {
    use ERC20::base::Self;
    use aptos_framework::object::Object;
    use aptos_framework::fungible_asset_u256::Metadata;

    const ASSET_SYMBOL: vector<u8> = b"USDC";
    const ASSET_NAME: vector<u8> = b"USD Coin";

    fun init_module(admin: &signer) {
        let metadata = base::create_token_metadata(
            ASSET_SYMBOL,
            ASSET_NAME,
            6,
            b"https://ethereum-optimism.github.io/data/USDC/logo.svg",
            b"https://www.circle.com/en/usdc",
        );
        base::init_token(admin, metadata);
    }

    public fun get_metadata(): Object<Metadata> {
        base::get_metadata(@ERC20, ASSET_SYMBOL)
    }

    public fun get_balance(account: address): u256 {
        base::get_balance(account, get_metadata())
    }

    public entry fun mint(admin: &signer, to: address, amount: u256) {
        base::mint(admin, to, amount, get_metadata())
    }

    public entry fun transfer(admin: &signer, from: address, to: address, amount: u256) {
        base::transfer(admin, from, to, amount, get_metadata())
    }

    public entry fun burn(admin: &signer, from: address, amount: u256) {
        base::burn(admin, from, amount, get_metadata())
    }

}
