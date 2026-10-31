use std::net::SocketAddr;

use redixel::prelude::{NetConfig, RuntimeConfig};

use crate::client::Client;

/// Android, iOS, and wasm have no argv, so the server address is fixed here —
/// edit it to your PC's LAN IP (e.g. `192.168.0.1:5000`) before building any
/// of those three clients.
const SERVER_ADDR: &str = "127.0.0.1:5000";

/// SHA-256 hash (64 lowercase hex chars, no separators) of the server's
/// self-signed certificate, logged by the native server on startup —
/// paste it here to let the browser trust a self-signed/LAN server.
/// Leave empty when connecting to a CA-trusted `server_name` deployment.
#[cfg(target_arch = "wasm32")]
const SERVER_CERT_HASH_HEX: &str = "SERVER_CERT_HASH_HEX";

pub fn client() -> Client {
    Client::new()
}

pub fn config() -> RuntimeConfig {
    let connect: SocketAddr = SERVER_ADDR.parse().expect("SERVER_ADDR must be a valid `ip:port`");

    redixel::build_config().with_net(pin_certificate(NetConfig::client(connect)))
}

/// Only the browser has to be told which certificate to trust: native clients
/// with no `server_name` connect with certificate validation disabled.
#[cfg(target_arch = "wasm32")]
fn pin_certificate(net_config: NetConfig) -> NetConfig {
    use crate::proto::parse_cert_hash_hex;

    if SERVER_CERT_HASH_HEX.is_empty() {
        return net_config;
    }

    match parse_cert_hash_hex(SERVER_CERT_HASH_HEX) {
        Some(hash) => net_config.with_server_cert_hash(hash),
        None => {
            log::error!("SERVER_CERT_HASH_HEX is not 64 hex characters; connecting without certificate pinning.");
            net_config
        }
    }
}

#[cfg(not(target_arch = "wasm32"))]
fn pin_certificate(net_config: NetConfig) -> NetConfig {
    net_config
}
