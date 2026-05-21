//! Module builder that compiles source files and resolves transitive dependencies from the node.

use std::collections::HashMap;
use std::path::PathBuf;

use meow_node_client::NodeClient;
use meow_types::address::Address;
use meow_vm_adapter::{Module, builder, inputs_resolver};

/// Build a module from a source file, loading all transitive dependency modules
/// from the node. Returns both the compiled module and the loaded deps map so
/// callers (e.g. the runner) can reuse the deps without a second fetch.
pub async fn build_module(
    client: &NodeClient,
    path: PathBuf,
) -> anyhow::Result<(Module, HashMap<Address, Module>)> {
    let source = builder::read_source_file(path)?;
    let dep_addresses: Vec<Address> = builder::extract_module_deps(&source)?
        .into_iter()
        .map(|(_, _, addr)| addr)
        .collect();

    let deps = inputs_resolver::load_deps_async(&dep_addresses, |addr| async move {
        let obj = client.get_object(&addr).await.ok().flatten()?;
        bcs::from_bytes::<Module>(obj.content()).ok()
    })
    .await;

    let deps_for_build = deps.iter().map(|(addr, m)| (*addr, m)).collect::<Vec<_>>();
    let module = builder::build(&source, &deps_for_build)?;

    Ok((module, deps))
}
