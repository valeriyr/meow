mod utils;

use meow_vm::gas_meter::GasMeter;
use meow_vm_types::{address::Address, types::Value};

//
// ─── Object ───
//

#[test]
fn object_construction_and_field_access() {
    let src = r#"
        mod test;

        object Coin { id: address, balance: u64 }

        pub fn make_coin(balance: u64) -> u64 {
            let c = Coin { id: meow_vm_fresh_id(), balance: balance };
            c.balance
        }
    "#;
    let vm = utils::vm_with_natives(src, vec![utils::fresh_id_native()]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("make_coin", vec![Value::U64(100)], &mut gas)
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(100)));
}

#[test]
fn object_can_be_returned_from_function() {
    let src = r#"
        mod test;

        object Coin { id: address, balance: u64 }

        fn make(balance: u64) -> Coin {
            Coin { id: meow_vm_fresh_id(), balance: balance }
        }

        pub fn make_and_return(balance: u64) -> Coin {
            make(balance)
        }
    "#;
    let vm = utils::vm_with_natives(src, vec![utils::fresh_id_native()]);
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call("make_and_return", vec![Value::U64(99)], &mut gas)
        .unwrap();
    assert!(matches!(
        &r.return_value,
        Some(Value::Object { type_name, fields })
            if type_name == "Coin"
                && fields.iter().any(|(k, v)| k == "balance" && *v == Value::U64(99))
    ));
}

#[test]
fn object_field_mutation_reflected_in_final_args() {
    let src = r#"
        mod test;

        object Coin { id: address, balance: u64 }

        pub fn double_balance(coin: Coin) -> u64 {
            coin.balance = coin.balance * 2;
            coin.balance
        }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();
    let r = vm
        .call(
            "double_balance",
            vec![test_coin(Address::fill(1), 50)],
            &mut gas,
        )
        .unwrap();
    assert_eq!(r.return_value, Some(Value::U64(100)));
    assert_eq!(r.final_args[0], Some(test_coin(Address::fill(1), 100)));
}

#[test]
fn object_string_field_round_trip() {
    let src = r#"
        mod test;

        object Msg { id: address, text: string }

        pub fn get_text(m: Msg) -> string { m.text }
    "#;
    let vm = utils::vm(utils::compile(src));
    let mut gas = GasMeter::unlimited();
    let msg = Value::Object {
        type_name: "Msg".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(Address::ZERO)),
            ("text".to_string(), Value::Str("hello".to_string())),
        ],
    };
    let r = vm.call("get_text", vec![msg], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::Str("hello".to_string())));
}

//
// ─── Utility functions ───
//

fn test_coin(id: Address, balance: u64) -> Value {
    Value::Object {
        type_name: "Coin".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("balance".to_string(), Value::U64(balance)),
        ],
    }
}
