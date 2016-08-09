use std::io::prelude::*;
use std::net::{SocketAddrV4, TcpStream};
use std::fmt;
use std::mem;

use rustc_serialize::hex::FromHex;

use bt::utils::*;

enum MessageType {
    Unknown = -3,
    Handshake = -2,
    KeepAlive = -1,
    Choke = 0,
    UnChoke = 1,
    Interested = 2,
    UnInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Port = 9,
}

impl MessageType {
    fn from_u8(id: u8) -> MessageType {
        if 0 <= id && id <= 9 {
            unsafe { mem::transmute(id) }
        } else {
            MessageType::Unknown
        }
    }
}

pub struct Peer {
     stream: TcpStream,
     info_hash: Hash,

     data: Vec<u8>,
     handshake_received: bool
}

impl Peer {
    pub fn new(addr: SocketAddrV4, h: Hash) -> Peer {
        println!("peer: connecting to {}", addr);
        let s = TcpStream::connect(&addr).unwrap();
        let mut peer = Peer{
          stream: s,
          info_hash: h,
          data: vec![],
          handshake_received: false
        };
        peer.send_handshake();
        peer
    }

    pub fn handle_read(&mut self) {
        let mut buffer = [0; 1024];
        let bytes_length = self.stream.read(&mut buffer).unwrap(); //FIXME: handle network errors
        let mut incoming_message = buffer[0..bytes_length].to_vec();
        self.data.append(&mut incoming_message.to_vec());

        loop {
            let message_length: usize = self.get_message_length(&self.data) as usize;
            println!("peer: data size is {}", self.data.len());
            println!("peer: data is {:?}", self.data);
            println!("peer: message_length is {:?}", message_length);
            if self.data.len() < message_length {
                break;
            }
            let message: Vec<u8> = self.data.drain(0..message_length).collect();
            self.handle_message(&message);
        }
    }

    fn handle_message(&mut self, message: &Vec<u8>) {
        match self.get_message_type(message) {
            MessageType::Handshake => self.recv_handshake(message), //FIXME: implement other types
            MessageType::KeepAlive => println!("peer: recv keepalive"),
            MessageType::Choke => println!("peer: recv choke"),
            MessageType::UnChoke => println!("peer: recv unchoke"),
            MessageType::Interested => println!("peer: recv interested"),
            MessageType::UnInterested => println!("peer: recv uninterested"),
            MessageType::Have => println!("peer: recv have"),
            MessageType::Bitfield => println!("peer: recv bitfield"),
            MessageType::Request => println!("peer: recv request"),
            MessageType::Piece => println!("peer: recv piece"),
            MessageType::Cancel => println!("peer: recv cancel"),
            MessageType::Port => println!("peer: recv port"),
            MessageType::Unknown => println!("peer: unknown message"),
        }
    }

    fn get_message_length(&self, data: &Vec<u8>) -> u32 {
        if !self.handshake_received {
            68
        } else if data.len() < 4 {
            u32::max_value()
        } else {
            unsafe { mem::transmute::<[u8; 4], u32>([data[3], data[2], data[1], data[0]]) + 4 }
        }
    }

    fn get_message_type(&self, message: &Vec<u8>) -> MessageType {
        if !self.handshake_received {
            MessageType::Handshake
        } else if message.len() == 4 { // FIXME: Check if message has only 0s
            MessageType::KeepAlive
        } else {
            MessageType::from_u8(message[4])
        }
    }

    fn send_handshake(&mut self) {
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

    fn recv_handshake(&mut self, message: &Vec<u8>) {
        println!("peer: recv_handshake from {}", self);
        if message.len() != 68 {
            panic!("peer: handshake should always be of length 68"); //FIXME: disconnect instead of panic
        }
        let mut bhash = [0; 20];
        bhash.copy_from_slice(&message[28..48]);
        let hash = Hash(bhash); //FIXME: simpler init
        if hash != self.info_hash {
            panic!("peer: handshake received is for wrong info hash"); //FIXME: disconnect instead of panic
        }
        self.handshake_received = true;
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.stream.peer_addr().unwrap())
    }
}
