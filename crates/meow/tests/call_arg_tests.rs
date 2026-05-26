use meow::{call_arg::CallArg, commands::DEFAULT_NODE_URL};
use meow_node_client::NodeClient;
use meow_types::{address::Address, transaction::input::Input};
use meow_vm_adapter::Value;

//
// ─── FromStr tests ───
//

#[test]
fn from_str_true_is_bool_true() {
    let arg: CallArg = "true".parse().unwrap();

    assert!(matches!(arg, CallArg::Bool(true)));
}

#[test]
fn from_str_false_is_bool_false() {
    let arg: CallArg = "false".parse().unwrap();

    assert!(matches!(arg, CallArg::Bool(false)));
}

#[test]
fn from_str_digits_is_u64() {
    let arg: CallArg = "42".parse().unwrap();

    assert!(matches!(arg, CallArg::U64(42)));
}

#[test]
fn from_str_zero_is_u64() {
    let arg: CallArg = "0".parse().unwrap();

    assert!(matches!(arg, CallArg::U64(0)));
}

#[test]
fn from_str_at_hex_address_is_address_variant() {
    let address = Address::suffixed(0xF1);

    let arg: CallArg = format!("@{}", address).parse().unwrap();

    assert!(matches!(arg, CallArg::Address(a) if a == address));
}

#[test]
fn from_str_hex_address_is_object_variant() {
    let address = Address::suffixed(0xF1);

    let arg: CallArg = address.to_string().parse().unwrap();

    assert!(matches!(arg, CallArg::Object(a) if a == address));
}

#[test]
fn from_str_plain_string_is_str() {
    let arg: CallArg = "hello".parse().unwrap();

    assert!(matches!(arg, CallArg::Str(ref s) if s == "hello"));
}

#[test]
fn from_str_empty_string_is_str_not_u64_error() {
    let arg: CallArg = "".parse().unwrap();

    assert!(matches!(arg, CallArg::Str(ref s) if s.is_empty()));
}

#[test]
fn from_str_at_prefix_with_invalid_address_returns_error() {
    let err = "@not_valid".parse::<CallArg>().unwrap_err();

    assert!(
        err.to_string().contains("prefix hex error"),
        "unexpected error: {err}"
    );
}

#[test]
fn from_str_object_prefix_with_invalid_address_returns_error() {
    let err = "0xinvalid".parse::<CallArg>().unwrap_err();

    assert!(
        err.to_string().contains("prefix hex error"),
        "unexpected error: {err}"
    );
}

//
// ─── into_input tests (non-network variants) ───
//

#[tokio::test]
async fn into_input_bool_true() {
    let input = CallArg::Bool(true)
        .into_input(&fake_client())
        .await
        .unwrap();

    assert_eq!(input, Input::raw(&true).unwrap());
}

#[tokio::test]
async fn into_input_bool_false() {
    let input = CallArg::Bool(false)
        .into_input(&fake_client())
        .await
        .unwrap();

    assert_eq!(input, Input::raw(&false).unwrap());
}

#[tokio::test]
async fn into_input_u64() {
    let input = CallArg::U64(99).into_input(&fake_client()).await.unwrap();

    assert_eq!(input, Input::raw(&99u64).unwrap());
}

#[tokio::test]
async fn into_input_address_preserves_bytes() {
    let addr = Address::suffixed(0xF1);
    let input = CallArg::Address(addr)
        .into_input(&fake_client())
        .await
        .unwrap();

    assert_eq!(input, Input::raw(&addr).unwrap());
}

#[tokio::test]
async fn into_input_str() {
    let input = CallArg::Str("world".to_string())
        .into_input(&fake_client())
        .await
        .unwrap();

    assert_eq!(input, Input::raw(&"world".to_string()).unwrap());
}

//
// ─── into_value tests (non-network variants) ───
//

#[tokio::test]
async fn into_value_bool_true() {
    let v = CallArg::Bool(true)
        .into_value(&fake_client())
        .await
        .unwrap();

    assert!(matches!(v, Value::Bool(true)));
}

#[tokio::test]
async fn into_value_bool_false() {
    let v = CallArg::Bool(false)
        .into_value(&fake_client())
        .await
        .unwrap();

    assert!(matches!(v, Value::Bool(false)));
}

#[tokio::test]
async fn into_value_u64() {
    let v = CallArg::U64(99).into_value(&fake_client()).await.unwrap();

    assert!(matches!(v, Value::U64(99)));
}

#[tokio::test]
async fn into_value_address_preserves_bytes() {
    let addr = Address::suffixed(0xF1);
    let v = CallArg::Address(addr)
        .into_value(&fake_client())
        .await
        .unwrap();

    assert!(matches!(v, Value::Address(a) if a == addr.into()));
}

#[tokio::test]
async fn into_value_str() {
    let v = CallArg::Str("world".to_string())
        .into_value(&fake_client())
        .await
        .unwrap();

    assert!(matches!(v, Value::Str(ref s) if s == "world"));
}

//
// ─── Utility functions ───
//

fn fake_client() -> NodeClient {
    NodeClient::with_url(DEFAULT_NODE_URL.parse().unwrap())
}
