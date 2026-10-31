pub type ClientId = u64;

/// The sender id reported for messages that arrive **from the server** on a
/// client. Reserved so it never collides with a real [`ClientId`].
pub const SERVER_ID: ClientId = u64::MAX;

/// Delivery guarantee for a message. Pick the weakest guarantee a message can
/// tolerate: `ReliableOrdered` for events that must not be lost or reordered
/// (spawns/despawns, damage, RPCs); `UnreliableSequenced` (newest-wins, no
/// retransmission) for continuous state streams where only the latest value
/// matters (positions, rotations).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum NetworkChannel {
    ReliableOrdered,
    UnreliableSequenced,
}

impl NetworkChannel {
    /// Stable wire index for this channel. Backends map their transport
    /// channels to these ids; also handy for array-indexed per-channel state.
    #[inline]
    pub const fn id(self) -> u8 {
        match self {
            NetworkChannel::ReliableOrdered => 0,
            NetworkChannel::UnreliableSequenced => 1,
        }
    }

    pub const COUNT: usize = 2;
}

/// An inbound network event drained from the transport: a peer connecting
/// (`client` is the new peer on a server, or fires once for the local
/// connection on a client) or disconnecting, or a message arriving from
/// `client` on `channel`.
///
/// The payload is **borrowed** from an internal receive buffer owned by the
/// [`NetworkManager`], so polling a message costs no allocation. Deserialize it
/// in place (e.g. `postcard::from_bytes`) or copy out only what you keep — the
/// borrow lives only until the next [`poll`](NetworkManager::poll).
pub enum NetworkEvent<'a> {
    Connected(ClientId),
    Disconnected(ClientId),
    Message(ClientId, NetworkChannel, &'a [u8]),
}

/// The unified transport interface exposed to game code via
/// [`GameContext::network`](crate::game::GameContext::network).
///
/// # Hot-path contract
/// [`poll`](Self::poll) hands out payloads borrowed from an internal receive
/// buffer and never allocates. [`send`](Self::send)/[`broadcast`](Self::broadcast)
/// copy `payload` once into the transport's send queue, since it crosses a
/// thread boundary; implementations must not copy it *again* per peer when
/// broadcasting, nor allocate a fresh scratch buffer to frame each packet.
///
/// # Usage (inside `on_fixed_update`)
/// ```rust,ignore
/// // 1. Drain inbound events — borrow ends each iteration.
/// while let Some(event) = ctx.network().poll() {
///     match event {
///         NetworkEvent::Connected(id) => log::info!("Player {id} joined!"),
///         NetworkEvent::Disconnected(id) => self.despawn_player(id),
///         NetworkEvent::Message(id, .., payload) => {
///             if let Ok(input) = postcard::from_bytes::<PlayerInput>(payload) {
///                 self.apply_input(id, input);
///             }
///         }
///     }
/// }
///
/// // 2. Broadcast authoritative state.
/// let bytes = postcard::to_stdvec(&self.world).unwrap();
/// ctx.network().broadcast(NetworkChannel::UnreliableSequenced, &bytes);
/// ```
pub trait NetworkManager {
    /// Returns the next buffered inbound event, or `None` when drained for this
    /// tick. Call in a `while let` loop; each returned event borrows `self`
    /// until the loop body ends.
    fn poll(&mut self) -> Option<NetworkEvent<'_>>;

    /// Queues `payload` for a single `client` on `channel`.
    ///
    /// On a client, `client` is ignored — messages always go to the server.
    fn send(&mut self, client: ClientId, channel: NetworkChannel, payload: &[u8]);

    /// Queues `payload` for every connected peer on `channel`.
    ///
    /// On a client this sends to the server (the only peer).
    fn broadcast(&mut self, channel: NetworkChannel, payload: &[u8]);

    /// `true` if this instance is the authoritative server.
    fn is_server(&self) -> bool;

    /// The local client id, or `None` on a pure server. Networked clients
    /// report `None` until the handshake assigns an id; the in-process
    /// loopback assigns its id up front.
    fn local_client(&self) -> Option<ClientId>;

    /// `true` once the transport is usable (server bound, or client connected).
    fn is_connected(&self) -> bool;

    /// Round-trip time to the server in seconds (client side). Returns `0.0` on
    /// a server, an unconnected client, or the no-op transport.
    fn rtt(&self) -> f32 {
        0.0
    }

    /// The server's authoritative fixed-update tickrate (Hz), once known.
    ///
    /// `None` until a client connection completes its handshake, or when
    /// negotiation doesn't apply (server, offline, no-op, same-process
    /// loopback — tickrate is already shared there). The runtime checks this
    /// once per frame, *before* feeding elapsed time into the fixed-step
    /// accumulator, and adopts it the moment it becomes `Some` — so a networked
    /// client's simulation rate tracks the server's exactly after a brief
    /// local-config bootstrap window, never dictated by its own config beyond
    /// that.
    fn server_tickrate(&self) -> Option<f64> {
        None
    }

    /// Receives pending packets into the internal buffer that
    /// [`poll`](Self::poll) drains. The runtime calls this once per fixed step,
    /// before `on_fixed_update`.
    fn update(&mut self);

    /// Sends queued outbound packets onto the wire. The runtime calls this once
    /// per fixed step, after `on_fixed_update`. Transports that deliver eagerly
    /// (loopback) or do nothing (no-op) leave this empty.
    fn flush(&mut self);
}

/// A zero-cost [`NetworkManager`] used when no networking is configured.
///
/// Installed as the default so `ctx.network()` is always callable — offline and
/// single-player games hit no special-casing. Every operation is a no-op and
/// [`poll`](NetworkManager::poll) never yields an event.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoOpNetwork;

impl NetworkManager for NoOpNetwork {
    #[inline]
    fn poll(&mut self) -> Option<NetworkEvent<'_>> {
        None
    }

    #[inline]
    fn send(&mut self, _client: ClientId, _channel: NetworkChannel, _payload: &[u8]) {}

    #[inline]
    fn broadcast(&mut self, _channel: NetworkChannel, _payload: &[u8]) {}

    #[inline]
    fn is_server(&self) -> bool {
        false
    }

    #[inline]
    fn local_client(&self) -> Option<ClientId> {
        None
    }

    #[inline]
    fn is_connected(&self) -> bool {
        false
    }

    #[inline]
    fn update(&mut self) {}

    #[inline]
    fn flush(&mut self) {}
}

/// A fixed-capacity ring buffer indexed by a monotonically increasing sequence
/// number — the standard structure for **client-side prediction and
/// reconciliation**.
///
/// Slot for a sequence is `seq % capacity`; each slot remembers which sequence
/// currently owns it, so stale reads (a wrapped-over sequence) return `None`
/// without a separate occupancy scan. Insertion and lookup are O(1) and never
/// allocate after construction.
///
/// This is the storage primitive only. Reconciliation on top of it also needs
/// the client to reproduce the server's simulation step for step — the engine
/// does not guarantee that today, and no shipped example depends on it. Treat
/// the pattern below as the shape of the solution, not a turnkey one.
///
/// # Prediction / reconciliation pattern
/// Tag everything with the fixed tick from
/// [`GameContext::fixed_tick`](crate::game::GameContext::fixed_tick):
///
/// ```rust,ignore
/// // --- each fixed tick on the client ---
/// let tick = ctx.fixed_tick();
/// let input = self.sample_input();
/// self.input_history.insert(tick, input);          // remember what we did
/// self.apply_input_locally(input);                 // predict immediately
/// ctx.network().send(server, NetworkChannel::ReliableOrdered,
///                    &postcard::to_stdvec(&(tick, input)).unwrap());
///
/// // --- when an authoritative snapshot arrives (stamped with last_acked tick) ---
/// self.state = snapshot.state;                      // snap to the truth
/// self.input_history.ack_through(snapshot.tick);    // drop confirmed inputs
/// for t in (snapshot.tick + 1)..=ctx.fixed_tick() { // replay the unconfirmed
///     if let Some(i) = self.input_history.get(t) {
///         self.apply_input_locally(*i);
///     }
/// }
/// ```
#[derive(Debug, Clone)]
pub struct SequenceBuffer<T> {
    slots: Box<[Slot<T>]>,
    capacity: u64,
}

#[derive(Debug, Clone)]
struct Slot<T> {
    seq: Option<u64>,
    value: Option<T>,
}

impl<T> Default for Slot<T> {
    fn default() -> Self {
        Slot { seq: None, value: None }
    }
}

impl<T> SequenceBuffer<T> {
    /// Creates a buffer holding the most recent `capacity` sequence numbers.
    ///
    /// Size it to cover your worst-case round-trip in ticks (e.g. 256 at 60 Hz
    /// ≈ 4.2 s of history). Panics if `capacity` is 0.
    pub fn new(capacity: usize) -> Self {
        assert!(capacity > 0, "SequenceBuffer capacity must be non-zero");

        let mut slots: Vec<Slot<T>> = Vec::with_capacity(capacity);
        slots.resize_with(capacity, Slot::default);

        Self {
            slots: slots.into_boxed_slice(),
            capacity: capacity as u64,
        }
    }

    #[inline]
    fn index(&self, seq: u64) -> usize {
        (seq % self.capacity) as usize
    }

    /// Stores `value` at `seq`, overwriting whatever (older) sequence shared the
    /// slot. Returns the displaced value if it belonged to a different sequence.
    pub fn insert(&mut self, seq: u64, value: T) -> Option<T> {
        let idx: usize = self.index(seq);
        let slot: &mut Slot<T> = &mut self.slots[idx];
        let displaced: Option<T> = if slot.seq != Some(seq) { slot.value.take() } else { None };
        slot.seq = Some(seq);
        slot.value = Some(value);
        displaced
    }

    /// Returns a reference to the value stored at `seq`, if it is still present.
    pub fn get(&self, seq: u64) -> Option<&T> {
        let slot: &Slot<T> = &self.slots[self.index(seq)];

        if slot.seq == Some(seq) {
            slot.value.as_ref()
        } else {
            None
        }
    }

    /// Returns a mutable reference to the value stored at `seq`, if present.
    pub fn get_mut(&mut self, seq: u64) -> Option<&mut T> {
        let idx: usize = self.index(seq);
        let slot: &mut Slot<T> = &mut self.slots[idx];

        if slot.seq == Some(seq) {
            slot.value.as_mut()
        } else {
            None
        }
    }

    /// `true` if a value for `seq` is currently stored.
    pub fn contains(&self, seq: u64) -> bool {
        self.slots[self.index(seq)].seq == Some(seq)
    }

    /// Removes and returns the value stored at `seq`, if present.
    pub fn remove(&mut self, seq: u64) -> Option<T> {
        let idx: usize = self.index(seq);
        let slot: &mut Slot<T> = &mut self.slots[idx];

        if slot.seq == Some(seq) {
            slot.seq = None;
            slot.value.take()
        } else {
            None
        }
    }

    /// Drops every entry whose sequence is `<= seq` (i.e. acknowledged by the
    /// server). Cheap: one pass over the fixed slot array.
    pub fn ack_through(&mut self, seq: u64) {
        for slot in self.slots.iter_mut() {
            if let Some(s) = slot.seq
                && s <= seq
            {
                slot.seq = None;
                slot.value = None;
            }
        }
    }

    /// Empties the buffer.
    pub fn clear(&mut self) {
        for slot in self.slots.iter_mut() {
            slot.seq = None;
            slot.value = None;
        }
    }

    /// Number of sequence slots this buffer can track simultaneously.
    pub fn capacity(&self) -> usize {
        self.capacity as usize
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn channel_ids_are_stable_and_distinct() {
        assert_eq!(NetworkChannel::ReliableOrdered.id(), 0);
        assert_eq!(NetworkChannel::UnreliableSequenced.id(), 1);
        assert_eq!(NetworkChannel::COUNT, 2);
    }

    #[test]
    fn noop_network_yields_nothing() {
        let mut net: NoOpNetwork = NoOpNetwork;
        assert!(net.poll().is_none());
        assert!(!net.is_server());
        assert!(!net.is_connected());
        assert_eq!(net.local_client(), None);
        net.send(1, NetworkChannel::ReliableOrdered, &[1, 2, 3]);
        net.broadcast(NetworkChannel::UnreliableSequenced, &[4, 5]);
        net.update();
    }

    #[test]
    fn sequence_buffer_insert_get_remove() {
        let mut buf: SequenceBuffer<u32> = SequenceBuffer::new(8);
        assert!(buf.get(0).is_none());

        assert!(buf.insert(3, 30).is_none());
        assert_eq!(buf.get(3), Some(&30));
        assert!(buf.contains(3));

        *buf.get_mut(3).unwrap() = 31;
        assert_eq!(buf.get(3), Some(&31));

        assert_eq!(buf.remove(3), Some(31));
        assert!(!buf.contains(3));
        assert!(buf.remove(3).is_none());
    }

    #[test]
    fn sequence_buffer_wraps_and_evicts_old_slot() {
        let mut buf: SequenceBuffer<u32> = SequenceBuffer::new(4);
        buf.insert(1, 10);
        assert_eq!(buf.get(1), Some(&10));

        let displaced: Option<u32> = buf.insert(5, 50);
        assert_eq!(displaced, Some(10));
        assert_eq!(buf.get(5), Some(&50));
        assert!(buf.get(1).is_none());
    }

    #[test]
    fn sequence_buffer_ack_through_clears_confirmed() {
        let mut buf: SequenceBuffer<u32> = SequenceBuffer::new(16);
        for seq in 1..=5 {
            buf.insert(seq, (seq * 10) as u32);
        }

        buf.ack_through(3);
        assert!(buf.get(1).is_none());
        assert!(buf.get(2).is_none());
        assert!(buf.get(3).is_none());
        assert_eq!(buf.get(4), Some(&40));
        assert_eq!(buf.get(5), Some(&50));
    }

    #[test]
    #[should_panic]
    fn sequence_buffer_rejects_zero_capacity() {
        SequenceBuffer::<u32>::new(0);
    }
}
