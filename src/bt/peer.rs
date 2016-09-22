use std::io::prelude::*;
use std::io;
use std::net::SocketAddr;
use std::fmt;
use std::mem;
use std::collections::HashMap;
use std::sync::mpsc;

use mio::*;
use mio::tcp::*;

use bt::utils::*;

// BitTorrent message types
#[derive(Debug)]
#[allow(dead_code)]
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

#[allow(unused_comparisons)]
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
pub enum Actions {
    AddPeer(SocketAddr),
    SendData(SocketAddr, Vec<u8>),
}

pub const SERVER_TOKEN: Token = Token(0);
pub const TIMER_TOKEN: Token = Token(1);

// Handler for the event loop
pub struct PeerHandler {
    pub socket: TcpListener,
    streams: HashMap<Token, TcpStream>,
    addr_to_token: HashMap<SocketAddr, Token>,
    token_counter: usize,
    data_channel: mpsc::Sender<(SocketAddr, Vec<u8>)>,
}

impl PeerHandler {
    pub fn new(socket: TcpListener, chn: mpsc::Sender<(SocketAddr, Vec<u8>)>) -> PeerHandler {
        PeerHandler {
            socket: socket,
            streams: HashMap::new(),
            addr_to_token: HashMap::new(),
            token_counter: 100,
            data_channel: chn,
        }
    }

    fn add_stream(&mut self, event_loop: &mut EventLoop<PeerHandler>, addr: SocketAddr, stream: TcpStream) {
        let new_token = Token(self.token_counter);
        self.token_counter += 1;
        println!("peer_handler: adding a new stream addr({}) token({})", addr, new_token.0);
        self.streams.insert(new_token, stream);
        self.addr_to_token.insert(addr, new_token);
        event_loop.register(&self.streams[&new_token], new_token, EventSet::readable() | EventSet::writable(), PollOpt::edge()).unwrap();
    }

    fn read(&mut self, token: &Token) -> io::Result<bool> {
        let mut buffer = [0; 2048];
        let socket = self.streams.get_mut(&token).unwrap();
        let bytes_length = try!(socket.read(&mut buffer));
        println!("peer_handler: got {} bytes of data for token {}", bytes_length, token.0);
        let data = buffer[0..bytes_length].to_vec();
        let addr = try!(socket.peer_addr());
        self.data_channel.send((addr, data)).unwrap();
        Ok(true)
    }

    fn disconnect(&mut self, event_loop: &mut EventLoop<PeerHandler>, token: &Token) {
        event_loop.deregister(&self.streams[token]).unwrap();
        self.streams.remove(token);  // FIXME: send disconnect back to torrent
    }
}

impl Handler for PeerHandler {
    type Timeout = Token;
    type Message = Actions;

    fn ready(&mut self, event_loop: &mut EventLoop<PeerHandler>, token: Token, events: EventSet) {
        if events.is_readable() {
            println!("peer_handler: socket is readable for token {}", token.0);
            match token {
                SERVER_TOKEN => {
                    println!("peer_handler: accepting a new connection");
                    let (peer_socket, addr) = match self.socket.accept() {
                        Ok(Some((sock, addr))) => (sock, addr),
                        Ok(None) => unreachable!(),
                        Err(e) => {
                            println!("peer_server: accept error {}", e);
                            return;
                        }
                    };
                    self.add_stream(event_loop, addr, peer_socket);
                }
                token => {
                    println!("peer_handler: received data for token {}", token.0);
                    match self.read(&token) {
                        Ok(_) => {},
                        Err(err) => {
                            println!("peer_handler: error while reading token({}): {}", token.0, err);
                            self.disconnect(event_loop, &token);
                            return;
                        },
                    }
                }
            }
        }

        if events.is_writable() {
            println!("peer_handler: socket is writable for token {}", token.0);
            let socket = self.streams.get_mut(&token).unwrap();
            let addr = socket.peer_addr().unwrap();
            self.data_channel.send((addr, vec![])).unwrap();
        }
    }

    fn notify(&mut self, event_loop: &mut EventLoop<Self>, msg: Self::Message) {
        match msg {
            Actions::AddPeer(addr) => {
                let socket = TcpStream::connect(&addr).unwrap();
                self.add_stream(event_loop, addr.clone(), socket);
            },
            Actions::SendData(addr, data) => {
                println!("peer_handler: actions: got data for addr {}", addr);
                let token = self.addr_to_token.get(&addr).unwrap();
                let socket = self.streams.get_mut(&token).unwrap();
                let len = socket.write(&data).unwrap();
                println!("peer_handler: wrote {} bytes to {}", len , addr);
            },
        }
    }
}

// Talks to the clients through BitTorrent Protocol
#[derive(Clone)]
pub struct Peer {
    addr: SocketAddr,
    info_hash: Hash,
    channel: Sender<Actions>,
    tpieces: mpsc::Sender<(usize, usize, Vec<u8>)>,

    data: Vec<u8>,
    pub is_handshake_received: bool,
    pub is_handshake_sent: bool,
    pub is_interested_sent: bool,
    pub is_choke_received: bool,
    pub blocks_requested: usize,
}

impl Peer {
    pub fn new(addr: SocketAddr, h: Hash, chn: Sender<Actions>, t: mpsc::Sender<(usize, usize, Vec<u8>)>) -> Peer {
        let mut p = Peer {
            addr: addr,
            info_hash: h,
            channel: chn,
            tpieces: t,
            data: vec![],
            is_handshake_received: false,
            is_handshake_sent: false,
            is_interested_sent: false,
            is_choke_received: true,
            blocks_requested: 0,
        };
        p.send_handshake();
        p
    }

    pub fn read(&mut self, data: Vec<u8>) {
        self.data.extend_from_slice(&data);
    }

    fn write(&self, data: Vec<u8>) {
        self.channel.send(Actions::SendData(self.addr, data)).unwrap();
    }

    pub fn process_data(&mut self) {
        //println!("peer: data(size={}) - {:?}", self.data.len(), self.data);

        loop {
            let message_length: usize = self.get_message_length(&self.data) as usize;
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
            MessageType::KeepAlive => self.recv_keepalive(message),
            MessageType::Choke => self.recv_choke(message),
            MessageType::UnChoke => self.recv_unchoke(message),
            MessageType::Interested => println!("peer: recv interested"),
            MessageType::UnInterested => println!("peer: recv uninterested"),
            MessageType::Have => println!("peer: recv have"),
            MessageType::Bitfield => println!("peer: recv bitfield"),
            MessageType::Request => println!("peer: recv request"),
            MessageType::Piece => self.recv_piece(message),
            MessageType::Cancel => println!("peer: recv cancel"),
            MessageType::Port => println!("peer: recv port"),
            MessageType::Unknown => println!("peer: unknown message"),
        }
    }

    fn get_message_length(&self, data: &Vec<u8>) -> u32 {
        if !self.is_handshake_received {
            68
        } else if data.len() < 4 {
            u32::max_value()
        } else {
            unsafe { mem::transmute::<[u8; 4], u32>([data[3], data[2], data[1], data[0]]) + 4 }
        }
    }

    fn get_message_type(&self, message: &Vec<u8>) -> MessageType {
        if !self.is_handshake_received {
            MessageType::Handshake
        } else if message.len() == 4 {
            MessageType::KeepAlive
        } else {
            MessageType::from_u8(message[4])
        }
    }

    fn send_handshake(&mut self) {
        println!("peer: send_handshake to {}", self);
        let mut data: Vec<u8> = vec![];
        data.push(19);
        data.extend_from_slice(b"BitTorrent protocol");
        data.extend_from_slice(&[0; 8]);
        data.extend_from_slice(&self.info_hash.0);
        data.extend_from_slice(&MY_PEER_ID.0);

        self.write(data);
        self.is_handshake_sent = true;
    }

    pub fn send_interested(&mut self) {
        if self.is_interested_sent {
            return;
        }
        println!("peer: send_interested to {}", self);
        let data: Vec<u8> = vec![0, 0, 0, 1, 2];

        self.write(data);
        self.is_interested_sent = true;
    }

    pub fn send_request(&mut self, piece: usize, begin: usize, length: usize) {
        println!("peer: send_request to {}", self);

        let index = u32_to_byte_slice(piece as u32);
        let begin = u32_to_byte_slice(begin as u32);
        let length = u32_to_byte_slice(length as u32);

        let mut data: Vec<u8> = vec![0, 0, 0, 13, 6];
        data.extend_from_slice(&index);
        data.extend_from_slice(&begin);
        data.extend_from_slice(&length);

        self.write(data);
        self.blocks_requested += 1;
    }

    fn recv_keepalive(&mut self, message: &Vec<u8>) {
        println!("peer: recv_keepalive from {}", self);
        if message.len() != 4 {
            println!("peer: invalid keepalive");
            return;
        }
        // FIXME: track last ping time
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
        self.is_handshake_received = true;
    }

    fn recv_choke(&mut self, message: &Vec<u8>) {
        println!("peer: recv_choke from {}", self);
        if message.len() != 5 { // FIXME: check for data in the bytes as well
            println!("peer: invalid choke");
            return;
        }
        self.is_choke_received = true;
    }

    fn recv_unchoke(&mut self, message: &Vec<u8>) {
        println!("peer: recv_unchoke from {}", self);
        if message.len() != 5 { // FIXME: check for data in the bytes as well
            println!("peer: invalid unchoke");
            return;
        }
        self.is_choke_received = false;
    }

    pub fn recv_piece(&mut self, message: &Vec<u8>) {
        println!("peer: recv_piece from {}", self);

        let index = byte_slice_to_u32(&message[5..9]) as usize;
        let begin = byte_slice_to_u32(&message[9..13]) as usize;
        let block = message[13..].to_vec();
        self.tpieces.send((index, begin, block)).unwrap();
    }

}

impl fmt::Display for Peer {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.addr)
    }
}
