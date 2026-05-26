//! HTTP client for the Meow node RPC API.

pub mod error;

use std::net::SocketAddr;

use meow_nakamoto_types::{block::Block, state_snapshot::StateSnapshot};
use serde::de::DeserializeOwned;
use url::Url;

use meow_types::{
    address::Address,
    digest::Digest,
    object::Object,
    transaction::{SignedTransaction, Transaction, execution_result::ExecutionResult},
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

    /// Create a new client with a base URL string.
    pub fn with_url_str<T: AsRef<str>>(base_url: T) -> Result<Self> {
        let base_url = Url::parse(base_url.as_ref())?;
        Ok(Self::with_url(base_url))
    }

    /// Create a new client by connecting to the given socket address.
    pub fn with_address(address: SocketAddr) -> Self {
        let base_url = Url::parse(&format!("http://{address}/")).expect("invalid URL");
        Self::with_url(base_url)
    }

    /// Returns the base URL of the node this client is connected to.
    pub fn base_url(&self) -> &Url {
        &self.base_url
    }

    /// POST /submit-transaction — submit a signed transaction.
    pub async fn submit_transaction(&self, transaction: &SignedTransaction) -> Result<()> {
        let url = self.base_url.join("submit-transaction")?;

        let response = self.inner.post(url).json(transaction).send().await?;

        if response.status().is_success() {
            return Ok(());
        }

        Err(Self::node_error(response).await)
    }

    /// POST /simulate-transaction — simulate an unsigned transaction without committing it.
    pub async fn simulate_transaction(&self, transaction: &Transaction) -> Result<ExecutionResult> {
        let url = self.base_url.join("simulate-transaction")?;

        let response = self.inner.post(url).json(transaction).send().await?;

        if response.status().is_success() {
            return Ok(response.json().await?);
        }

        Err(Self::node_error(response).await)
    }

    /// GET /object/{addr} — fetch a live object by address.
    pub async fn get_object(&self, addr: &Address) -> Result<Option<Object>> {
        let url = self.base_url.join(&format!("object/{addr}"))?;
        self.get_optional(url).await
    }

    /// GET /objects?address=...&address=... — fetch live objects by a list of addresses.
    pub async fn get_objects(&self, addresses: &[Address]) -> Result<Vec<Option<Object>>> {
        let mut url = self.base_url.join("objects")?;
        for addr in addresses {
            url.query_pairs_mut()
                .append_pair("address", &addr.to_string());
        }
        self.get_list(url).await
    }

    /// GET /objects_owned/{owner} — fetch all live objects by owner address.
    pub async fn get_objects_owned(&self, owner: &Address) -> Result<Vec<Object>> {
        let url = self.base_url.join(&format!("objects_owned/{owner}"))?;
        self.get_list(url).await
    }

    /// GET /transaction/{digest} — fetch a committed transaction by digest.
    pub async fn get_transaction(&self, digest: &Digest) -> Result<Option<SignedTransaction>> {
        let url = self.base_url.join(&format!("transaction/{digest}"))?;
        self.get_optional(url).await
    }

    /// GET /transaction-result/{digest} — fetch the execution result for a committed transaction.
    pub async fn get_transaction_result(&self, digest: &Digest) -> Result<Option<ExecutionResult>> {
        let url = self
            .base_url
            .join(&format!("transaction-result/{digest}"))?;
        self.get_optional(url).await
    }

    /// GET /blocks-since/{height} — fetch committed blocks from the given height onwards.
    pub async fn get_blocks_since(&self, height: u64) -> Result<Vec<Block>> {
        let url = self.base_url.join(&format!("blocks-since/{height}"))?;
        self.get_list(url).await
    }

    /// GET /state-snapshot — fetch a full state snapshot.
    pub async fn get_state_snapshot(&self) -> Result<StateSnapshot> {
        let url = self.base_url.join("state-snapshot")?;

        let response = self.inner.get(url).send().await?;

        if response.status().is_success() {
            return Ok(response.json().await?);
        }

        Err(Self::node_error(response).await)
    }

    /// Sends a GET request and deserializes the JSON body on success.
    /// The server returns `null` when the resource is not found, which deserializes as `Ok(None)`.
    async fn get_optional<T: DeserializeOwned>(&self, url: Url) -> Result<Option<T>> {
        let response = self.inner.get(url).send().await?;

        if response.status().is_success() {
            return Ok(response.json().await?);
        }

        Err(Self::node_error(response).await)
    }

    /// Sends a GET request and deserializes the JSON array body on success.
    async fn get_list<T: DeserializeOwned>(&self, url: Url) -> Result<Vec<T>> {
        let response = self.inner.get(url).send().await?;

        if response.status().is_success() {
            return Ok(response.json().await?);
        }

        Err(Self::node_error(response).await)
    }

    /// Extracts a [`NodeClientError::NodeError`] from a non-success response.
    async fn node_error(response: reqwest::Response) -> NodeClientError {
        let status = response.status();
        let body = response.text().await.unwrap_or_default();
        NodeClientError::NodeError { status, body }
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

#[cfg(test)]
mod tests {
    use url::Url;

    use super::normalize_url;

    #[test]
    fn normalize_url_adds_trailing_slash_when_missing() {
        let url = Url::parse("http://localhost:8080").unwrap();
        let normalized = normalize_url(url);
        assert_eq!(normalized.path(), "/");
    }

    #[test]
    fn normalize_url_keeps_trailing_slash_when_present() {
        let url = Url::parse("http://localhost:8080/").unwrap();
        let normalized = normalize_url(url.clone());
        assert_eq!(normalized, url);
    }

    #[test]
    fn normalize_url_preserves_sub_path_with_trailing_slash() {
        let url = Url::parse("http://localhost:8080/api/v1/").unwrap();
        let normalized = normalize_url(url.clone());
        assert_eq!(normalized.path(), "/api/v1/");
    }

    #[test]
    fn normalize_url_appends_slash_to_sub_path() {
        let url = Url::parse("http://localhost:8080/api/v1").unwrap();
        let normalized = normalize_url(url);
        assert_eq!(normalized.path(), "/api/v1/");
    }
}
