use {
    alloy_consensus::TxEnvelope,
    alloy_primitives::{Address, Bytes, B256, U256, U64},
    alloy_rlp::{Buf, Decodable, Encodable, RlpDecodable, RlpEncodable},
    serde::{Deserialize, Serialize},
};

const DEPOSITED_TYPE_BYTE: u8 = 0x7e;

/// OP-stack special transactions defined in
/// https://specs.optimism.io/protocol/deposits.html#the-deposited-transaction-type
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize, RlpDecodable, RlpEncodable)]
pub struct DepositedTx {
    pub source_hash: B256,
    pub from: Address,
    pub to: Address,
    pub mint: U256,
    pub value: U256,
    pub gas: U64,
    pub is_system_tx: bool,
    pub data: Bytes,
}

/// Same as `alloy_consensus::TxEnvelope` except extended to
/// include the new Deposited transaction defined in OP-stack.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum ExtendedTxEnvelope {
    Canonical(TxEnvelope),
    DepositedTx(DepositedTx),
}

impl Encodable for ExtendedTxEnvelope {
    fn length(&self) -> usize {
        match self {
            Self::Canonical(tx) => tx.length(),
            Self::DepositedTx(tx) => tx.length() + 1,
        }
    }

    fn encode(&self, out: &mut dyn alloy_rlp::BufMut) {
        match self {
            Self::Canonical(tx) => {
                // For some reason Alloy double encodes the transaction
                // by default. So we use their default method then decode
                // one level.
                let mut buf = Vec::with_capacity(tx.length());
                tx.encode(&mut buf);
                let bytes = Bytes::decode(&mut buf.as_slice()).expect("Must be RLP decodable");
                out.put_slice(&bytes);
            }
            Self::DepositedTx(tx) => {
                out.put_u8(DEPOSITED_TYPE_BYTE);
                tx.encode(out);
            }
        }
    }
}

impl Decodable for ExtendedTxEnvelope {
    fn decode(buf: &mut &[u8]) -> alloy_rlp::Result<Self> {
        match buf.first().copied() {
            Some(DEPOSITED_TYPE_BYTE) => {
                buf.advance(1);
                let tx = DepositedTx::decode(buf)?;
                Ok(Self::DepositedTx(tx))
            }
            _ => {
                let tx = TxEnvelope::decode(buf)?;
                Ok(Self::Canonical(tx))
            }
        }
    }
}

#[test]
fn test_extended_tx_envelope_rlp() {
    use std::str::FromStr;

    // Deposited Transaction
    rlp_roundtrip(&Bytes::from_str("0x7ef8f8a0672dfee56b1754d9fb99b11dae8eab6dfb7246470f6f7354d7acab837eab12b294deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000004000000006672f4bd000000000000020e00000000000000000000000000000000000000000000000000000000000000070000000000000000000000000000000000000000000000000000000000000001bc6d63f57e9fd865ae9a204a4db7fe1cff654377442541b06d020ddab88c2eeb000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425").unwrap());

    // Canonical Transaction
    rlp_roundtrip(&Bytes::from_str("0x02f86f82a45580808346a8928252089465d08a056c17ae13370565b04cf77d2afa1cb9fa8806f05b59d3b2000080c080a0dd50efde9a4d2f01f5248e1a983165c8cfa5f193b07b4b094f4078ad4717c1e4a017db1be1e8751b09e033bcffca982d0fe4919ff6b8594654e06647dee9292750").unwrap())
}

#[cfg(test)]
fn rlp_roundtrip(encoded: &[u8]) {
    let mut re_encoded = Vec::with_capacity(encoded.len());
    let mut slice = encoded;
    let tx = ExtendedTxEnvelope::decode(&mut slice).unwrap();
    tx.encode(&mut re_encoded);
    assert_eq!(re_encoded, encoded);
}