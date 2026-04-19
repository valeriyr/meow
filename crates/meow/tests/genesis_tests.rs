use std::path::PathBuf;

use meow::genesis::GenesisCommand;
use meow_types::address::Address;
use temp_dir::TempDir;

//
// ─── Build tests ───
//

#[test]
fn build_creates_output_file() {
    let tmp = TempDir::new().unwrap();
    let allocations = write_allocations(&tmp, &[(Address::fill(0xAA), 100)]);
    let output = tmp.path().join("genesis.bin");

    GenesisCommand::Build {
        allocations,
        output: output.clone(),
    }
    .run()
    .unwrap();

    assert!(output.exists());
}

#[test]
fn build_with_single_allocation_creates_module_and_coin() {
    let tmp = TempDir::new().unwrap();
    let allocations = write_allocations(&tmp, &[(Address::fill(0xAA), 500)]);

    let output = GenesisCommand::Build {
        allocations,
        output: tmp.path().join("genesis.bin"),
    }
    .run()
    .unwrap();

    // meow_object module + meow_coin module + one coin object.
    assert_eq!(output.objects.len(), 3);
    assert_eq!(output.objects[0].type_, "module");
    assert_eq!(
        output.objects[0].address,
        "0x0000000000000000000000000000000000000000000000000000000000000001"
    );
    assert_eq!(output.objects[1].type_, "module");
    assert_eq!(
        output.objects[1].address,
        "0x0000000000000000000000000000000000000000000000000000000000000010"
    );
    assert_eq!(
        output.objects[2].type_,
        "0x0000000000000000000000000000000000000000000000000000000000000010::MeowCoin"
    );
    assert_eq!(
        output.objects[2]
            .content
            .as_ref()
            .and_then(|c| c.get("balance"))
            .map(String::as_str),
        Some("500")
    );
}

#[test]
fn build_with_multiple_allocations_creates_one_coin_per_allocation() {
    let tmp = TempDir::new().unwrap();
    let allocations = write_allocations(
        &tmp,
        &[
            (Address::fill(0xAA), 100),
            (Address::fill(0xBB), 200),
            (Address::fill(0xCC), 300),
        ],
    );

    let output = GenesisCommand::Build {
        allocations,
        output: tmp.path().join("genesis.bin"),
    }
    .run()
    .unwrap();

    // meow_object module + meow_coin module + three coin objects.
    assert_eq!(output.objects.len(), 5);
    assert_eq!(output.objects[0].type_, "module");
    assert_eq!(
        output.objects[0].address,
        "0x0000000000000000000000000000000000000000000000000000000000000001"
    );
    assert_eq!(output.objects[1].type_, "module");
    assert_eq!(
        output.objects[1].address,
        "0x0000000000000000000000000000000000000000000000000000000000000010"
    );
    assert_eq!(
        output.objects[2].type_,
        "0x0000000000000000000000000000000000000000000000000000000000000010::MeowCoin"
    );
    assert_eq!(
        output.objects[2]
            .content
            .as_ref()
            .and_then(|c| c.get("balance"))
            .map(String::as_str),
        Some("100")
    );
    assert_eq!(
        output.objects[3].type_,
        "0x0000000000000000000000000000000000000000000000000000000000000010::MeowCoin"
    );
    assert_eq!(
        output.objects[3]
            .content
            .as_ref()
            .and_then(|c| c.get("balance"))
            .map(String::as_str),
        Some("200")
    );
    assert_eq!(
        output.objects[4].type_,
        "0x0000000000000000000000000000000000000000000000000000000000000010::MeowCoin"
    );
    assert_eq!(
        output.objects[4]
            .content
            .as_ref()
            .and_then(|c| c.get("balance"))
            .map(String::as_str),
        Some("300")
    );
}

#[test]
fn build_with_empty_allocations_creates_only_modules() {
    let tmp = TempDir::new().unwrap();
    let allocations = write_allocations(&tmp, &[]);

    let output = GenesisCommand::Build {
        allocations,
        output: tmp.path().join("genesis.bin"),
    }
    .run()
    .unwrap();

    // meow_object module + meow_coin module.
    assert_eq!(output.objects.len(), 2);
    assert_eq!(output.objects[0].type_, "module");
    assert_eq!(
        output.objects[0].address,
        "0x0000000000000000000000000000000000000000000000000000000000000001"
    );
    assert_eq!(output.objects[1].type_, "module");
    assert_eq!(
        output.objects[1].address,
        "0x0000000000000000000000000000000000000000000000000000000000000010"
    );
}

#[test]
fn build_invalid_csv_line_returns_error() {
    let tmp = TempDir::new().unwrap();
    let allocations = write_raw_allocations(&tmp, "not a valid csv line");

    let err = GenesisCommand::Build {
        allocations,
        output: tmp.path().join("genesis.bin"),
    }
    .run()
    .unwrap_err();

    assert!(
        err.to_string()
            .contains("invalid allocation line: not a valid csv line"),
        "unexpected error: {err}"
    );
}

#[test]
fn build_invalid_address_in_csv_returns_error() {
    let tmp = TempDir::new().unwrap();
    let allocations = write_raw_allocations(&tmp, "not_an_address,100");

    let err = GenesisCommand::Build {
        allocations,
        output: tmp.path().join("genesis.bin"),
    }
    .run()
    .unwrap_err();

    assert!(
        err.to_string().contains("prefix hex error"),
        "unexpected error: {err}"
    );
}

#[test]
fn build_invalid_amount_in_csv_returns_error() {
    let tmp = TempDir::new().unwrap();
    let addr = Address::fill(0xAA);
    let allocations = write_raw_allocations(&tmp, &format!("{addr},not_a_number"));

    let err = GenesisCommand::Build {
        allocations,
        output: tmp.path().join("genesis.bin"),
    }
    .run()
    .unwrap_err();

    assert!(
        err.to_string().contains("invalid digit found in string"),
        "unexpected error: {err}"
    );
}

//
// ─── Utility functions ───
//

fn write_allocations(tmp: &TempDir, allocations: &[(Address, u64)]) -> PathBuf {
    let content = allocations
        .iter()
        .map(|(addr, amount)| format!("{addr},{amount}"))
        .collect::<Vec<_>>()
        .join("\n");
    write_raw_allocations(tmp, &content)
}

fn write_raw_allocations(tmp: &TempDir, content: &str) -> PathBuf {
    let path = tmp.path().join("allocations.csv");
    std::fs::write(&path, content).expect("allocations write must succeed");
    path
}
