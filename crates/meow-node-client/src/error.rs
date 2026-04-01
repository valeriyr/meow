/// An error related to the MEOW node client.
#[derive(Debug, thiserror::Error)]
pub enum NodeClientError {
    #[error("reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("node rejected transaction ({status}): {body}")]
    NodeRejectedTransaction {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("node error ({status}): {body}")]
    NodeError {
        status: reqwest::StatusCode,
        body: String,
    },
}
