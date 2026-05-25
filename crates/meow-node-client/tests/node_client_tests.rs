use std::net::SocketAddr;

use meow_node_client::{NodeClient, error::NodeClientError};
use url::Url;

//
// ─── NodeClient construction ───
//

#[test]
fn with_url_normalizes_missing_trailing_slash() {
    let url = Url::parse("http://127.0.0.1:9000").unwrap();
    let client = NodeClient::with_url(url);
    assert_eq!(client.base_url().as_str(), "http://127.0.0.1:9000/");
}

#[test]
fn with_url_str_normalizes_missing_trailing_slash() {
    let client = NodeClient::with_url_str("http://127.0.0.1:9000").unwrap();
    assert_eq!(client.base_url().as_str(), "http://127.0.0.1:9000/");
}

#[test]
fn with_address_builds_correct_url() {
    let addr: SocketAddr = "127.0.0.1:9000".parse().unwrap();
    let client = NodeClient::with_address(addr);
    assert_eq!(client.base_url().as_str(), "http://127.0.0.1:9000/");
}

#[test]
fn with_url_str_returns_error_on_invalid_url() {
    assert!(matches!(
        NodeClient::with_url_str("not a url"),
        Err(NodeClientError::UrlParseError(_))
    ));
}
