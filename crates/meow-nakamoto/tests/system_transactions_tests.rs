use meow_genesis::Genesis;
use meow_nakamoto::{store::Store, system_transactions};
use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_owner::ObjectOwner, object_ref::ObjectRef, object_type::ObjectType,
        object_version::ObjectVersion,
    },
    system_framework::{
        meow_coin::{
            MEOW_COIN_MINT_FUNCTION_NAME, MEOW_COIN_MODULE_ADDRESS, MEOW_COIN_OBJECT_NAME,
        },
        meow_object::MEOW_OBJECT_MODULE_ADDRESS,
    },
    transaction::{Transaction, call::Call, input::Input, transaction_type::TransactionType},
};

//
// ─── make_reward_transaction ───
//

/// The transaction produced by `make_reward_transaction` must pass `is_valid_reward_transaction`.
#[test]
fn make_reward_transaction_produces_valid_reward_call() {
    let mining_block_hash = test_mining_block_hash();
    let transaction = system_transactions::make_reward_transaction(
        SENDER_ADDRESS,
        REWARD_ADDRESS,
        5_000,
        mining_block_hash,
    );

    assert!(system_transactions::is_valid_reward_transaction(
        &transaction,
        5_000,
        mining_block_hash
    ));
}

/// Embedding different block hashes into the gas-coin placeholder must produce
/// transactions with different digests, preventing object-ID collisions across blocks.
#[test]
fn make_reward_transaction_digest_varies_with_mining_block_hash() {
    let transaction1 = system_transactions::make_reward_transaction(
        SENDER_ADDRESS,
        REWARD_ADDRESS,
        1_000,
        Digest::from([0xEE; 32]),
    );
    let transaction2 = system_transactions::make_reward_transaction(
        SENDER_ADDRESS,
        REWARD_ADDRESS,
        1_000,
        Digest::from([0xFF; 32]),
    );

    assert_ne!(transaction1.digest(), transaction2.digest());
}

//
// ─── is_valid_reward_transaction ───
//

/// A reward transaction checked against a different amount must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_for_wrong_amount() {
    let mining_block_hash = test_mining_block_hash();
    let transaction = system_transactions::make_reward_transaction(
        SENDER_ADDRESS,
        REWARD_ADDRESS,
        1_000,
        mining_block_hash,
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        999,
        mining_block_hash
    ));
}

/// A reward transaction checked against the wrong block hash must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_for_wrong_mining_block_hash() {
    let transaction = system_transactions::make_reward_transaction(
        SENDER_ADDRESS,
        REWARD_ADDRESS,
        1_000,
        test_mining_block_hash(),
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        Digest::from([0xAB; 32])
    ));
}

/// A transaction whose gas-coin address is not `Address::ZERO` violates the placeholder
/// convention and must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_when_gas_coin_address_is_not_zero() {
    let mining_block_hash = test_mining_block_hash();
    let transaction = make_meow_call_transaction(
        ObjectRef::new(
            Address::suffixed(0xF1),
            ObjectVersion::ZERO,
            mining_block_hash,
        ),
        MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_MINT_FUNCTION_NAME,
        reward_transaction_args(1_000, REWARD_ADDRESS),
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        mining_block_hash
    ));
}

/// A transaction whose gas-coin version is not `ObjectVersion::ZERO` violates the placeholder
/// convention and must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_when_gas_coin_version_is_not_zero() {
    let mining_block_hash = test_mining_block_hash();
    let transaction = make_meow_call_transaction(
        ObjectRef::new(Address::ZERO, ObjectVersion::ONE, mining_block_hash),
        MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_MINT_FUNCTION_NAME,
        reward_transaction_args(1_000, REWARD_ADDRESS),
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        mining_block_hash
    ));
}

/// A `MeowCall` with the wrong number of arguments must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_for_wrong_argument_count() {
    let transaction = make_meow_call_transaction(
        gas_object_ref(),
        MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_MINT_FUNCTION_NAME,
        vec![Input::raw(&1_000u64).unwrap()], // one arg instead of two
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        test_mining_block_hash()
    ));
}

/// A `MeowCall` whose first argument is an object reference (not raw bytes) must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_when_first_argument_is_not_raw() {
    let transaction = make_meow_call_transaction(
        gas_object_ref(),
        MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_MINT_FUNCTION_NAME,
        vec![
            Input::Object(gas_object_ref()),
            Input::raw(&REWARD_ADDRESS).unwrap(),
        ],
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        test_mining_block_hash()
    ));
}

/// A `MeowCall` whose second argument is an object reference (not raw bytes) must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_when_second_argument_is_not_raw() {
    let transaction = make_meow_call_transaction(
        gas_object_ref(),
        MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_MINT_FUNCTION_NAME,
        vec![
            Input::raw(&1_000u64).unwrap(),
            Input::Object(gas_object_ref()),
        ],
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        test_mining_block_hash()
    ));
}

/// A `MeowCall` whose second argument cannot be deserialized as an `Address` must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_when_second_argument_is_not_an_address() {
    let transaction = make_meow_call_transaction(
        gas_object_ref(),
        MEOW_COIN_MODULE_ADDRESS,
        MEOW_COIN_MINT_FUNCTION_NAME,
        vec![
            Input::raw(&1_000u64).unwrap(),
            Input::raw(&42u8).unwrap(), // 1 byte — too short to be a 32-byte Address
        ],
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        test_mining_block_hash()
    ));
}

/// A `MeowCall` targeting the wrong module address must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_for_wrong_module() {
    let transaction = make_meow_call_transaction(
        gas_object_ref(),
        MEOW_OBJECT_MODULE_ADDRESS, // wrong: must be MEOW_COIN_MODULE_ADDRESS
        MEOW_COIN_MINT_FUNCTION_NAME,
        reward_transaction_args(1_000, REWARD_ADDRESS),
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        test_mining_block_hash()
    ));
}

/// A `MeowCall` to the correct module but the wrong function must be rejected.
#[test]
fn is_valid_reward_transaction_returns_false_for_wrong_function_name() {
    let transaction = make_meow_call_transaction(
        gas_object_ref(),
        MEOW_COIN_MODULE_ADDRESS,
        "transfer", // wrong function
        reward_transaction_args(1_000, REWARD_ADDRESS),
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        test_mining_block_hash()
    ));
}

/// A non-`MeowCall` transaction type must be rejected immediately.
#[test]
fn is_valid_reward_transaction_returns_false_for_non_meow_call() {
    let transaction = Transaction::new(
        SENDER_ADDRESS,
        gas_object_ref(),
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    );

    assert!(!system_transactions::is_valid_reward_transaction(
        &transaction,
        1_000,
        test_mining_block_hash()
    ));
}

//
// ─── collect_inputs_for_reward_transaction ───
//

/// Both framework module objects present in the store must be returned.
#[test]
fn collect_inputs_returns_both_framework_modules() {
    let store = Store::with_objects([
        make_module_object(MEOW_OBJECT_MODULE_ADDRESS),
        make_module_object(MEOW_COIN_MODULE_ADDRESS),
    ]);

    let inputs = system_transactions::collect_inputs_for_reward_transaction(&store);

    assert_eq!(inputs.len(), 2);

    let addresses: Vec<_> = inputs.iter().map(|o| *o.address()).collect();

    assert!(addresses.contains(&MEOW_OBJECT_MODULE_ADDRESS));
    assert!(addresses.contains(&MEOW_COIN_MODULE_ADDRESS));
}

/// A missing framework module is an invariant violation — the function must panic.
#[test]
#[should_panic(expected = "framework module must be present in store")]
fn collect_inputs_panics_when_framework_module_is_absent() {
    let store = Store::with_objects([make_module_object(MEOW_COIN_MODULE_ADDRESS)]);
    system_transactions::collect_inputs_for_reward_transaction(&store);
}

//
// ─── execute_reward_transaction ───
//

/// A valid reward transaction must produce exactly one `MeowCoin` object owned by the reward
/// address, with no changed or destroyed objects.
#[test]
fn execute_reward_transaction_returns_result_with_created_coin() {
    let store = genesis_store();
    let transaction = system_transactions::make_reward_transaction(
        SENDER_ADDRESS,
        REWARD_ADDRESS,
        1_000,
        test_mining_block_hash(),
    );

    let result = system_transactions::execute_reward_transaction(&transaction, &store)
        .expect("reward transaction must succeed");

    assert_eq!(result.created_objects().len(), 1);
    let coin = &result.created_objects()[0];
    assert_eq!(coin.owner(), &ObjectOwner::Address(REWARD_ADDRESS));
    let ObjectType::Object(decl_ref) = coin.type_() else {
        panic!("expected ObjectType::Object, got Module");
    };
    assert_eq!(decl_ref.module(), &MEOW_COIN_MODULE_ADDRESS);
    assert_eq!(decl_ref.name().as_ref(), MEOW_COIN_OBJECT_NAME);

    assert!(result.changed_objects().is_empty());
    assert!(result.destroyed_objects().is_empty());
}

/// A reward transaction with type `MeowModulePublish` must be rejected with `Err`.
#[test]
fn execute_reward_transaction_returns_err_for_non_meow_call() {
    let store = genesis_store();
    let transaction = Transaction::new(
        SENDER_ADDRESS,
        gas_object_ref(),
        TransactionType::MeowModulePublish(vec![]),
    );

    assert!(system_transactions::execute_reward_transaction(&transaction, &store).is_err());
}

//
// ─── Utility functions ───
//

const SENDER_ADDRESS: Address = Address::suffixed(0xE1);
const REWARD_ADDRESS: Address = Address::suffixed(0xE2);

/// Build a `MeowCall` transaction with a placeholder gas-coin and a fixed signer.
fn make_meow_call_transaction(
    gas_object_ref: ObjectRef,
    module: Address,
    function: &str,
    args: Vec<Input>,
) -> Transaction {
    Transaction::new(
        SENDER_ADDRESS,
        gas_object_ref,
        TransactionType::MeowCall(Call::new(module, Identifier::new(function).unwrap(), args)),
    )
}

fn gas_object_ref() -> ObjectRef {
    ObjectRef::new(Address::ZERO, ObjectVersion::ZERO, test_mining_block_hash())
}

/// Mining block hash embedded in `gas_object_ref()` and used as the expected block hash
/// in tests that construct transactions via `make_meow_call_transaction`.
fn test_mining_block_hash() -> Digest {
    Digest::from([0xFF; 32])
}

/// Encode the standard two-argument list for a `meow_coin::mint` call.
fn reward_transaction_args(amount: u64, recipient: Address) -> Vec<Input> {
    vec![
        Input::raw(&amount).unwrap(),
        Input::raw(&recipient).unwrap(),
    ]
}

fn make_module_object(addr: Address) -> Object {
    Object::fresh_module(addr, Digest::ZERO, vec![])
}

/// Build a `Store` pre-populated with compiled framework modules from genesis.
fn genesis_store() -> Store {
    let genesis = Genesis::build(&[]).expect("genesis must build");
    Store::with_objects(genesis.objects().iter().cloned())
}
