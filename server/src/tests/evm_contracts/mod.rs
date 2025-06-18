use {
    alloy::{
        consensus::{SignableTransaction, TxEip1559, TxEnvelope},
        network::TxSignerSync,
        primitives::{Address, TxKind},
        signers::local::PrivateKeySigner,
    },
    umi_execution::transaction::{ScriptOrDeployment, TransactionData},
};

mod account_storage;
mod blockhash;

fn deploy_evm_contract(chain_id: u64, bytecode: &[u8]) -> TxEnvelope {
    let signer = PrivateKeySigner::random();
    let input = ScriptOrDeployment::EvmContract(bytecode.to_vec());
    sign_transaction(
        chain_id,
        TxKind::Create,
        || bcs::to_bytes(&input).unwrap(),
        &signer,
    )
}

fn call_contract(chain_id: u64, to: Address, selector: [u8; 4]) -> TxEnvelope {
    let signer = PrivateKeySigner::random();
    let input = TransactionData::EvmContract {
        address: to,
        data: selector.to_vec(),
    };
    sign_transaction(
        chain_id,
        TxKind::Call(to),
        || input.to_bytes().unwrap(),
        &signer,
    )
}

fn sign_transaction<F: FnOnce() -> Vec<u8>>(
    chain_id: u64,
    to: TxKind,
    input: F,
    signer: &PrivateKeySigner,
) -> TxEnvelope {
    let mut tx = TxEip1559 {
        chain_id,
        nonce: 0,
        gas_limit: u64::MAX,
        max_fee_per_gas: 0,
        max_priority_fee_per_gas: 0,
        to,
        value: Default::default(),
        access_list: Default::default(),
        input: input().into(),
    };
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    TxEnvelope::Eip1559(tx.into_signed(signature))
}
