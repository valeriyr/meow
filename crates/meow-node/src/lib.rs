//! Meow full node: wires together the miner, gossip network, and HTTP RPC server.

pub mod commands;
pub mod node;

mod gossip_service;
mod miner_service;
mod rpc;
