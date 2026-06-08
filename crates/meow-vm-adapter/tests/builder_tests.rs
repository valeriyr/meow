use std::str::FromStr;

use meow_types::{
    address::Address,
    config::NATIVE_FUNCTION_NAMES,
    system_framework::{
        meow_coin::MEOW_COIN_MODULE_ADDRESS, meow_object::MEOW_OBJECT_MODULE_ADDRESS,
    },
};
use meow_vm_adapter::builder::{self, MAX_SOURCE_SIZE, error::BuilderError};
use meow_vm_types::identifier::RESERVED_FUNCTION_NAMES;

//
// ─── Success cases ───
//

#[test]
fn build_module_successful() {
    let src = r#"
        mod test;

        struct Point { x: u64, y: u64 }
        struct Token { id: address, amount: u64 }

        fn make(x: u64, y: u64) -> Point { Point { x: x, y: y } }
    "#;

    let module = builder::build(src, &[]).unwrap();

    assert_eq!(module.name, "test");

    let function = module.get_function("make").unwrap();
    assert_eq!(function.params.len(), 2);
    assert!(function.return_type.is_some());

    let point = module.get_struct("Point").unwrap();
    assert_eq!(point.fields.len(), 2);

    let token = module.get_struct("Token").unwrap();
    assert_eq!(token.fields.len(), 2);
}

#[test]
fn build_module_name_comes_from_source_declaration() {
    let src = r#"
        mod my_module;

        fn noop() {}
    "#;
    let module = builder::build(src, &[]).unwrap();
    assert_eq!(module.name, "my_module");
}

#[test]
fn build_module_from_file_successful() {
    let meow_object_module = builder::build_from_file(MEOW_OBJECT_MODULE_PATH, &[]).unwrap();

    assert_eq!(meow_object_module.name, "meow_object");

    let meow_coin_module = builder::build_from_file(
        MEOW_COIN_MODULE_PATH,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_module)],
    )
    .unwrap();

    assert_eq!(meow_coin_module.name, "meow_coin");
}

#[test]
fn build_from_file_with_dep_successful() {
    // Write the main module source to a temp file and build it with a pre-compiled dep.
    let dep_addr = Address::from_str("0xFD").unwrap();
    let dep = builder::build(
        r#"
            mod math;

            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
        &[],
    )
    .unwrap();

    let path = std::env::temp_dir().join("meow_build_from_file_with_dep_test.meow");
    std::fs::write(
        &path,
        r#"
            mod main;

            use math@0xFD;

            fn run() -> u64 { math::add(1, 2) }
        "#,
    )
    .unwrap();

    let module = builder::build_from_file(&path, &[(dep_addr, &dep)]).unwrap();
    assert_eq!(module.name, "main");
    assert!(module.get_function("run").is_some());
}

//
// ─── Errors ───
//

#[test]
fn build_invalid_source_returns_error() {
    let src = "this is not valid meow";
    let err = builder::build(src, &[]).unwrap_err();
    assert!(matches!(&err, BuilderError::CompileError(e) if e.to_string().contains("found 't'")));
}

#[test]
fn build_from_nonexistent_file_returns_io_error() {
    assert!(matches!(
        builder::build_from_file("/nonexistent/path/module.meow", &[]).unwrap_err(),
        BuilderError::IoError(_)
    ));
}

#[test]
fn build_source_size_limit() {
    // Create a source string that exceeds MAX_SOURCE_SIZE by 1 byte.
    let oversized = " ".repeat(MAX_SOURCE_SIZE + 1);

    assert!(
        matches!(
            builder::build(&oversized, &[]).unwrap_err(),
            BuilderError::SourceTooLarge { .. }
        ),
        "source exceeding MAX_SOURCE_SIZE must return SourceTooLarge"
    );
}

//
// ─── extract_module_deps ───
//

#[test]
fn extract_module_deps_returns_empty_for_no_deps() {
    let src = r#"
        mod main;

        fn noop() {}
    "#;
    assert!(builder::extract_module_deps(src).unwrap().is_empty());
}

#[test]
fn extract_module_deps_returns_declared_imports_in_order() {
    let src = r#"
        mod main;

        use math@0xD1;
        use util@0xD2;

        fn noop() {}
    "#;
    let deps = builder::extract_module_deps(src).unwrap();
    assert_eq!(deps.len(), 2);
    assert_eq!(
        deps[0],
        ("math".to_string(), None, Address::from_str("0xD1").unwrap())
    );
    assert_eq!(
        deps[1],
        ("util".to_string(), None, Address::from_str("0xD2").unwrap())
    );
}

#[test]
fn extract_module_deps_with_alias_returns_some_alias() {
    let src = r#"
        mod main;

        use math@0xFD as m;

        fn noop() {}
    "#;
    let deps = builder::extract_module_deps(src).unwrap();
    assert_eq!(deps.len(), 1);
    assert_eq!(
        deps[0],
        (
            "math".to_string(),
            Some("m".to_string()),
            Address::from_str("0xFD").unwrap()
        )
    );
}

#[test]
fn extract_module_deps_source_too_large_returns_error() {
    let oversized = " ".repeat(MAX_SOURCE_SIZE + 1);
    assert!(matches!(
        builder::extract_module_deps(&oversized).unwrap_err(),
        BuilderError::SourceTooLarge { .. }
    ));
}

#[test]
fn extract_module_deps_missing_module_decl_returns_error() {
    assert!(matches!(
        builder::extract_module_deps("use helper@0xFD;").unwrap_err(),
        BuilderError::CompileError(_)
    ));
}

#[test]
fn extract_module_deps_duplicate_use_returns_error() {
    let src = r#"
        mod main;

        use helper@0xFD;
        use helper@0xFD;
    "#;
    assert!(matches!(
        builder::extract_module_deps(src).unwrap_err(),
        BuilderError::CompileError(_)
    ));
}

//
// ─── read_source_file ───
//

#[test]
fn read_source_file_returns_content() {
    let content = builder::read_source_file(MEOW_COIN_MODULE_PATH).unwrap();
    assert!(!content.is_empty());
    assert!(content.contains("mod meow_coin"));
}

#[test]
fn read_source_file_nonexistent_returns_io_error() {
    assert!(matches!(
        builder::read_source_file("/nonexistent/path.meow").unwrap_err(),
        BuilderError::IoError(_)
    ));
}

#[test]
fn read_source_file_too_large_returns_error() {
    // read_source_file checks the file's metadata size before reading content.
    let path = std::env::temp_dir().join("meow_read_source_oversized_test.meow");
    std::fs::write(&path, " ".repeat(MAX_SOURCE_SIZE + 1)).unwrap();
    assert!(matches!(
        builder::read_source_file(&path).unwrap_err(),
        BuilderError::SourceTooLarge { .. }
    ));
}

//
// ─── build with deps ───
//

#[test]
fn build_with_dep_cross_module_function_call() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let math = builder::build(
        r#"
            mod math;

            pub fn add(a: u64, b: u64) -> u64 { a + b }
        "#,
        &[],
    )
    .unwrap();

    let caller = builder::build(
        r#"
            mod caller;

            use math@0xFD;

            fn double_add(a: u64, b: u64) -> u64 {
                math::add(a, b) + math::add(a, b)
            }
        "#,
        &[(d_addr, &math)],
    )
    .unwrap();

    assert!(caller.get_function("double_add").is_some());
    assert_eq!(caller.imports, vec![d_addr.into()]);
}

#[test]
fn build_with_dep_cross_module_struct() {
    let d_addr = Address::from_str("0xFD").unwrap();
    let shapes = builder::build(
        r#"
            mod shapes;

            pub struct Point { x: u64, y: u64 }

            pub fn make_point(x: u64, y: u64) -> Point { Point { x: x, y: y } }
            pub fn to_x(p: Point) -> u64 { let Point { x, .. } = p; x }
        "#,
        &[],
    )
    .unwrap();

    let user = builder::build(
        r#"
            mod user;

            use shapes@0xFD;

            fn make_and_read() -> u64 {
                let p = shapes::make_point(5, 9);
                shapes::to_x(p)
            }
        "#,
        &[(d_addr, &shapes)],
    )
    .unwrap();

    assert!(user.get_function("make_and_read").is_some());
}

#[test]
fn build_with_declared_dep_not_provided_returns_error() {
    // Source declares `use math@0xFD` but no dep module is provided.
    let src = r#"
        mod main;

        use math@0xFD;

        fn run() -> u64 { math::add(1, 2) }
    "#;
    assert!(matches!(
        builder::build(src, &[]).unwrap_err(),
        BuilderError::CompileError(_)
    ));
}

#[test]
fn build_with_extra_undeclared_dep_is_accepted() {
    // Providing a dep that is not declared via `use` in source is fine — it is silently ignored.
    let d_addr = Address::from_str("0xFD").unwrap();
    let extra = builder::build(
        r#"
            mod extra;

            fn noop() {}
        "#,
        &[],
    )
    .unwrap();

    let module = builder::build(
        r#"
            mod main;

            fn run() -> u64 { 1 }
        "#,
        &[(d_addr, &extra)],
    )
    .unwrap();

    assert!(module.imports.is_empty());
}

#[test]
fn meow_vm_transfer_on_cross_module_struct_rejected() {
    // meow_vm_transfer only accepts structs defined in the calling module.
    // Passing a meow_coin::MeowCoin (defined in meow_coin, not in this module) must
    // be rejected at compile time with a clear error message.
    let meow_object_mod = builder::build_from_file(MEOW_OBJECT_MODULE_PATH, &[]).unwrap();
    let meow_coin_mod = builder::build_from_file(
        MEOW_COIN_MODULE_PATH,
        &[(MEOW_OBJECT_MODULE_ADDRESS, &meow_object_mod)],
    )
    .unwrap();
    let err = builder::build(
        &format!(
            r#"
                mod test;

                use meow_coin@{MEOW_COIN_MODULE_ADDRESS};

                pub fn touch(coin: meow_coin::MeowCoin) {{
                    meow_vm_transfer(coin, meow_vm_sender());
                }}
            "#
        ),
        &[
            (MEOW_OBJECT_MODULE_ADDRESS, &meow_object_mod),
            (MEOW_COIN_MODULE_ADDRESS, &meow_coin_mod),
        ],
    )
    .unwrap_err();
    assert!(
        matches!(&err, BuilderError::CompileError(e) if e.to_string().contains("expected a struct defined in this module")),
        "unexpected error: {err}"
    );
}

//
// ─── Reserved native names ───
//

#[test]
fn defining_adapter_native_function_name_is_rejected() {
    let mut native_functions = RESERVED_FUNCTION_NAMES.to_vec();
    native_functions.extend(NATIVE_FUNCTION_NAMES);

    for name in native_functions {
        let src = format!(
            r#"
                mod test;

                fn {name}() {{}}
            "#
        );
        assert!(
            matches!(
                builder::build(&src, &[]).unwrap_err(),
                BuilderError::CompileError(_)
            ),
            "defining a function named '{name}' must be rejected by the adapter builder"
        );
    }
}

//
// ─── Helpers ───
//

const MEOW_OBJECT_MODULE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../meow-framework/modules/meow_object.meow"
);

const MEOW_COIN_MODULE_PATH: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../meow-framework/modules/meow_coin.meow"
);
