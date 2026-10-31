use std::sync::mpsc::{Receiver, Sender, TryRecvError, channel};

use redixel_core::{ClientId, NetworkChannel, NetworkEvent, NetworkManager, SERVER_ID};

use crate::{
    inbound::{Inbound, InboundQueue},
    seq,
};

/// The id the loopback server assigns to its single peer.
const LOOPBACK_CLIENT_ID: ClientId = 1;

struct Packet {
    channel: NetworkChannel,
    seq:     u32,
    data:    Vec<u8>,
}

/// One end of an in-process server/client link, useful for tests and
/// single-process hosting. Create a connected pair with
/// [`LoopbackNetwork::pair`].
pub struct LoopbackNetwork {
    is_server: bool,
    local_id:  ClientId,
    tx:        Sender<Packet>,
    rx:        Receiver<Packet>,
    queue:     InboundQueue<Vec<u8>>,
    last_seq:  Option<u32>,
    send_seq:  u32,
    announced: bool,
    peer_gone: bool,
}

impl LoopbackNetwork {
    /// Creates a connected `(server, client)` pair. Drive both with
    /// [`update`](NetworkManager::update) each tick; the first update on each
    /// side yields a [`Connected`](NetworkEvent::Connected) event.
    pub fn pair() -> (LoopbackNetwork, LoopbackNetwork) {
        let (s2c_tx, s2c_rx): (Sender<Packet>, Receiver<Packet>) = channel();
        let (c2s_tx, c2s_rx): (Sender<Packet>, Receiver<Packet>) = channel();

        let server: LoopbackNetwork = LoopbackNetwork {
            is_server: true,
            local_id:  SERVER_ID,
            tx:        s2c_tx,
            rx:        c2s_rx,
            queue:     InboundQueue::new(),
            last_seq:  None,
            send_seq:  0,
            announced: false,
            peer_gone: false,
        };

        let client: LoopbackNetwork = LoopbackNetwork {
            is_server: false,
            local_id:  LOOPBACK_CLIENT_ID,
            tx:        c2s_tx,
            rx:        s2c_rx,
            queue:     InboundQueue::new(),
            last_seq:  None,
            send_seq:  0,
            announced: false,
            peer_gone: false,
        };

        (server, client)
    }

    /// The client id this side reports messages as coming *from*: a remote
    /// client (`1`) on the server, the server sentinel on the client.
    fn sender_id(&self) -> ClientId {
        if self.is_server { LOOPBACK_CLIENT_ID } else { SERVER_ID }
    }

    fn enqueue(&mut self, channel: NetworkChannel, payload: &[u8]) {
        let seq: u32 = if channel == NetworkChannel::UnreliableSequenced {
            let s: u32 = self.send_seq;
            self.send_seq = self.send_seq.wrapping_add(1);
            s
        } else {
            0
        };
        self.tx
            .send(Packet {
                channel,
                seq,
                data: payload.to_vec(),
            })
            .ok();
    }
}

impl NetworkManager for LoopbackNetwork {
    fn poll(&mut self) -> Option<NetworkEvent<'_>> {
        self.queue.poll()
    }

    fn send(&mut self, _client: ClientId, channel: NetworkChannel, payload: &[u8]) {
        self.enqueue(channel, payload);
    }

    fn broadcast(&mut self, channel: NetworkChannel, payload: &[u8]) {
        self.enqueue(channel, payload);
    }

    fn is_server(&self) -> bool {
        self.is_server
    }

    fn local_client(&self) -> Option<ClientId> {
        if self.is_server { None } else { Some(self.local_id) }
    }

    fn is_connected(&self) -> bool {
        !self.peer_gone
    }

    fn update(&mut self) {
        self.queue.reset();

        if !self.announced {
            self.announced = true;
            let id: ClientId = if self.is_server {
                LOOPBACK_CLIENT_ID
            } else {
                self.local_id
            };
            self.queue.push(Inbound::Connected(id));
        }

        let sender: ClientId = self.sender_id();
        loop {
            match self.rx.try_recv() {
                Ok(pkt) => match pkt.channel {
                    NetworkChannel::ReliableOrdered => self.queue.push(Inbound::Message {
                        client:  sender,
                        channel: pkt.channel,
                        payload: pkt.data,
                    }),
                    NetworkChannel::UnreliableSequenced => {
                        if seq::accept_seq(&mut self.last_seq, pkt.seq) {
                            self.queue.push(Inbound::Message {
                                client:  sender,
                                channel: pkt.channel,
                                payload: pkt.data,
                            });
                        }
                    }
                },
                Err(TryRecvError::Empty) => break,
                Err(TryRecvError::Disconnected) => {
                    if !self.peer_gone {
                        self.peer_gone = true;
                        self.queue.push(Inbound::Disconnected(sender));
                    }
                    break;
                }
            }
        }
    }

    fn flush(&mut self) {}
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drain(net: &mut LoopbackNetwork, out: &mut Vec<String>) {
        while let Some(ev) = net.poll() {
            out.push(match ev {
                NetworkEvent::Connected(id) => format!("conn:{id}"),
                NetworkEvent::Disconnected(id) => format!("disc:{id}"),
                NetworkEvent::Message(id, ch, payload) => {
                    format!("msg:{id}:{}:{}", ch.id(), String::from_utf8_lossy(payload))
                }
            });
        }
    }

    #[test]
    fn pair_reports_connection_on_first_update() {
        let (mut server, mut client) = LoopbackNetwork::pair();
        assert!(server.is_server());
        assert!(!client.is_server());
        assert_eq!(client.local_client(), Some(LOOPBACK_CLIENT_ID));
        assert_eq!(server.local_client(), None);

        server.update();
        client.update();

        let mut s_events: Vec<String> = Vec::new();
        let mut c_events: Vec<String> = Vec::new();
        drain(&mut server, &mut s_events);
        drain(&mut client, &mut c_events);

        assert_eq!(s_events, vec!["conn:1"]);
        assert_eq!(c_events, vec!["conn:1"]);
    }

    #[test]
    fn reliable_message_round_trips_client_to_server() {
        let (mut server, mut client) = LoopbackNetwork::pair();
        server.update();
        client.update();

        client.broadcast(NetworkChannel::ReliableOrdered, b"hello");
        client.flush();

        server.update();
        let mut events: Vec<String> = Vec::new();
        drain(&mut server, &mut events);
        assert_eq!(events, vec!["msg:1:0:hello"]);
    }

    #[test]
    fn unreliable_sequenced_delivers_in_order_stream() {
        let (mut server, mut client) = LoopbackNetwork::pair();
        server.update();
        client.update();

        server.broadcast(NetworkChannel::UnreliableSequenced, b"a");
        server.broadcast(NetworkChannel::UnreliableSequenced, b"b");
        server.broadcast(NetworkChannel::UnreliableSequenced, b"c");

        client.update();
        let mut events: Vec<String> = Vec::new();
        drain(&mut client, &mut events);
        assert_eq!(
            events,
            vec![
                format!("msg:{SERVER_ID}:1:a"),
                format!("msg:{SERVER_ID}:1:b"),
                format!("msg:{SERVER_ID}:1:c"),
            ]
        );
    }

    #[test]
    fn dropping_one_end_disconnects_the_other() {
        let (mut server, mut client) = LoopbackNetwork::pair();
        server.update();
        client.update();
        drop(client);

        server.update();
        let mut events: Vec<String> = Vec::new();
        drain(&mut server, &mut events);
        assert_eq!(events, vec!["disc:1"]);
        assert!(!server.is_connected());
    }
}
