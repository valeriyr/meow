/// An error related to the MEOW node client.
#[derive(Debug, thiserror::Error)]
pub enum NodeClientError {
    #[error("node error ({status}): {body}")]
    NodeError {
        status: reqwest::StatusCode,
        body: String,
    },
    #[error("reqwest error: {0}")]
    ReqwestError(#[from] reqwest::Error),
    #[error("URL parse error: {0}")]
    UrlParseError(#[from] url::ParseError),
}
