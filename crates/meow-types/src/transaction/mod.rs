pub mod call;
pub mod execution_result;
pub mod input;
pub mod transaction_type;
pub mod validator;

use serde::{Deserialize, Serialize};

use crate::{
    address::Address,
    digest::Digest,
    keypair::{KeyPair, signature::Signature},
    object::object_ref::ObjectRef,
    transaction::transaction_type::TransactionType,
};

/// The meow transaction.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct Transaction {
    /// The transaction sender address.
    sender: Address,
    /// The MEOW coin to be used for paying the transaction fee.
    /// Acts as a nonce of the transaction.
    gas_coin: ObjectRef,
    /// The transaction type.
    type_: TransactionType,
}

/// A signed transaction.
#[derive(Serialize, Deserialize, Debug, Clone, PartialEq, Eq)]
pub struct SignedTransaction(Transaction, Signature);

//
// ─── Implementation of [Transaction] ───
//

impl Transaction {
    /// Creates a new transaction.
    pub fn new(sender: Address, gas_coin: ObjectRef, type_: TransactionType) -> Self {
        Self {
            sender,
            gas_coin,
            type_,
        }
    }

    /// Returns the transaction sender.
    pub fn sender(&self) -> &Address {
        &self.sender
    }

    /// Returns the gas coin used for the transaction.
    pub fn gas_coin(&self) -> &ObjectRef {
        &self.gas_coin
    }

    /// Returns the transaction type.
    pub fn type_(&self) -> &TransactionType {
        &self.type_
    }

    /// Computes the transaction digest.
    pub fn digest(&self) -> Digest {
        Digest::compute(self).expect("Failed to compute a transaction digest")
    }

    /// Sign a transaction with the given keypair, returning a signed transaction and the message which was signed.
    pub fn sign(self, keypair: &KeyPair) -> (SignedTransaction, Digest) {
        let digest = self.digest();
        let signature = keypair.sign(digest);

        (SignedTransaction(self, signature), digest)
    }
}

//
// ─── Implementation of [SignedTransaction] ───
//

impl SignedTransaction {
    /// Creates a new signed transaction with a precomputed signature.
    /// The signature is expected to be valid for the transaction, but this is not verified by this constructor.
    /// It is preferred to use the `sign` method of [Transaction] to create a signed transaction, which guarantees the validity of the signature.
    pub fn new(transaction: Transaction, signature: Signature) -> Self {
        Self(transaction, signature)
    }

    /// Returns the signed transaction.
    pub fn transaction(&self) -> &Transaction {
        &self.0
    }

    /// Returns the transaction signature.
    pub fn signature(&self) -> &Signature {
        &self.1
    }
}
