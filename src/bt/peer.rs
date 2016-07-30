use std::io::prelude::*;
use std::net::{SocketAddrV4, TcpStream};
use std::fmt;

pub struct Peer {
    stream: TcpStream
}

impl Peer {
    pub fn new(addr: SocketAddrV4) -> Peer {
        println!("peer: connecting to {}", addr);
        let s = TcpStream::connect(&addr).unwrap();
        Peer{stream: s}
    }
}

impl fmt::Display for Peer {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.stream.peer_addr().unwrap())
    }
}