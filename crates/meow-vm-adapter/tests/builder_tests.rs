use meow_vm_adapter::builder::{self, error::BuilderError};

#[test]
fn build_module_successful() {
    let src = r#"
        struct Point { x: u64, y: u64 }
        object Token { id: address, amount: u64 }

        fn make(x: u64, y: u64): Point { return Point { x: x, y: y }; }
    "#;

    let module = builder::build("test", src).unwrap();

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
fn build_invalid_source_returns_error() {
    let src = "this is not valid meow";
    assert!(matches!(
        builder::build("test", src).unwrap_err(),
        BuilderError::CompileError(_)
    ));
}
