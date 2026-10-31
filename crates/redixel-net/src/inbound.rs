use redixel_core::{ClientId, NetworkChannel, NetworkEvent};

/// One buffered inbound item. Generic over payload storage so each backend can
/// choose its own (the WebTransport and loopback backends hold `Vec<u8>`), handed out
/// without copying on [`InboundQueue::poll`].
pub(crate) enum Inbound<P> {
    Connected(ClientId),
    Disconnected(ClientId),
    Message {
        client:  ClientId,
        channel: NetworkChannel,
        payload: P,
    },
}

/// A re-fillable queue of inbound events drained once per tick.
///
/// The runtime calls [`reset`](Self::reset) at the top of each `update`, the
/// backend [`push`](Self::push)es freshly received items, and the game drains
/// them with [`poll`](Self::poll). Items live in `items` until the next reset,
/// so [`poll`](Self::poll) can hand out borrowed payload slices safely. The backing `Vec`
/// keeps its capacity across ticks → no steady-state allocation.
pub(crate) struct InboundQueue<P> {
    items:  Vec<Inbound<P>>,
    cursor: usize,
}

impl<P: AsRef<[u8]>> InboundQueue<P> {
    pub(crate) fn new() -> Self {
        Self {
            items:  Vec::new(),
            cursor: 0,
        }
    }

    /// Clears items and rewinds the read cursor, retaining capacity.
    pub(crate) fn reset(&mut self) {
        self.items.clear();
        self.cursor = 0;
    }

    pub(crate) fn push(&mut self, item: Inbound<P>) {
        self.items.push(item);
    }

    /// Returns the next event, borrowing its payload from the queue, or `None`
    /// once drained for this tick.
    pub(crate) fn poll(&mut self) -> Option<NetworkEvent<'_>> {
        let idx: usize = self.cursor;
        if idx >= self.items.len() {
            return None;
        }

        self.cursor += 1;

        Some(match &self.items[idx] {
            Inbound::Connected(id) => NetworkEvent::Connected(*id),
            Inbound::Disconnected(id) => NetworkEvent::Disconnected(*id),
            Inbound::Message {
                client,
                channel,
                payload,
            } => NetworkEvent::Message(*client, *channel, payload.as_ref()),
        })
    }
}
