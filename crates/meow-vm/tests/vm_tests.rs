use meow_vm::{
    compiler::Compiler,
    error::VmError,
    types::Value,
    vm::{GasMeter, NativeFnEntry, NativeResult, Vm, VmCallResult},
};

fn make_vm(source: &str) -> Vm {
    let module = Compiler::compile("test", source).expect("compilation failed");
    Vm::new(module, vec![])
}

fn run(source: &str, fn_name: &str, args: Vec<Value>) -> VmCallResult {
    let vm = make_vm(source);
    let mut gas = GasMeter::unlimited();
    vm.call(fn_name, args, &mut gas).expect("execution failed")
}

// ─── Arithmetic ───────────────────────────────────────────────────────────────

#[test]
fn test_add() {
    let r = run("fn add(a: u64, b: u64): u64 { return a + b; }", "add", vec![Value::U64(3), Value::U64(4)]);
    assert_eq!(r.return_value, Some(Value::U64(7)));
}

#[test]
fn test_sub() {
    let r = run("fn sub(a: u64, b: u64): u64 { return a - b; }", "sub", vec![Value::U64(10), Value::U64(3)]);
    assert_eq!(r.return_value, Some(Value::U64(7)));
}

#[test]
fn test_mul() {
    let r = run("fn mul(a: u64, b: u64): u64 { return a * b; }", "mul", vec![Value::U64(6), Value::U64(7)]);
    assert_eq!(r.return_value, Some(Value::U64(42)));
}

#[test]
fn test_div() {
    let r = run("fn div(a: u64, b: u64): u64 { return a / b; }", "div", vec![Value::U64(20), Value::U64(4)]);
    assert_eq!(r.return_value, Some(Value::U64(5)));
}

#[test]
fn test_division_by_zero() {
    let vm = make_vm("fn div(a: u64, b: u64): u64 { return a / b; }");
    let mut gas = GasMeter::unlimited();
    let err = vm.call("div", vec![Value::U64(10), Value::U64(0)], &mut gas).unwrap_err();
    assert!(matches!(err, VmError::DivisionByZero));
}

// ─── Comparisons ─────────────────────────────────────────────────────────────

#[test]
fn test_eq() {
    let src = "fn eq(a: u64, b: u64): bool { return a == b; }";
    let r1 = run(src, "eq", vec![Value::U64(5), Value::U64(5)]);
    assert_eq!(r1.return_value, Some(Value::Bool(true)));
    let r2 = run(src, "eq", vec![Value::U64(5), Value::U64(6)]);
    assert_eq!(r2.return_value, Some(Value::Bool(false)));
}

#[test]
fn test_lt() {
    let src = "fn lt(a: u64, b: u64): bool { return a < b; }";
    let r = run(src, "lt", vec![Value::U64(3), Value::U64(5)]);
    assert_eq!(r.return_value, Some(Value::Bool(true)));
}

// ─── Let bindings ─────────────────────────────────────────────────────────────

#[test]
fn test_let_binding() {
    let src = r#"
        fn compute(x: u64): u64 {
            let a = x + 1;
            let b = a * 2;
            return b;
        }
    "#;
    let r = run(src, "compute", vec![Value::U64(4)]);
    assert_eq!(r.return_value, Some(Value::U64(10)));
}

// ─── Structs ──────────────────────────────────────────────────────────────────

#[test]
fn test_struct_construction_and_field_access() {
    let src = r#"
        struct Point { x: u64, y: u64 }

        fn make(x: u64, y: u64): Point {
            return Point { x: x, y: y };
        }

        fn get_x(p: Point): u64 {
            return p.x;
        }
    "#;

    let module = Compiler::compile("test", src).unwrap();
    let vm = Vm::new(module, vec![]);
    let mut gas = GasMeter::unlimited();

    let result = vm.call("make", vec![Value::U64(3), Value::U64(7)], &mut gas).unwrap();
    let point = result.return_value.unwrap();

    assert_eq!(
        point,
        Value::Struct {
            type_name: "Point".to_string(),
            fields: vec![
                ("x".to_string(), Value::U64(3)),
                ("y".to_string(), Value::U64(7)),
            ],
        }
    );

    let r2 = vm.call("get_x", vec![point], &mut gas).unwrap();
    assert_eq!(r2.return_value, Some(Value::U64(3)));
}

#[test]
fn test_struct_is_copyable() {
    // Structs use value semantics — they can be passed to a function
    // and the original is still accessible.
    let src = r#"
        struct Counter { value: u64 }

        fn get_value(c: Counter): u64 {
            return c.value;
        }
    "#;
    let module = Compiler::compile("test", src).unwrap();
    let vm = Vm::new(module, vec![]);
    let mut gas = GasMeter::unlimited();

    let c = Value::Struct {
        type_name: "Counter".to_string(),
        fields: vec![("value".to_string(), Value::U64(42))],
    };

    // Call twice with same struct — it is copyable.
    let r1 = vm.call("get_value", vec![c.clone()], &mut gas).unwrap();
    assert_eq!(r1.return_value, Some(Value::U64(42)));
    let r2 = vm.call("get_value", vec![c], &mut gas).unwrap();
    assert_eq!(r2.return_value, Some(Value::U64(42)));
}

// ─── Address type ─────────────────────────────────────────────────────────────

#[test]
fn test_address_round_trip() {
    let src = r#"
        fn identity(a: address): address { return a; }
    "#;
    let addr: [u8; 32] = [0xABu8; 32];
    let r = run(src, "identity", vec![Value::Address(addr)]);
    assert_eq!(r.return_value, Some(Value::Address(addr)));
}

#[test]
fn test_address_equality() {
    let src = r#"
        fn same(a: address, b: address): bool { return a == b; }
    "#;
    let addr: [u8; 32] = [1u8; 32];
    let other: [u8; 32] = [2u8; 32];
    let r1 = run(src, "same", vec![Value::Address(addr), Value::Address(addr)]);
    assert_eq!(r1.return_value, Some(Value::Bool(true)));
    let r2 = run(src, "same", vec![Value::Address(addr), Value::Address(other)]);
    assert_eq!(r2.return_value, Some(Value::Bool(false)));
}

// ─── Object creation and field access ────────────────────────────────────────

#[test]
fn test_object_construction() {
    let src = r#"
        object Coin { id: address, balance: u64 }

        fn make_coin(id: address, balance: u64): u64 {
            let c = Coin { id: id, balance: balance };
            return c.balance;
        }
    "#;
    let id: [u8; 32] = [1u8; 32];
    let r = run(src, "make_coin", vec![Value::Address(id), Value::U64(100)]);
    assert_eq!(r.return_value, Some(Value::U64(100)));
}

#[test]
fn test_object_field_mutation() {
    let src = r#"
        object Coin { id: address, balance: u64 }

        fn double_balance(coin: Coin): u64 {
            coin.balance = coin.balance * 2;
            return coin.balance;
        }
    "#;
    let id: [u8; 32] = [1u8; 32];
    let coin = Value::Object {
        type_name: "Coin".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("balance".to_string(), Value::U64(50)),
        ],
    };
    let module = Compiler::compile("test", src).unwrap();
    let vm = Vm::new(module, vec![]);
    let mut gas = GasMeter::unlimited();
    let r = vm.call("double_balance", vec![coin], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::U64(100)));
    // The coin was mutated — final_args[0] should have balance=100
    let final_coin = r.final_args[0].as_ref().unwrap();
    assert_eq!(
        *final_coin,
        Value::Object {
            type_name: "Coin".to_string(),
            fields: vec![
                ("id".to_string(), Value::Address(id)),
                ("balance".to_string(), Value::U64(100)),
            ],
        }
    );
}

// ─── If statements ───────────────────────────────────────────────────────────

#[test]
fn test_if_taken() {
    let src = r#"
        fn max(a: u64, b: u64): u64 {
            if a > b {
                return a;
            }
            return b;
        }
    "#;
    let r = run(src, "max", vec![Value::U64(10), Value::U64(5)]);
    assert_eq!(r.return_value, Some(Value::U64(10)));
}

#[test]
fn test_if_not_taken() {
    let src = r#"
        fn max(a: u64, b: u64): u64 {
            if a > b {
                return a;
            }
            return b;
        }
    "#;
    let r = run(src, "max", vec![Value::U64(3), Value::U64(8)]);
    assert_eq!(r.return_value, Some(Value::U64(8)));
}

#[test]
fn test_if_modifies_local() {
    let src = r#"
        fn clamp(x: u64, max: u64): u64 {
            let result = x;
            if x > max {
                result = max;
            }
            return result;
        }
    "#;
    let r1 = run(src, "clamp", vec![Value::U64(15), Value::U64(10)]);
    assert_eq!(r1.return_value, Some(Value::U64(10)));
    let r2 = run(src, "clamp", vec![Value::U64(5), Value::U64(10)]);
    assert_eq!(r2.return_value, Some(Value::U64(5)));
}

// ─── Field mutation via StoreField ───────────────────────────────────────────

#[test]
fn test_field_mutation_in_struct() {
    let src = r#"
        struct Counter { value: u64 }

        fn increment(c: Counter): Counter {
            c.value = c.value + 1;
            return c;
        }
    "#;
    let input = Value::Struct {
        type_name: "Counter".to_string(),
        fields: vec![("value".to_string(), Value::U64(5))],
    };
    let r = run(src, "increment", vec![input]);
    assert_eq!(
        r.return_value,
        Some(Value::Struct {
            type_name: "Counter".to_string(),
            fields: vec![("value".to_string(), Value::U64(6))],
        })
    );
}

// ─── Native function calls ───────────────────────────────────────────────────

#[test]
fn test_native_function_call() {
    let src = r#"
        fn compute(a: u64, b: u64): u64 {
            let sum = add_native(a, b);
            return sum;
        }
    "#;
    let module = Compiler::compile("test", src).unwrap();

    let add_native = NativeFnEntry {
        name: "add_native".to_string(),
        param_count: 2,
        gas_cost: 5,
        func: Box::new(|args| {
            let a = args[0].as_u64().unwrap();
            let b = args[1].as_u64().unwrap();
            NativeResult::Return(Some(Value::U64(a + b)))
        }),
    };

    let vm = Vm::new(module, vec![add_native]);
    let mut gas = GasMeter::unlimited();
    let r = vm.call("compute", vec![Value::U64(3), Value::U64(4)], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::U64(7)));
}

#[test]
fn test_void_native_does_not_leave_garbage() {
    // A void native (returns None) should push Void onto the stack.
    // The compiler emits Pop after expression-statement calls.
    // If a void native pushed nothing, the Pop would underflow.
    let src = r#"
        fn run_side_effect(x: u64): u64 {
            log_native(x);
            return x + 1;
        }
    "#;
    let module = Compiler::compile("test", src).unwrap();

    let log_native = NativeFnEntry {
        name: "log_native".to_string(),
        param_count: 1,
        gas_cost: 1,
        func: Box::new(|_args| NativeResult::Return(None)),
    };

    let vm = Vm::new(module, vec![log_native]);
    let mut gas = GasMeter::unlimited();
    let r = vm.call("run_side_effect", vec![Value::U64(10)], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::U64(11)));
}

// ─── meow_vm_abort ───────────────────────────────────────────────────────────

#[test]
fn test_abort_returns_aborted_error() {
    let src = r#"
        fn check(x: u64) {
            if x == 0 {
                meow_vm_abort(42, "must not be zero");
            }
        }
    "#;
    let module = Compiler::compile("test", src).unwrap();

    let abort = NativeFnEntry {
        name: "meow_vm_abort".to_string(),
        param_count: 2,
        gas_cost: 1,
        func: Box::new(|args| {
            let code = args[0].as_u64().unwrap_or(0);
            let message = args[1].as_str().unwrap_or("").to_string();
            NativeResult::Abort { code, message }
        }),
    };

    let vm = Vm::new(module, vec![abort]);
    let mut gas = GasMeter::unlimited();

    // x == 0 → abort is triggered
    let err = vm.call("check", vec![Value::U64(0)], &mut gas).unwrap_err();
    assert!(matches!(err, VmError::Aborted { code: 42, .. }));

    // x != 0 → no abort
    let mut gas2 = GasMeter::unlimited();
    let r = vm.call("check", vec![Value::U64(1)], &mut gas2).unwrap();
    assert_eq!(r.return_value, None);
}

// ─── Move semantics: UseAfterMove ────────────────────────────────────────────

#[test]
fn test_use_after_move_error() {
    // An object is passed to a consuming native (simulated by Load twice).
    // We test by writing a function that tries to use an object slot after it's been consumed.
    let src = r#"
        object Token { id: address, amount: u64 }

        fn consume_twice(tok: Token) {
            move_native(tok);
            move_native(tok);
        }
    "#;
    let module = Compiler::compile("test", src).unwrap();

    let move_native = NativeFnEntry {
        name: "move_native".to_string(),
        param_count: 1,
        gas_cost: 1,
        func: Box::new(|_args| NativeResult::Return(None)),
    };

    let vm = Vm::new(module, vec![move_native]);
    let mut gas = GasMeter::unlimited();

    let id: [u8; 32] = [0u8; 32];
    let tok = Value::Object {
        type_name: "Token".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("amount".to_string(), Value::U64(100)),
        ],
    };

    let err = vm.call("consume_twice", vec![tok], &mut gas).unwrap_err();
    assert!(matches!(err, VmError::UseAfterMove(_)));
}

// ─── Void functions ───────────────────────────────────────────────────────────

#[test]
fn test_void_function_returns_none() {
    let src = r#"
        fn do_nothing() {}
    "#;
    let r = run(src, "do_nothing", vec![]);
    assert_eq!(r.return_value, None);
    assert!(r.final_args.is_empty());
}

#[test]
fn test_void_function_call_in_expression_statement() {
    // Calling a void module function as a statement should not corrupt the stack.
    let src = r#"
        fn noop() {}

        fn compute(x: u64): u64 {
            noop();
            return x * 2;
        }
    "#;
    let r = run(src, "compute", vec![Value::U64(5)]);
    assert_eq!(r.return_value, Some(Value::U64(10)));
}

// ─── Function calls ───────────────────────────────────────────────────────────

#[test]
fn test_function_call_chain() {
    let src = r#"
        fn double(n: u64): u64 { return n * 2; }
        fn quad(n: u64): u64 { return double(double(n)); }
    "#;
    let r = run(src, "quad", vec![Value::U64(3)]);
    assert_eq!(r.return_value, Some(Value::U64(12)));
}

// ─── Gas metering ─────────────────────────────────────────────────────────────

#[test]
fn test_gas_is_consumed() {
    let src = "fn add(a: u64, b: u64): u64 { return a + b; }";
    let module = Compiler::compile("test", src).unwrap();
    let vm = Vm::new(module, vec![]);
    let mut gas = GasMeter::new(10_000);
    vm.call("add", vec![Value::U64(1), Value::U64(2)], &mut gas).unwrap();
    assert!(gas.consumed() > 0);
}

#[test]
fn test_out_of_gas() {
    let src = "fn add(a: u64, b: u64): u64 { return a + b; }";
    let module = Compiler::compile("test", src).unwrap();
    let vm = Vm::new(module, vec![]);
    let mut gas = GasMeter::new(1);
    assert!(vm.call("add", vec![Value::U64(1), Value::U64(2)], &mut gas).is_err());
}

// ─── Boolean logic ────────────────────────────────────────────────────────────

#[test]
fn test_bool_and_or() {
    let src = r#"
        fn both(a: bool, b: bool): bool { return a && b; }
        fn either(a: bool, b: bool): bool { return a || b; }
    "#;
    let module = Compiler::compile("test", src).unwrap();
    let vm = Vm::new(module, vec![]);
    let mut gas = GasMeter::unlimited();

    let r = vm.call("both", vec![Value::Bool(true), Value::Bool(false)], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::Bool(false)));

    let r = vm.call("either", vec![Value::Bool(true), Value::Bool(false)], &mut gas).unwrap();
    assert_eq!(r.return_value, Some(Value::Bool(true)));
}

// ─── Compiler validation ──────────────────────────────────────────────────────

#[test]
fn test_object_must_have_id_field_first() {
    // An object without id: address as first field must fail to compile.
    let src = r#"
        object BadObject { balance: u64, id: address }

        fn make(id: address, balance: u64) {}
    "#;
    assert!(Compiler::compile("test", src).is_err());
}

#[test]
fn test_cannot_return_object_from_function() {
    let src = r#"
        object Coin { id: address, balance: u64 }

        fn make(id: address, balance: u64): Coin {
            return Coin { id: id, balance: balance };
        }
    "#;
    assert!(Compiler::compile("test", src).is_err());
}

#[test]
fn test_struct_cannot_have_object_field() {
    // Struct fields may only be primitives — no nested objects.
    let src = r#"
        object Token { id: address, amount: u64 }
        struct Wrapper { tok: Token }

        fn make(id: address, amount: u64) {}
    "#;
    assert!(Compiler::compile("test", src).is_err());
}

// ─── final_args tracking ──────────────────────────────────────────────────────

#[test]
fn test_final_args_reflects_primitive_params() {
    let src = "fn f(a: u64, b: u64): u64 { return a + b; }";
    let module = Compiler::compile("test", src).unwrap();
    let vm = Vm::new(module, vec![]);
    let mut gas = GasMeter::unlimited();
    let r = vm.call("f", vec![Value::U64(3), Value::U64(4)], &mut gas).unwrap();
    // Primitives are copyable — final_args holds their final slot values.
    assert_eq!(r.final_args.len(), 2);
    assert_eq!(r.final_args[0], Some(Value::U64(3)));
    assert_eq!(r.final_args[1], Some(Value::U64(4)));
}

#[test]
fn test_final_args_none_for_consumed_object() {
    let src = r#"
        object Token { id: address, amount: u64 }

        fn consume(tok: Token) {
            take_native(tok);
        }
    "#;
    let module = Compiler::compile("test", src).unwrap();

    let take = NativeFnEntry {
        name: "take_native".to_string(),
        param_count: 1,
        gas_cost: 1,
        func: Box::new(|_args| NativeResult::Return(None)),
    };

    let vm = Vm::new(module, vec![take]);
    let mut gas = GasMeter::unlimited();

    let id: [u8; 32] = [0u8; 32];
    let tok = Value::Object {
        type_name: "Token".to_string(),
        fields: vec![
            ("id".to_string(), Value::Address(id)),
            ("amount".to_string(), Value::U64(50)),
        ],
    };

    let r = vm.call("consume", vec![tok], &mut gas).unwrap();
    // The object was moved out of the slot — final_args[0] is None.
    assert_eq!(r.final_args.len(), 1);
    assert_eq!(r.final_args[0], None);
}
