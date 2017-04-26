use std::io::{self, Read, Write};
use std::net::{SocketAddr, TcpListener, TcpStream};
use std::fmt;
use std::mem;
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{Sender, Receiver};
use std::time::{Duration, Instant};
use std::thread;

use utils::*;
use torrent::*;

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
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
    Port = 9,
}

#[allow(unused_comparisons)]
impl From<u8> for MessageType {
    fn from(id: u8) -> Self {
        if 0 <= id && id <= 9 {
            unsafe { mem::transmute(id) }
        } else {
            MessageType::Unknown
        }
    }
}

// Notification types for the Handler
pub enum Message {
    AddPeer(SocketAddr),
    Data(SocketAddr, Vec<u8>),
    Disconnect(SocketAddr),
}

// Peer Connection
struct Connection {
    addr: SocketAddr,
    socket: TcpStream,
    send_queue: VecDeque<Vec<u8>>,
}

impl Connection {
    fn new(addr: SocketAddr, socket: TcpStream) -> Connection {
        Connection {
            addr: addr,
            socket: socket,
            send_queue: VecDeque::new(),
        }
    }

    fn send_data(&mut self, data: Vec<u8>) {
        self.send_queue.push_front(data);
    }

    fn writable(&mut self) -> io::Result<()> {
        while let Some(data) = self.send_queue.pop_back() {
            match self.socket.write(&data) {
                Ok(len) => {
                    println!("connection: wrote {} bytes", len);
                }
                Err(ref err) if io::ErrorKind::WouldBlock == err.kind() => {
                    self.send_queue.push_back(data);
                    return Ok(());
                }
                Err(err) => {
                    println!("connection: failed to write!");
                    return Err(err);
                }
            }
        }
        Ok(())
    }

    fn readable(&mut self) -> io::Result<(Vec<u8>)> {
        let mut data = vec![];
        let mut buffer = [0; 2048];
        self.socket.set_nonblocking(true)?;
        loop {
            match self.socket.read(&mut buffer) {
                Ok(len) if len == 0 => {
                    break;
                }
                Ok(len) => {
                    println!("connection: read {} bytes for {:?}", len, self.addr);
                    data.extend_from_slice(&buffer[0..len]);
                }
                Err(ref err) if io::ErrorKind::WouldBlock == err.kind() => {
                    break;
                }
                Err(err) => {
                    println!("connection: failed to write!");
                    return Err(err);
                }
            }
        }
        Ok(data)
    }
}

// Handler for the event loop
pub struct Handler {
    pub socket: TcpListener,
    conns: HashMap<SocketAddr, Connection>,
    data_channel: Sender<Message>,
    notifications: Receiver<Message>,
    disconnects: Vec<SocketAddr>,
}


impl Handler {
    pub fn new(socket: TcpListener, chn: Sender<Message>, notifications: Receiver<Message>) -> Handler {
        Handler {
            socket: socket,
            conns: HashMap::new(),
            data_channel: chn,
            notifications: notifications,
            disconnects: vec![],
        }
    }

    fn add_conn(&mut self, addr: SocketAddr, stream: TcpStream) {
        self.conns.insert(addr, Connection::new(addr, stream));
        self.data_channel.send(Message::AddPeer(addr)).unwrap();
        println!("handler: new peer created with {:?}", addr);
    }

    fn disconnect(&mut self, addr: &SocketAddr) {
        self.conns.remove(addr);
        self.data_channel.send(Message::Disconnect(*addr)).unwrap();
    }

    pub fn run(&mut self) -> io::Result<()> {
        self.socket.set_nonblocking(true)?;
        loop {
            // process messages from client
            while let Ok(msg) = self.notifications.try_recv() {
                self.notify(msg);
            }

            // accept new connections
            while let Ok((sock, addr)) = self.socket.accept() {
                sock.set_nonblocking(true)?;
                self.add_conn(addr, sock);
            }

            // r/w for connections
            self.process_rw();

            // cleanup connections
            while let Some(ref addr) = self.disconnects.pop() {
                self.disconnect(addr);
            }

            thread::sleep(Duration::from_millis(1));
        }
    }

    fn process_rw(&mut self) {
        for (addr, conn) in self.conns.iter_mut() {
            match conn.readable() {
                Ok(data) => {
                    if !data.is_empty() {
                        self.data_channel.send(Message::Data(*addr, data)).unwrap();
                    }
                },
                Err(err) => {
                    println!("handler: error while reading {:?} {}", addr, err);
                    self.disconnects.push(*addr);
                    continue;
                },
            }
            match conn.writable() {
                Ok(_) => {},
                Err(err) => {
                    println!("handler: error while writing {:?} {}", addr, err);
                    self.disconnects.push(*addr);
                    continue;
                },
            }
        }
    }

    fn notify(&mut self, msg: Message) {
        match msg {
            Message::AddPeer(addr) => {
                let socket = TcpStream::connect(&addr).unwrap();
                self.add_conn(addr, socket);
            },
            Message::Data(addr, data) => {
                self.conns.get_mut(&addr).expect("missing connection").send_data(data);
            },
            Message::Disconnect(addr) => {
                self.disconnect(&addr);
            },
        }
    }
}

// Talks to the clients through BitTorrent Protocol
#[derive(Clone)]
pub struct Peer {
    addr: SocketAddr,
    info_hash: Hash,
    channel: Sender<Message>,
    tpieces: Sender<(usize, usize, Vec<u8>)>,

    data: Vec<u8>,
    last_active: Instant,
    last_keepalive: Instant,
    pub is_handshake_received: bool,
    pub is_handshake_sent: bool,
    pub is_interested_sent: bool,
    pub is_choke_received: bool,
    pub blocks_requested: usize,
    pub is_piece_downloaded: Vec<bool>,
    pub is_block_requested: Vec<Vec<bool>>,
    bitfield: Vec<bool>,
}

impl Peer {
    pub fn new(addr: SocketAddr, torrent: &Torrent, chn: Sender<Message>, t: Sender<(usize, usize, Vec<u8>)>) -> Peer {
        let mut p = Peer {
            addr: addr,
            info_hash: torrent.info_hash.clone(),
            channel: chn,
            tpieces: t,
            data: vec![],
            last_active: Instant::now(),
            last_keepalive: Instant::now(),
            is_handshake_received: false,
            is_handshake_sent: false,
            is_interested_sent: false,
            is_choke_received: true,
            blocks_requested: 0,
            is_piece_downloaded: vec![false; torrent.no_of_pieces],
            is_block_requested: {
                (0..torrent.no_of_pieces).map(|piece| { vec![false; torrent.get_block_count(piece)] }).collect()
            },
            bitfield: torrent.is_piece_downloaded.clone(),
        };
        p.send_handshake();
        p
    }

    pub fn read(&mut self, data: Vec<u8>) {
        self.data.extend_from_slice(&data);
    }

    fn write(&self, data: Vec<u8>) {
        self.channel.send(Message::Data(self.addr, data)).unwrap();
    }

    fn disconnect(&self) {
        self.channel.send(Message::Disconnect(self.addr)).unwrap();
    }

    pub fn process_data(&mut self) {
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
        self.last_active = Instant::now();

        match self.get_message_type(message) {
            MessageType::Handshake => self.recv_handshake(message), //FIXME: implement other types
            MessageType::KeepAlive => self.recv_keepalive(message),
            MessageType::Choke => self.recv_choke(message),
            MessageType::UnChoke => self.recv_unchoke(message),
            MessageType::Interested => println!("peer: recv interested"),
            MessageType::NotInterested => println!("peer: recv not interested"),
            MessageType::Have => self.recv_have(message),
            MessageType::Bitfield => self.recv_bitfield(message),
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
            MessageType::from(message[4])
        }
    }

    pub fn no_of_blocks_requested(&self) -> u32 {
        self.is_block_requested
            .iter()
            .fold(0, |sum, ref x| {
                sum + x.iter().filter(|&b| *b ).collect::<Vec<_>>().len() as u32
            })
    }

    pub fn is_timed_out(&self) -> bool {
        self.last_active.elapsed().as_secs() > 30
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

    pub fn send_keepalive(&mut self) {
        if self.last_keepalive.elapsed().as_secs() < 30 {
            return;
        }
        println!("peer: send_keepalive to {}", self);

        let data: Vec<u8> = vec![0; 4];
        self.write(data);
        self.last_keepalive = Instant::now();
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

    pub fn send_not_interested(&mut self) {
        if !self.is_interested_sent {
            return;
        }
        println!("peer: send_not_interested to {}", self);
        let data: Vec<u8> = vec![0, 0, 0, 1, 3];

        self.write(data);
        self.is_interested_sent = false;
    }

    pub fn send_have(&mut self, piece: usize) {
        println!("peer: send_have to {}", self);
        let piece = u32_to_byte_slice(piece as u32);

        let mut data: Vec<u8> = vec![0, 0, 0, 5, 4];
        data.extend_from_slice(&piece);

        self.write(data);
    }

    pub fn send_bitfield(&mut self) {
        println!("peer: send_bitfield to {}", self);

        let bits: Vec<u8> = self.bitfield.iter().map(|&b| { if b { 1 } else { 0 } }).collect();
        let bitfield = from_bits(&bits);
        let length = bitfield.len() as u32 + 1;

        let mut data: Vec<u8> = vec![];
        data.extend_from_slice(&u32_to_byte_slice(length));
        data.push(5);
        data.extend_from_slice(&bitfield);

        self.write(data);
    }

    pub fn send_request(&mut self, index: usize, begin: usize, length: usize) {
        println!("peer: send_request to {}", self);

        let piece = index;
        let block = begin / BLOCK_SIZE;
        let index = u32_to_byte_slice(index as u32);
        let begin = u32_to_byte_slice(begin as u32);
        let length = u32_to_byte_slice(length as u32);

        let mut data: Vec<u8> = vec![0, 0, 0, 13, 6];
        data.extend_from_slice(&index);
        data.extend_from_slice(&begin);
        data.extend_from_slice(&length);

        self.write(data);
        self.is_block_requested[piece][block] = true;
    }

    fn recv_keepalive(&mut self, message: &Vec<u8>) {
        println!("peer: recv_keepalive from {}", self);
        if message.len() != 4 {
            println!("peer: invalid keepalive");
            return;
        }
    }

    fn recv_handshake(&mut self, message: &Vec<u8>) {
        println!("peer: recv_handshake from {}", self);
        if message.len() != 68 {
            println!("peer: invalid handshake");
            self.disconnect();
            return;
        }
        let hash = Hash::from_slice(&message[28..48]);
        if hash != self.info_hash {
            println!("peer: invalid info hash in handshake. expected({}) received({})", self.info_hash, hash);
            self.disconnect();
            return;
        }
        self.is_handshake_received = true;
        self.send_bitfield();
    }

    fn recv_choke(&mut self, message: &Vec<u8>) {
        println!("peer: recv_choke from {}", self);
        if message.len() != 5 { // FIXME: check for data in the bytes as well
            println!("peer: invalid choke");
            return;
        }
        self.is_choke_received = true;
    }

    fn recv_bitfield(&mut self, message: &Vec<u8>) {
        println!("peer: recv_bitfield from {}", self);
        let no_of_pieces = self.is_piece_downloaded.len();
        let expected_length: usize = (no_of_pieces/8) + 1;
        if message.len() != expected_length + 5 {
            println!("peer: invalid bitfield length");
            return;
        }
        let bits = to_bits(&message[5..]);
        for piece in 0..no_of_pieces {
            self.is_piece_downloaded[piece] = if bits[piece] == 1 { true } else { false };
        }
    }

    fn recv_unchoke(&mut self, message: &Vec<u8>) {
        println!("peer: recv_unchoke from {}", self);
        if message.len() != 5 { // FIXME: check for data in the bytes as well
            println!("peer: invalid unchoke");
            return;
        }
        self.is_choke_received = false;
    }

    fn recv_have(&mut self, message: &Vec<u8>) {
        println!("peer: recv_have from {}", self);
        if message.len() != 9 { // FIXME: check for data in the bytes as well
            println!("peer: invalid have");
            return;
        }
        let piece = byte_slice_to_u32(&message[5..9]) as usize;
        self.is_piece_downloaded[piece] = true;
    }

    fn recv_piece(&mut self, message: &Vec<u8>) {
        println!("peer: recv_piece from {}", self);

        let index = byte_slice_to_u32(&message[5..9]) as usize;
        let begin = byte_slice_to_u32(&message[9..13]) as usize;
        let block = message[13..].to_vec();
        self.tpieces.send((index, begin / BLOCK_SIZE, block)).unwrap();

        let piece = index;
        let block = begin / BLOCK_SIZE;
        self.is_block_requested[piece][block] = false;
    }

}

impl fmt::Display for Peer {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.addr)
    }
}
