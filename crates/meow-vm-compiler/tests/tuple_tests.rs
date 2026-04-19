mod utils;

use std::str::FromStr;

use meow_vm_compiler::Result;
use meow_vm_types::{address::Address, module::Module};

//
// ─── Tuple return types ───
//

#[test]
fn tuple_return_with_primitives() {
    utils::compile(
        r#"
            mod test;
            pub fn pair(a: u64, b: u64) -> (u64, u64) { (a, b) }
        "#,
    )
    .expect("tuple return with only primitives must compile");
}

#[test]
fn tuple_return_mixed_types() {
    utils::compile(
        r#"
            mod test;
            pub fn describe(x: u64, flag: bool) -> (u64, bool) { (x, flag) }
        "#,
    )
    .expect("tuple return with mixed primitive types must compile");
}

#[test]
fn tuple_return_with_struct() {
    utils::compile(
        r#"
            mod test;
            struct Token { value: u64 }
            pub fn split(t: Token) -> (Token, Token) {
                let half = t.value / 2;
                t.value = half;
                let t2 = Token { value: half };
                (t, t2)
            }
        "#,
    )
    .expect("struct types in tuple return must compile");
}

//
// ─── Tuple destructuring ───
//

#[test]
fn tuple_destructuring_with_primitives() {
    utils::compile(
        r#"
            mod test;
            fn swap(a: u64, b: u64) -> (u64, u64) { (b, a) }
            pub fn run() -> u64 {
                let (x, y) = swap(1, 2);
                x
            }
        "#,
    )
    .expect("tuple destructuring of primitives must compile");
}

#[test]
fn tuple_destructuring_cross_module() {
    with_dep(
        r#"
            mod math;
            pub fn divmod(a: u64, b: u64) -> (u64, u64) { (a / b, a % b) }
        "#,
        r#"
            mod user;
            use math@0x01;
            pub fn quotient(a: u64, b: u64) -> u64 {
                let (q, _r) = math::divmod(a, b);
                q
            }
        "#,
    )
    .expect("cross-module tuple destructuring must compile");
}

//
// ─── Helper ───
//

fn with_dep(dep_src: &str, src: &str) -> Result<Module> {
    let dep = utils::compile(dep_src).expect("dep must compile");
    let addr = Address::from_str("0x01").unwrap();
    utils::compile_with_deps(src, &[(addr, &dep)])
}
