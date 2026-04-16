use std::{collections::HashMap, str::FromStr};

use meow_vm::{Vm, error::VmError, gas_meter::GasMeter, gas_schedule::GasSchedule};
use meow_vm_compiler::Compiler;
use meow_vm_types::{
    address::Address,
    config::{CompilerConfig, VmConfig},
};

mod utils;

//
// ─── max_dep_modules ───
//

#[test]
fn dep_count_exceeding_limit_returns_error() {
    // main → B → C: two transitive deps, limit = 1 → call must fail.
    let cfg = CompilerConfig::default();
    let b_addr = Address::from_str("0x02").unwrap();
    let c_addr = Address::from_str("0x03").unwrap();

    let c_module = Compiler::compile(
        r#"
            module c;
            fn get(): u64 { return 1; }
        "#,
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let b_module = Compiler::compile(
        r#"
            module b;
            use c@0x03;
            fn get(): u64 { return c::get(); }
        "#,
        &[(c_addr, &c_module)],
        cfg.clone(),
    )
    .expect("b must compile");
    let main_module = Compiler::compile(
        r#"
            module main;
            use b@0x02;
            fn run(): u64 { return b::get(); }
        "#,
        &[(b_addr, &b_module), (c_addr, &c_module)],
        cfg,
    )
    .expect("main must compile");

    let mut deps = HashMap::new();
    deps.insert(b_addr, b_module);
    deps.insert(c_addr, c_module);

    let vm = Vm::new(
        main_module,
        vec![],
        GasSchedule::default(),
        deps,
        VmConfig::default().with_max_dep_modules(1), // 2 deps provided, only 1 allowed
    );
    let mut gas = GasMeter::unlimited();
    let err = vm.call("run", vec![], &mut gas).unwrap_err();

    assert!(
        matches!(err, VmError::TooManyDepModules(1)),
        "expected TooManyDepModules(1), got: {err:?}"
    );
}

#[test]
fn dep_count_at_limit_succeeds() {
    // Same two-dep chain, limit = 2 — must succeed.
    let cfg = CompilerConfig::default();
    let b_addr = Address::from_str("0x02").unwrap();
    let c_addr = Address::from_str("0x03").unwrap();

    let c_module = Compiler::compile(
        r#"
            module c;
            fn get(): u64 { return 1; }
        "#,
        &[],
        cfg.clone(),
    )
    .expect("c must compile");
    let b_module = Compiler::compile(
        r#"
            module b;
            use c@0x03;
            fn get(): u64 { return c::get(); }
        "#,
        &[(c_addr, &c_module)],
        cfg.clone(),
    )
    .expect("b must compile");
    let main_module = Compiler::compile(
        r#"
            module main;
            use b@0x02;
            fn run(): u64 { return b::get(); }
        "#,
        &[(b_addr, &b_module), (c_addr, &c_module)],
        cfg,
    )
    .expect("main must compile");

    let mut deps = HashMap::new();
    deps.insert(b_addr, b_module);
    deps.insert(c_addr, c_module);

    let vm = Vm::new(
        main_module,
        vec![],
        GasSchedule::default(),
        deps,
        VmConfig::default().with_max_dep_modules(2), // exactly 2 deps — at the limit
    );
    let mut gas = GasMeter::unlimited();
    assert!(vm.call("run", vec![], &mut gas).is_ok());
}

//
// ─── max_call_depth ───
//

#[test]
fn call_depth_exceeding_limit_returns_error() {
    // fn a calls fn b — depth-2 chain. With max_call_depth = 1, the call to b
    // (depth 1) must fail since 1 >= limit of 1.
    let module = utils::compile(
        r#"
        module depth_test;
        fn b(): u64 { return 1; }
        fn a(): u64 { return b(); }
    "#,
    );

    let vm = Vm::new(
        module,
        vec![],
        GasSchedule::default(),
        HashMap::new(),
        VmConfig::default().with_max_call_depth(1),
    );
    let mut gas = GasMeter::unlimited();
    let err = vm.call("a", vec![], &mut gas).unwrap_err();

    assert!(
        matches!(err, VmError::CallStackOverflow(1)),
        "expected CallStackOverflow(1), got: {err:?}"
    );
}

#[test]
fn call_depth_at_limit_succeeds() {
    // Same module, limit = 2 — a (depth 0) → b (depth 1) fits within the limit.
    let module = utils::compile(
        r#"
        module depth_test;
        fn b(): u64 { return 1; }
        fn a(): u64 { return b(); }
    "#,
    );

    let vm = Vm::new(
        module,
        vec![],
        GasSchedule::default(),
        HashMap::new(),
        VmConfig::default().with_max_call_depth(2),
    );
    let mut gas = GasMeter::unlimited();
    assert!(vm.call("a", vec![], &mut gas).is_ok());
}
