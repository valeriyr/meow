pub mod error;

use std::net::SocketAddr;

use url::Url;

use meow_types::{
    address::Address,
    digest::Digest,
    object::Object,
    transaction::{SignedTransaction, execution_result::ExecutionResult},
};

use crate::error::NodeClientError;

/// The result type for the node client.
pub type Result<T> = std::result::Result<T, NodeClientError>;

/// HTTP client for the meow-node RPC API.
pub struct NodeClient {
    base_url: Url,
    inner: reqwest::Client,
}

impl NodeClient {
    /// Create a new client with the given base URL (e.g. `http://localhost:8080/`).
    pub fn with_url(base_url: Url) -> Self {
        Self {
            base_url: normalize_url(base_url),
            inner: reqwest::Client::new(),
        }
    }

    /// Create a new client by connecting to the given socket address.
    pub fn with_address(address: SocketAddr) -> Self {
        let base_url = Url::parse(&format!("http://{address}/")).expect("invalid URL");
        Self::with_url(base_url)
    }

    /// POST /tx — submit a signed transaction.
    pub async fn submit_transaction(&self, transaction: &SignedTransaction) -> Result<()> {
        let url = self.base_url.join("tx")?;

        let response = self.inner.post(url).json(transaction).send().await?;

        if response.status().is_success() {
            return Ok(());
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        Err(NodeClientError::NodeError { status, body })
    }

    /// GET /object/:addr — fetch a live object by address.
    pub async fn get_object(&self, addr: &Address) -> Result<Option<Object>> {
        let url = self.base_url.join(&format!("object/{addr}"))?;

        let response = self.inner.get(url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if response.status().is_success() {
            return Ok(Some(response.json().await?));
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        Err(NodeClientError::NodeError { status, body })
    }

    /// GET /tx/:digest — fetch the execution result for a committed transaction.
    pub async fn get_transaction_result(&self, digest: &Digest) -> Result<Option<ExecutionResult>> {
        let url = self.base_url.join(&format!("tx/{digest}"))?;

        let response = self.inner.get(url).send().await?;

        if response.status() == reqwest::StatusCode::NOT_FOUND {
            return Ok(None);
        }

        if response.status().is_success() {
            return Ok(Some(response.json().await?));
        }

        let status = response.status();
        let body = response.text().await.unwrap_or_default();

        Err(NodeClientError::NodeError { status, body })
    }
}

/// Ensure the base URL always ends with a slash, so that path joining works correctly.
fn normalize_url(mut url: Url) -> Url {
    if !url.path().ends_with('/') {
        let path = format!("{}/", url.path());
        url.set_path(&path);
    }
    url
}
