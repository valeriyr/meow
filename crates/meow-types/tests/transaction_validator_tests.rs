use meow_types::{
    address::Address,
    config::{
        self, MAX_BCS_SERIALIZED_MODULE_SIZE, MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE,
        MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE,
    },
    digest::Digest,
    identifier::Identifier,
    object::{object_ref::ObjectRef, object_version::ObjectVersion},
    transaction::{
        Transaction,
        call::{Call, Input},
        transaction_type::TransactionType,
        validator::{self, ValidationError},
    },
};

//
// ─── MeowCall ───
//

#[test]
fn valid_call_with_no_args_passes() {
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), vec![]);

    assert!(validate_transaction(&tx).is_ok());
}

#[test]
fn valid_call_with_mixed_args_passes() {
    let args = vec![
        obj_input(Address::fill(0x10)),
        obj_input(Address::fill(0x11)),
        Input::Raw(vec![1, 2, 3]),
    ];
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), args);

    assert!(validate_transaction(&tx).is_ok());
}

#[test]
fn raw_args_with_same_content_do_not_alias() {
    let args = vec![Input::Raw(vec![1, 2, 3]), Input::Raw(vec![1, 2, 3])];
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), args);

    assert!(validate_transaction(&tx).is_ok());
}

#[test]
fn call_too_large_returns_error() {
    let large_arg = Input::Raw(vec![0u8; MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE]);
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), vec![large_arg]);

    assert!(matches!(
        validate_transaction(&tx),
        Err(ValidationError::TransactionTooLarge { size, limit }) if size > MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE && limit == MEOW_CALL_TRANSACTION_BCS_BYTES_MAX_SIZE
    ));
}

#[test]
fn too_many_args_returns_error() {
    let config = config::compiler_config();

    let args_amount = config.max_params() + 1;
    let args: Vec<_> = (0..(args_amount as u8))
        .map(|i| obj_input(Address::fill(i)))
        .collect();
    let tx = call_tx(Address::fill(0xAA), gas_coin_ref(0xBB), args);

    assert!(matches!(
        validator::validate_transaction(&tx, &config),
        Err(ValidationError::TooManyCallArguments { amount, limit })
            if amount == args_amount && limit == config.max_params()
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
        validate_transaction(&tx),
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
        validate_transaction(&tx),
        Err(ValidationError::AliasedCallArgument(a)) if a == obj
    ));
}

//
// ─── MeowModulePublish ───
//

#[test]
fn valid_publish_module_passes() {
    let tx = publish_tx(vec![0u8; 1024]);
    assert!(validate_transaction(&tx).is_ok());
}

#[test]
fn module_at_exact_limit_passes() {
    let tx = publish_tx(vec![0u8; MAX_BCS_SERIALIZED_MODULE_SIZE]);
    assert!(validate_transaction(&tx).is_ok());
}

#[test]
fn module_too_large_returns_error() {
    let module_size = MAX_BCS_SERIALIZED_MODULE_SIZE + 1;
    let tx = publish_tx(vec![0u8; module_size]);
    assert!(matches!(
        validate_transaction(&tx),
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
        validate_transaction(&tx),
        Err(ValidationError::TransactionTooLarge { size, limit }) if size > MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE && limit == MEOW_PUBLISH_MODULE_TRANSACTION_BCS_BYTES_MAX_SIZE
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
    Transaction::new(
        Address::fill(0xAA),
        gas_coin_ref(0xBB),
        TransactionType::MeowModulePublish(module_bytes),
    )
}

fn validate_transaction(transaction: &Transaction) -> validator::Result<()> {
    validator::validate_transaction(transaction, &config::compiler_config())
}
