use std::{collections::HashMap, path::PathBuf};

use meow_node_client::NodeClient;
use meow_types::address::Address;
use meow_vm_adapter::{Module, builder};

/// High-level helper for building a module from a source file,
/// including loading all declared dependencies from the node.
pub async fn build_module(client: &NodeClient, path: PathBuf) -> anyhow::Result<Module> {
    let source = builder::read_source_file(path)?;
    let dep_decl = builder::extract_module_deps(&source)?;

    let deps = load_dependencies(client, &dep_decl).await?;
    let deps = deps
        .iter()
        .map(|(addr, module)| (*addr, module))
        .collect::<Vec<_>>();

    Ok(builder::build(&source, &deps)?)
}

/// Load dependency modules from the node, including all transitive dependencies.
///
/// Uses BFS starting from `deps_decl`. Each loaded module's own `.imports` are
/// enqueued so their modules are fetched too. Diamond deps are deduplicated.
async fn load_dependencies(
    client: &NodeClient,
    deps_decl: &[(String, Address)],
) -> anyhow::Result<Vec<(Address, Module)>> {
    let mut loaded: HashMap<Address, Module> = HashMap::new();
    let mut queue: Vec<Address> = deps_decl.iter().map(|(_, addr)| *addr).collect();

    while let Some(addr) = queue.pop() {
        if loaded.contains_key(&addr) {
            continue;
        }

        let dep_obj = client
            .get_object(&addr)
            .await?
            .ok_or_else(|| anyhow::anyhow!("dependency module at {addr} not found on-chain"))?;
        let module: Module = bcs::from_bytes(dep_obj.content())
            .map_err(|e| anyhow::anyhow!("failed to deserialize module at {addr}: {e}"))?;

        // Enqueue this module's own imports for transitive resolution.
        for import in &module.imports {
            let import_addr = (*import).into();
            if !loaded.contains_key(&import_addr) {
                queue.push(import_addr);
            }
        }

        loaded.insert(addr, module);
    }

    Ok(loaded.into_iter().collect())
}
