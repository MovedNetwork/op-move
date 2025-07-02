use {
    self::abi_bindings::Erc20::{
        allowanceCall, approveCall, balanceOfCall, decimalsCall, nameCall, symbolCall,
        totalSupplyCall, transferCall, transferFromCall,
    },
    alloy::sol_types::SolCall,
};

pub mod abi_bindings {
    alloy::sol!(
        #[sol(rpc)]
        Erc20,
        "src/res/ERC20.json"
    );
}

pub enum Erc20Methods {
    BalanceOf(balanceOfCall),
    Transfer(transferCall),
    TransferFrom(transferFromCall),
    Approve(approveCall),
    Allowance(allowanceCall),
    TotalSupply(totalSupplyCall),
    Name(nameCall),
    Symbol(symbolCall),
    Decimals(decimalsCall),
}

impl Erc20Methods {
    pub fn try_parse(bytes: &[u8]) -> Option<Self> {
        if bytes.len() < 4 {
            return None;
        }
        let selector: [u8; 4] = bytes[0..4].try_into().ok()?;
        match selector {
            balanceOfCall::SELECTOR => balanceOfCall::abi_decode(bytes, true)
                .ok()
                .map(Self::BalanceOf),
            transferCall::SELECTOR => transferCall::abi_decode(bytes, true)
                .ok()
                .map(Self::Transfer),
            transferFromCall::SELECTOR => transferFromCall::abi_decode(bytes, true)
                .ok()
                .map(Self::TransferFrom),
            approveCall::SELECTOR => approveCall::abi_decode(bytes, true).ok().map(Self::Approve),
            allowanceCall::SELECTOR => allowanceCall::abi_decode(bytes, true)
                .ok()
                .map(Self::Allowance),
            totalSupplyCall::SELECTOR => totalSupplyCall::abi_decode(bytes, true)
                .ok()
                .map(Self::TotalSupply),
            nameCall::SELECTOR => nameCall::abi_decode(bytes, true).ok().map(Self::Name),
            symbolCall::SELECTOR => symbolCall::abi_decode(bytes, true).ok().map(Self::Symbol),
            decimalsCall::SELECTOR => decimalsCall::abi_decode(bytes, true)
                .ok()
                .map(Self::Decimals),
            _ => None,
        }
    }

    pub fn abi_encode(&self) -> Vec<u8> {
        match self {
            Erc20Methods::BalanceOf(balance_of_call) => balance_of_call.abi_encode(),
            Erc20Methods::Transfer(transfer_call) => transfer_call.abi_encode(),
            Erc20Methods::TransferFrom(transfer_from_call) => transfer_from_call.abi_encode(),
            Erc20Methods::Approve(approve_call) => approve_call.abi_encode(),
            Erc20Methods::Allowance(allowance_call) => allowance_call.abi_encode(),
            Erc20Methods::TotalSupply(total_supply_call) => total_supply_call.abi_encode(),
            Erc20Methods::Name(name_call) => name_call.abi_encode(),
            Erc20Methods::Symbol(symbol_call) => symbol_call.abi_encode(),
            Erc20Methods::Decimals(decimals_call) => decimals_call.abi_encode(),
        }
    }
}
