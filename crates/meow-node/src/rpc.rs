use std::sync::{Arc, Mutex};

use axum::{
    Json, Router,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
};
use meow_nakamoto::{
    mempool::error::MempoolError,
    miner::{Miner, error::MinerError},
};
use meow_types::{address::Address, digest::Digest, transaction::SignedTransaction};

use crate::gossip::{GossipHandle, TOPIC_TXS};

#[derive(Clone)]
pub struct AppState {
    pub miner: Arc<Mutex<Miner>>,
    pub gossip: GossipHandle,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/tx", post(submit_tx))
        .route("/object/{addr}", get(get_object))
        .route("/tx/{digest}", get(get_tx_result))
        .with_state(state)
}

/// POST /tx — submit a `SignedTransaction` (JSON-encoded).
async fn submit_tx(
    State(state): State<AppState>,
    Json(tx): Json<SignedTransaction>,
) -> impl IntoResponse {
    let result = state.miner.lock().unwrap().submit_tx(tx.clone());

    match result {
        Ok(()) => {
            if let Ok(data) = bcs::to_bytes(&tx) {
                state.gossip.publish(TOPIC_TXS, data);
            }
            StatusCode::ACCEPTED.into_response()
        }
        Err(MinerError::MempoolError(MempoolError::InvalidSignature)) => {
            (StatusCode::BAD_REQUEST, "invalid signature").into_response()
        }
        Err(MinerError::MempoolError(MempoolError::DuplicateTransaction(digest))) => (
            StatusCode::CONFLICT,
            format!("duplicate transaction: {digest}"),
        )
            .into_response(),
        Err(MinerError::MempoolError(MempoolError::GasCoinNotFound(addr))) => (
            StatusCode::BAD_REQUEST,
            format!("gas coin not found: {addr}"),
        )
            .into_response(),
    }
}

/// GET /object/:addr — returns the live `Object` at the given address.
async fn get_object(State(state): State<AppState>, Path(addr): Path<String>) -> impl IntoResponse {
    let addr: Address = match addr.parse() {
        Ok(a) => a,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid address").into_response(),
    };
    let m = state.miner.lock().unwrap();
    match m.head_store().get_object(&addr) {
        Some(obj) => Json(obj.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "object not found").into_response(),
    }
}

/// GET /tx/:digest — returns the `ExecutionResult` for a committed transaction.
async fn get_tx_result(
    State(state): State<AppState>,
    Path(digest): Path<String>,
) -> impl IntoResponse {
    let digest: Digest = match digest.parse() {
        Ok(d) => d,
        Err(_) => return (StatusCode::BAD_REQUEST, "invalid digest").into_response(),
    };
    let m = state.miner.lock().unwrap();
    match m.get_result(&digest) {
        Some(result) => Json(result.clone()).into_response(),
        None => (StatusCode::NOT_FOUND, "transaction not found").into_response(),
    }
}
