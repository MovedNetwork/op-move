use std::fmt::Display;

use umi_shared::error::UserError;

use crate::schema;

#[derive(Debug, PartialEq, Eq, serde::Serialize)]
pub struct JsonRpcError {
    pub code: i64,
    pub data: serde_json::Value,
    pub message: String,
}

impl JsonRpcError {
    pub fn without_data(code: i64, message: impl Display) -> Self {
        Self {
            code,
            message: format!("{}", message),
            data: serde_json::Value::Null,
        }
    }

    pub fn invalid_fc_state() -> Self {
        Self::without_data(-38002, "Invalid forkchoice state")
    }

    pub fn invalid_attributes() -> Self {
        Self::without_data(-38003, "Invalid payload attributes")
    }

    pub fn parse_error(request: serde_json::Value, message: impl Display) -> Self {
        Self {
            // invalid params code as defined in geth's beacon/engine/errors.go
            code: -32602,
            message: format!("{}", message),
            data: request,
        }
    }

    pub fn too_many_params_error(request: serde_json::Value) -> Self {
        Self::parse_error(request, "Too many params")
    }

    pub fn not_enough_params_error(request: serde_json::Value) -> Self {
        Self::parse_error(request, "Not enough params")
    }

    pub fn missing_method(request: serde_json::Value) -> Self {
        Self::parse_error(request, "method field not found")
    }

    pub fn invalid_method(method_name: impl Display) -> Self {
        Self::without_data(
            -32601,
            format!("Invalid or unimplemented method: {method_name}"),
        )
    }

    pub fn block_not_found(block_number: impl Display) -> Self {
        // block number invalid code as defined in geth's internal/ethapi/errors.go
        Self::without_data(-38020, format!("Block not found: {block_number}"))
    }

    pub fn unknown_payload(payload_id: schema::PayloadId) -> Self {
        // code as defined in geth's beacon/engine/errors.go
        Self {
            code: -38001,
            data: serde_json::to_value(payload_id).expect("Must serialize payload id"),
            message: "Unknown payload".into(),
        }
    }

    pub fn internal_error(error: impl Display) -> Self {
        Self::without_data(
            // internal error code as defined in geth's internal/ethapi/errors.go
            -32603,
            format!("Internal error: {error}"),
        )
    }

    pub fn transaction_error(error: impl Display) -> JsonRpcError {
        // code as defined in geth's internal/ethapi/errors.go
        JsonRpcError::without_data(-32000, format!("Execution reverted: {error}"))
    }
}

impl From<umi_shared::error::Error> for JsonRpcError {
    fn from(value: umi_shared::error::Error) -> Self {
        match value {
            umi_shared::error::Error::User(user_error) => match user_error {
                e if matches!(e, UserError::Vm(_) | UserError::PartialVm(_)) => Self::without_data(
                    // VM error code
                    -32015, e,
                ),
                e if matches!(
                    e,
                    UserError::InvalidBlockHeight(_)
                        | UserError::InvalidBlockHash(_)
                        | UserError::InvalidBlockCount(_)
                ) =>
                {
                    Self::block_not_found(e)
                }

                UserError::InvalidPayloadId(id) => {
                    Self::unknown_payload(schema::PayloadId::new(id))
                }
                ref e @ UserError::InvalidRewardPercentiles(ref reward) => {
                    Self::parse_error(reward.clone().into(), e)
                }
                other => Self::transaction_error(other),
            },
            umi_shared::error::Error::InvalidTransaction(invalid_transaction_cause) => {
                match invalid_transaction_cause {
                    e @ umi_shared::error::InvalidTransactionCause::IncorrectNonce {
                        expected,
                        given,
                    } => {
                        if expected < given {
                            // nonce too high error code
                            Self::without_data(-38011, e)
                        } else {
                            // nonce too low error code
                            Self::without_data(-38010, e)
                        }
                    }
                    e @ umi_shared::error::InvalidTransactionCause::ExhaustedAccount => {
                        // still counts as nonce being too high
                        Self::without_data(-38011, e)
                    }
                    e @ umi_shared::error::InvalidTransactionCause::InsufficientIntrinsicGas => {
                        Self::without_data(-38013, e)
                    }
                    e if matches!(
                        e,
                        umi_shared::error::InvalidTransactionCause::FailedToPayL1Fee
                            | umi_shared::error::InvalidTransactionCause::FailedToPayL2Fee
                    ) =>
                    {
                        // insufficient funds error code
                        Self::without_data(-38014, e)
                    }
                    other => Self::transaction_error(other),
                }
            }
            e @ umi_shared::error::Error::DatabaseState => Self::internal_error(e),
            umi_shared::error::Error::InvariantViolation(e) => panic!("{e}"),
        }
    }
}

#[derive(Debug, serde::Serialize)]
pub struct JsonRpcResponse {
    pub id: serde_json::Value,
    pub jsonrpc: serde_json::Value,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<serde_json::Value>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<JsonRpcError>,
}
