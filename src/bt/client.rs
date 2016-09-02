use std::sync::mpsc;
use std::net::SocketAddr;
use std::thread;
use std::time::Duration;

use mio::*;
use mio::tcp::*;
use bt::torrent::*;
use bt::tracker::*;
use bt::peer::*;
use bt::utils::*;

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
            println!("client: main loop");
            // Push data packets received from event loop to each Peer
            while let Ok(packet) = rx.try_recv() {
                let (addr, data) = packet;
                if !self.torrent.peers.contains_key(&addr) {
                    let event_loop_channel = event_loop_channel.clone();
                    let tpieces = tpieces.clone();
                    let peer = Peer::new(addr, self.torrent.info_hash.clone(), event_loop_channel, tpieces);
                    self.torrent.peers.insert(addr, peer);
                }
                self.torrent.peers.get_mut(&addr).unwrap().read(data);
            }

            // Write received blocks/pieces to files
            while let Ok(packet) = rpieces.try_recv() {
                let (piece, begin, block) = packet;
                self.torrent.write_block(piece, begin, block);
            }

            // Process Peers
            for (addr, peer) in &mut self.torrent.peers {
                peer.process_data();
                // FIXME: disconnect inactive peers
                if !peer.is_handshake_sent || !peer.is_handshake_received {
                    continue;
                }
    
                peer.send_interested();
                // FIXME: send keep alive
    
                if !peer.is_choke_received && !self.torrent.seeders.contains_key(&addr) {
                    println!("client: adding {} to seeders", addr);
                    self.torrent.seeders.insert(*addr, peer.clone());
                }
            }

            // Process Downloads
            for piece in 0..self.torrent.no_of_pieces {
                // Check if the piece is already downloaded
                if self.torrent.is_piece_downloaded[piece] {
                    continue;
                }

                let block_count = self.torrent.get_block_count(piece);
                println!("block count: {}", block_count);
                // Go through the seeders and request pieces
                for block in 0..block_count {
                    let size = self.torrent.get_block_size(piece, block);
                    for (_, seeder) in &mut self.torrent.seeders {
                        // FIXME: check bitfield and only dl from peers who have the piece

                        // Request only 1 block from each seeder at a time
                        if seeder.blocks_requested > 0 {
                            continue;
                        }

                        seeder.send_request(piece, block * BLOCK_SIZE, size);
                    }
                }
            }
            thread::sleep(Duration::from_secs(1));
        }
    }

    fn spawn_event_loop(&self, sender: mpsc::Sender<(SocketAddr, Vec<u8>)>) -> Sender<Actions> {
        println!("client: creating event loop");

        let address = "0.0.0.0:56789".parse::<SocketAddr>().unwrap();
        let server_socket = TcpListener::bind(&address).unwrap();

        let mut event_loop = EventLoop::new().unwrap();
        let event_loop_channel: Sender<Actions> = event_loop.channel();
        let mut handler = PeerHandler::new(server_socket, sender);

        event_loop.register(&handler.socket, SERVER_TOKEN, EventSet::readable(), PollOpt::edge()).unwrap();
        event_loop.timeout_ms(TIMER_TOKEN, 5000).unwrap();
        thread::spawn(move || {
            event_loop.run(&mut handler).unwrap();
        });
        event_loop_channel
    }

    fn spawn_tracker_update(&self, event_loop_channel: Sender<Actions>, tracker: Tracker) {
        thread::spawn(move || {
            loop {
                let peer_addresses = tracker.get_peers_addresses();
                if peer_addresses.is_empty() {
                    println!("torrent: no peers found!");
                } else {
                    for peer in &peer_addresses {
                        event_loop_channel.send(Actions::AddPeer(*peer)).unwrap();
                    }
                }
                thread::sleep(Duration::from_secs(30 * 60)); // 30 mins
            }
        });
    }
}
