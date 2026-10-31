use std::fmt;

/// Bytes of header prepended to each unreliable datagram:
/// `[u32 seq][u16 fragment index][u16 fragment count]`, little-endian.
pub const HEADER_LEN: usize = 8;

/// Most fragments one unreliable message may be split into.
///
/// A peer picks `count` freely, and the receiver sizes its slot table from it,
/// so this bounds the allocation a single datagram can provoke. At a ~1.2 KB
/// datagram limit this still covers messages past [`MAX_MESSAGE_LEN`].
pub const MAX_FRAGMENTS: u16 = 1024;

/// Upper bound on a reassembled unreliable message. Fragments accumulate in
/// memory until the message completes, so this bounds what a peer can pin per
/// connection before anything is delivered.
pub const MAX_MESSAGE_LEN: usize = 1024 * 1024;

/// How one unreliable message is split across datagrams: [`count`](Self::count)
/// fragments of at most `chunk` payload bytes each, carved out of the payload it
/// was planned for.
///
/// Holding the payload is what keeps the split self-consistent: there is no way
/// to iterate one message's fragments while reporting another's count, so the
/// `count` stamped into every header is always the number of fragments that
/// actually follow it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Plan<'a> {
    payload: &'a [u8],
    chunk:   usize,
    count:   u16,
}

impl<'a> Plan<'a> {
    /// Decides how to split `payload` under the peer's `max_datagram` limit, or
    /// why it must be dropped.
    ///
    /// Shared by every backend so the wire framing cannot drift between them: an
    /// empty message still costs one fragment, which is why `count` is floored
    /// at 1.
    pub fn new(max_datagram: usize, payload: &'a [u8]) -> Result<Self, PlanError> {
        if max_datagram <= HEADER_LEN {
            return Err(PlanError::DatagramTooSmall(max_datagram));
        }

        if payload.len() > MAX_MESSAGE_LEN {
            return Err(PlanError::MessageTooLarge(payload.len()));
        }

        let chunk: usize = max_datagram - HEADER_LEN;
        let count: usize = payload.len().div_ceil(chunk).max(1);
        if count > MAX_FRAGMENTS as usize {
            return Err(PlanError::TooManyFragments {
                len: payload.len(),
                count,
            });
        }

        Ok(Self {
            payload,
            chunk,
            count: count as u16,
        })
    }

    /// The number of fragments [`fragments`](Self::fragments) will yield — the
    /// `count` every fragment header must carry.
    pub fn count(&self) -> u16 {
        self.count
    }

    /// Yields `(fragment index, fragment payload)` for each fragment, in index
    /// order.
    ///
    /// An empty payload yields one empty fragment rather than nothing, so that a
    /// zero-length message still reaches the peer — matching the floor of 1.
    pub fn fragments(&self) -> impl Iterator<Item = (u16, &'a [u8])> + 'a {
        let payload: &'a [u8] = self.payload;
        let empty: usize = usize::from(payload.is_empty());

        std::iter::once((0u16, &payload[..0])).take(empty).chain(
            payload
                .chunks(self.chunk)
                .enumerate()
                .map(|(index, fragment): (usize, &[u8])| (index as u16, fragment)),
        )
    }
}

/// Why an unreliable message cannot be sent. Every variant means the message is
/// **dropped** — the unreliable channel never promotes a message it cannot
/// fragment onto the reliable stream.
///
/// - `DatagramTooSmall`: the peer's datagram limit leaves no room for the header
///   itself.
/// - `MessageTooLarge`: the message is larger than a receiver will reassemble
///   ([`MAX_MESSAGE_LEN`]).
/// - `TooManyFragments`: the message needs more fragments than a receiver will
///   track ([`MAX_FRAGMENTS`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanError {
    DatagramTooSmall(usize),
    MessageTooLarge(usize),
    TooManyFragments { len: usize, count: usize },
}

impl fmt::Display for PlanError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PlanError::DatagramTooSmall(max_datagram) => {
                write!(
                    f,
                    "Datagram limit of {max_datagram} bytes cannot fit the sequence header; dropping."
                )
            }
            PlanError::MessageTooLarge(len) => write!(
                f,
                "Dropping a {len}-byte unreliable message over the {MAX_MESSAGE_LEN}-byte reassembly limit."
            ),
            PlanError::TooManyFragments { len, count } => write!(
                f,
                "An unreliable message of {len} bytes needs {count} fragments, over the {MAX_FRAGMENTS} limit; dropping."
            ),
        }
    }
}

/// Returns `true` if `a` is strictly newer than `b`, using serial-number
/// arithmetic (RFC 1982) so the comparison still works after `u32` wraps.
#[inline]
pub fn seq_greater(a: u32, b: u32) -> bool {
    const HALF: u32 = u32::MAX / 2;
    ((a > b) && (a - b <= HALF)) || ((a < b) && (b - a > HALF))
}

/// Accepts `seq` if it is newer than the last accepted sequence `last`, updating
/// `last` on acceptance.
#[inline]
pub fn accept_seq(last: &mut Option<u32>, seq: u32) -> bool {
    let accept: bool = match *last {
        None => true,
        Some(prev) => seq_greater(seq, prev),
    };

    if accept {
        *last = Some(seq);
    }

    accept
}

/// Writes the header for fragment `index` of `count` belonging to `seq` into
/// `out` (cleared first), then appends `payload`, reusing `out`'s capacity.
#[inline]
pub fn frame(out: &mut Vec<u8>, seq: u32, index: u16, count: u16, payload: &[u8]) {
    out.clear();
    out.extend_from_slice(&seq.to_le_bytes());
    out.extend_from_slice(&index.to_le_bytes());
    out.extend_from_slice(&count.to_le_bytes());
    out.extend_from_slice(payload);
}

/// Splits a received datagram into `(seq, fragment index, fragment count, payload)`,
/// or `None` if it is too short to contain a header.
#[inline]
pub fn split(msg: &[u8]) -> Option<(u32, u16, u16, &[u8])> {
    if msg.len() < HEADER_LEN {
        return None;
    }

    let seq: u32 = u32::from_le_bytes(msg[0..4].try_into().ok()?);
    let index: u16 = u16::from_le_bytes(msg[4..6].try_into().ok()?);
    let count: u16 = u16::from_le_bytes(msg[6..8].try_into().ok()?);

    Some((seq, index, count, &msg[HEADER_LEN..]))
}

/// Reassembles the fragments of one unreliable message stream, enforcing
/// newest-wins: fragments of a sequence older than the one currently being
/// assembled are dropped, and a newer sequence abandons an incomplete older one.
///
/// A message is yielded exactly once, on the arrival of its final missing
/// fragment. Duplicate fragments — and any fragment of an already-delivered
/// sequence — are ignored, so an at-most-once delivery holds per sequence.
///
/// Both the fragment count and the accumulated payload are bounded
/// ([`MAX_FRAGMENTS`], [`MAX_MESSAGE_LEN`]): the counts come straight off the
/// wire, so a peer must not be able to size our allocations for us.
#[derive(Debug, Default)]
pub struct Reassembler {
    seq:          Option<u32>,
    parts:        Vec<Option<Vec<u8>>>,
    remaining:    usize,
    received_len: usize,
}

impl Reassembler {
    pub fn new() -> Self {
        Self::default()
    }

    /// Drops the in-flight sequence and releases its buffers, marking it done
    /// (`remaining == 0`) so its remaining fragments are ignored instead of
    /// restarting it.
    fn abandon(&mut self) {
        self.remaining = 0;
        self.received_len = 0;
        self.parts.clear();
        self.parts.shrink_to_fit();
    }

    /// Feeds one fragment. Returns the fully reassembled payload once the last
    /// missing fragment of the newest sequence arrives, `None` otherwise.
    pub fn push(&mut self, seq: u32, index: u16, count: u16, payload: &[u8]) -> Option<Vec<u8>> {
        if count == 0 || count > MAX_FRAGMENTS || index >= count {
            return None;
        }

        let fresh: bool = match self.seq {
            None => true,
            Some(current) if seq == current => false,
            Some(current) => {
                if !seq_greater(seq, current) {
                    return None;
                }
                true
            }
        };

        if fresh {
            self.seq = Some(seq);
            self.remaining = count as usize;
            self.received_len = 0;
            self.parts.clear();
            self.parts.resize_with(count as usize, || None);
        } else if self.remaining == 0 || self.parts.len() != count as usize {
            return None;
        }

        let slot: &mut Option<Vec<u8>> = &mut self.parts[index as usize];
        if slot.is_some() {
            return None;
        }

        if self.received_len + payload.len() > MAX_MESSAGE_LEN {
            self.abandon();
            return None;
        }

        self.received_len += payload.len();
        *slot = Some(payload.to_vec());
        self.remaining -= 1;

        if self.remaining > 0 {
            return None;
        }

        let total: usize = self
            .parts
            .iter()
            .map(|p: &Option<Vec<u8>>| p.as_ref().map_or(0, Vec::len))
            .sum();
        let mut out: Vec<u8> = Vec::with_capacity(total);
        for part in self.parts.iter_mut() {
            if let Some(bytes) = part.take() {
                out.extend_from_slice(&bytes);
            }
        }

        Some(out)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const MAX_DATAGRAM: usize = HEADER_LEN + 100;

    fn mock_payload(len: usize) -> Vec<u8> {
        (0..len).map(|i: usize| (i % 251) as u8).collect()
    }

    #[test]
    fn seq_greater_basic_ordering() {
        assert!(seq_greater(2, 1));
        assert!(!seq_greater(1, 2));
        assert!(!seq_greater(5, 5));
    }

    #[test]
    fn seq_greater_handles_wraparound() {
        assert!(seq_greater(1, u32::MAX));
        assert!(!seq_greater(u32::MAX, 1));
    }

    #[test]
    fn accept_seq_drops_stale_and_duplicate() {
        let mut last: Option<u32> = None;
        assert!(accept_seq(&mut last, 10));
        assert!(accept_seq(&mut last, 11));
        assert!(!accept_seq(&mut last, 11));
        assert!(!accept_seq(&mut last, 9));
        assert!(accept_seq(&mut last, 12));
        assert_eq!(last, Some(12));
    }

    #[test]
    fn frame_and_split_round_trip() {
        let mut buf: Vec<u8> = Vec::new();
        frame(&mut buf, 0xDEAD_BEEF, 2, 5, b"hello");
        let (seq, index, count, body): (u32, u16, u16, &[u8]) = split(&buf).unwrap();
        assert_eq!(seq, 0xDEAD_BEEF);
        assert_eq!(index, 2);
        assert_eq!(count, 5);
        assert_eq!(body, b"hello");
    }

    #[test]
    fn split_rejects_short_message() {
        assert!(split(&[1, 2, 3]).is_none());
    }

    #[test]
    fn reassembler_passes_through_single_fragment() {
        let mut r: Reassembler = Reassembler::new();
        assert_eq!(r.push(1, 0, 1, b"solo").unwrap(), b"solo".to_vec());
    }

    #[test]
    fn reassembler_joins_fragments_in_any_arrival_order() {
        let mut r: Reassembler = Reassembler::new();
        assert!(r.push(7, 2, 3, b"c").is_none());
        assert!(r.push(7, 0, 3, b"a").is_none());
        assert_eq!(r.push(7, 1, 3, b"b").unwrap(), b"abc".to_vec());
    }

    #[test]
    fn reassembler_drops_stale_sequence() {
        let mut r: Reassembler = Reassembler::new();
        assert_eq!(r.push(5, 0, 1, b"new").unwrap(), b"new".to_vec());
        assert!(r.push(4, 0, 1, b"old").is_none());
    }

    #[test]
    fn reassembler_abandons_incomplete_older_sequence() {
        let mut r: Reassembler = Reassembler::new();
        assert!(r.push(1, 0, 2, b"partial").is_none());
        assert_eq!(r.push(2, 0, 1, b"newer").unwrap(), b"newer".to_vec());
        assert!(r.push(1, 1, 2, b"late").is_none());
    }

    #[test]
    fn reassembler_ignores_duplicate_fragments_and_redelivery() {
        let mut r: Reassembler = Reassembler::new();
        assert!(r.push(3, 0, 2, b"a").is_none());
        assert!(r.push(3, 0, 2, b"a").is_none());
        assert_eq!(r.push(3, 1, 2, b"b").unwrap(), b"ab".to_vec());
        assert!(r.push(3, 1, 2, b"b").is_none());
    }

    #[test]
    fn reassembler_rejects_malformed_counts() {
        let mut r: Reassembler = Reassembler::new();
        assert!(r.push(1, 0, 0, b"x").is_none());
        assert!(r.push(1, 3, 3, b"x").is_none());
    }

    #[test]
    fn reassembler_refuses_a_peer_dictated_fragment_count() {
        let mut r: Reassembler = Reassembler::new();
        assert!(
            r.push(1, 0, MAX_FRAGMENTS + 1, b"x").is_none(),
            "a peer must not size our slot table past MAX_FRAGMENTS"
        );
        assert!(r.parts.is_empty(), "the oversized count must allocate nothing");
    }

    #[test]
    fn reassembler_abandons_a_message_past_the_size_limit() {
        let mut r: Reassembler = Reassembler::new();
        let chunk: Vec<u8> = vec![0u8; 4096];
        let count: u16 = MAX_FRAGMENTS;

        let mut delivered: Option<Vec<u8>> = None;
        for index in 0..count {
            if let Some(msg) = r.push(1, index, count, &chunk) {
                delivered = Some(msg);
            }
        }

        assert!(delivered.is_none(), "a message past MAX_MESSAGE_LEN must never assemble");
        assert!(r.parts.is_empty(), "the abandoned sequence must release its buffers");
        assert_eq!(r.push(2, 0, 1, b"ok").unwrap(), b"ok".to_vec());
    }

    #[test]
    fn reassembler_survives_sequence_wraparound() {
        let mut r: Reassembler = Reassembler::new();
        assert_eq!(r.push(u32::MAX, 0, 1, b"last").unwrap(), b"last".to_vec());
        assert_eq!(r.push(0, 0, 1, b"wrapped").unwrap(), b"wrapped".to_vec());
    }

    #[test]
    fn plan_fits_an_exact_multiple_of_the_chunk() {
        let bytes: Vec<u8> = mock_payload(300);
        let plan: Plan<'_> = Plan::new(MAX_DATAGRAM, &bytes).expect("300 bytes fits in 3 chunks");
        assert_eq!(plan.count(), 3, "an exact multiple must not round up to a spare fragment");
    }

    #[test]
    fn plan_rounds_a_partial_last_fragment_up() {
        let over: Vec<u8> = mock_payload(301);
        let under: Vec<u8> = mock_payload(299);
        assert_eq!(Plan::new(MAX_DATAGRAM, &over).expect("301 bytes fits").count(), 4);
        assert_eq!(Plan::new(MAX_DATAGRAM, &under).expect("299 bytes fits").count(), 3);
    }

    #[test]
    fn plan_charges_one_fragment_for_a_tiny_or_empty_message() {
        assert_eq!(
            Plan::new(MAX_DATAGRAM, b"")
                .expect("an empty message still plans")
                .count(),
            1
        );
        assert_eq!(Plan::new(MAX_DATAGRAM, b"x").expect("a one-byte message plans").count(), 1);
    }

    #[test]
    fn plan_rejects_a_datagram_limit_that_cannot_hold_the_header() {
        assert_eq!(Plan::new(HEADER_LEN, b"payload"), Err(PlanError::DatagramTooSmall(HEADER_LEN)));
        assert_eq!(
            Plan::new(HEADER_LEN - 1, b"payload"),
            Err(PlanError::DatagramTooSmall(HEADER_LEN - 1))
        );
        assert_eq!(Plan::new(0, b"payload"), Err(PlanError::DatagramTooSmall(0)));
        assert!(
            Plan::new(HEADER_LEN + 1, b"payload").is_ok(),
            "one spare byte is enough to fragment"
        );
    }

    #[test]
    fn plan_rejects_a_message_past_the_reassembly_limit() {
        let max_datagram: usize = HEADER_LEN + MAX_MESSAGE_LEN.div_ceil(MAX_FRAGMENTS as usize);
        let at_limit: Vec<u8> = mock_payload(MAX_MESSAGE_LEN);
        let over_limit: Vec<u8> = mock_payload(MAX_MESSAGE_LEN + 1);

        assert!(Plan::new(max_datagram, &at_limit).is_ok(), "the limit itself is allowed");
        assert_eq!(
            Plan::new(max_datagram, &over_limit),
            Err(PlanError::MessageTooLarge(MAX_MESSAGE_LEN + 1))
        );
    }

    #[test]
    fn plan_rejects_a_message_needing_more_fragments_than_the_receiver_tracks() {
        let max_datagram: usize = HEADER_LEN + 1;
        let len: usize = MAX_FRAGMENTS as usize + 1;
        let over: Vec<u8> = mock_payload(len);
        let at_limit: Vec<u8> = mock_payload(MAX_FRAGMENTS as usize);

        assert_eq!(
            Plan::new(max_datagram, &over),
            Err(PlanError::TooManyFragments { len, count: len }),
            "a tiny datagram limit must not be able to demand an unbounded slot table"
        );
        assert!(Plan::new(max_datagram, &at_limit).is_ok(), "exactly MAX_FRAGMENTS is allowed");
    }

    #[test]
    fn fragments_yields_one_empty_fragment_for_an_empty_payload() {
        let plan: Plan<'_> = Plan::new(MAX_DATAGRAM, b"").expect("an empty message plans");
        let parts: Vec<(u16, &[u8])> = plan.fragments().collect();
        assert_eq!(parts, vec![(0u16, &[][..])], "an empty message must still reach the peer");
    }

    #[test]
    fn fragments_yields_exactly_the_planned_count_in_index_order() {
        let bytes: Vec<u8> = mock_payload(250);
        let plan: Plan<'_> = Plan::new(MAX_DATAGRAM, &bytes).expect("250 bytes plans");
        let parts: Vec<(u16, &[u8])> = plan.fragments().collect();

        assert_eq!(parts.len(), plan.count() as usize);
        assert_eq!(parts.iter().map(|(i, _)| *i).collect::<Vec<u16>>(), vec![0, 1, 2]);
        assert_eq!(parts[2].1.len(), 50, "the last fragment carries the remainder");
        assert!(parts[..2].iter().all(|(_, f)| f.len() == 100));
    }

    #[test]
    fn planned_fragments_round_trip_through_the_reassembler() {
        let lengths: [usize; 8] = [0, 1, 99, 100, 101, 250, 1000, 4096];
        let limits: [usize; 4] = [HEADER_LEN + 1, HEADER_LEN + 100, HEADER_LEN + 1200, HEADER_LEN + 8192];

        for (message, max_datagram) in lengths.iter().zip(limits.iter().cycle()) {
            let bytes: Vec<u8> = mock_payload(*message);
            let Ok(plan): Result<Plan<'_>, PlanError> = Plan::new(*max_datagram, &bytes) else {
                continue;
            };

            let mut reassembler: Reassembler = Reassembler::new();
            let mut delivered: Option<Vec<u8>> = None;
            let mut buf: Vec<u8> = Vec::new();

            for (index, fragment) in plan.fragments() {
                frame(&mut buf, 7, index, plan.count(), fragment);

                let (seq, index, count, body): (u32, u16, u16, &[u8]) =
                    split(&buf).expect("a framed fragment must split back");
                assert_eq!(seq, 7);

                if let Some(msg) = reassembler.push(seq, index, count, body) {
                    delivered = Some(msg);
                }
            }

            assert_eq!(
                delivered.as_ref(),
                Some(&bytes),
                "a {message}-byte message under a {max_datagram}-byte datagram limit must reassemble intact"
            );
        }
    }
}
