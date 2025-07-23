use {
    super::*,
    alloy::rpc::types::TransactionRequest,
    umi_evm_ext::erc20::{
        Erc20Methods,
        abi_bindings::Erc20::{
            allowanceCall, approveCall, balanceOfCall, decimalsCall, nameCall, symbolCall,
            totalSupplyCall, transferCall, transferFromCall,
        },
    },
};

/// Implementation of the ERC-20 interface using the Solidity ABI directly.
pub struct TxImpl;

impl interface::Erc20Token for TxImpl {
    fn balance_of(ctx: &TestContext, token_address: Address, account_address: Address) -> U256 {
        let method = Erc20Methods::BalanceOf(balanceOfCall {
            owner: account_address,
        });
        let output = erc20_view_call(ctx, token_address, method);
        U256::from_be_slice(&output)
    }

    fn transfer(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        to_address: Address,
        transfer_amount: U256,
    ) -> EvmNativeOutcome {
        let method = Erc20Methods::Transfer(transferCall {
            to: to_address,
            amount: transfer_amount,
        });
        execute_erc20_method(ctx, caller_address, token_address, method)
    }

    fn allowance(
        ctx: &TestContext,
        token_address: Address,
        owner_address: Address,
        spender_address: Address,
    ) -> U256 {
        let method = Erc20Methods::Allowance(allowanceCall {
            owner: owner_address,
            spender: spender_address,
        });
        let output = erc20_view_call(ctx, token_address, method);
        U256::from_be_slice(&output)
    }

    fn approve(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        spender_address: Address,
        approve_amount: U256,
    ) -> EvmNativeOutcome {
        let method = Erc20Methods::Approve(approveCall {
            spender: spender_address,
            amount: approve_amount,
        });
        execute_erc20_method(ctx, caller_address, token_address, method)
    }

    fn transfer_from(
        ctx: &mut TestContext,
        caller_address: Address,
        token_address: Address,
        from_address: Address,
        to_address: Address,
        transfer_amount: U256,
    ) -> EvmNativeOutcome {
        let method = Erc20Methods::TransferFrom(transferFromCall {
            from: from_address,
            to: to_address,
            amount: transfer_amount,
        });
        execute_erc20_method(ctx, caller_address, token_address, method)
    }

    fn total_supply(ctx: &TestContext, token_address: Address) -> U256 {
        let method = Erc20Methods::TotalSupply(totalSupplyCall {});
        let output = erc20_view_call(ctx, token_address, method);
        U256::from_be_slice(&output)
    }

    fn decimals(ctx: &TestContext, token_address: Address) -> u8 {
        let method = Erc20Methods::Decimals(decimalsCall {});
        let output = erc20_view_call(ctx, token_address, method);
        U256::from_be_slice(&output).as_limbs()[0] as u8
    }

    fn name(ctx: &TestContext, token_address: Address) -> String {
        let method = Erc20Methods::Name(nameCall {});
        let output = erc20_view_call(ctx, token_address, method);
        String::abi_decode(&output, true).unwrap()
    }

    fn symbol(ctx: &TestContext, token_address: Address) -> String {
        let method = Erc20Methods::Symbol(symbolCall {});
        let output = erc20_view_call(ctx, token_address, method);
        String::abi_decode(&output, true).unwrap()
    }
}

fn execute_erc20_method(
    ctx: &mut TestContext,
    caller_address: Address,
    token_address: Address,
    method: Erc20Methods,
) -> EvmNativeOutcome {
    let tx: NormalizedEthTransaction = TransactionRequest::default()
        .from(caller_address)
        .nonce(ctx.get_nonce(caller_address))
        .to(token_address)
        .input(method.abi_encode().into())
        .into();
    let outcome = ctx
        .execute_tx(&TestTransaction::new(
            NormalizedExtendedTxEnvelope::Canonical(tx),
        ))
        .unwrap();
    ctx.state.apply(outcome.changes.move_vm).unwrap();
    ctx.evm_storage.apply(outcome.changes.evm).unwrap();

    EvmNativeOutcome {
        is_success: outcome.vm_outcome.is_ok(),
        output: Vec::new(),
        logs: outcome.logs,
    }
}

fn erc20_view_call(ctx: &TestContext, token_address: Address, method: Erc20Methods) -> Vec<u8> {
    let request = TransactionRequest::default()
        .to(token_address)
        .input(method.abi_encode().into());
    crate::simulate::call_transaction(
        request,
        ctx.state.resolver(),
        &ctx.evm_storage,
        Default::default(),
        &ctx.genesis_config,
        &(),
        &(),
    )
    .unwrap()
}
