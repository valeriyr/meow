use base64::{Engine, engine::general_purpose};
use meow::{client::ClientCommand, commands::DEFAULT_NODE_URL};
use meow_node_client::NodeClient;

//
// ─── SubmitTransaction tests ───
//

#[tokio::test]
async fn submit_transaction_invalid_base64_returns_error() {
    let err = ClientCommand::SubmitTransaction {
        transaction: "not valid base64 !!!".to_string(),
    }
    .run(&fake_client(), false)
    .await
    .unwrap_err();

    assert!(
        err.to_string().contains("Invalid symbol"),
        "unexpected error: {err}"
    );
}

#[tokio::test]
async fn submit_transaction_valid_base64_but_invalid_bcs_returns_error() {
    let junk = general_purpose::STANDARD.encode(b"this is not a BCS-encoded SignedTransaction");

    let err = ClientCommand::SubmitTransaction { transaction: junk }
        .run(&fake_client(), false)
        .await
        .unwrap_err();

    assert!(
        err.to_string().contains("unexpected end of input"),
        "unexpected error: {err}"
    );
}

//
// ─── Utility functions ───
//

fn fake_client() -> NodeClient {
    NodeClient::with_url(DEFAULT_NODE_URL.parse().unwrap())
}
