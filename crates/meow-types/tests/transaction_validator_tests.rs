use meow_types::{
    address::Address,
    config::{
        self, MAX_BCS_SERIALIZED_MODULE_SIZE, MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE,
        MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE,
    },
    digest::Digest,
    identifier::Identifier,
    keypair::{KeyPair, error::KeyPairError, signature_scheme::SignatureScheme},
    object::{object_ref::ObjectRef, object_version::ObjectVersion},
    transaction::{
        SignedTransaction, Transaction,
        call::Call,
        input::Input,
        transaction_type::TransactionType,
        validator::{self, ValidationError},
    },
};
use rand::{SeedableRng, rngs::StdRng};

//
// ─── MeowCall ───
//

#[test]
fn valid_call_with_no_args_passes() {
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), vec![]);

    assert!(validator::validate_transaction(&tx).is_ok());
}

#[test]
fn valid_call_with_mixed_args_passes() {
    let args = vec![
        obj_input(Address::fill(0x10)),
        obj_input(Address::fill(0x11)),
        Input::Raw(vec![1, 2, 3]),
    ];
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), args);

    assert!(validator::validate_transaction(&tx).is_ok());
}

#[test]
fn raw_args_with_same_content_do_not_alias() {
    let args = vec![Input::Raw(vec![1, 2, 3]), Input::Raw(vec![1, 2, 3])];
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), args);

    assert!(validator::validate_transaction(&tx).is_ok());
}

#[test]
fn call_too_large_returns_error() {
    let large_arg = Input::Raw(vec![0u8; MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE]);
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), vec![large_arg]);

    assert!(matches!(
        validator::validate_transaction(&tx),
        Err(ValidationError::TransactionTooLarge { size, limit }) if size > MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE && limit == MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE
    ));
}

#[test]
fn too_many_args_returns_error() {
    // Default config is used for validation so we can use it here.
    let config = config::compiler_config();

    let max_params = config.max_params();
    // Build one more arg than the limit allows.
    let args: Vec<_> = (0..=max_params)
        .map(|i| obj_input(Address::fill(i)))
        .collect();
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), args);
    let expected_limit = max_params as usize;

    assert!(matches!(
        validator::validate_transaction(&tx),
        Err(ValidationError::TooManyCallArguments { amount, limit })
            if amount == expected_limit + 1 && limit == expected_limit
    ));
}

#[test]
fn gas_coin_as_call_arg_returns_error() {
    let byte = 0xCC;
    let gas_coin = Address::fill(byte);
    let tx = call_tx(
        Address::fill(0xAA),
        gas_coin_ref(byte),
        vec![obj_input(gas_coin)],
    );

    assert!(matches!(
        validator::validate_transaction(&tx),
        Err(ValidationError::GasCoinUsedAsCallArgument(a)) if a == gas_coin
    ));
}

#[test]
fn aliased_object_arg_returns_error() {
    let obj = Address::fill(0x10);
    let tx = call_tx(
        Address::fill(0xAA),
        gas_coin_ref(0xBB),
        vec![obj_input(obj), obj_input(obj)],
    );

    assert!(matches!(
        validator::validate_transaction(&tx),
        Err(ValidationError::AliasedCallArgument(a)) if a == obj
    ));
}

//
// ─── MeowModulePublish ───
//

#[test]
fn valid_publish_module_passes() {
    let tx = publish_tx(vec![0u8; 1024]);
    assert!(validator::validate_transaction(&tx).is_ok());
}

#[test]
fn module_at_exact_limit_passes() {
    let tx = publish_tx(vec![0u8; MAX_BCS_SERIALIZED_MODULE_SIZE]);
    assert!(validator::validate_transaction(&tx).is_ok());
}

#[test]
fn module_too_large_returns_error() {
    let module_size = MAX_BCS_SERIALIZED_MODULE_SIZE + 1;
    let tx = publish_tx(vec![0u8; module_size]);

    assert!(matches!(
        validator::validate_transaction(&tx),
        Err(ValidationError::ModuleTooLarge { size, limit }) if size == module_size && limit == MAX_BCS_SERIALIZED_MODULE_SIZE
    ));
}

#[test]
fn publish_module_transaction_too_large_returns_error() {
    let tx = publish_tx(vec![
        0u8;
        MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE
    ]);

    assert!(matches!(
        validator::validate_transaction(&tx),
        Err(ValidationError::TransactionTooLarge { size, limit }) if size > MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE && limit == MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE
    ));
}

//
// ─── SignedTransaction ───
//

#[test]
fn signed_transaction_verify_valid() {
    let key_pair = test_keypair();
    let tx = publish_tx_with_sender(key_pair.public().into(), vec![1, 2, 3]);

    let (signed, _) = tx.sign(&key_pair);

    assert!(validator::validate_signed_transaction(&signed).is_ok());
}

#[test]
fn signed_transaction_verify_wrong_digest() {
    let key_pair = test_keypair();
    let sender = key_pair.public().into();

    let tx1 = publish_tx_with_sender(sender, vec![1, 2, 3]);
    let tx2 = publish_tx_with_sender(sender, vec![4, 5, 6]);

    let sig = key_pair.sign(tx1.digest().as_ref());
    let signed = SignedTransaction::new(tx2, sig);

    assert!(matches!(
        validator::validate_signed_transaction(&signed).unwrap_err(),
        ValidationError::KeyPairError(KeyPairError::Ed25519ConsensusError(e))
            if e.to_string() == "Invalid signature."
    ));
}

#[test]
fn signed_transaction_verify_wrong_signer() {
    let key_pair = test_keypair();
    let tx_signer = Address::from(key_pair.public());

    let tx = publish_tx(vec![1, 2, 3]);
    let tx_sender = *tx.sender();

    let sig = test_keypair().sign(tx.digest().as_ref());
    let signed = SignedTransaction::new(tx, sig);

    assert!(matches!(
        validator::validate_signed_transaction(&signed).unwrap_err(),
        ValidationError::SignerMismatch { sender, signer } if sender == tx_sender && signer == tx_signer
    ));
}

#[test]
fn signed_transaction_verify_invalid_module_too_large() {
    let key_pair = test_keypair();
    let sender = key_pair.public().into();

    let module_size = MAX_BCS_SERIALIZED_MODULE_SIZE + 1;
    let tx = publish_tx_with_sender(sender, vec![0u8; module_size]);

    let sig = test_keypair().sign(tx.digest().as_ref());
    let signed = SignedTransaction::new(tx, sig);

    assert!(matches!(
        validator::validate_signed_transaction(&signed),
        Err(ValidationError::ModuleTooLarge { size, limit }) if size == module_size && limit == MAX_BCS_SERIALIZED_MODULE_SIZE
    ));
}

//
// ─── Utility functions ───
//

fn gas_coin_ref(byte: u8) -> ObjectRef {
    ObjectRef::new(Address::fill(byte), ObjectVersion::ONE, Digest::ZERO)
}

fn obj_input(address: Address) -> Input {
    Input::Object(ObjectRef::new(address, ObjectVersion::ONE, Digest::ZERO))
}

fn call_tx(sender: Address, gas_coin: ObjectRef, args: Vec<Input>) -> Transaction {
    Transaction::new(
        sender,
        gas_coin,
        TransactionType::MeowCall(Call::new(
            Address::fill(0x01),
            Identifier::new("run").unwrap(),
            args,
        )),
    )
}

fn publish_tx(module_bytes: Vec<u8>) -> Transaction {
    publish_tx_with_sender(Address::fill(0xAA), module_bytes)
}

fn publish_tx_with_sender(sender: Address, module_bytes: Vec<u8>) -> Transaction {
    Transaction::new(
        sender,
        gas_coin_ref(0xBB),
        TransactionType::MeowModulePublish(module_bytes),
    )
}

fn test_keypair() -> KeyPair {
    KeyPair::random(SignatureScheme::Ed25519, StdRng::from_seed([0; 32]))
}
