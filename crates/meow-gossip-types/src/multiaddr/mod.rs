//! Opaque wrapper around a libp2p multiaddr used to identify gossip peers.

pub mod error;

use std::str::FromStr;

use crate::multiaddr::error::MultiaddrError;

/// The result type related to multiaddr.
pub type Result<T> = std::result::Result<T, MultiaddrError>;

/// Opaque identifier for a multiaddr in the gossip network.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub struct Multiaddr(libp2p::Multiaddr);

impl std::fmt::Display for Multiaddr {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

impl From<libp2p::Multiaddr> for Multiaddr {
    fn from(multiaddr: libp2p::Multiaddr) -> Self {
        Multiaddr(multiaddr)
    }
}

impl From<Multiaddr> for libp2p::Multiaddr {
    fn from(multiaddr: Multiaddr) -> Self {
        multiaddr.0
    }
}

impl From<Multiaddr> for libp2p::swarm::dial_opts::DialOpts {
    fn from(multiaddr: Multiaddr) -> Self {
        multiaddr.0.into()
    }
}

impl FromStr for Multiaddr {
    type Err = MultiaddrError;

    fn from_str(s: &str) -> Result<Self> {
        Ok(Multiaddr(s.parse()?))
    }
}
