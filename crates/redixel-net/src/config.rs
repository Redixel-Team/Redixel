use std::{net::SocketAddr, path::PathBuf, time::Duration};

use redixel_core::{NetworkManager, NoOpNetwork};

/// Default netcode protocol id. Override per game to reject clients built
/// against a mismatched version early.
pub const DEFAULT_PROTOCOL_ID: u64 = 0x5245_4449_5845_4C00;

/// How long a handshake may take before either side gives up on it.
///
/// A server reclaims the `max_clients` slot a pending handshake occupies, so a
/// peer cannot open the cap's worth of connections, never send its hello frame,
/// and lock every slot indefinitely.
///
/// A client bounds the **whole** handshake — opening the session, opening the
/// stream, and waiting for the welcome frame — not just its last step, so an
/// unresponsive server cannot hang it at any one of them.
///
/// Shared by every backend so the two ends cannot disagree on the budget.
pub const HANDSHAKE_TIMEOUT: Duration = Duration::from_secs(10);

/// What role this peer plays on the network.
#[derive(Debug, Clone)]
pub enum NetMode {
    Server { bind: SocketAddr },
    Client { connect: SocketAddr },
    Offline,
}

/// TLS certificate source for the server. `SelfSigned` (default) works for
/// LAN/self-host, since native clients skip validation; use `Pem` with a real
/// domain certificate for public/online hosting.
#[derive(Debug, Clone, Default)]
pub enum CertSource {
    #[default]
    SelfSigned,
    Pem {
        cert: PathBuf,
        key:  PathBuf,
    },
}

/// Declarative networking configuration consumed by [`build`].
///
/// - `max_clients` (server only): peers admitted simultaneously. Further
///   sessions are turned away during the handshake, before the game ever sees
///   them. Clamped to a minimum of 1.
/// - `protocol_id`: rejects a peer whose id differs, closing the connection
///   during the handshake. Both sides must agree — bump it per game (and per
///   wire-format change) so mismatched builds fail loudly instead of exchanging
///   garbage.
/// - `server_name` (client only) selects TLS mode: `Some` validates against a
///   real domain cert (online), `None` connects by IP with validation disabled
///   (LAN).
/// - `server_cert_hash`: SHA-256 digest of the server's self-signed
///   certificate, pinned via `WebTransportOptions.serverCertificateHashes`.
///   Consumed only by the wasm client (browsers require it to trust a
///   self-signed cert); native and mobile clients ignore this field entirely.
#[derive(Debug, Clone)]
pub struct NetConfig {
    pub mode:             NetMode,
    pub max_clients:      usize,
    pub protocol_id:      u64,
    pub cert:             CertSource,
    pub server_name:      Option<String>,
    pub server_cert_hash: Option<[u8; 32]>,
}

impl NetConfig {
    /// A server bound to `bind` with sensible defaults.
    pub fn server(bind: SocketAddr) -> Self {
        Self {
            mode:             NetMode::Server { bind },
            max_clients:      64,
            protocol_id:      DEFAULT_PROTOCOL_ID,
            cert:             CertSource::default(),
            server_name:      None,
            server_cert_hash: None,
        }
    }

    /// A client connecting to `connect` with sensible defaults.
    pub fn client(connect: SocketAddr) -> Self {
        Self {
            mode:             NetMode::Client { connect },
            max_clients:      1,
            protocol_id:      DEFAULT_PROTOCOL_ID,
            cert:             CertSource::default(),
            server_name:      None,
            server_cert_hash: None,
        }
    }

    /// Pins the server's self-signed certificate hash for the wasm client
    /// (see [`NetConfig::server_cert_hash`]).
    pub fn with_server_cert_hash(mut self, hash: [u8; 32]) -> Self {
        self.server_cert_hash = Some(hash);
        self
    }
}

/// Builds the [`NetworkManager`] for `config` on the current platform.
///
/// `tickrate` is the local authoritative rate: a server embeds it in its
/// welcome frame for clients to adopt; a client ignores its own value entirely
/// and adopts whatever the server announces instead (see
/// [`redixel_core::NetworkManager::server_tickrate`]).
///
/// Never fails fatally: a transport that cannot start (e.g. a server whose
/// socket fails to bind) logs an error and degrades to a [`NoOpNetwork`] so the
/// game keeps running. Use the backend constructors directly if you need to
/// handle binding errors explicitly.
pub fn build(config: &NetConfig, tickrate: f64) -> Box<dyn NetworkManager> {
    if matches!(config.mode, NetMode::Offline) {
        return Box::new(NoOpNetwork);
    }

    #[cfg(not(target_arch = "wasm32"))]
    {
        crate::webtransport::build(config, tickrate)
    }

    #[cfg(target_arch = "wasm32")]
    {
        crate::wasm::build(config, tickrate)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn with_server_cert_hash_round_trips() {
        let hash: [u8; 32] = [7; 32];
        let config: NetConfig = NetConfig::client("127.0.0.1:0".parse().unwrap()).with_server_cert_hash(hash);
        assert_eq!(config.server_cert_hash, Some(hash));
    }
}
