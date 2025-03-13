use {
    alloy::{
        consensus::{
            Receipt, ReceiptWithBloom, Signed, Transaction, TxEip1559, TxEip2930, TxEnvelope,
            TxLegacy,
        },
        eips::eip2930::AccessList,
        primitives::{address, Address, Bloom, Bytes, Log, LogData, TxKind, B256, U256, U64},
        rlp::{Buf, Decodable, Encodable, RlpDecodable, RlpEncodable},
        rpc::types::TransactionRequest,
    },
    aptos_types::transaction::{EntryFunction, Module, Script},
    move_core_types::{
        account_address::AccountAddress, effects::ChangeSet, language_storage::ModuleId,
    },
    moved_shared::{
        error::{Error, InvalidTransactionCause, UserError},
        primitives::ToMoveAddress,
    },
    op_alloy::consensus::{
        OpDepositReceipt, OpDepositReceiptWithBloom, OpReceiptEnvelope, OpTxEnvelope,
    },
    revm::primitives::keccak256,
    serde::{Deserialize, Serialize},
};

const DEPOSITED_TYPE_BYTE: u8 = 0x7e;
pub const L2_LOWEST_ADDRESS: Address = address!("4200000000000000000000000000000000000000");
pub const L2_HIGHEST_ADDRESS: Address = address!("42000000000000000000000000000000000000ff");

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
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
#[serde(untagged)]
pub enum ExtendedTxEnvelope {
    Canonical(TxEnvelope),
    DepositedTx(DepositedTx),
}

impl ExtendedTxEnvelope {
    pub fn compute_hash(&self) -> B256 {
        let mut buf = Vec::with_capacity(self.length());
        self.encode(&mut buf);
        keccak256(&mut buf)
    }

    pub fn sender(&self) -> Option<Address> {
        match self {
            Self::Canonical(tx) => tx.recover_signer().ok(),
            Self::DepositedTx(tx) => Some(tx.from),
        }
    }

    /// In case this transaction is a deposit, returns `Some` containing a reference to the
    /// underlying [`DepositedTx`]. Otherwise, returns `None`.
    pub fn as_deposited(&self) -> Option<&DepositedTx> {
        if let Self::DepositedTx(deposited_tx) = self {
            Some(deposited_tx)
        } else {
            None
        }
    }

    pub fn wrap_receipt(&self, receipt: Receipt, bloom: Bloom) -> OpReceiptEnvelope {
        match self {
            ExtendedTxEnvelope::Canonical(TxEnvelope::Legacy(_)) => {
                OpReceiptEnvelope::Legacy(ReceiptWithBloom {
                    receipt,
                    logs_bloom: bloom,
                })
            }
            ExtendedTxEnvelope::Canonical(TxEnvelope::Eip1559(_)) => {
                OpReceiptEnvelope::Eip1559(ReceiptWithBloom {
                    receipt,
                    logs_bloom: bloom,
                })
            }
            ExtendedTxEnvelope::Canonical(TxEnvelope::Eip2930(_)) => {
                OpReceiptEnvelope::Eip2930(ReceiptWithBloom {
                    receipt,
                    logs_bloom: bloom,
                })
            }
            ExtendedTxEnvelope::DepositedTx(_) => {
                OpReceiptEnvelope::Deposit(OpDepositReceiptWithBloom {
                    receipt: OpDepositReceipt {
                        inner: receipt,
                        // TODO: what are these fields supposed to be?
                        deposit_nonce: None,
                        deposit_receipt_version: None,
                    },
                    logs_bloom: bloom,
                })
            }
            ExtendedTxEnvelope::Canonical(_) => unreachable!("Not supported"),
        }
    }
}

impl From<ExtendedTxEnvelope> for OpTxEnvelope {
    fn from(value: ExtendedTxEnvelope) -> Self {
        match value {
            ExtendedTxEnvelope::Canonical(TxEnvelope::Legacy(tx)) => OpTxEnvelope::Legacy(tx),
            ExtendedTxEnvelope::Canonical(TxEnvelope::Eip1559(tx)) => OpTxEnvelope::Eip1559(tx),
            ExtendedTxEnvelope::Canonical(TxEnvelope::Eip2930(tx)) => OpTxEnvelope::Eip2930(tx),
            ExtendedTxEnvelope::Canonical(TxEnvelope::Eip7702(tx)) => OpTxEnvelope::Eip7702(tx),
            ExtendedTxEnvelope::DepositedTx(tx) => {
                let mint = if tx.mint.is_zero() {
                    None
                } else {
                    Some(tx.mint.saturating_to())
                };
                let deposit_tx = op_alloy::consensus::TxDeposit {
                    source_hash: tx.source_hash,
                    from: tx.from,
                    to: TxKind::Call(tx.to),
                    mint,
                    value: tx.value,
                    gas_limit: tx.gas.saturating_to(),
                    is_system_transaction: tx.is_system_tx,
                    input: tx.data,
                };
                deposit_tx.into()
            }
            ExtendedTxEnvelope::Canonical(_) => unreachable!("Not supported"),
        }
    }
}

impl Encodable for ExtendedTxEnvelope {
    fn length(&self) -> usize {
        match self {
            Self::Canonical(tx) => tx.length(),
            Self::DepositedTx(tx) => tx.length() + 1,
        }
    }

    fn encode(&self, out: &mut dyn alloy::rlp::BufMut) {
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
    fn decode(buf: &mut &[u8]) -> alloy::rlp::Result<Self> {
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

#[derive(Debug, Clone)]
pub enum NormalizedExtendedTxEnvelope {
    Canonical(NormalizedEthTransaction),
    DepositedTx(DepositedTx),
}

impl TryFrom<ExtendedTxEnvelope> for NormalizedExtendedTxEnvelope {
    type Error = Error;

    fn try_from(value: ExtendedTxEnvelope) -> Result<Self, Self::Error> {
        Ok(match value {
            ExtendedTxEnvelope::Canonical(tx) => {
                NormalizedExtendedTxEnvelope::Canonical(NormalizedEthTransaction::try_from(tx)?)
            }
            ExtendedTxEnvelope::DepositedTx(tx) => NormalizedExtendedTxEnvelope::DepositedTx(tx),
        })
    }
}

impl NormalizedExtendedTxEnvelope {
    pub fn into_canonical(self) -> Option<NormalizedEthTransaction> {
        if let Self::Canonical(tx) = self {
            Some(tx)
        } else {
            None
        }
    }

    pub fn tip_per_gas(&self, base_fee: U256) -> U256 {
        match self {
            Self::DepositedTx(..) => U256::ZERO,
            Self::Canonical(tx) => tx.tip_per_gas(base_fee),
        }
    }

    pub fn gas_limit(&self) -> u64 {
        match self {
            Self::DepositedTx(..) => 0,
            Self::Canonical(tx) => tx.gas_limit(),
        }
    }

    pub fn effective_gas_price(&self, base_fee: U256) -> U256 {
        match self {
            Self::DepositedTx(..) => U256::ZERO,
            Self::Canonical(tx) => tx.effective_gas_price(base_fee),
        }
    }
}

type MoveChanges = ChangeSet;
type EvmChanges = moved_evm_ext::state::StorageTriesChanges;

#[derive(Debug, Clone)]
pub struct Changes {
    pub r#move: MoveChanges,
    pub evm: EvmChanges,
}

impl Changes {
    pub fn new(r#move: MoveChanges, evm: EvmChanges) -> Self {
        Self { r#move, evm }
    }
}

impl From<MoveChanges> for Changes {
    fn from(value: MoveChanges) -> Self {
        Self::new(value, EvmChanges::empty())
    }
}

#[derive(Debug)]
pub struct TransactionExecutionOutcome {
    /// The final outcome of the transaction execution.
    ///
    /// * In case of invalid user input, the result variant is set to [`Err`] containing the actual
    ///   [`UserError`].
    /// * Otherwise, the result variant is set to [`Ok`] containing no data represented by an empty
    ///   tuple.
    pub vm_outcome: Result<(), UserError>,
    /// All changes to accounts and resources generated by the transaction execution to be applied
    /// to Move blockchain state.
    pub changes: Changes,
    /// Total amount of gas spent during the transaction execution.
    pub gas_used: u64,
    /// Effective L2 gas price during transaction execution.
    pub l2_price: U256,
    /// All emitted Move events converted to Ethereum logs.
    pub logs: Vec<Log<LogData>>,
    /// AccountAddress + ModuleId of a deployed module (if any).
    pub deployment: Option<(AccountAddress, ModuleId)>,
}

impl TransactionExecutionOutcome {
    pub fn new(
        vm_outcome: Result<(), UserError>,
        changes: Changes,
        gas_used: u64,
        l2_price: U256,
        logs: Vec<Log<LogData>>,
        deployment: Option<(AccountAddress, ModuleId)>,
    ) -> Self {
        Self {
            vm_outcome,
            changes,
            gas_used,
            l2_price,
            logs,
            deployment,
        }
    }
}

#[derive(Debug, Clone)]
pub struct NormalizedEthTransaction {
    pub signer: Address,
    pub to: TxKind,
    pub nonce: u64,
    pub value: U256,
    pub data: Bytes,
    pub chain_id: Option<u64>,
    gas_limit: U256,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub access_list: AccessList,
}

impl NormalizedEthTransaction {
    pub fn gas_limit(&self) -> u64 {
        // Gas limit cannot be larger than a `u64`, so
        // if any higher limb is non-zero simply return `u64::MAX`.
        match self.gas_limit.into_limbs() {
            [x, 0, 0, 0] => x,
            _ => u64::MAX,
        }
    }
}

impl TryFrom<TxEnvelope> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(tx: TxEnvelope) -> Result<Self, Self::Error> {
        Ok(match tx {
            TxEnvelope::Eip1559(tx) => tx.try_into()?,
            TxEnvelope::Eip2930(tx) => tx.try_into()?,
            TxEnvelope::Legacy(tx) => tx.try_into()?,
            TxEnvelope::Eip4844(_) | TxEnvelope::Eip7702(_) => {
                Err(InvalidTransactionCause::UnsupportedType)?
            }
            t => Err(InvalidTransactionCause::UnknownType(t.tx_type()))?,
        })
    }
}

impl TryFrom<Signed<TxEip1559>> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(value: Signed<TxEip1559>) -> Result<Self, Self::Error> {
        let address = value.recover_signer()?;
        let tx = value.strip_signature();

        Ok(Self {
            signer: address,
            to: tx.to,
            nonce: tx.nonce,
            value: tx.value,
            chain_id: tx.chain_id(),
            gas_limit: U256::from(tx.gas_limit()),
            max_priority_fee_per_gas: U256::from(tx.max_priority_fee_per_gas),
            max_fee_per_gas: U256::from(tx.max_fee_per_gas),
            data: tx.input,
            access_list: tx.access_list,
        })
    }
}

impl TryFrom<Signed<TxEip2930>> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(value: Signed<TxEip2930>) -> Result<Self, Self::Error> {
        let address = value.recover_signer()?;
        let tx = value.strip_signature();

        Ok(Self {
            signer: address,
            to: tx.to,
            nonce: tx.nonce,
            value: tx.value,
            chain_id: tx.chain_id(),
            gas_limit: U256::from(tx.gas_limit()),
            max_priority_fee_per_gas: U256::from(tx.gas_price),
            max_fee_per_gas: U256::from(tx.gas_price),
            data: tx.input,
            access_list: tx.access_list,
        })
    }
}

impl TryFrom<Signed<TxLegacy>> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(value: Signed<TxLegacy>) -> Result<Self, Self::Error> {
        let address = value.recover_signer()?;
        let tx = value.strip_signature();

        Ok(Self {
            signer: address,
            to: tx.to,
            nonce: tx.nonce,
            value: tx.value,
            chain_id: tx.chain_id(),
            gas_limit: U256::from(tx.gas_limit()),
            max_priority_fee_per_gas: U256::from(tx.gas_price),
            max_fee_per_gas: U256::from(tx.gas_price),
            data: tx.input,
            access_list: AccessList(Vec::new()),
        })
    }
}

#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub enum ScriptOrModule {
    Script(Script),
    Module(Module),
}

/// Possible parsings of transaction data from a non-deposit transaction.
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub enum TransactionData {
    EoaBaseTokenTransfer(Address),
    ScriptOrModule(ScriptOrModule),
    // Entry function should be the 3rd option to match the SDK TransactionPayload
    EntryFunction(EntryFunction),
    L2Contract(Address),
}

impl TransactionData {
    pub fn parse_from(tx: &NormalizedEthTransaction) -> moved_shared::error::Result<Self> {
        match tx.to {
            TxKind::Call(to) => {
                if to.ge(&L2_LOWEST_ADDRESS) && to.le(&L2_HIGHEST_ADDRESS) {
                    Ok(Self::L2Contract(to))
                } else if tx.data.is_empty() {
                    // When there is no transaction data then we interpret the
                    // transaction as a base token transfer between EOAs.
                    Ok(Self::EoaBaseTokenTransfer(to))
                } else {
                    let tx_data: TransactionData = bcs::from_bytes(&tx.data)?;
                    // Inner value should be an entry function type
                    let Some(entry_fn) = tx_data.maybe_entry_fn() else {
                        Err(InvalidTransactionCause::InvalidPayload(bcs::Error::Custom(
                            "Not an entry function".to_string(),
                        )))?
                    };
                    if entry_fn.module().address() != &to.to_move_address() {
                        Err(InvalidTransactionCause::InvalidDestination)?
                    }
                    Ok(tx_data)
                }
            }
            TxKind::Create => {
                // Assume EVM create type transactions are either scripts or module deployments
                let script_or_module: ScriptOrModule = bcs::from_bytes(&tx.data)?;
                Ok(Self::ScriptOrModule(script_or_module))
            }
        }
    }

    pub fn maybe_entry_fn(&self) -> Option<&EntryFunction> {
        if let Self::EntryFunction(entry_fn) = self {
            Some(entry_fn)
        } else {
            None
        }
    }

    pub fn script_hash(&self) -> Option<B256> {
        if let Self::ScriptOrModule(ScriptOrModule::Script(script)) = self {
            let bytes = bcs::to_bytes(script).expect("Script must serialize");
            let hash = alloy::primitives::keccak256(bytes);
            Some(hash)
        } else {
            None
        }
    }
}

impl From<TransactionRequest> for NormalizedEthTransaction {
    fn from(value: TransactionRequest) -> Self {
        Self {
            signer: value.from.unwrap_or_default(),
            to: value.to.unwrap_or_default(),
            nonce: value.nonce.unwrap_or_default(),
            value: value.value.unwrap_or_default(),
            chain_id: value.chain_id,
            gas_limit: U256::from(value.gas.unwrap_or(u64::MAX)),
            max_priority_fee_per_gas: U256::from(
                value.max_priority_fee_per_gas.unwrap_or_default(),
            ),
            max_fee_per_gas: U256::from(value.max_fee_per_gas.unwrap_or_default()),
            data: value.input.into_input().unwrap_or_default(),
            access_list: value.access_list.unwrap_or_default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::{
            primitives::{address, hex},
            rlp::{Decodable, Encodable},
        },
    };

    #[test]
    fn test_deposited_tx_hash() {
        let tx = ExtendedTxEnvelope::DepositedTx(DepositedTx {
            source_hash: B256::new(hex!("ad2cd5c72f8d6b25e4da049d76790993af597050965f2aee87e12f98f8c2427f")),
            from: address!("4a04a3191b7a44a99bfd3184f0d2c2c82b98b939"),
            to: address!("4200000000000000000000000000000000000007"),
            mint: U256::from(0x56bc75e2d63100000_u128),
            value: U256::from(0x56bc75e2d63100000_u128),
            gas: U64::from(0x77d2e_u64),
            is_system_tx: false,
            data: hex!("d764ad0b0001000000000000000000000000000000000000000000000000000000000000000000000000000000000000c8088d0362bb4ac757ca77e211c30503d39cef4800000000000000000000000042000000000000000000000000000000000000100000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000030d4000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000a41635f5fd00000000000000000000000084a124e4ec6f0f9914b49dcc71669a8cac556ad600000000000000000000000084a124e4ec6f0f9914b49dcc71669a8cac556ad60000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").into(),
        });
        assert_eq!(
            tx.compute_hash(),
            B256::new(hex!(
                "ab9985077953a6544cd83c3c2a0ade7de83c19254124a74f5e9644ee8be4fc2f"
            ))
        );
    }

    #[test]
    fn test_extended_tx_envelope_rlp() {
        // Deposited Transaction
        rlp_roundtrip(&Bytes::from_static(&hex!("7ef8f8a0672dfee56b1754d9fb99b11dae8eab6dfb7246470f6f7354d7acab837eab12b294deaddeaddeaddeaddeaddeaddeaddeaddead00019442000000000000000000000000000000000000158080830f424080b8a4440a5e2000000558000c5fc50000000000000004000000006672f4bd000000000000020e00000000000000000000000000000000000000000000000000000000000000070000000000000000000000000000000000000000000000000000000000000001bc6d63f57e9fd865ae9a204a4db7fe1cff654377442541b06d020ddab88c2eeb000000000000000000000000e25583099ba105d9ec0a67f5ae86d90e50036425")));

        // Canonical Transaction
        rlp_roundtrip(&Bytes::from_static(&hex!("02f86f82a45580808346a8928252089465d08a056c17ae13370565b04cf77d2afa1cb9fa8806f05b59d3b2000080c080a0dd50efde9a4d2f01f5248e1a983165c8cfa5f193b07b4b094f4078ad4717c1e4a017db1be1e8751b09e033bcffca982d0fe4919ff6b8594654e06647dee9292750")));
    }

    fn rlp_roundtrip(encoded: &[u8]) {
        let mut re_encoded = Vec::with_capacity(encoded.len());
        let mut slice = encoded;
        let tx = ExtendedTxEnvelope::decode(&mut slice).unwrap();
        tx.encode(&mut re_encoded);
        assert_eq!(re_encoded, encoded);
    }
}
