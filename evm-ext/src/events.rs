use {
    super::{EVM_NATIVE_ADDRESS, EVM_NATIVE_MODULE},
    alloy::primitives::{B256, Log, LogData, U256},
    move_core_types::{
        account_address::AccountAddress,
        ident_str,
        language_storage::StructTag,
        value::{MoveStructLayout, MoveTypeLayout, MoveValue},
    },
    moved_shared::primitives::ToEthAddress,
    std::{cell::RefCell, sync::LazyLock},
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EthTransfer {
    pub from: AccountAddress,
    pub to: AccountAddress,
    pub amount: U256,
}

pub trait EthTransferLog {
    fn push_transfer(&self, transfer: EthTransfer);
    fn take_transfers(&self) -> Vec<EthTransfer>;
    fn add_tx_origin(&self, address: AccountAddress, amount: U256);
    fn take_origins(&self) -> Vec<(AccountAddress, U256)>;
}

/// Struct external to the EVM to capture transfer events.
/// This is used for bookkeeping token balances between Move and EVM.
///
/// It needs to use a `RefCell` for interior mutability because the
/// EVM handler hooks require the closures to be immutable.
/// The usage of interior mutability is safe because execution of the single
/// EVM instance is single threaded.
#[derive(Debug, Default)]
pub struct EthTransfersLogger {
    transfers: RefCell<Vec<EthTransfer>>,
    origins: RefCell<Vec<(AccountAddress, U256)>>,
}

impl EthTransferLog for EthTransfersLogger {
    fn push_transfer(&self, transfer: EthTransfer) {
        self.transfers.borrow_mut().push(transfer);
    }

    fn take_transfers(&self) -> Vec<EthTransfer> {
        self.transfers.take()
    }

    fn add_tx_origin(&self, address: AccountAddress, amount: U256) {
        self.origins.borrow_mut().push((address, amount));
    }

    fn take_origins(&self) -> Vec<(AccountAddress, U256)> {
        self.origins.take()
    }
}

impl EthTransferLog for () {
    fn push_transfer(&self, _transfer: EthTransfer) {}

    fn take_transfers(&self) -> Vec<EthTransfer> {
        Vec::new()
    }

    fn add_tx_origin(&self, _address: AccountAddress, _amount: U256) {}

    fn take_origins(&self) -> Vec<(AccountAddress, U256)> {
        Vec::new()
    }
}

/// Marker struct defined in our framework for marking data as FixedBytes in the Solidity ABI.
pub static EVM_LOGS_EVENT_TAG: LazyLock<StructTag> = LazyLock::new(|| StructTag {
    address: EVM_NATIVE_ADDRESS,
    module: EVM_NATIVE_MODULE.into(),
    name: ident_str!("EvmLogsEvent").into(),
    type_args: Vec::new(),
});

pub static EVM_LOGS_EVENT_LAYOUT: LazyLock<MoveTypeLayout> = LazyLock::new(|| {
    MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![MoveTypeLayout::Vector(
        Box::new(MoveTypeLayout::Struct(MoveStructLayout::Runtime(vec![
            MoveTypeLayout::Address,                                // address
            MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U256)), // topics
            MoveTypeLayout::Vector(Box::new(MoveTypeLayout::U8)),   // data
        ]))),
    )]))
});

pub fn evm_logs_event_to_log(value: MoveValue, dest: &mut Vec<Log<LogData>>) -> Option<()> {
    // Expected EvmLogsEvent struct
    let MoveValue::Struct(object) = value else {
        return None;
    };

    let mut fields = object.into_optional_variant_and_fields().1;
    // EvmLogsEvent has one field
    if fields.len() != 1 {
        return None;
    }
    // EvmLogsEvent field is a vector
    let Some(MoveValue::Vector(logs)) = fields.pop() else {
        return None;
    };

    for value in logs {
        // Each element of the vector is EvmLog struct
        let MoveValue::Struct(object) = value else {
            return None;
        };
        let mut fields = object.into_optional_variant_and_fields().1;
        // EvmLog has 3 fields
        if fields.len() != 3 {
            return None;
        }
        // Last field is vector<u8>
        let Some(MoveValue::Vector(data)) = fields.pop() else {
            return None;
        };
        // second field is vector<u256>
        let Some(MoveValue::Vector(topics)) = fields.pop() else {
            return None;
        };
        // first field is address
        let Some(MoveValue::Address(address)) = fields.pop() else {
            return None;
        };

        let data = data.into_iter().map(as_u8).collect::<Option<Vec<u8>>>()?;
        let topics = topics
            .into_iter()
            .map(as_b256)
            .collect::<Option<Vec<B256>>>()?;
        dest.push(Log {
            address: address.to_eth_address(),
            data: LogData::new(topics, data.into())?,
        });
    }
    Some(())
}

fn as_u8(value: MoveValue) -> Option<u8> {
    if let MoveValue::U8(x) = value {
        Some(x)
    } else {
        None
    }
}

fn as_b256(value: MoveValue) -> Option<B256> {
    if let MoveValue::U256(value) = value {
        Some(B256::new(value.to_le_bytes()))
    } else {
        None
    }
}
