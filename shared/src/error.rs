//! # The error module
//!
//! This module is responsible for providing structured error types that are easily processable.
//! They implement [`Display`] and [`Debug`] traits so that they are representable and printable to
//! log files.
//!
//! It is important that any logic processing the error only uses the structured data. No logic
//! should be dependent on the particular error message that are reachable by the [`Debug`] or
//! [`Display`] trait, they serve only an informative purpose and a human-readable representation.   

use {
    alloy::consensus::TxType,
    move_binary_format::errors::{PartialVMError, VMError},
    move_core_types::language_storage::TypeTag,
    thiserror::Error,
};

/// The result type with its error type set to [`Error`].
pub type Result<T> = std::result::Result<T, Error>;

/// Error for operations in [`op-move`].
///
/// # Variants
/// * [`UserError`] is an error caused by an invalid user input.
/// * [`InvalidTransaction`] is an error caused by an invalid transaction input parameter.
/// * [`InvariantViolation`] is an error caused by an internal system issue.
#[derive(Debug, Error)]
pub enum Error {
    #[error("{0}")]
    User(UserError),
    #[error("{0}")]
    InvalidTransaction(InvalidTransactionCause),
    #[error("{0}")]
    InvariantViolation(InvariantViolation),
}

impl Error {
    pub const fn nonce_invariant_violation(invariant: NonceChecking) -> Self {
        Self::InvariantViolation(InvariantViolation::NonceChecking(invariant))
    }

    pub const fn eth_token_invariant_violation(invariant: EthToken) -> Self {
        Self::InvariantViolation(InvariantViolation::EthToken(invariant))
    }

    pub const fn entry_fn_invariant_violation(invariant: EntryFunctionValue) -> Self {
        Self::InvariantViolation(InvariantViolation::EntryFunctionValue(invariant))
    }

    pub const fn script_tx_invariant_violation(invariant: ScriptTransaction) -> Self {
        Self::InvariantViolation(InvariantViolation::ScriptTransaction(invariant))
    }
}

impl<T> From<T> for Error
where
    UserError: From<T>,
{
    fn from(value: T) -> Self {
        Error::User(UserError::from(value))
    }
}

/// The error caused by invalid user input.
#[derive(Debug, Error)]
pub enum UserError {
    #[error("{0}")]
    Vm(#[from] VMError),
    #[error("{0}")]
    PartialVm(#[from] PartialVMError),
    #[error("{0}")]
    InvalidSignature(#[from] alloy::primitives::SignatureError),
    #[error("Error during EVM execution for L2 bridge {0:?}")]
    DepositFailure(Vec<u8>),
    #[error("L2 contract call failure")]
    L2ContractCallFailure,
    #[error("EVM contract creation failure")]
    EvmContractCreationFailure,
}

/// The error caused by invalid transaction input parameter.
#[derive(Debug, Error)]
pub enum InvalidTransactionCause {
    #[error("tx.to must match payload module address")]
    InvalidDestination,
    #[error("Signer does not match transaction signature")]
    InvalidSigner,
    #[error("{0}")]
    InvalidPayload(bcs::Error),
    #[error("Incorrect number of arguments")]
    MismatchedArgumentCount,
    #[error("Failed to deserialize entry function argument")]
    FailedArgumentDeserialization,
    #[error("Invalid nested references")]
    UnsupportedNestedReference,
    #[error("Blob transactions are not supported")]
    UnsupportedType,
    #[error("Unknown transaction type: {0}")]
    UnknownType(TxType),
    #[error("Incorrect nonce: given={given} expected={expected}")]
    IncorrectNonce { expected: u64, given: u64 },
    #[error("Account exhausted, no more nonce values remain")]
    ExhaustedAccount,
    #[error("Incorrect chain id")]
    IncorrectChainId,
    #[error("Argument type not allowed in entry function: {0}")]
    DisallowedEntryFunctionType(TypeTag),
    #[error("Insufficient intrinsic gas")]
    InsufficientIntrinsicGas,
    #[error("String must be UTF-8 encoded bytes")]
    InvalidString,
    #[error("Option is a Move Vector with 0 or 1 elements")]
    InvalidOption,
    #[error("Object must already exist to pass as an entry function argument")]
    InvalidObject,
    #[error("Failed to pay L1 fee")]
    FailedToPayL1Fee,
    #[error("Failed to pay L2 fee")]
    FailedToPayL2Fee,
    #[error("Failed to recover tx signer")]
    FailedToRecoverSigner,
}

impl From<InvalidTransactionCause> for Error {
    fn from(value: InvalidTransactionCause) -> Self {
        Error::InvalidTransaction(value)
    }
}

impl From<bcs::Error> for Error {
    fn from(value: bcs::Error) -> Self {
        Error::InvalidTransaction(InvalidTransactionCause::InvalidPayload(value))
    }
}

#[derive(Debug, Error)]
pub enum InvariantViolation {
    #[error("Nonce check invariant violation: {0}")]
    NonceChecking(NonceChecking),
    #[error("ETH token invariant violation: {0}")]
    EthToken(EthToken),
    #[error("Entry function type check invariant violation: {0}")]
    EntryFunctionValue(EntryFunctionValue),
    #[error("Script transaction invariant violation: {0}")]
    ScriptTransaction(ScriptTransaction),
    #[error("Mempool admitted transactions cannot be unsupported type")]
    MempoolTransaction,
}

#[derive(Debug, Error)]
pub enum NonceChecking {
    #[error("Any account can be created")]
    AnyAccountCanBeCreated,
    #[error("Function get_sequence_number always succeeds")]
    GetNonceAlwaysSucceeds,
    #[error("Function get_sequence_number has a return value")]
    GetNonceReturnsAValue,
    #[error("Function get_sequence_number return value can be deserialized")]
    GetNoneReturnDeserializes,
    #[error("Function get_sequence_number returns a value of type u64")]
    GetNonceReturnsU64,
    #[error("Function increment_sequence_number always succeeds")]
    IncrementNonceAlwaysSucceeds,
}

#[derive(Debug, Error)]
pub enum EthToken {
    #[error("Function mint always succeeds")]
    MintAlwaysSucceeds,
    #[error("Function get_balance always succeeds")]
    GetBalanceAlwaysSucceeds,
    #[error("Function get_balance has a return value")]
    GetBalanceReturnsAValue,
    #[error("Function get_balance return value can be deserialized")]
    GetBalanceReturnDeserializes,
    #[error("Function get_balance returns a value of type u256")]
    GetBalanceReturnsU256,
    #[error("Unused gas refund should never fail")]
    RefundAlwaysSucceeds,
}

#[derive(Debug, Error)]
pub enum EntryFunctionValue {
    #[error("Only allowed structs pass the type check")]
    OnlyAllowedStructs,
    #[error("String struct has a field")]
    StringStructHasField,
    #[error("String struct field is a vector")]
    StringStructFieldIsVector,
    #[error("String is composed of u8")]
    StringElementsAreU8,
    #[error("Option struct has a type parameter")]
    OptionHasInnerType,
    #[error("Option struct has a field")]
    OptionStructHasField,
    #[error("Option struct field is a vector")]
    OptionStructFieldIsVector,
    #[error("Object struct has a type parameter")]
    ObjectStructHasTypeParameter,
    #[error("Object struct has a field")]
    ObjectStructHasField,
    #[error("Object struct field is an address")]
    ObjectStructFieldIsAddress,
    #[error("ObjectCore is a type defined in the standard library")]
    ObjectCoreTypeExists,
    #[error("Object parameter type must be defined")]
    ObjectInnerTypeExists,
}

#[derive(Debug, Error)]
pub enum ScriptTransaction {
    #[error("Script arguments must serialize")]
    ArgsMustSerialize,
}

#[cfg(test)]
mod tests {
    use {
        super::*,
        move_binary_format::errors::Location,
        move_core_types::{
            account_address::AccountAddress, language_storage::StructTag, vm_status::StatusCode,
        },
        test_case::test_case,
    };

    #[test_case(
        InvalidTransactionCause::InvalidDestination,
        "tx.to must match payload module address"
    )]
    #[test_case(
        InvalidTransactionCause::InvalidSigner,
        "Signer does not match transaction signature"
    )]
    #[test_case(
        InvalidTransactionCause::UnsupportedNestedReference,
        "Invalid nested references"
    )]
    #[test_case(
        InvalidTransactionCause::MismatchedArgumentCount,
        "Incorrect number of arguments"
    )]
    #[test_case(
        InvalidTransactionCause::FailedArgumentDeserialization,
        "Failed to deserialize entry function argument"
    )]
    #[test_case(
        InvalidTransactionCause::UnsupportedType,
        "Blob transactions are not supported"
    )]
    #[test_case(
        InvalidTransactionCause::UnknownType(TxType::Legacy),
        "Unknown transaction type: Legacy"
    )]
    #[test_case(
        alloy::primitives::SignatureError::InvalidParity(0),
        "invalid parity: 0"
    )]
    #[test_case(bcs::Error::Eof, "unexpected end of input")]
    #[test_case(
        PartialVMError::new(StatusCode::ABORTED),
        "PartialVMError with status ABORTED"
    )]
    #[test_case(
        PartialVMError::new(StatusCode::ABORTED).finish(Location::Undefined),
        "VMError with status ABORTED at location UNDEFINED"
    )]
    #[test_case(
        InvalidTransactionCause::IncorrectNonce{ expected: 1, given: 2 },
        "Incorrect nonce: given=2 expected=1"
    )]
    #[test_case(
        InvalidTransactionCause::ExhaustedAccount,
        "Account exhausted, no more nonce values remain"
    )]
    #[test_case(InvalidTransactionCause::IncorrectChainId, "Incorrect chain id")]
    #[test_case(
        InvalidTransactionCause::DisallowedEntryFunctionType(TypeTag::Struct(Box::new(StructTag {
            address: AccountAddress::ONE,
            module: "token".parse().unwrap(),
            name: "Token".parse().unwrap(),
            type_args: vec![TypeTag::U8],
        }))),
        "Argument type not allowed in entry function: 0x1::token::Token<u8>"
    )]
    #[test_case(
        InvalidTransactionCause::InsufficientIntrinsicGas,
        "Insufficient intrinsic gas"
    )]
    fn test_error_converts_and_displays(actual: impl Into<Error>, expected: impl Into<String>) {
        let actual = actual.into().to_string();
        let expected = expected.into();

        assert_eq!(actual, expected);
    }
}
