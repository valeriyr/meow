use std::collections::BTreeMap;

use meow_types::{
    address::Address, object::Object, transaction::execution_result::ExecutionResult,
};

/// In-memory object store — the canonical state of the chain at a given block.
///
/// Intentionally kept minimal: it tracks live objects only.
/// Result indexing and chain history live in [`crate::chain::ChainState`].
#[derive(Clone)]
pub struct Store {
    objects: BTreeMap<Address, Object>,
}

impl Store {
    /// Creates a store pre-populated with the given objects.
    pub fn with_objects(objects: impl IntoIterator<Item = Object>) -> Self {
        let mut store = Self::default();
        for obj in objects {
            store.objects.insert(obj.address().clone(), obj);
        }
        store
    }

    /// Gets an object by address, if it exists.
    pub fn get_object(&self, addr: &Address) -> Option<&Object> {
        self.objects.get(addr)
    }

    /// Checks if the store contains an object with the given address.
    pub fn contains(&self, addr: &Address) -> bool {
        self.objects.contains_key(addr)
    }

    /// Applies an execution result:
    /// - created objects → inserted
    /// - changed objects → overwritten
    /// - destroyed objects → removed
    pub fn apply_execution_result(&mut self, result: &ExecutionResult) {
        for obj in result.created_objects() {
            self.objects.insert(obj.address().clone(), obj.clone());
        }
        for obj in result.changed_objects() {
            self.objects.insert(obj.address().clone(), obj.clone());
        }
        for obj in result.destroyed_objects() {
            self.objects.remove(obj.address());
        }
    }

    /// Iterates over all live objects in deterministic (sorted) order.
    pub fn objects(&self) -> impl Iterator<Item = &Object> {
        self.objects.values()
    }
}

impl Default for Store {
    fn default() -> Self {
        Self {
            objects: BTreeMap::new(),
        }
    }
}
