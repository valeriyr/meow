use std::str::FromStr;

use meow_types::{
    address::Address,
    digest::Digest,
    identifier::Identifier,
    object::{
        Object, object_decl_ref::ObjectDeclRef, object_owner::ObjectOwner, object_ref::ObjectRef,
        object_type::ObjectType, object_version::ObjectVersion,
    },
    system_framework::meow_coin::MEOW_COIN_MODULE_ADDRESS,
    transaction::{Transaction, call::Call, input::Input, transaction_type::TransactionType},
};
use meow_vm_adapter::{Module, Value, builder, inputs_resolver};

//
// ─── collect_inputs ───
//

#[test]
fn collect_inputs_publish_tx_returns_only_gas_coin() {
    let gas_obj = make_gas_obj();
    let tx = make_publish_tx(vec![]);

    let inputs = inputs_resolver::collect_inputs(&tx, |addr| {
        if addr == gas_obj.address() {
            Some(gas_obj.clone())
        } else {
            None
        }
    });

    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].address(), gas_obj.address());
}

#[test]
fn collect_inputs_meow_call_returns_gas_then_module() {
    let module_addr = Address::from_str("0x01").unwrap();
    let module_obj = make_module_obj(module_addr, SIMPLE_SRC, &[]);
    let gas_obj = make_gas_obj();
    let tx = make_call_tx(module_addr, "noop", vec![]);

    let inputs = inputs_resolver::collect_inputs(&tx, |addr| {
        if addr == gas_obj.address() {
            return Some(gas_obj.clone());
        }
        if addr == &module_addr {
            return Some(module_obj.clone());
        }
        None
    });

    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].address(), gas_obj.address());
    assert_eq!(inputs[1].address(), &module_addr);
}

#[test]
fn collect_inputs_dep_module_comes_before_main_module() {
    let dep_addr = Address::from_str("0x10").unwrap();
    let main_addr = Address::from_str("0x20").unwrap();

    let dep_module = compile(DEP_SRC, &[]);
    let main_module = compile(&main_src(dep_addr), &[(dep_addr, &dep_module)]);

    let dep_obj = make_obj_from_module(dep_addr, &dep_module);
    let main_obj = make_obj_from_module(main_addr, &main_module);
    let gas_obj = make_gas_obj();

    let tx = make_call_tx(main_addr, "noop", vec![]);

    let inputs = inputs_resolver::collect_inputs(&tx, |addr| {
        if addr == gas_obj.address() {
            return Some(gas_obj.clone());
        }
        if addr == &main_addr {
            return Some(main_obj.clone());
        }
        if addr == &dep_addr {
            return Some(dep_obj.clone());
        }
        None
    });

    // Expected order: gas, dep, main
    assert_eq!(inputs.len(), 3);
    assert_eq!(inputs[0].address(), gas_obj.address());
    assert_eq!(inputs[1].address(), &dep_addr, "dep must come before main");
    assert_eq!(inputs[2].address(), &main_addr);
}

#[test]
fn collect_inputs_missing_gas_coin_is_skipped() {
    let module_addr = Address::from_str("0x01").unwrap();
    let module_obj = make_module_obj(module_addr, SIMPLE_SRC, &[]);
    let tx = make_call_tx(module_addr, "noop", vec![]);

    let inputs = inputs_resolver::collect_inputs(&tx, |addr| {
        if addr == &module_addr {
            Some(module_obj.clone())
        } else {
            None
        }
    });

    // Gas coin missing — only module present
    assert!(!inputs.iter().any(|o| o.address() == &GAS_ADDR));
    assert!(inputs.iter().any(|o| o.address() == &module_addr));
}

#[test]
fn collect_inputs_missing_module_is_skipped() {
    let module_addr = Address::from_str("0x01").unwrap();
    let gas_obj = make_gas_obj();
    let tx = make_call_tx(module_addr, "noop", vec![]);

    let inputs = inputs_resolver::collect_inputs(&tx, |addr| {
        if addr == gas_obj.address() {
            Some(gas_obj.clone())
        } else {
            None
        }
    });

    // Module missing — only gas coin
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].address(), gas_obj.address());
}

#[test]
fn collect_inputs_includes_object_call_args() {
    let module_addr = Address::from_str("0x01").unwrap();
    let arg_addr = Address::from_str("0xCC").unwrap();
    let module_obj = make_module_obj(module_addr, SIMPLE_SRC, &[]);
    let arg_obj = make_gas_obj_at(arg_addr);
    let gas_obj = make_gas_obj();

    let arg_ref = ObjectRef::new(arg_addr, ObjectVersion::ONE, Digest::ZERO);
    let tx = make_call_tx(module_addr, "noop", vec![Input::Object(arg_ref)]);

    let inputs = inputs_resolver::collect_inputs(&tx, |addr| {
        if addr == gas_obj.address() {
            return Some(gas_obj.clone());
        }
        if addr == &module_addr {
            return Some(module_obj.clone());
        }
        if addr == &arg_addr {
            return Some(arg_obj.clone());
        }
        None
    });

    assert_eq!(inputs.len(), 3); // gas, module, arg
    assert_eq!(inputs[2].address(), &arg_addr);
}

//
// ─── collect_inputs_async ───
//

#[tokio::test]
async fn collect_inputs_async_returns_gas_and_module() {
    let module_addr = Address::from_str("0x01").unwrap();
    let module_obj = make_module_obj(module_addr, SIMPLE_SRC, &[]);
    let gas_obj = make_gas_obj();
    let tx = make_call_tx(module_addr, "noop", vec![]);

    let inputs = inputs_resolver::collect_inputs_async(&tx, |addr| {
        let gas = gas_obj.clone();
        let module = module_obj.clone();
        async move {
            if addr == *gas.address() {
                Some(gas)
            } else if addr == module_addr {
                Some(module)
            } else {
                None
            }
        }
    })
    .await;

    assert_eq!(inputs.len(), 2);
    assert_eq!(inputs[0].address(), gas_obj.address());
    assert_eq!(inputs[1].address(), &module_addr);
}

#[tokio::test]
async fn collect_inputs_async_fetches_transitive_dep() {
    let dep_addr = Address::from_str("0x10").unwrap();
    let main_addr = Address::from_str("0x20").unwrap();

    let dep_module = compile(DEP_SRC, &[]);
    let main_module = compile(&main_src(dep_addr), &[(dep_addr, &dep_module)]);

    let dep_obj = make_obj_from_module(dep_addr, &dep_module);
    let main_obj = make_obj_from_module(main_addr, &main_module);
    let gas_obj = make_gas_obj();

    let tx = make_call_tx(main_addr, "noop", vec![]);

    let inputs = inputs_resolver::collect_inputs_async(&tx, |addr| {
        let gas = gas_obj.clone();
        let dep = dep_obj.clone();
        let main = main_obj.clone();
        async move {
            if addr == *gas.address() {
                Some(gas)
            } else if addr == dep_addr {
                Some(dep)
            } else if addr == main_addr {
                Some(main)
            } else {
                None
            }
        }
    })
    .await;

    // Expected order: gas, dep, main
    assert_eq!(inputs.len(), 3);
    assert_eq!(inputs[0].address(), gas_obj.address());
    assert_eq!(inputs[1].address(), &dep_addr, "dep must come before main");
    assert_eq!(inputs[2].address(), &main_addr);
}

//
// ─── load_deps_async ───
//

#[tokio::test]
async fn load_deps_async_returns_empty_for_no_deps() {
    let loaded = inputs_resolver::load_deps_async(&[], |_addr| async { None::<Module> }).await;
    assert!(loaded.is_empty());
}

#[tokio::test]
async fn load_deps_async_fetches_direct_dep() {
    let dep_addr = Address::from_str("0x10").unwrap();
    let dep_module = compile(DEP_SRC, &[]);

    let dep_decl = vec![("point".to_string(), dep_addr)];
    let loaded = inputs_resolver::load_deps_async(&dep_decl, |addr| {
        let m = dep_module.clone();
        async move { if addr == dep_addr { Some(m) } else { None } }
    })
    .await;

    assert_eq!(loaded.len(), 1);
    assert!(loaded.contains_key(&dep_addr));
}

#[tokio::test]
async fn load_deps_async_follows_transitive_imports() {
    let dep_addr = Address::from_str("0x10").unwrap();
    let main_addr = Address::from_str("0x20").unwrap();

    let dep_module = compile(DEP_SRC, &[]);
    let main_module = compile(&main_src(dep_addr), &[(dep_addr, &dep_module)]);

    // Caller only declares main as a direct dep; dep should be discovered transitively.
    let dep_decl = vec![("shapes".to_string(), main_addr)];
    let loaded = inputs_resolver::load_deps_async(&dep_decl, |addr| {
        let dep = dep_module.clone();
        let main = main_module.clone();
        async move {
            if addr == main_addr {
                Some(main)
            } else if addr == dep_addr {
                Some(dep)
            } else {
                None
            }
        }
    })
    .await;

    assert_eq!(
        loaded.len(),
        2,
        "both direct and transitive dep must be loaded"
    );
    assert!(loaded.contains_key(&main_addr));
    assert!(loaded.contains_key(&dep_addr));
}

#[tokio::test]
async fn load_deps_async_deduplicates_diamond_deps() {
    // A and B both depend on C. C should be fetched once.
    let c_addr = Address::from_str("0x10").unwrap();
    let a_addr = Address::from_str("0x20").unwrap();
    let b_addr = Address::from_str("0x30").unwrap();

    let c_module = compile(DEP_SRC, &[]);
    let a_module = compile(&main_src(c_addr), &[(c_addr, &c_module)]);
    let b_module = compile(&main_src(c_addr), &[(c_addr, &c_module)]);

    let dep_decl = vec![("a".to_string(), a_addr), ("b".to_string(), b_addr)];

    let fetch_count = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let count = fetch_count.clone();

    let loaded = inputs_resolver::load_deps_async(&dep_decl, move |addr| {
        let c = c_module.clone();
        let a = a_module.clone();
        let b = b_module.clone();
        let cnt = count.clone();
        async move {
            if addr == c_addr {
                cnt.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                Some(c)
            } else if addr == a_addr {
                Some(a)
            } else if addr == b_addr {
                Some(b)
            } else {
                None
            }
        }
    })
    .await;

    assert_eq!(loaded.len(), 3);
    assert_eq!(
        fetch_count.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "C must be fetched only once"
    );
}

//
// ─── Helpers ───
//

const SENDER: Address = Address::fill(0xAA);
const GAS_ADDR: Address = Address::fill(0xBB);

const SIMPLE_SRC: &str = r#"
    mod simple;
    pub fn noop() {}
    "#;
const DEP_SRC: &str = r#"
    mod point;
    pub struct Point { pub x: u64, pub y: u64 }
    "#;

/// Returns source for a module that imports `point` at `dep_addr`.
/// The address is embedded in the `use` declaration so the compiler can resolve it.
fn main_src(dep_addr: Address) -> String {
    format!(
        r#"
            mod shapes;

            use point@{dep_addr};

            pub struct Line {{ pub a: point::Point, pub b: point::Point }}
            pub fn noop() {{}}
         "#
    )
}

fn compile(src: &str, deps: &[(Address, &Module)]) -> Module {
    builder::build(src, deps).expect("must compile")
}

fn make_module_obj(addr: Address, src: &str, deps: &[(Address, &Module)]) -> Object {
    let module = compile(src, deps);
    make_obj_from_module(addr, &module)
}

fn make_obj_from_module(addr: Address, module: &Module) -> Object {
    let bytes = bcs::to_bytes(module).expect("module must serialize");
    Object::fresh_module(addr, Digest::ZERO, bytes)
}

fn make_gas_obj() -> Object {
    make_gas_obj_at(GAS_ADDR)
}

fn make_gas_obj_at(addr: Address) -> Object {
    let fields: Vec<(String, Value)> = vec![("balance".to_string(), Value::U64(1_000_000))];
    let content = bcs::to_bytes(&fields).expect("fields must serialize");
    let decl_ref = ObjectDeclRef::new(
        MEOW_COIN_MODULE_ADDRESS,
        Identifier::new("MeowCoin").unwrap(),
    );
    Object::new(
        addr,
        ObjectOwner::Address(SENDER),
        Digest::ZERO,
        ObjectVersion::ONE,
        ObjectType::Object(decl_ref),
        content,
    )
}

fn make_call_tx(module: Address, fn_name: &str, args: Vec<Input>) -> Transaction {
    let gas_ref = ObjectRef::new(GAS_ADDR, ObjectVersion::ONE, Digest::ZERO);
    let call = Call::new(module, Identifier::new(fn_name).unwrap(), args);
    Transaction::new(SENDER, gas_ref, TransactionType::MeowCall(call))
}

fn make_publish_tx(_bytes: Vec<u8>) -> Transaction {
    let gas_ref = ObjectRef::new(GAS_ADDR, ObjectVersion::ONE, Digest::ZERO);
    Transaction::new(
        SENDER,
        gas_ref,
        TransactionType::MeowModulePublish(vec![1, 2, 3]),
    )
}
