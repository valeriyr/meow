//! Serialisable representation of a signed transaction for CLI output.

use base64::{Engine, engine::general_purpose};
use meow_types::{
    object::object_ref::ObjectRef,
    transaction::{SignedTransaction, call::Call, input::Input, transaction_type::TransactionType},
};
use serde::Serialize;

/// Human-readable output representation of a committed signed transaction.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionOutput {
    pub digest: String,
    pub sender: String,
    pub gas_coin: ObjectRefOutput,
    pub signature: String,
    #[serde(rename = "type")]
    pub type_: TransactionTypeOutput,
}

/// Output representation of an object reference.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ObjectRefOutput {
    pub address: String,
    pub version: String,
    pub digest: String,
}

/// Output representation of a transaction's type.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum TransactionTypeOutput {
    Call(CallOutput),
    PublishModule(PublishModuleOutput),
}

/// Output representation of a MEOW call transaction payload.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CallOutput {
    pub module: String,
    pub function: String,
    pub arguments: Vec<CallArgumentOutput>,
}

/// Output representation of a single call argument.
#[derive(Debug, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum CallArgumentOutput {
    Object { object_ref: ObjectRefOutput },
    Raw { bytes: String },
}

/// Output representation of a module publish payload.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PublishModuleOutput {
    pub bytes: String,
    pub byte_size: usize,
}

impl From<SignedTransaction> for TransactionOutput {
    fn from(signed_transaction: SignedTransaction) -> Self {
        let transaction = signed_transaction.transaction();
        let signature = signed_transaction.signature();

        Self {
            digest: transaction.digest().to_string(),
            sender: transaction.sender().to_string(),
            gas_coin: transaction.gas_coin().into(),
            signature: signature.to_string(),
            type_: transaction.type_().into(),
        }
    }
}

impl From<&TransactionType> for TransactionTypeOutput {
    fn from(t: &TransactionType) -> Self {
        match t {
            TransactionType::MeowCall(call) => TransactionTypeOutput::Call(call.into()),
            TransactionType::MeowModulePublish(bytes) => {
                TransactionTypeOutput::PublishModule(PublishModuleOutput {
                    bytes: bytes_to_string(bytes),
                    byte_size: bytes.len(),
                })
            }
        }
    }
}

impl From<&ObjectRef> for ObjectRefOutput {
    fn from(object_ref: &ObjectRef) -> Self {
        Self {
            address: object_ref.address().to_string(),
            version: object_ref.version().to_string(),
            digest: object_ref.digest().to_string(),
        }
    }
}

impl From<&Call> for CallOutput {
    fn from(call: &Call) -> Self {
        Self {
            module: call.module().to_string(),
            function: call.function().to_string(),
            arguments: call.arguments().iter().map(|i| i.into()).collect(),
        }
    }
}

impl From<&Input> for CallArgumentOutput {
    fn from(input: &Input) -> Self {
        match input {
            Input::Object(object_ref) => CallArgumentOutput::Object {
                object_ref: object_ref.into(),
            },
            Input::Raw(bytes) => CallArgumentOutput::Raw {
                bytes: bytes_to_string(bytes),
            },
        }
    }
}

/// Helper function to convert bytes to a base64 string.
fn bytes_to_string(bytes: &[u8]) -> String {
    general_purpose::STANDARD.encode(bytes)
}
