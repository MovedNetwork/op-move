use {
    crate::L1GasFeeInput,
    alloy::{
        consensus::{
            Receipt, ReceiptWithBloom, Sealed, Signed, Transaction, TxEip1559, TxEip2930,
            TxEnvelope, TxLegacy,
        },
        eips::eip2930::AccessList,
        primitives::{
            Address, B256, Bloom, Bytes, Log, LogData, PrimitiveSignature, TxKind, U256, address,
        },
        rlp::{Decodable, Encodable},
        rpc::types::TransactionRequest,
    },
    aptos_types::transaction::{EntryFunction, Module, Script},
    move_core_types::{account_address::AccountAddress, language_storage::ModuleId},
    op_alloy::consensus::{
        OpDepositReceipt, OpDepositReceiptWithBloom, OpReceiptEnvelope, OpTxEnvelope, TxDeposit,
    },
    serde::{Deserialize, Serialize},
    std::borrow::Cow,
    umi_shared::{
        error::{Error, InvalidTransactionCause, UserError},
        primitives::ToMoveAddress,
    },
};

pub const L2_LOWEST_ADDRESS: Address = address!("4200000000000000000000000000000000000000");
pub const L2_HIGHEST_ADDRESS: Address = address!("42000000000000000000000000000000000000ff");
pub const DEPOSIT_RECEIPT_VERSION: u64 = 1;

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum UmiTxType {
    Legacy,
    Eip2930,
    Eip1559,
}

/// Custom canonical transaction envelope that only holds transaction types we support.
/// This excludes EIP-4844 (blob transactions) and EIP-7702 (account abstraction).
/// Otherwise derived straight from alloy's `TxEnvelope`.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum UmiTxEnvelope {
    Legacy(Signed<TxLegacy>),
    Eip2930(Signed<TxEip2930>),
    Eip1559(Signed<TxEip1559>),
}

impl TryFrom<TxEnvelope> for UmiTxEnvelope {
    type Error = Error;

    fn try_from(envelope: TxEnvelope) -> Result<Self, Self::Error> {
        match envelope {
            TxEnvelope::Legacy(tx) => Ok(Self::Legacy(tx)),
            TxEnvelope::Eip2930(tx) => Ok(Self::Eip2930(tx)),
            TxEnvelope::Eip1559(tx) => Ok(Self::Eip1559(tx)),
            TxEnvelope::Eip4844(_) | TxEnvelope::Eip7702(_) => {
                Err(InvalidTransactionCause::UnsupportedType)?
            }
        }
    }
}

impl From<UmiTxEnvelope> for TxEnvelope {
    fn from(supported: UmiTxEnvelope) -> Self {
        match supported {
            UmiTxEnvelope::Legacy(tx) => TxEnvelope::Legacy(tx),
            UmiTxEnvelope::Eip2930(tx) => TxEnvelope::Eip2930(tx),
            UmiTxEnvelope::Eip1559(tx) => TxEnvelope::Eip1559(tx),
        }
    }
}

impl TryFrom<UmiTxEnvelope> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(envelope: UmiTxEnvelope) -> Result<Self, Self::Error> {
        match envelope {
            UmiTxEnvelope::Legacy(tx) => tx.try_into(),
            UmiTxEnvelope::Eip2930(tx) => tx.try_into(),
            UmiTxEnvelope::Eip1559(tx) => tx.try_into(),
        }
    }
}

impl Decodable for UmiTxEnvelope {
    fn decode(buf: &mut &[u8]) -> alloy::rlp::Result<Self> {
        let envelope = TxEnvelope::decode(buf)?;
        envelope
            .try_into()
            .map_err(|_| alloy::rlp::Error::Custom("Unsupported transaction type"))
    }
}

impl Encodable for UmiTxEnvelope {
    fn encode(&self, out: &mut dyn alloy::rlp::BufMut) {
        let envelope: TxEnvelope = self.clone().into();
        envelope.encode(out);
    }
}

pub trait WrapReceipt {
    fn wrap_receipt(&self, receipt: Receipt, bloom: Bloom) -> OpReceiptEnvelope;
}

impl WrapReceipt for NormalizedExtendedTxEnvelope {
    fn wrap_receipt(&self, receipt: Receipt, bloom: Bloom) -> OpReceiptEnvelope {
        match self {
            Self::Canonical(norm_tx) => match norm_tx.tx_type {
                UmiTxType::Legacy => OpReceiptEnvelope::Legacy(ReceiptWithBloom {
                    receipt,
                    logs_bloom: bloom,
                }),
                UmiTxType::Eip2930 => OpReceiptEnvelope::Eip2930(ReceiptWithBloom {
                    receipt,
                    logs_bloom: bloom,
                }),
                UmiTxType::Eip1559 => OpReceiptEnvelope::Eip1559(ReceiptWithBloom {
                    receipt,
                    logs_bloom: bloom,
                }),
            },
            Self::DepositedTx(dep_tx) => OpReceiptEnvelope::Deposit(OpDepositReceiptWithBloom {
                receipt: OpDepositReceipt {
                    inner: receipt,
                    // Per OP stack spec <https://specs.optimism.io/protocol/deposits.html#execution>
                    deposit_nonce: Some(dep_tx.nonce()),
                    deposit_receipt_version: Some(DEPOSIT_RECEIPT_VERSION),
                },
                logs_bloom: bloom,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub enum NormalizedExtendedTxEnvelope {
    Canonical(NormalizedEthTransaction),
    DepositedTx(Sealed<TxDeposit>),
}

// TODO: avoid conversion to op-alloy and do it internally
impl Decodable for NormalizedExtendedTxEnvelope {
    fn decode(buf: &mut &[u8]) -> alloy::rlp::Result<Self> {
        let envelope = OpTxEnvelope::decode(buf)?;
        envelope
            .try_into()
            .map_err(|_| alloy::rlp::Error::Custom("Unsupported transaction type"))
    }
}

impl Encodable for NormalizedExtendedTxEnvelope {
    fn encode(&self, out: &mut dyn alloy::rlp::BufMut) {
        let envelope: OpTxEnvelope = self.clone().into();
        envelope.encode(out);
    }
}

impl TryFrom<OpTxEnvelope> for NormalizedExtendedTxEnvelope {
    type Error = Error;

    fn try_from(value: OpTxEnvelope) -> Result<Self, Self::Error> {
        Ok(match value {
            OpTxEnvelope::Eip1559(tx) => NormalizedExtendedTxEnvelope::Canonical(tx.try_into()?),
            OpTxEnvelope::Eip2930(tx) => NormalizedExtendedTxEnvelope::Canonical(tx.try_into()?),
            OpTxEnvelope::Legacy(tx) => NormalizedExtendedTxEnvelope::Canonical(tx.try_into()?),
            OpTxEnvelope::Deposit(tx) => NormalizedExtendedTxEnvelope::DepositedTx(tx),
            _ => Err(InvalidTransactionCause::UnsupportedType)?,
        })
    }
}

impl From<NormalizedEthTransaction> for NormalizedExtendedTxEnvelope {
    fn from(tx: NormalizedEthTransaction) -> Self {
        NormalizedExtendedTxEnvelope::Canonical(tx)
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

    pub fn as_deposit(&self) -> Option<&TxDeposit> {
        match self {
            Self::DepositedTx(tx) => Some(tx),
            _ => None,
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

    pub fn tx_hash(&self) -> B256 {
        match self {
            Self::Canonical(tx) => tx.tx_hash,
            Self::DepositedTx(tx) => B256::from(*tx.hash()),
        }
    }

    pub fn l1_gas_fee_input(&self) -> L1GasFeeInput {
        match self {
            Self::Canonical(tx) => tx.l1_gas_fee_input.clone(),
            Self::DepositedTx(_) => L1GasFeeInput::default(),
        }
    }

    pub fn wrap_receipt(&self, receipt: Receipt, bloom: Bloom) -> OpReceiptEnvelope {
        match self {
            Self::Canonical(_) => {
                // For canonical transactions, we use EIP-1559 receipt format
                OpReceiptEnvelope::Eip1559(ReceiptWithBloom {
                    receipt,
                    logs_bloom: bloom,
                })
            }
            Self::DepositedTx(dep) => OpReceiptEnvelope::Deposit(OpDepositReceiptWithBloom {
                receipt: OpDepositReceipt {
                    inner: receipt,
                    deposit_nonce: Some(dep.nonce()),
                    deposit_receipt_version: Some(DEPOSIT_RECEIPT_VERSION),
                },
                logs_bloom: bloom,
            }),
        }
    }

    pub fn trie_hash(&self) -> B256 {
        self.tx_hash()
    }
}

type MoveChanges = umi_state::Changes;
type EvmChanges = umi_evm_ext::state::StorageTriesChanges;

#[derive(Debug, Clone)]
pub struct Changes {
    pub move_vm: MoveChanges,
    pub evm: EvmChanges,
}

impl Changes {
    pub fn new(move_vm: MoveChanges, evm: EvmChanges) -> Self {
        Self { move_vm, evm }
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

#[derive(Debug, Clone, PartialEq, Eq, Hash, Deserialize, Serialize)]
pub struct NormalizedEthTransaction {
    pub tx_type: UmiTxType,
    pub signer: Address,
    pub tx_hash: B256,
    pub to: TxKind,
    pub nonce: u64,
    pub value: U256,
    pub data: Bytes,
    pub chain_id: Option<u64>,
    gas_limit: U256,
    pub max_priority_fee_per_gas: U256,
    pub max_fee_per_gas: U256,
    pub access_list: AccessList,
    pub l1_gas_fee_input: L1GasFeeInput,
    pub signature: PrimitiveSignature,
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

impl TryFrom<Signed<TxEip1559>> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(value: Signed<TxEip1559>) -> Result<Self, Self::Error> {
        let address = value.recover_signer()?;

        let mut encoded = Vec::new();
        value.rlp_encode(&mut encoded);
        let l1_gas_fee_input = encoded.as_slice().into();

        let (tx, signature, tx_hash) = value.into_parts();

        Ok(Self {
            tx_type: UmiTxType::Eip1559,
            signer: address,
            tx_hash,
            to: tx.to,
            nonce: tx.nonce,
            value: tx.value,
            chain_id: tx.chain_id(),
            gas_limit: U256::from(tx.gas_limit()),
            max_priority_fee_per_gas: U256::from(tx.max_priority_fee_per_gas),
            max_fee_per_gas: U256::from(tx.max_fee_per_gas),
            data: tx.input,
            access_list: tx.access_list,
            l1_gas_fee_input,
            signature,
        })
    }
}

impl TryFrom<Signed<TxEip2930>> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(value: Signed<TxEip2930>) -> Result<Self, Self::Error> {
        let address = value.recover_signer()?;

        let mut encoded = Vec::new();
        value.rlp_encode(&mut encoded);
        let l1_gas_fee_input = encoded.as_slice().into();

        let (tx, signature, tx_hash) = value.into_parts();

        Ok(Self {
            tx_type: UmiTxType::Eip2930,
            signer: address,
            tx_hash,
            to: tx.to,
            nonce: tx.nonce,
            value: tx.value,
            chain_id: tx.chain_id(),
            gas_limit: U256::from(tx.gas_limit()),
            max_priority_fee_per_gas: U256::from(tx.gas_price),
            max_fee_per_gas: U256::from(tx.gas_price),
            data: tx.input,
            access_list: tx.access_list,
            l1_gas_fee_input,
            signature,
        })
    }
}

impl TryFrom<Signed<TxLegacy>> for NormalizedEthTransaction {
    type Error = Error;

    fn try_from(value: Signed<TxLegacy>) -> Result<Self, Self::Error> {
        let address = value.recover_signer()?;

        let mut encoded = Vec::new();
        value.rlp_encode(&mut encoded);
        let l1_gas_fee_input = encoded.as_slice().into();

        let (tx, signature, tx_hash) = value.into_parts();

        Ok(Self {
            tx_type: UmiTxType::Legacy,
            signer: address,
            to: tx.to,
            tx_hash,
            nonce: tx.nonce,
            value: tx.value,
            chain_id: tx.chain_id(),
            gas_limit: U256::from(tx.gas_limit()),
            max_priority_fee_per_gas: U256::from(tx.gas_price),
            max_fee_per_gas: U256::from(tx.gas_price),
            data: tx.input,
            access_list: AccessList(Vec::new()),
            l1_gas_fee_input,
            signature,
        })
    }
}

impl From<NormalizedEthTransaction> for OpTxEnvelope {
    fn from(normalized: NormalizedEthTransaction) -> Self {
        match normalized.tx_type {
            UmiTxType::Legacy => {
                let tx_legacy = TxLegacy {
                    chain_id: normalized.chain_id,
                    nonce: normalized.nonce,
                    gas_price: normalized.max_fee_per_gas.saturating_to(),
                    gas_limit: normalized.gas_limit.saturating_to(),
                    to: normalized.to,
                    value: normalized.value,
                    input: normalized.data,
                };

                let signed_tx =
                    Signed::new_unchecked(tx_legacy, normalized.signature, normalized.tx_hash);

                OpTxEnvelope::Legacy(signed_tx)
            }
            UmiTxType::Eip2930 => {
                let tx_eip2930 = TxEip2930 {
                    chain_id: normalized
                        .chain_id
                        .expect("Chain ID can be unset only for legacy txs"),
                    nonce: normalized.nonce,
                    gas_price: normalized.max_fee_per_gas.saturating_to(),
                    gas_limit: normalized.gas_limit.saturating_to(),
                    to: normalized.to,
                    value: normalized.value,
                    access_list: normalized.access_list,
                    input: normalized.data,
                };

                let signed_tx =
                    Signed::new_unchecked(tx_eip2930, normalized.signature, normalized.tx_hash);

                OpTxEnvelope::Eip2930(signed_tx)
            }
            UmiTxType::Eip1559 => {
                let tx_eip1559 = TxEip1559 {
                    chain_id: normalized
                        .chain_id
                        .expect("Chain ID can be unset only for legacy txs"),
                    nonce: normalized.nonce,
                    gas_limit: normalized.gas_limit.saturating_to(),
                    max_fee_per_gas: normalized.max_fee_per_gas.saturating_to(),
                    max_priority_fee_per_gas: normalized.max_priority_fee_per_gas.saturating_to(),
                    to: normalized.to,
                    value: normalized.value,
                    access_list: normalized.access_list,
                    input: normalized.data,
                };

                let signed_tx =
                    Signed::new_unchecked(tx_eip1559, normalized.signature, normalized.tx_hash);

                OpTxEnvelope::Eip1559(signed_tx)
            }
        }
    }
}

impl From<NormalizedExtendedTxEnvelope> for OpTxEnvelope {
    fn from(normalized_envelope: NormalizedExtendedTxEnvelope) -> Self {
        match normalized_envelope {
            NormalizedExtendedTxEnvelope::Canonical(normalized_tx) => normalized_tx.into(),
            NormalizedExtendedTxEnvelope::DepositedTx(sealed_deposit) => {
                OpTxEnvelope::Deposit(sealed_deposit)
            }
        }
    }
}
#[derive(Debug, PartialEq, Eq, Clone, Deserialize, Serialize)]
pub enum ScriptOrDeployment {
    Script(Script),
    Module(Module),
    EvmContract(Vec<u8>),
}

/// Possible parsings of transaction data from a non-deposit transaction.
#[derive(Debug, PartialEq, Eq, Clone)]
pub enum TransactionData {
    EoaBaseTokenTransfer(Address),
    ScriptOrDeployment(ScriptOrDeployment),
    EntryFunction(EntryFunction),
    L2Contract(Address),
    EvmContract { address: Address, data: Vec<u8> },
}

impl TransactionData {
    pub fn parse_from(tx: &NormalizedEthTransaction) -> umi_shared::error::Result<Self> {
        match tx.to {
            TxKind::Call(to) => {
                if to.ge(&L2_LOWEST_ADDRESS) && to.le(&L2_HIGHEST_ADDRESS) {
                    Ok(Self::L2Contract(to))
                } else if tx.data.is_empty() {
                    // When there is no transaction data then we interpret the
                    // transaction as a base token transfer between EOAs.
                    Ok(Self::EoaBaseTokenTransfer(to))
                } else {
                    let tx_data: SerializableTransactionData = bcs::from_bytes(&tx.data)?;
                    // Inner value should be an entry function type or EVM contract.
                    match tx_data {
                        SerializableTransactionData::EntryFunction(entry_fn) => {
                            if entry_fn.module().address() != &to.to_move_address() {
                                Err(InvalidTransactionCause::InvalidDestination)?
                            }
                            Ok(TransactionData::EntryFunction(entry_fn.into_owned()))
                        }
                        SerializableTransactionData::EvmContract { data } => {
                            Ok(TransactionData::EvmContract {
                                address: to,
                                data: data.into_owned(),
                            })
                        }
                        _ => Err(InvalidTransactionCause::InvalidPayload(bcs::Error::Custom(
                            "Expected entry function or EVM contract".to_string(),
                        )))?,
                    }
                }
            }
            TxKind::Create => {
                // Assume EVM create type transactions are either scripts or module deployments
                let script_or_module: ScriptOrDeployment = bcs::from_bytes(&tx.data)?;
                Ok(Self::ScriptOrDeployment(script_or_module))
            }
        }
    }

    /// Serialize this type into bytes suitable for using in the `data` field of
    /// an Ethereum transaction.
    pub fn to_bytes(&self) -> Result<Vec<u8>, bcs::Error> {
        let serializable: SerializableTransactionData = self.into();
        bcs::to_bytes(&serializable)
    }

    pub fn maybe_entry_fn(&self) -> Option<&EntryFunction> {
        if let Self::EntryFunction(entry_fn) = self {
            Some(entry_fn)
        } else {
            None
        }
    }

    pub fn script_hash(&self) -> Option<B256> {
        if let Self::ScriptOrDeployment(ScriptOrDeployment::Script(script)) = self {
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
            // As it's exclusively for simulation, we can get away with this
            tx_type: UmiTxType::Eip1559,
            tx_hash: B256::random(),
            l1_gas_fee_input: L1GasFeeInput::default(),
            signature: PrimitiveSignature::new(U256::ZERO, U256::ZERO, false),
        }
    }
}

// Intentionally left private to hide the serialization details
// from users of `TransactionData`. This allows making changes to
// `TransactionData` itself while remaining backwards compatible with
// the serialization format.
// The purpose of `Cow` wrapping all the data is to allow serializing
// from a reference to `TransactionData` without cloning while also allowing
// deserializing to `SerializableTransactionData` with owned data.
// Data type which are `Copy` are left without `Cow` references.
#[derive(Deserialize, Serialize)]
enum SerializableTransactionData<'a> {
    EoaBaseTokenTransfer(Address),
    ScriptOrDeployment(Cow<'a, ScriptOrDeployment>),
    // Entry function should be the 3rd option to match the SDK TransactionPayload
    EntryFunction(Cow<'a, EntryFunction>),
    L2Contract(Address),
    // Note: we only include the data here not the address as in the `TransactionData` type
    // because the address is taken from the Ethereum transaction `to` field. Therefore
    // encoding it here would be redundant.
    EvmContract { data: Cow<'a, [u8]> },
}

impl<'a> From<&'a TransactionData> for SerializableTransactionData<'a> {
    fn from(value: &'a TransactionData) -> Self {
        match value {
            TransactionData::EoaBaseTokenTransfer(to) => Self::EoaBaseTokenTransfer(*to),
            TransactionData::ScriptOrDeployment(x) => Self::ScriptOrDeployment(Cow::Borrowed(x)),
            TransactionData::EntryFunction(x) => Self::EntryFunction(Cow::Borrowed(x)),
            TransactionData::L2Contract(x) => Self::L2Contract(*x),
            TransactionData::EvmContract { data, .. } => Self::EvmContract {
                data: Cow::Borrowed(data),
            },
        }
    }
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        alloy::{
            consensus::Sealed,
            primitives::{address, hex},
        },
    };

    #[test]
    fn test_deposited_tx_hash() {
        let tx = OpTxEnvelope::Deposit(Sealed::new(TxDeposit{
            source_hash: B256::new(hex!("ad2cd5c72f8d6b25e4da049d76790993af597050965f2aee87e12f98f8c2427f")),
            from: address!("4a04a3191b7a44a99bfd3184f0d2c2c82b98b939"),
            to: TxKind::Call(address!("4200000000000000000000000000000000000007")),
            mint: Some(0x56bc75e2d63100000_u128),
            value: U256::from(0x56bc75e2d63100000_u128),
            gas_limit: 0x77d2e_u64,
            is_system_transaction: false,
            input: hex!("d764ad0b0001000000000000000000000000000000000000000000000000000000000000000000000000000000000000c8088d0362bb4ac757ca77e211c30503d39cef4800000000000000000000000042000000000000000000000000000000000000100000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000030d4000000000000000000000000000000000000000000000000000000000000000c000000000000000000000000000000000000000000000000000000000000000a41635f5fd00000000000000000000000084a124e4ec6f0f9914b49dcc71669a8cac556ad600000000000000000000000084a124e4ec6f0f9914b49dcc71669a8cac556ad60000000000000000000000000000000000000000000000056bc75e2d631000000000000000000000000000000000000000000000000000000000000000000080000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000000").into(),
        }));
        assert_eq!(
            tx.tx_hash(),
            B256::new(hex!(
                "ab9985077953a6544cd83c3c2a0ade7de83c19254124a74f5e9644ee8be4fc2f"
            ))
        );
    }
}
