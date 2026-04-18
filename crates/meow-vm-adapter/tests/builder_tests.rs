use std::str::FromStr;

use meow_types::{
    address::Address, config::NATIVE_FUNCTION_NAMES,
    system_framework::meow_coin::MEOW_COIN_MODULE_PATH,
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
        object Token { id: address, amount: u64 }

        fn make(x: u64, y: u64) -> Point { return Point { x: x, y: y }; }
    "#;

    let module = builder::build(src, &[]).unwrap();

    assert_eq!(module.name, "test");

    let function = module.get_function("make").unwrap();
    assert_eq!(function.params.len(), 2);
    assert!(function.return_type.is_some());

    let point = module.get_struct("Point").unwrap();
    assert!(!point.is_object);
    assert_eq!(point.fields.len(), 2);

    let token = module.get_struct("Token").unwrap();
    assert!(token.is_object);
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
    let module = builder::build_from_file(MEOW_COIN_MODULE_PATH, &[]).unwrap();

    assert_eq!(module.name, "meow_coin");
}

#[test]
fn build_from_file_with_dep_successful() {
    // Write the main module source to a temp file and build it with a pre-compiled dep.
    let dep_addr = Address::from_str("0x42").unwrap();
    let dep = builder::build(
        r#"
            mod math;
            pub fn add(a: u64, b: u64) -> u64 { return a + b; }
        "#,
        &[],
    )
    .unwrap();

    let path = std::env::temp_dir().join("meow_build_from_file_with_dep_test.meow");
    std::fs::write(
        &path,
        r#"
            mod main;
            use math@0x42;
            fn run() -> u64 { return math::add(1, 2); }
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
    let deps = builder::extract_module_deps(src).unwrap();
    assert!(deps.is_empty());
}

#[test]
fn extract_module_deps_returns_declared_imports_in_order() {
    let src = r#"
        mod main;

        use math@0x01;
        use util@0x02;

        fn noop() {}
    "#;
    let deps = builder::extract_module_deps(src).unwrap();
    assert_eq!(deps.len(), 2);
    assert_eq!(
        deps[0],
        ("math".to_string(), Address::from_str("0x01").unwrap())
    );
    assert_eq!(
        deps[1],
        ("util".to_string(), Address::from_str("0x02").unwrap())
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
        builder::extract_module_deps("use helper@0x01;").unwrap_err(),
        BuilderError::CompileError(_)
    ));
}

#[test]
fn extract_module_deps_duplicate_use_returns_error() {
    let src = r#"
        mod main;

        use helper@0x42;
        use helper@0x42;
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

//
// ─── build with deps ───
//

#[test]
fn build_with_dep_cross_module_function_call() {
    let dep_addr = Address::from_str("0x01").unwrap();
    let math = builder::build(
        r#"
            mod math;
            pub fn add(a: u64, b: u64) -> u64 { return a + b; }
        "#,
        &[],
    )
    .unwrap();

    let caller = builder::build(
        r#"
            mod caller;

            use math@0x01;

            fn double_add(a: u64, b: u64) -> u64 {
                return math::add(a, b) + math::add(a, b);
            }
        "#,
        &[(dep_addr, &math)],
    )
    .unwrap();

    assert!(caller.get_function("double_add").is_some());
    assert_eq!(caller.imports, vec![dep_addr.into()]);
}

#[test]
fn build_with_dep_cross_module_struct() {
    let dep_addr = Address::from_str("0x10").unwrap();
    let shapes = builder::build(
        r#"
            mod shapes;

            pub struct Point { pub x: u64, y: u64 }

            pub fn make_point(x: u64, y: u64) -> Point { return Point { x: x, y: y }; }
            pub fn get_x(p: Point) -> u64 { return p.x; }
        "#,
        &[],
    )
    .unwrap();

    let user = builder::build(
        r#"
            mod user;

            use shapes@0x10;

            fn make_and_read() -> u64 {
                let p = shapes::make_point(5, 9);
                return shapes::get_x(p);
            }
        "#,
        &[(dep_addr, &shapes)],
    )
    .unwrap();

    assert!(user.get_function("make_and_read").is_some());
}

#[test]
fn build_with_declared_dep_not_provided_returns_error() {
    // Source declares `use math@0x01` but no dep module is provided.
    let src = r#"
        mod main;
        use math@0x01;
        fn run() -> u64 { return math::add(1, 2); }
    "#;
    assert!(matches!(
        builder::build(src, &[]).unwrap_err(),
        BuilderError::CompileError(_)
    ));
}

#[test]
fn build_with_extra_undeclared_dep_is_accepted() {
    // Providing a dep that is not declared via `use` in source is fine — it is silently ignored.
    let dep_addr = Address::from_str("0x99").unwrap();
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
            fn run() -> u64 { return 1; }
        "#,
        &[(dep_addr, &extra)],
    )
    .unwrap();

    assert!(module.imports.is_empty());
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
