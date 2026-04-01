use meow_types::system_framework::meow_coin::MEOW_COIN_MODULE_PATH;
use meow_vm_adapter::builder::{self, MAX_SOURCE_SIZE, error::BuilderError};

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
fn build_module_from_file_successful() {
    let module = builder::build_from_file(MEOW_COIN_MODULE_PATH).unwrap();

    assert_eq!(module.name, "meow_coin");
}

//
// ─── Errors ───
//

#[test]
fn build_invalid_source_returns_error() {
    let src = "this is not valid meow";
    let err = builder::build("test", src).unwrap_err();
    assert!(matches!(&err, BuilderError::CompileError(e) if e.to_string().contains("found 't'")));
}

#[test]
fn build_from_nonexistent_file_returns_io_error() {
    assert!(matches!(
        builder::build_from_file("/nonexistent/path/module.meow").unwrap_err(),
        BuilderError::IoError(_)
    ));
}

#[test]
fn build_from_file_without_extension_returns_invalid_file_name() {
    // A path with no file stem (e.g. "/") cannot produce a module name.
    assert!(matches!(
        builder::build_from_file("/").unwrap_err(),
        BuilderError::InvalidFileName(_)
    ));
}

#[test]
fn build_source_size_limit() {
    // Create a source string that exceeds MAX_SOURCE_SIZE by 1 byte.
    // Padding with spaces produces syntactically irrelevant but still-too-large input.
    let oversized = " ".repeat(MAX_SOURCE_SIZE + 1);

    assert!(
        matches!(
            builder::build("test", &oversized).unwrap_err(),
            BuilderError::SourceTooLarge { .. }
        ),
        "source exceeding MAX_SOURCE_SIZE must return SourceTooLarge"
    );
}
