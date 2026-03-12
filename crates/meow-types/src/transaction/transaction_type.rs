use serde::{Deserialize, Serialize};

use crate::transaction::call::Call;

/// The type of a transaction.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub enum TransactionType {
    /// Transaction is a MEOW VM call.
    MeowCall(Call),
    /// Transaction is a publish operation.
    /// The content is a raw BCS serialized data of a module to be published.
    MeowModulePublish(Vec<u8>),
}
