//! Transaction signing helper that looks up the sender's keypair from a keystore.

use meow_types::{
    keystore::Keystore,
    transaction::{SignedTransaction, Transaction},
};

/// Look up the keypair for `signer`, and sign `transaction`.
pub fn sign_transaction(
    transaction: Transaction,
    keystore: &Keystore,
) -> anyhow::Result<SignedTransaction> {
    let signer = transaction.sender();

    let keypair = keystore
        .get_key(signer)
        .ok_or_else(|| anyhow::anyhow!("A key has not been found in the keystore for {signer}"))?;

    let (signed, _) = transaction.sign(keypair);

    Ok(signed)
}
