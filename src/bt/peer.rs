use std::io::prelude::*;
use std::net::{SocketAddrV4, TcpStream};
use std::fmt;
use rustc_serialize::hex::FromHex;
use bt::utils::*;

pub struct Peer {
     stream: TcpStream,
     info_hash: Hash,
     handshake_received: bool
}

impl Peer {
    pub fn new(addr: SocketAddrV4, h: Hash) -> Peer {
        println!("peer: connecting to {}", addr);
        let s = TcpStream::connect(&addr).unwrap();
        let mut peer = Peer{stream: s, info_hash: h, handshake_received: false};
        peer.send_handshake();
        peer.recv_handshake();
        peer
    }

    pub fn send_handshake(&mut self) {
        println!("peer: send_handshake to {}", self);
        let mut data: Vec<u8> = vec![];
        // <pstrlen><pstr><reserved><info_hash><peer_id>
        data.push(19);
        data.append(&mut b"BitTorrent protocol".to_vec());
        data.append(&mut [0; 8].to_vec());
        data.append(&mut self.info_hash.0.to_vec());
        data.append(&mut MY_PEER_ID.0.to_vec());

        self.stream.write(&data);
    }

    pub fn recv_handshake(&mut self) {
        println!("peer: recv_handshake from {}", self);
        let mut data = [0; 128];
        let read_length = self.stream.read(&mut data).unwrap();
        if read_length != 68 {
            panic!("peer: handshake should always be of length 68"); //TODO: disconnect instead of panic
        }
        let mut bhash = [0; 20];
        bhash.copy_from_slice(&data[28..48]);
        let hash = Hash(bhash); //TODO: simpler init
        if hash != self.info_hash {
            panic!("peer: handshake received is for wrong info hash"); //TODO: disconnect instead of panic
        }
        self.handshake_received = true;
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.stream.peer_addr().unwrap())
    }
}
