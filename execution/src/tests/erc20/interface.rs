use super::*;

pub trait Erc20Token {
    fn balance_of(ctx: &TestContext, token_address: Address, account_address: Address) -> U256;

    fn transfer(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        to_address: Address,
        transfer_amount: U256,
    ) -> EvmNativeOutcome;

    fn allowance(
        ctx: &TestContext,
        token_address: Address,
        owner_address: Address,
        spender_address: Address,
    ) -> U256;

    fn approve(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        spender_address: Address,
        approve_amount: U256,
    ) -> EvmNativeOutcome;

    fn transfer_from(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        from_address: Address,
        to_address: Address,
        transfer_amount: U256,
    ) -> EvmNativeOutcome;

    fn total_supply(ctx: &TestContext, token_address: Address) -> U256;

    fn decimals(ctx: &TestContext, token_address: Address) -> u8;

    fn name(ctx: &TestContext, token_address: Address) -> String;

    fn symbol(ctx: &TestContext, token_address: Address) -> String;
}
