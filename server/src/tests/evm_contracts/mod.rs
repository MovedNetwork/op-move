use alloy::{
    consensus::{SignableTransaction, TxEip1559, TxEnvelope},
    network::TxSignerSync,
    primitives::{Address, TxKind},
    rpc::types::TransactionRequest,
    signers::local::PrivateKeySigner,
};

mod account_storage;
mod block_env;
mod blockhash;

fn deploy_evm_contract(chain_id: u64, bytecode: &[u8]) -> TxEnvelope {
    let signer = PrivateKeySigner::random();
    sign_transaction(chain_id, TxKind::Create, bytecode.to_vec(), &signer)
}

fn call_contract(chain_id: u64, to: Address, evm_input: Vec<u8>) -> TxEnvelope {
    let signer = PrivateKeySigner::random();
    sign_transaction(chain_id, TxKind::Call(to), evm_input, &signer)
}

fn view_contract(to: Address, evm_input: Vec<u8>) -> TransactionRequest {
    let from = Address::random();
    TransactionRequest::default()
        .to(to)
        .from(from)
        .input(evm_input.into())
}

fn sign_transaction(
    chain_id: u64,
    to: TxKind,
    input: Vec<u8>,
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
        input: input.into(),
    };
    let signature = signer.sign_transaction_sync(&mut tx).unwrap();
    TxEnvelope::Eip1559(tx.into_signed(signature))
}
