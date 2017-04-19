use std::sync::mpsc;
use std::net::SocketAddr;
use std::thread;
use std::time::Duration;

use mio::*;
use mio::tcp::*;

use torrent::*;
use tracker::*;
use peer::*;
use utils::*;

pub struct Client {
    torrent: Torrent,
}

impl Client {
    pub fn new(file: &str) -> Client {
        Client {
            torrent: Torrent::new(&file).unwrap()
        }
    }

    pub fn start(&mut self) {
        let (tx, rx) = mpsc::channel();
        let (tpieces, rpieces) = mpsc::channel();
        let event_loop_channel = self.spawn_event_loop(tx);
        {
            let event_loop_channel = event_loop_channel.clone();
            let tracker = { self.torrent.tracker.clone() };
            self.spawn_tracker_update(event_loop_channel, tracker);
        }

        loop {
            // Push data packets received from event loop to each Peer
            while let Ok(packet) = rx.try_recv() {
                match packet {
                    Message::AddPeer(addr) => {
                        if !self.torrent.peers.contains_key(&addr) {
                            let event_loop_channel = event_loop_channel.clone();
                            let tpieces = tpieces.clone();
                            let peer = Peer::new(addr, &self.torrent, event_loop_channel, tpieces);
                            self.torrent.peers.insert(addr, peer);
                        }
                    }
                    Message::Data(addr, data) => {
                        self.torrent.peers.get_mut(&addr).unwrap().read(data);
                    }
                    Message::Disconnect(_addr) => {}
                }
            }

            // Process Peers
            self.process_peers();

            // Write received blocks/pieces to files
            while let Ok(packet) = rpieces.try_recv() {
                let (piece, begin, block) = packet;
                self.torrent.write_block(piece, begin, block);
            }

            // Process Downloads
            self.process_downloads();

            thread::sleep(Duration::from_millis(29));
        }
    }

    fn spawn_event_loop(&self, sender: mpsc::Sender<Message>) -> Sender<Message> {
        println!("client: spawning event loop thread");

        let address = "0.0.0.0:56789".parse::<SocketAddr>().unwrap();
        let server_socket = TcpListener::bind(&address).unwrap();

        let mut event_loop = EventLoop::new().unwrap();
        let event_loop_channel: Sender<Message> = event_loop.channel();
        let mut handler = PeerHandler::new(server_socket, sender);

        event_loop.register(&handler.socket, SERVER_TOKEN, EventSet::readable(), PollOpt::edge()).unwrap();
        thread::spawn(move || {
            event_loop.run(&mut handler).unwrap();
        });
        event_loop_channel
    }

    fn spawn_tracker_update(&self, event_loop_channel: Sender<Message>, tracker: Tracker) {
        println!("client: spawning tracker thread");

        thread::spawn(move || {
            loop {
                let peer_addresses = tracker.get_peers_addresses();
                if peer_addresses.is_empty() {
                    println!("torrent: no peers found!");
                } else {
                    for peer in &peer_addresses {
                        event_loop_channel.send(Message::AddPeer(*peer)).unwrap();
                    }
                }
                thread::sleep(Duration::from_secs(30 * 60)); // 30 mins
            }
        });
    }

    fn process_peers(&mut self) {
        let is_complete = self.torrent.is_complete();
        for (addr, peer) in &mut self.torrent.peers {
            peer.process_data();

            if peer.is_timed_out() {
                //FIXME: disconnect the peer
                continue;
            }

            if !peer.is_handshake_sent || !peer.is_handshake_received {
                continue;
            }

            if is_complete {
                peer.send_not_interested();
            } else {
                peer.send_interested();
            }

            peer.send_keepalive();

            if !is_complete
                && !peer.is_choke_received
                && self.torrent.seeders.len() < 7
                && !self.torrent.seeders.contains(&addr)
            { // FIXME: make the number of seeders configurable
                println!("client: adding {} to seeders", addr);
                self.torrent.seeders.push(*addr);
            }
        }
    }

    fn process_downloads(&mut self) {
        if self.torrent.is_complete() {
            return;
        }

        for piece in 0..self.torrent.no_of_pieces {
            // Check if the piece is already downloaded
            if self.torrent.is_piece_downloaded[piece] {
                continue;
            }

            let block_count = self.torrent.get_block_count(piece);
            // Go through the seeders and request pieces
            for block in 0..block_count {
                // Skip if the block is already downloaded
                if self.torrent.is_block_downloaded[piece][block] {
                    continue;
                }

                // Skip if the block is already requested
                if self.torrent.is_block_requested(piece, block) {
                    continue;
                }

                let size = self.torrent.get_block_size(piece, block);
                for addr in &mut self.torrent.seeders {
                    let seeder = self.torrent.peers.get_mut(addr).unwrap();
                    if !seeder.is_piece_downloaded[piece] {
                        continue;
                    }

                    if seeder.no_of_blocks_requested() > 5 { // FIXME: make this configurable
                        continue;
                    }

                    seeder.send_request(piece, block * BLOCK_SIZE, size);
                    break;
                }
            }
        }
    }
}
