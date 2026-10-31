use std::{
    cell::{Cell, RefCell, RefMut},
    collections::VecDeque,
    mem,
    net::SocketAddr,
    rc::Rc,
};

use js_sys::{Function, Promise, Uint8Array};

use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::{JsFuture, spawn_local};

use web_sys::{
    ReadableStreamDefaultReader, WebTransport, WebTransportBidirectionalStream, WebTransportDatagramDuplexStream,
    WebTransportHash, WebTransportOptions, Window, WorkerGlobalScope, WritableStreamDefaultWriter,
};

use redixel_core::{ClientId, NetworkChannel, NetworkEvent, NetworkManager, NoOpNetwork, SERVER_ID};

use crate::{
    config::{HANDSHAKE_TIMEOUT, NetConfig, NetMode},
    frame::parse_welcome,
    inbound::{Inbound, InboundQueue},
    seq,
};

/// Hard cap on a single reliable frame — mirrors native's `MAX_FRAME_LEN`
/// (`webtransport.rs`, which doesn't exist on wasm32, so it isn't shared).
/// The receiver allocates exactly the length a peer announces, so this bounds
/// that allocation.
const MAX_FRAME_LEN: usize = 4 * 1024 * 1024;

/// Shared inbound buffer: spawned `spawn_local` tasks push into it, `update`
/// drains it into the client's [`InboundQueue`]. wasm32 has no OS threads, so
/// unlike native's mpsc channel bridge, this is a plain `Rc<RefCell<..>>` —
/// tasks and `update`/`poll` only ever interleave at `.await` points on the
/// same thread.
type Mailbox = Rc<RefCell<VecDeque<Inbound<Vec<u8>>>>>;

/// Reliable frames waiting to be written, in send order. Drained by a single
/// [`reliable_pump`] task so that frames reach the wire in the order they were
/// queued — see [`WasmWebTransportClient::flush`].
type SendQueue = Rc<RefCell<VecDeque<Vec<u8>>>>;

/// Latches once the connection is terminally gone — a handshake that never
/// completed, a reliable write that failed, or a reader stream that ended.
///
/// A dropped connection is terminal (the transport never reconnects, matching
/// native), so this both stops [`flush`](WasmWebTransportClient::flush) from
/// queueing traffic nobody will ever read and bounds the outbound backlog: a
/// client whose server never comes up buffers only until the handshake watchdog
/// fires, not forever.
type Dead = Rc<Cell<bool>>;

/// The reliable-stream writer, datagram writer, and the datagram duplex
/// stream itself (needed for `max_datagram_size()`), available once the
/// handshake completes. Cloning any of these clones the underlying JS
/// reference, not the stream itself.
#[derive(Clone)]
struct Writers {
    reliable: WritableStreamDefaultWriter,
    datagram_writer: WritableStreamDefaultWriter,
    datagrams: WebTransportDatagramDuplexStream,
}

/// Builds the `WebTransportOptions` pinning `hash` via
/// `serverCertificateHashes`, for connecting to a self-signed/LAN server (see
/// [`NetConfig::server_cert_hash`]).
fn build_options(hash: [u8; 32]) -> WebTransportOptions {
    let options: WebTransportOptions = WebTransportOptions::new();

    let value: Uint8Array = Uint8Array::from(hash.as_slice());
    let wt_hash: WebTransportHash = WebTransportHash::new();
    wt_hash.set_algorithm("sha-256");
    wt_hash.set_value_u8_array(&value);

    options.set_server_certificate_hashes(&[wt_hash]);
    options
}

/// Constructs the `WebTransport` session object for `url`, pinning
/// `config.server_cert_hash` when set (self-signed/LAN), otherwise leaving
/// default WebPKI validation (CA-trusted domain cert).
fn new_transport(url: &str, config: &NetConfig) -> Result<WebTransport, JsValue> {
    match config.server_cert_hash {
        Some(hash) => WebTransport::new_with_options(url, &build_options(hash)),
        None => WebTransport::new(url),
    }
}

/// Resolves after `ms` milliseconds, via the browser's `setTimeout`. wasm32 has
/// no tokio, so this is the only timer available to the transport.
///
/// Resolves through whichever global scope is hosting us — a `Window` on the
/// main thread, a `WorkerGlobalScope` in a worker — and, if neither exposes
/// `setTimeout`, resolves immediately rather than leaving the caller pending
/// forever. A watchdog that never fires is the very failure it exists to
/// prevent, so it must fail closed.
async fn sleep(ms: i32) {
    let promise: Promise = Promise::new(&mut |resolve: Function, _reject: Function| {
        let global: JsValue = js_sys::global().into();

        let scheduled: Option<i32> = match global.dyn_ref::<Window>() {
            Some(w) => w
                .set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
                .ok(),
            None => global.dyn_ref::<WorkerGlobalScope>().and_then(|w: &WorkerGlobalScope| {
                w.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms)
                    .ok()
            }),
        };

        if scheduled.is_none() {
            log::error!("wasm webtransport: no setTimeout in this global scope; firing the timer immediately.");
            resolve.call0(&JsValue::NULL).ok();
        }
    });

    JsFuture::from(promise).await.ok();
}

/// Writes one length-prefixed (`[u32 len][payload]`, little-endian) frame to
/// a reliable stream writer — the wasm equivalent of native's `write_frame`.
async fn write_frame_wasm(writer: &WritableStreamDefaultWriter, payload: &[u8]) -> Result<(), JsValue> {
    let mut framed: Vec<u8> = Vec::with_capacity(4 + payload.len());
    framed.extend_from_slice(&(payload.len() as u32).to_le_bytes());
    framed.extend_from_slice(payload);

    let chunk: Uint8Array = Uint8Array::from(framed.as_slice());
    JsFuture::from(writer.write_with_chunk(&JsValue::from(chunk))).await?;
    Ok(())
}

/// Awaits the next `{ done, value }` result from `reader.read()`, returning
/// the chunk as a `Uint8Array`, or `None` once the stream ends or errors.
/// Reads the result via `js_sys::Reflect` rather than a typed dictionary
/// binding, since `ReadableStreamReadResult` isn't otherwise needed.
async fn read_next_chunk(reader: &ReadableStreamDefaultReader) -> Option<Uint8Array> {
    let result: JsValue = JsFuture::from(reader.read()).await.ok()?;

    let done: bool = js_sys::Reflect::get(&result, &JsValue::from_str("done"))
        .ok()?
        .as_bool()
        .unwrap_or(true);

    if done {
        return None;
    }

    let value: JsValue = js_sys::Reflect::get(&result, &JsValue::from_str("value")).ok()?;
    Some(value.unchecked_into())
}

/// Reads one length-prefixed frame from `reader`, accumulating `buf` across
/// however many chunked `read()` calls it takes. Unlike native's
/// `RecvStream::read_exact`, the browser gives no guarantee that a stream
/// chunk aligns with a frame boundary. `None` once the stream ends, errors, or
/// announces a frame larger than [`MAX_FRAME_LEN`].
async fn read_frame_wasm(reader: &ReadableStreamDefaultReader, buf: &mut Vec<u8>) -> Option<Vec<u8>> {
    loop {
        if buf.len() >= 4 {
            let len_bytes: [u8; 4] = buf[0..4].try_into().expect("checked buf.len() >= 4 above");
            let len: usize = u32::from_le_bytes(len_bytes) as usize;

            if len > MAX_FRAME_LEN {
                log::warn!(
                    "Peer announced a {len}-byte frame over the {MAX_FRAME_LEN}-byte limit; dropping the stream."
                );
                return None;
            }

            if buf.len() >= 4 + len {
                let payload: Vec<u8> = buf[4..4 + len].to_vec();
                buf.drain(0..4 + len);
                return Some(payload);
            }
        }

        let chunk: Uint8Array = read_next_chunk(reader).await?;
        buf.extend(chunk.to_vec());
    }
}

/// Forwards inbound reliable frames as [`Inbound::Message`]s until the stream
/// ends, then signals disconnection — the wasm counterpart to native's
/// `reliable_reader` plus its connection-closed watcher, since a browser
/// `WebTransport` exposes no separate connection-level close signal here.
async fn reliable_reader_wasm(reader: ReadableStreamDefaultReader, mut buf: Vec<u8>, mailbox: Mailbox, dead: Dead) {
    while let Some(payload) = read_frame_wasm(&reader, &mut buf).await {
        mailbox.borrow_mut().push_back(Inbound::Message {
            client: SERVER_ID,
            channel: NetworkChannel::ReliableOrdered,
            payload,
        });
    }

    dead.set(true);
    mailbox.borrow_mut().push_back(Inbound::Disconnected(SERVER_ID));
}

/// Reassembles inbound datagram fragments (newest-wins) into whole messages
/// and forwards them as [`Inbound::Message`]s — the wasm counterpart to
/// native's `datagram_reader`. Each `read()` yields one whole datagram
/// (message boundaries are preserved, unlike the byte-stream reader above).
async fn datagram_reader_wasm(reader: ReadableStreamDefaultReader, mailbox: Mailbox) {
    let mut reassembler: seq::Reassembler = seq::Reassembler::new();

    while let Some(chunk) = read_next_chunk(&reader).await {
        let bytes: Vec<u8> = chunk.to_vec();

        let Some((seq_num, index, count, fragment)) = seq::split(&bytes) else {
            continue;
        };

        let Some(payload) = reassembler.push(seq_num, index, count, fragment) else {
            continue;
        };

        mailbox.borrow_mut().push_back(Inbound::Message {
            client: SERVER_ID,
            channel: NetworkChannel::UnreliableSequenced,
            payload,
        });
    }
}

/// Fragments `payload` into already-framed datagrams sized to `max_datagram`,
/// appending each to `out` — the synchronous half of native's `send_unreliable`.
/// [`flush`](WasmWebTransportClient::flush) then hands each framed fragment to
/// the browser with a synchronous `write_with_chunk` call, so the split exists
/// only to advance `send_seq` in tick order before any write is issued.
///
/// The chunking itself comes from [`seq::Plan`] and its `fragments`, shared with
/// native, so the two backends cannot drift apart on the wire format.
///
/// Unreliable by contract: a message that cannot be sent is **dropped**,
/// never promoted onto the reliable stream, matching native.
fn frame_unreliable(max_datagram: u32, send_seq: &mut u32, payload: &[u8], out: &mut Vec<Vec<u8>>) {
    let plan: seq::Plan<'_> = match seq::Plan::new(max_datagram as usize, payload) {
        Ok(plan) => plan,
        Err(e) => {
            log::warn!("{e}");
            return;
        }
    };

    let current: u32 = *send_seq;
    *send_seq = send_seq.wrapping_add(1);

    for (index, fragment) in plan.fragments() {
        let mut framed: Vec<u8> = Vec::with_capacity(seq::HEADER_LEN + fragment.len());
        seq::frame(&mut framed, current, index, plan.count(), fragment);
        out.push(framed);
    }
}

/// Drains `queue` onto the reliable stream, one length-prefixed frame at a
/// time, until it runs dry — then clears `running` so the next
/// [`flush`](WasmWebTransportClient::flush) starts a fresh pump.
///
/// Exactly one pump is ever alive (guarded by `running`), which is what keeps
/// [`NetworkChannel::ReliableOrdered`] ordered: a browser write can stall on
/// backpressure for many ticks, and a fresh task per flush would let a later
/// tick's frame enqueue ahead of an earlier tick's remaining ones. This mirrors
/// native's single long-lived `reliable_writer`.
///
/// A failed write means the stream is gone for good: the connection is marked
/// dead, the queue is dropped and the pump stops, matching native's
/// break-on-error. Marking it dead is what stops the next `flush` from starting
/// a fresh pump over the same broken writer, once per tick, forever.
async fn reliable_pump(writer: WritableStreamDefaultWriter, queue: SendQueue, running: Rc<Cell<bool>>, dead: Dead) {
    loop {
        let next: Option<Vec<u8>> = queue.borrow_mut().pop_front();

        let Some(payload) = next else {
            running.set(false);
            return;
        };

        if let Err(e) = write_frame_wasm(&writer, &payload).await {
            log::warn!(
                "wasm webtransport: reliable write failed: {e:?}; dropping {} queued frames.",
                queue.borrow().len()
            );

            dead.set(true);
            queue.borrow_mut().clear();
            running.set(false);
            return;
        }
    }
}

/// Closes `transport` if the handshake has not finished within
/// [`HANDSHAKE_TIMEOUT`], so a server that accepts the stream and then goes
/// silent cannot leave the client waiting for a welcome frame forever.
///
/// Closing the session makes the pending welcome read resolve, which drops
/// [`connect_wasm`] into its existing failure path — no cross-future
/// cancellation machinery needed.
async fn handshake_watchdog(transport: WebTransport, done: Rc<Cell<bool>>) {
    sleep(HANDSHAKE_TIMEOUT.as_millis() as i32).await;

    if !done.get() {
        log::error!("wasm webtransport: handshake did not complete within {HANDSHAKE_TIMEOUT:?}; closing.");
        transport.close();
    }
}

/// Drives the handshake to completion: awaits the session, opens the
/// bidirectional stream, announces `protocol_id` and exchanges the
/// hello/welcome (reusing the same wire format as native via
/// [`parse_welcome`]), then hands off to the reliable and datagram reader
/// tasks. Returns `false` if the connection never came up, so the caller can
/// mark it [`Dead`] — a client that fails here stays permanently unconnected,
/// matching native's fail-soft behavior.
///
/// Both datagram queues get a max age of one server tick (from the welcome's
/// tickrate, which [`parse_welcome`] guarantees is finite and positive): every
/// datagram on this transport is newest-wins state superseded each tick
/// (inputs up, snapshots down), so the browser expiring a queued one instead
/// of delivering it late mirrors native quinn, whose send path never sits on
/// a paced queue long enough to go stale.
///
/// The whole handshake runs under a [`handshake_watchdog`], so it cannot hang
/// indefinitely on an unresponsive server.
async fn connect_wasm(
    transport: WebTransport,
    protocol_id: u64,
    mailbox: Mailbox,
    writers: Rc<RefCell<Option<Writers>>>,
    server_tickrate: Rc<Cell<f64>>,
    dead: Dead,
) -> bool {
    let done: Rc<Cell<bool>> = Rc::new(Cell::new(false));
    spawn_local(handshake_watchdog(transport.clone(), done.clone()));

    if let Err(e) = JsFuture::from(transport.ready()).await {
        log::error!("wasm webtransport: connection failed: {e:?}");
        return false;
    }

    let bidi: WebTransportBidirectionalStream = match JsFuture::from(transport.create_bidirectional_stream()).await {
        Ok(stream) => stream.unchecked_into(),
        Err(e) => {
            log::error!("wasm webtransport: create_bidirectional_stream failed: {e:?}");
            return false;
        }
    };

    let send_writer: WritableStreamDefaultWriter = match bidi.writable().get_writer() {
        Ok(writer) => writer,
        Err(e) => {
            log::error!("wasm webtransport: get_writer failed: {e:?}");
            return false;
        }
    };

    let recv_reader: ReadableStreamDefaultReader = bidi.readable().get_reader().unchecked_into();

    if write_frame_wasm(&send_writer, &protocol_id.to_le_bytes())
        .await
        .is_err()
    {
        log::error!("wasm webtransport: failed to send hello");
        return false;
    }

    let mut buf: Vec<u8> = Vec::new();
    let welcome: Vec<u8> = match read_frame_wasm(&recv_reader, &mut buf).await {
        Some(frame) => frame,
        None => {
            log::error!("wasm webtransport: server closed before the welcome frame (protocol id mismatch?)");
            return false;
        }
    };

    let (id, tickrate): (ClientId, f64) = match parse_welcome(&welcome) {
        Some(parsed) => parsed,
        None => {
            log::error!("wasm webtransport: malformed welcome");
            return false;
        }
    };

    done.set(true);
    server_tickrate.set(tickrate);

    let datagrams: WebTransportDatagramDuplexStream = transport.datagrams();
    let tick_ms: f64 = 1000.0 / tickrate;
    datagrams.set_outgoing_max_age(tick_ms);
    datagrams.set_incoming_max_age(tick_ms);
    let dgram_writer: WritableStreamDefaultWriter = match datagrams.writable().get_writer() {
        Ok(writer) => writer,
        Err(e) => {
            log::error!("wasm webtransport: datagram get_writer failed: {e:?}");
            return false;
        }
    };

    let dgram_reader: ReadableStreamDefaultReader = datagrams.readable().get_reader().unchecked_into();

    *writers.borrow_mut() = Some(Writers {
        reliable: send_writer,
        datagram_writer: dgram_writer,
        datagrams,
    });

    mailbox.borrow_mut().push_back(Inbound::Connected(id));
    spawn_local(reliable_reader_wasm(recv_reader, buf, mailbox.clone(), dead));
    spawn_local(datagram_reader_wasm(dgram_reader, mailbox));

    true
}

/// Browser client connection to a [`WebTransportServer`](crate::WebTransportServer),
/// speaking the same wire protocol over `web-sys`'s `WebTransport` bindings.
/// wasm32 is single-threaded, so unlike native/mobile there is no background
/// runtime here: `spawn_local` tasks and the synchronous [`NetworkManager`]
/// calls interleave on the same thread.
pub struct WasmWebTransportClient {
    mailbox: Mailbox,
    writers: Rc<RefCell<Option<Writers>>>,
    outbound_reliable: Vec<Vec<u8>>,
    outbound_unreliable: Vec<Vec<u8>>,
    reliable_queue: SendQueue,
    pump_running: Rc<Cell<bool>>,
    dead: Dead,
    queue: InboundQueue<Vec<u8>>,
    id: Option<ClientId>,
    connected: bool,
    server_tickrate: Rc<Cell<f64>>,
    send_seq: u32,
}

impl WasmWebTransportClient {
    /// Starts connecting to `connect`, announcing `config.protocol_id` in its
    /// hello frame — a server configured with a different id closes the
    /// connection immediately. With `config.server_name` set, uses
    /// `https://{name}:{port}` (production, WebPKI-validated); otherwise
    /// `https://{connect}`, pinning `config.server_cert_hash` when present
    /// (LAN / self-signed — see [`NetConfig::with_server_cert_hash`]).
    /// Connection itself happens asynchronously; construction only fails if
    /// the browser rejects the `WebTransport` session synchronously.
    pub fn new(connect: SocketAddr, config: &NetConfig) -> Result<Self, JsValue> {
        let url: String = match &config.server_name {
            Some(name) => format!("https://{name}:{}", connect.port()),
            None => format!("https://{connect}"),
        };

        let transport: WebTransport = new_transport(&url, config)?;
        let mailbox: Mailbox = Rc::new(RefCell::new(VecDeque::new()));
        let writers: Rc<RefCell<Option<Writers>>> = Rc::new(RefCell::new(None));
        let server_tickrate: Rc<Cell<f64>> = Rc::new(Cell::new(0.0));
        let dead: Dead = Rc::new(Cell::new(false));

        spawn_local({
            let (mailbox, writers): (Mailbox, Rc<RefCell<Option<Writers>>>) = (mailbox.clone(), writers.clone());
            let (server_tickrate, dead): (Rc<Cell<f64>>, Dead) = (server_tickrate.clone(), dead.clone());
            let protocol_id: u64 = config.protocol_id;

            async move {
                if !connect_wasm(transport, protocol_id, mailbox, writers, server_tickrate, dead.clone()).await {
                    dead.set(true);
                }
            }
        });

        Ok(Self {
            mailbox,
            writers,
            outbound_reliable: Vec::new(),
            outbound_unreliable: Vec::new(),
            reliable_queue: Rc::new(RefCell::new(VecDeque::new())),
            pump_running: Rc::new(Cell::new(false)),
            dead,
            queue: InboundQueue::new(),
            id: None,
            connected: false,
            server_tickrate,
            send_seq: 0,
        })
    }
}

// Uses the trait's default `rtt()` (`0.0`): `WebTransportStats`
// (`transport.get_stats()`) is new and inconsistent across browsers relative
// to base WebTransport support — not worth a second polling task for a
// debug-only metric in this first cut.
impl NetworkManager for WasmWebTransportClient {
    fn poll(&mut self) -> Option<NetworkEvent<'_>> {
        self.queue.poll()
    }

    fn send(&mut self, _client: ClientId, channel: NetworkChannel, payload: &[u8]) {
        match channel {
            NetworkChannel::ReliableOrdered => self.outbound_reliable.push(payload.to_vec()),
            NetworkChannel::UnreliableSequenced => self.outbound_unreliable.push(payload.to_vec()),
        }
    }

    fn broadcast(&mut self, channel: NetworkChannel, payload: &[u8]) {
        self.send(SERVER_ID, channel, payload);
    }

    fn is_server(&self) -> bool {
        false
    }

    fn local_client(&self) -> Option<ClientId> {
        self.id
    }

    fn is_connected(&self) -> bool {
        self.connected && !self.dead.get()
    }

    fn server_tickrate(&self) -> Option<f64> {
        let rate: f64 = self.server_tickrate.get();
        (rate > 0.0).then_some(rate)
    }

    fn update(&mut self) {
        self.queue.reset();

        let mut mailbox: RefMut<'_, VecDeque<Inbound<Vec<u8>>>> = self.mailbox.borrow_mut();
        while let Some(item) = mailbox.pop_front() {
            match &item {
                Inbound::Connected(id) => {
                    self.id = Some(*id);
                    self.connected = true;
                }
                Inbound::Disconnected(_) => self.connected = false,
                Inbound::Message { .. } => {}
            }

            self.queue.push(item);
        }
    }

    /// Hands this tick's queued traffic to the browser.
    ///
    /// Reliable frames go onto the shared queue that a single [`reliable_pump`]
    /// drains, which is what preserves their order across ticks. Datagrams are
    /// framed and handed to the browser **synchronously**, right here:
    /// `write_with_chunk` initiates the send the moment it is called, so an
    /// input datagram leaves mid-frame exactly like native's `send_datagram`,
    /// instead of waiting for a spawned task to run after the whole frame
    /// callback (render included) has returned — on a tick-quantized server
    /// that deferral shows up as a full extra tick of input latency whenever
    /// it crosses a tick boundary. The spawned task below only awaits the
    /// already-issued writes to log failures; it never gates a send.
    ///
    /// Before the handshake completes there is nothing to write to: reliable
    /// messages keep buffering (they are guaranteed delivery, so dropping them
    /// would break the contract), while unreliable ones are dropped — they are
    /// droppable by definition, and flushing a tick-old backlog of superseded
    /// snapshots the moment the connection opens would be worse than useless.
    ///
    /// Once the connection is [`Dead`] everything is dropped instead: there is
    /// no peer left to deliver to, and buffering for one would grow without
    /// bound.
    fn flush(&mut self) {
        if self.dead.get() {
            self.outbound_reliable.clear();
            self.outbound_unreliable.clear();
            return;
        }

        let writers: Option<Writers> = self.writers.borrow().clone();
        let Some(writers) = writers else {
            self.outbound_unreliable.clear();
            return;
        };

        if !self.outbound_unreliable.is_empty() {
            let unreliable: Vec<Vec<u8>> = mem::take(&mut self.outbound_unreliable);
            let max_datagram: u32 = writers.datagrams.max_datagram_size();

            let mut framed_fragments: Vec<Vec<u8>> = Vec::new();
            for payload in &unreliable {
                frame_unreliable(max_datagram, &mut self.send_seq, payload, &mut framed_fragments);
            }

            let mut pending: Vec<Promise> = Vec::with_capacity(framed_fragments.len());
            for framed in &framed_fragments {
                let chunk: Uint8Array = Uint8Array::from(framed.as_slice());
                pending.push(writers.datagram_writer.write_with_chunk(&JsValue::from(chunk)));
            }

            spawn_local(async move {
                for promise in pending {
                    if let Err(e) = JsFuture::from(promise).await {
                        log::warn!("wasm webtransport: datagram write failed: {e:?}");
                    }
                }
            });
        }

        if self.outbound_reliable.is_empty() {
            return;
        }

        {
            let mut queue: RefMut<'_, VecDeque<Vec<u8>>> = self.reliable_queue.borrow_mut();
            for payload in self.outbound_reliable.drain(..) {
                if payload.len() > MAX_FRAME_LEN {
                    log::error!(
                        "Dropping a {}-byte reliable message over the {MAX_FRAME_LEN}-byte frame limit.",
                        payload.len()
                    );

                    continue;
                }

                queue.push_back(payload);
            }
        }

        if !self.pump_running.get() {
            self.pump_running.set(true);
            spawn_local(reliable_pump(
                writers.reliable.clone(),
                self.reliable_queue.clone(),
                self.pump_running.clone(),
                self.dead.clone(),
            ));
        }
    }
}

/// Builds the browser WebTransport [`NetworkManager`] for `config`, degrading
/// to a [`NoOpNetwork`] (with a logged error) on failure. `tickrate` is
/// unused here (unlike native, a browser can never be `NetMode::Server`, so
/// there is no welcome frame for this side to embed a tickrate into) but kept
/// for signature parity with the native backend's `build`.
pub fn build(config: &NetConfig, _tickrate: f64) -> Box<dyn NetworkManager> {
    match &config.mode {
        NetMode::Server { .. } => {
            log::error!("WebTransport server mode is not supported in the browser. Falling back to no-op network.");
            Box::new(NoOpNetwork)
        }
        NetMode::Client { connect } => match WasmWebTransportClient::new(*connect, config) {
            Ok(client) => {
                log::info!("WebTransport client connecting to {connect}.");
                Box::new(client)
            }
            Err(e) => {
                log::error!("Failed to start WebTransport client to {connect}: {e:?}. Falling back to no-op network.");
                Box::new(NoOpNetwork)
            }
        },
        NetMode::Offline => Box::new(NoOpNetwork),
    }
}
