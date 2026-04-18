use std::{
    collections::{HashMap, HashSet},
    future::Future,
};

use meow_types::{
    address::Address,
    object::Object,
    transaction::{Transaction, input::Input, transaction_type::TransactionType},
};
use meow_vm_types::module::Module;

/// Collect all input objects a transaction needs, in executor-ready order:
/// the gas coin, transitive dependency modules (post-order), the main module,
/// and call arguments.
///
/// `get_object` is called for each address as needed. Missing objects (returning
/// `None`) are silently skipped — the executor will produce an appropriate error
/// when it fails to locate a required input.
pub fn collect_inputs<F>(transaction: &Transaction, get_object: F) -> Vec<Object>
where
    F: Fn(&Address) -> Option<Object>,
{
    let mut inputs = Vec::new();

    if let Some(obj) = get_object(transaction.gas_coin().address()) {
        inputs.push(obj);
    }

    match transaction.type_() {
        TransactionType::MeowCall(call) => {
            if let Some(module_obj) = get_object(call.module()) {
                let mut seen = HashSet::new();
                seen.insert(*call.module());
                collect_dep_modules(&module_obj, &get_object, &mut inputs, &mut seen);
                inputs.push(module_obj);
            }
            for arg in call.arguments() {
                if let Input::Object(obj_ref) = arg
                    && let Some(obj) = get_object(obj_ref.address())
                {
                    inputs.push(obj);
                }
            }
        }
        TransactionType::MeowModulePublish(module_bytes) => {
            if let Ok(module) = bcs::from_bytes::<Module>(module_bytes) {
                let mut seen = HashSet::new();
                for vm_addr in &module.imports {
                    let addr = Address::from(*vm_addr);
                    if seen.insert(addr)
                        && let Some(dep_obj) = get_object(&addr)
                    {
                        collect_dep_modules(&dep_obj, &get_object, &mut inputs, &mut seen);
                        inputs.push(dep_obj);
                    }
                }
            }
        }
    }

    inputs
}

/// Async variant of [`collect_inputs`]. Fetches objects on demand
/// via `get_object`, then delegates to the sync version for ordering.
///
/// Internally performs a BFS to discover and fetch all transitive dependency
/// modules before calling the sync ordering pass — no async recursion needed.
/// `Address` is passed by value since it is `Copy`.
pub async fn collect_inputs_async<F, Fut>(transaction: &Transaction, get_object: F) -> Vec<Object>
where
    F: Fn(Address) -> Fut,
    Fut: Future<Output = Option<Object>>,
{
    let mut cache: HashMap<Address, Object> = HashMap::new();

    let gas_addr = *transaction.gas_coin().address();
    if let Some(obj) = get_object(gas_addr).await {
        cache.insert(gas_addr, obj);
    }

    match transaction.type_() {
        TransactionType::MeowCall(call) => {
            bfs_fetch_modules(vec![*call.module()], &get_object, &mut cache).await;
            for arg in call.arguments() {
                if let Input::Object(obj_ref) = arg {
                    let addr = *obj_ref.address();
                    if let Some(obj) = get_object(addr).await {
                        cache.insert(addr, obj);
                    }
                }
            }
        }
        TransactionType::MeowModulePublish(module_bytes) => {
            if let Ok(module) = bcs::from_bytes::<Module>(module_bytes) {
                let seeds = module.imports.iter().map(|a| Address::from(*a)).collect();
                bfs_fetch_modules(seeds, &get_object, &mut cache).await;
            }
        }
    }

    collect_inputs(transaction, |addr| cache.get(addr).cloned())
}

/// Fetch all transitive dependency modules needed to compile or run a module.
///
/// Starts from the direct deps declared in source (`deps_decl`), then follows
/// each module's own `.imports` transitively (BFS). Diamond deps are deduplicated.
///
/// `get_module` is called for each address. Missing modules (`None`) are silently
/// skipped — a missing dep will surface as a compiler or VM error at the call site.
pub async fn load_deps_async<F, Fut>(
    deps_decl: &[(String, Address)],
    get_module: F,
) -> HashMap<Address, Module>
where
    F: Fn(Address) -> Fut,
    Fut: Future<Output = Option<Module>>,
{
    let mut loaded: HashMap<Address, Module> = HashMap::new();
    let mut queue: Vec<Address> = deps_decl.iter().map(|(_, addr)| *addr).collect();

    while let Some(addr) = queue.pop() {
        if loaded.contains_key(&addr) {
            continue;
        }
        if let Some(module) = get_module(addr).await {
            for vm_addr in &module.imports {
                let import_addr = Address::from(*vm_addr);
                if !loaded.contains_key(&import_addr) {
                    queue.push(import_addr);
                }
            }
            loaded.insert(addr, module);
        }
    }

    loaded
}

/// BFS over module objects reachable from `seeds`, fetching each via `get_object`
/// and populating `cache`. Already-visited addresses are skipped.
async fn bfs_fetch_modules<F, Fut>(
    seeds: Vec<Address>,
    get_object: &F,
    cache: &mut HashMap<Address, Object>,
) where
    F: Fn(Address) -> Fut,
    Fut: Future<Output = Option<Object>>,
{
    let mut queue = seeds;
    let mut seen = HashSet::new();
    while let Some(addr) = queue.pop() {
        if !seen.insert(addr) {
            continue;
        }
        if let Some(obj) = get_object(addr).await {
            if let Ok(module) = bcs::from_bytes::<Module>(obj.content()) {
                for vm_addr in &module.imports {
                    queue.push(Address::from(*vm_addr));
                }
            }
            cache.insert(addr, obj);
        }
    }
}

/// Recursive post-order traversal: dependency modules are pushed before the
/// module that imports them, so the executor sees deps before dependents.
///
/// `seen` deduplicates diamond dependencies. Recursion depth is bounded by the
/// compiler-enforced `max_dep_modules` limit (default 64).
fn collect_dep_modules<F>(
    root_obj: &Object,
    get_object: &F,
    inputs: &mut Vec<Object>,
    seen: &mut HashSet<Address>,
) where
    F: Fn(&Address) -> Option<Object>,
{
    let Ok(module) = bcs::from_bytes::<Module>(root_obj.content()) else {
        return;
    };
    for vm_addr in &module.imports {
        let addr = Address::from(*vm_addr);
        if seen.insert(addr)
            && let Some(dep_obj) = get_object(&addr)
        {
            collect_dep_modules(&dep_obj, get_object, inputs, seen);
            inputs.push(dep_obj);
        }
    }
}
