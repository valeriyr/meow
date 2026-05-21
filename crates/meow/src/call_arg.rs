//! Command-line call argument type parsed from user-supplied strings.

use std::str::FromStr;

use meow_node_client::NodeClient;
use meow_types::{
    address::Address,
    object::{Object, object_conversion::object_to_vm_object_value},
    transaction::input::Input,
};
use meow_vm_adapter::Value;

/// A typed argument parsed from the command line.
///
/// Parsing rules (applied in order):
/// - `true` / `false`    → [`Bool`](CallArg::Bool)
/// - all-digit string    → [`U64`](CallArg::U64)
/// - `@0x<hex>`          → [`Address`](CallArg::Address) — raw 32-byte value
/// - `0x<hex>`           → [`Object`](CallArg::Object) — resolved against the node at call time
/// - anything else       → [`Str`](CallArg::Str)
#[derive(Debug, Clone)]
pub enum CallArg {
    Bool(bool),
    U64(u64),
    /// Raw address value (prefixed with `@`).
    Address(Address),
    /// Object reference — the address is used to look up the live object on the node.
    Object(Address),
    Str(String),
}

impl CallArg {
    /// Convert to a transaction [`Input`], resolving any `Object` variant against the node.
    pub async fn into_input(self, client: &NodeClient) -> anyhow::Result<Input> {
        Ok(match self {
            CallArg::Bool(b) => Input::raw(&b)?,
            CallArg::U64(n) => Input::raw(&n)?,
            CallArg::Address(addr) => Input::raw(&addr)?,
            CallArg::Str(str) => Input::raw(&str)?,
            CallArg::Object(addr) => Input::Object(get_object(client, &addr).await?.object_ref()),
        })
    }

    /// Convert a [`CallArg`] to a VM [`Value`] for the offline contract runner.
    pub async fn into_value(self, client: &NodeClient) -> anyhow::Result<Value> {
        Ok(match self {
            CallArg::Bool(b) => Value::Bool(b),
            CallArg::U64(n) => Value::U64(n),
            CallArg::Address(addr) => Value::Address(addr.into()),
            CallArg::Object(addr) => {
                let obj = get_object(client, &addr).await?;
                object_to_vm_object_value(&obj)?
            }
            CallArg::Str(str) => Value::Str(str),
        })
    }
}

impl FromStr for CallArg {
    type Err = anyhow::Error;

    fn from_str(s: &str) -> anyhow::Result<Self> {
        if s == "true" {
            return Ok(CallArg::Bool(true));
        }
        if s == "false" {
            return Ok(CallArg::Bool(false));
        }
        if s.chars().all(|c| c.is_ascii_digit()) {
            return Ok(s.parse::<u64>().map(CallArg::U64)?);
        }
        // @0x... → raw address value
        if let Some(hex) = s.strip_prefix('@') {
            return Ok(Address::from_str(hex).map(CallArg::Address)?);
        }
        // 0x... → object reference
        if s.starts_with("0x") || s.starts_with("0X") {
            return Ok(Address::from_str(s).map(CallArg::Object)?);
        }
        Ok(CallArg::Str(s.to_owned()))
    }
}

async fn get_object(client: &NodeClient, addr: &Address) -> anyhow::Result<Object> {
    client
        .get_object(addr)
        .await?
        .ok_or_else(|| anyhow::anyhow!("object not found on node: {addr}"))
}
