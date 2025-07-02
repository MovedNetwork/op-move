use super::*;

/// Implementation of the ERC-20 interface using Move types.
pub struct MoveImpl;

impl interface::Erc20Token for MoveImpl {
    fn balance_of(ctx: &TestContext, token_address: Address, account_address: Address) -> U256 {
        let outcome = ctx
            .quick_call(
                vec![
                    MoveValue::Address(token_address.to_move_address()),
                    MoveValue::Address(account_address.to_move_address()),
                ],
                "erc20",
                "balance_of",
            )
            .0;
        U256::from_be_slice(&outcome.output)
    }

    fn transfer(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        to_address: Address,
        transfer_amount: U256,
    ) -> EvmNativeOutcome {
        ctx.quick_send(
            vec![
                MoveValue::Signer(caller_address.to_move_address()),
                MoveValue::Address(token_address.to_move_address()),
                MoveValue::Address(to_address.to_move_address()),
                MoveValue::U256(transfer_amount.to_move_u256()),
            ],
            "erc20",
            "transfer",
        )
    }

    fn allowance(
        ctx: &TestContext,
        token_address: Address,
        owner_address: Address,
        spender_address: Address,
    ) -> U256 {
        let outcome = ctx
            .quick_call(
                vec![
                    MoveValue::Address(token_address.to_move_address()),
                    MoveValue::Address(owner_address.to_move_address()),
                    MoveValue::Address(spender_address.to_move_address()),
                ],
                "erc20",
                "allowance",
            )
            .0;
        U256::from_be_slice(&outcome.output)
    }

    fn approve(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        spender_address: Address,
        approve_amount: U256,
    ) -> EvmNativeOutcome {
        ctx.quick_send(
            vec![
                MoveValue::Signer(caller_address.to_move_address()),
                MoveValue::Address(token_address.to_move_address()),
                MoveValue::Address(spender_address.to_move_address()),
                MoveValue::U256(approve_amount.to_move_u256()),
            ],
            "erc20",
            "approve",
        )
    }

    fn transfer_from(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        from_address: Address,
        to_address: Address,
        transfer_amount: U256,
    ) -> EvmNativeOutcome {
        ctx.quick_send(
            vec![
                MoveValue::Signer(caller_address.to_move_address()),
                MoveValue::Address(token_address.to_move_address()),
                MoveValue::Address(from_address.to_move_address()),
                MoveValue::Address(to_address.to_move_address()),
                MoveValue::U256(transfer_amount.to_move_u256()),
            ],
            "erc20",
            "transfer_from",
        )
    }

    fn total_supply(ctx: &TestContext, token_address: Address) -> U256 {
        let outcome = ctx
            .quick_call(
                vec![MoveValue::Address(token_address.to_move_address())],
                "erc20",
                "total_supply",
            )
            .0;
        U256::from_be_slice(&outcome.output)
    }

    fn decimals(ctx: &TestContext, token_address: Address) -> u8 {
        let outcome = ctx
            .quick_call(
                vec![MoveValue::Address(token_address.to_move_address())],
                "erc20",
                "decimals",
            )
            .0;
        let val = U256::from_be_slice(&outcome.output);
        val.as_limbs()[0] as u8
    }

    fn name(ctx: &TestContext, token_address: Address) -> String {
        let outcome = ctx
            .quick_call(
                vec![MoveValue::Address(token_address.to_move_address())],
                "erc20",
                "name",
            )
            .0;
        let name = outcome.output;
        String::abi_decode(&name, true).unwrap()
    }

    fn symbol(ctx: &TestContext, token_address: Address) -> String {
        let outcome = ctx
            .quick_call(
                vec![MoveValue::Address(token_address.to_move_address())],
                "erc20",
                "symbol",
            )
            .0;
        let symbol = outcome.output;
        String::abi_decode(&symbol, true).unwrap()
    }
}
