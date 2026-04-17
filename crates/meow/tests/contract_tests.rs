use std::path::PathBuf;

use meow::{
    call_arg::CallArg,
    commands::DEFAULT_NODE_URL,
    contract::{ContractCommand, output::ContractCommandOutput},
    output_encoder::OutputEncoder,
};
use meow_node_client::NodeClient;
use meow_types::identifier::Identifier;
use temp_dir::TempDir;

//
// ─── Build tests ───
//

#[tokio::test]
async fn build_valid_source_succeeds() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(&tmp, ADD_SRC);

    let output = ContractCommand::Build {
        path,
        encoder: OutputEncoder::Base64,
    }
    .run(&fake_client())
    .await
    .unwrap();

    assert!(matches!(output, ContractCommandOutput::Build(_)));
}

#[tokio::test]
async fn build_module_name_comes_from_source_declaration() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(&tmp, ADD_SRC);

    let output = ContractCommand::Build {
        path,
        encoder: OutputEncoder::Base64,
    }
    .run(&fake_client())
    .await
    .unwrap();

    let name = match output {
        ContractCommandOutput::Build(m) => m.name,
        _ => panic!("expected Build output"),
    };
    assert_eq!(name, "math");
}

#[tokio::test]
async fn build_invalid_source_returns_compiler_error() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(&tmp, "this is not valid meow source !!!");

    let err = ContractCommand::Build {
        path,
        encoder: OutputEncoder::Base64,
    }
    .run(&fake_client())
    .await
    .unwrap_err();

    // The compiler returns an error describing what went wrong.
    assert!(
        err.to_string().contains("compile error:"),
        "unexpected error: {err}"
    );
}

//
// ─── Run tests ───
//

#[tokio::test]
async fn run_returns_computed_return_value() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(&tmp, ADD_SRC);

    let output = ContractCommand::Run {
        path,
        function: Identifier::new("add").unwrap(),
        args: vec![CallArg::U64(3), CallArg::U64(5)],
    }
    .run(&fake_client())
    .await
    .unwrap();

    let result = match output {
        ContractCommandOutput::Run(r) => r,
        _ => panic!("expected Run output"),
    };
    assert_eq!(result.return_value, Some("8".to_string()));
    assert_eq!(result.gas_spent, 6);
}

#[tokio::test]
async fn run_void_function_produces_no_return_value() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(&tmp, NOOP_SRC);

    let output = ContractCommand::Run {
        path,
        function: Identifier::new("noop").unwrap(),
        args: vec![],
    }
    .run(&fake_client())
    .await
    .unwrap();

    let result = match output {
        ContractCommandOutput::Run(r) => r,
        _ => panic!("expected Run output"),
    };
    assert_eq!(result.return_value, None);
    assert_eq!(result.gas_spent, 2);
}

#[tokio::test]
async fn run_unknown_function_returns_error() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(&tmp, ADD_SRC);

    let err = ContractCommand::Run {
        path,
        function: Identifier::new("missing").unwrap(),
        args: vec![],
    }
    .run(&fake_client())
    .await
    .unwrap_err();

    assert!(
        err.to_string().contains("undefined function: missing"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn run_rejects_private_function() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(&tmp, PRIVATE_SRC);

    let err = ContractCommand::Run {
        path,
        function: Identifier::new("secret").unwrap(),
        args: vec![],
    }
    .run(&fake_client())
    .await
    .unwrap_err();

    assert!(
        err.to_string().contains("function 'secret' is private"),
        "unexpected error: {err}"
    );
}

//
// ─── RunPrivileged tests ───
//

#[tokio::test]
async fn run_privileged_can_call_private_function() {
    let tmp = TempDir::new().unwrap();
    let path = write_source(&tmp, PRIVATE_SRC);

    let output = ContractCommand::RunPrivileged {
        path,
        function: Identifier::new("secret").unwrap(),
        args: vec![],
    }
    .run(&fake_client())
    .await
    .unwrap();

    let result = match output {
        ContractCommandOutput::Run(r) => r,
        _ => panic!("expected Run output"),
    };
    assert_eq!(result.return_value, Some("42".to_string()));
}

//
// ─── Utility functions ───
//

const ADD_SRC: &str = r#"
        module math;
        pub fn add(a: u64, b: u64): u64 { return a + b; }
    "#;
const NOOP_SRC: &str = r#"
        module utils;
        pub fn noop() {}
    "#;
const PRIVATE_SRC: &str = r#"
        module secrets;
        fn secret(): u64 { return 42; }
    "#;

fn fake_client() -> NodeClient {
    NodeClient::with_url(DEFAULT_NODE_URL.parse().unwrap())
}

fn write_source(tmp: &TempDir, src: &str) -> PathBuf {
    let path = tmp.path().join("test.meow");
    std::fs::write(&path, src).expect("source write must succeed");
    path
}
