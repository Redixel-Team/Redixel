# Multiplayer

Redixel supports an **authoritative server** architecture via WebTransport (QUIC over UDP). The headless server owns all simulation; clients send input and render server snapshots. Works on LAN out of the box. For internet play, forward **UDP 5000** on the server's router.

```bash
# Server — binds to 0.0.0.0:5000 by default, or pass [bind_addr:port]
cargo run --release --manifest-path examples/shooter_mp/Cargo.toml -- server [bind_addr:port]

# Client — connects to 127.0.0.1:5000 by default, or pass [server_addr:port]
cargo run --release --manifest-path examples/shooter_mp/Cargo.toml -- client [server_addr:port]
```

> If clients on other machines can't connect, check that **UDP 5000** is open on the server's firewall.

**Cross-play:** every platform — Windows, Linux, macOS, Android, iOS, and the browser — shares the exact same `NetworkManager`/WebTransport wire protocol; only the transport binding differs (`wtransport` natively, `web-sys` in the browser). A headless server on Linux, a windowed client on Windows, and a client running in a browser tab can all join the same match interchangeably.

## Playing from the browser

The browser client needs the WASM toolchain from [Running on Web](./platforms/web.md) installed first. Browsers cannot simply disable certificate validation the way native/mobile clients do, so connecting to a self-signed LAN server requires **pinning its certificate hash**:

1. **Start the server** and copy the hash it logs on startup:

   ```bash
   cargo run --release --manifest-path examples/shooter_mp/Cargo.toml -- server [bind_addr:port]
   # WebTransport server certificate hash (sha-256): <64 hex characters>
   ```

2. **Paste it** into `SERVER_CERT_HASH_HEX` in `examples/shooter_mp/src/lib.rs` (the `wasm` module), replacing the `"SERVER_CERT_HASH_HEX"` placeholder. If the server binds to anything other than `127.0.0.1:5000`, also update `SERVER_ADDR` in the same file — the browser client has no argv, so the address is fixed at build time.

3. **Run the client:**

   ```bash
   cargo run --release --manifest-path examples/shooter_mp/Cargo.toml --target wasm32-unknown-unknown
   ```

> Restart the server and the pinned hash goes stale — a fresh self-signed certificate is generated each run, so re-copy it and rebuild the client. Connecting to a CA-trusted `server_name` deployment needs no pinning: leave `SERVER_CERT_HASH_HEX` empty.
