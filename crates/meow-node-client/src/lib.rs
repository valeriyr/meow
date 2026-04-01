pub mod error;

use meow_types::{
    address::Address,
    digest::Digest,
    object::Object,
    transaction::{SignedTransaction, execution_result::ExecutionResult},
};

use crate::error::NodeClientError;

/// The result type related to addresses.
pub type Result<T> = std::result::Result<T, NodeClientError>;

/// Blocking HTTP client for the meow-node RPC API.
pub struct NodeClient {
    base_url: String,
    inner: reqwest::blocking::Client,
}

impl NodeClient {
    pub fn new(base_url: impl Into<String>) -> Self {
        Self {
            base_url: base_url.into(),
            inner: reqwest::blocking::Client::new(),
        }
    }

    /// POST /tx — submit a signed transaction.
    ///
    /// Returns `Ok(())` on 202 Accepted, or a descriptive error on failure.
    pub fn submit_transaction(&self, transaction: &SignedTransaction) -> Result<()> {
        let response = self
            .inner
            .post(format!("{}/tx", self.base_url))
            .json(transaction)
            .send()?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().unwrap_or_default();

        Err(NodeClientError::NodeRejectedTransaction { status, body })
    }

    /// GET /object/:addr — fetch a live object by address.
    pub fn get_object(&self, addr: &Address) -> Result<Option<Object>> {
        let response = self
            .inner
            .get(format!("{}/object/{addr}", self.base_url))
            .send()?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if response.status().is_success() {
            return Ok(Some(response.json()?));
        }

        let status = response.status();
        let body = response.text().unwrap_or_default();

        Err(NodeClientError::NodeError { status, body })
    }

    /// GET /tx/:digest — fetch the execution result for a committed transaction.
    pub fn get_transaction_result(&self, digest: &Digest) -> Result<Option<ExecutionResult>> {
        let response = self
            .inner
            .get(format!("{}/tx/{digest}", self.base_url))
            .send()?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if response.status().is_success() {
            return Ok(Some(response.json()?));
        }

        let status = response.status();
        let body = response.text().unwrap_or_default();

        Err(NodeClientError::NodeError { status, body })
    }
}
