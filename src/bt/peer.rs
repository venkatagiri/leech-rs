use std::io::prelude::*;
use std::net::SocketAddr;
use std::fmt;
use std::mem;
use std::collections::HashMap;

use rustc_serialize::hex::FromHex;
use mio::*;
use mio::tcp::*;

use bt::utils::*;

// BitTorrent message types
#[derive(Debug)]
pub enum MessageType {
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

// Notification types for the PeerHandler
pub enum NotifyTypes {
    Peer(Peer),
    Action(MessageType)
}

// Handler for the event loop
pub struct PeerHandler {
    pub socket: TcpListener,
    peers: HashMap<Token, Peer>,
    token_counter: usize,
    info_hash: Hash
}

pub const SERVER_TOKEN: Token = Token(0);
pub const TIMER_TOKEN: Token = Token(1);

impl PeerHandler {
    pub fn new(socket: TcpListener, info_hash: &Hash) -> PeerHandler {
        PeerHandler {
            socket: socket,
            peers: HashMap::new(),
            token_counter: 100,
            info_hash: info_hash.clone()
        }
    }
}

impl Handler for PeerHandler {
    type Timeout = Token;
    type Message = NotifyTypes;

    fn ready(&mut self, event_loop: &mut EventLoop<PeerHandler>, token: Token, events: EventSet) {
        match token {
            SERVER_TOKEN => {
                let peer_socket = match self.socket.accept() {
                    Ok(Some((sock, addr))) => sock,
                    Ok(None) => unreachable!(),
                    Err(e) => {
                        println!("peer_server: accept error {}", e);
                        return;
                    }
                };

                let new_token = Token(self.token_counter);
                self.peers.insert(new_token, Peer::new(peer_socket, self.info_hash.clone()));
                self.token_counter += 1;

                event_loop.register(&self.peers[&new_token].socket, new_token, EventSet::readable(), PollOpt::edge()).unwrap();
            }
            token => {
                let mut peer = self.peers.get_mut(&token).unwrap();
                peer.handle_read();
            }
        }
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Self>, msg: Self::Message) {
        //println!("peer_handler: peer address is {}", msg);
        println!("peer_handler: got a notification");
        match msg {
            NotifyTypes::Peer(peer) => {
                let new_token = Token(self.token_counter);
                self.peers.insert(new_token, peer);
                self.token_counter += 1;
                event_loop.register(&self.peers[&new_token].socket, new_token, EventSet::readable() | EventSet::writable(), PollOpt::edge()).unwrap();
            },
            NotifyTypes::Action(message_type) => {
                println!("peer_handler: message type is {:?}", message_type);
            }
        }
    }

    fn timeout(&mut self, event_loop: &mut EventLoop<Self>, timeout: Self::Timeout) {
        println!("peer_handler: timeout occured: no of peers: {}", self.peers.len());
        event_loop.timeout_ms(TIMER_TOKEN, 5000).unwrap();
    }
}

// Talks to the clients through BitTorrent Protocol
pub struct Peer {
     socket: TcpStream,
     info_hash: Hash,

     data: Vec<u8>,
     handshake_received: bool,
     handshake_sent: bool
}

impl Peer {
    pub fn new(socket: TcpStream, h: Hash) -> Peer {
        //println!("peer: connecting to {}", socket.peer_addr().unwrap());
        let mut peer = Peer{
          socket: socket,
          info_hash: h,
          data: vec![],
          handshake_received: false,
          handshake_sent: false
        };
        peer
    }

    pub fn from_addr(addr: &SocketAddr, h: Hash) -> Peer {
        let socket = TcpStream::connect(addr).unwrap();
        Self::new(socket, h)
    }

    pub fn handle_read(&mut self) {
        if !self.handshake_sent {
            self.send_handshake();
        }

        let mut buffer = [0; 1024];
        let bytes_length = match self.socket.try_read(&mut buffer) {
            Err(e) => {
                println!("Error while reading socket: {:?}", e); // FIXME: disconnect and remove peer
                return;
            },
            Ok(None) => return, // Socket buffer has got no more bytes.
            Ok(Some(len)) => {
                len
            }
        };
        let mut incoming_message = buffer[0..bytes_length].to_vec();
        self.data.append(&mut incoming_message.to_vec());
        println!("peer: bytes_length is {}", bytes_length);
        if bytes_length == 0 {
            println!("peer: returning");
            return;
        }

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

        self.socket.write(&data);
        self.handshake_sent = true;
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
        write!(f, "{}", self.socket.peer_addr().unwrap())
    }
}
