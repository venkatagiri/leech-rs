use std::io::{Read, Result};
use std::fmt;
use std::string::String;
use std::str::FromStr;
use std::net::{
    IpAddr,
    SocketAddr,
    ToSocketAddrs,
    UdpSocket,
};
use std::mem;

use hyper::client::Client;
use bt::bencoding::*;
use bt::utils::*;

fn parse_peers(peers: &[u8]) -> Vec<SocketAddr> {
    peers.chunks(6).filter_map(|peer| {
        let addr = format!("{}.{}.{}.{}", peer[0], peer[1], peer[2], peer[3]);
        let port = unsafe { mem::transmute::<[u8; 2], u16>([peer[5], peer[4]]) };
        let ip = IpAddr::from_str(&addr).unwrap();
        if port != 56789 { //FIXME: check both your ip and port
            Some(SocketAddr::new(ip, port))
        } else {
            None
        }
    }).collect()
}

struct UDPTrackerProtocol {
    addr: SocketAddr,
    info_hash: Hash,
}

impl UDPTrackerProtocol {
    fn new(url: &String, info_hash: Hash) -> UDPTrackerProtocol {
        UDPTrackerProtocol {
            addr: Self::get_addr_from_url(url),
            info_hash: info_hash,
        }
    }

    fn get_addr_from_url(url: &String) -> SocketAddr {
        let parts: Vec<_> = url.split("/").collect();
        let addr = parts[2];
        let addrs: Vec<_> = addr.to_socket_addrs().expect("domain resolution failed!").collect();
        addrs.first().expect("domain resolution failed").clone()
    }

    fn get_peers_addresses(&self) -> Vec<SocketAddr> {
        match self.request_addresses() {
            Ok(res) => {
                println!("udptracker: got {} peers back", res.len());
                res
            },
            Err(err) => {
                println!("udptracker: udp request failed! {}", err);
                vec![]
            },
        }
    }

    fn request_addresses(&self) -> Result<Vec<SocketAddr>> {
        let mut socket = UdpSocket::bind("0.0.0.0:56788").expect("udp bind failed");
        let _ = try!(self.send_connect(&mut socket));
        let connection_id = try!(self.recv_connect(&mut socket));
        let _ = try!(self.send_announce(&mut socket, connection_id));
        let peers = try!(self.recv_announce(&mut socket));
        Ok(peers)
    }

    fn send_connect(&self, socket: &mut UdpSocket) -> Result<usize> {
        let connection_id = u64_to_byte_slice(0x41727101980);
        let action = u32_to_byte_slice(0);
        let transaction_id = u32_to_byte_slice(0x1337);

        let mut buffer = vec![];
        buffer.extend_from_slice(&connection_id);
        buffer.extend_from_slice(&action);
        buffer.extend_from_slice(&transaction_id);

        socket.send_to(&buffer, self.addr)
    }

    fn recv_connect(&self, socket: &mut UdpSocket) -> Result<u64> {
        let mut buffer = [0; 32];
        let _ = try!(socket.recv(&mut buffer));
        let _ = byte_slice_to_u32(&buffer[4..8]); // FIXME: validate len, transaction_id, action
        let connection_id = byte_slice_to_u64(&buffer[8..16]);
        Ok(connection_id)
    }

    fn send_announce(&self, socket: &mut UdpSocket, connection_id: u64) -> Result<usize> {
        let connection_id = u64_to_byte_slice(connection_id);
        let action = u32_to_byte_slice(1);
        let transaction_id = u32_to_byte_slice(0x1337);
        let event = u32_to_byte_slice(2);

        let mut buffer = vec![];
        buffer.extend_from_slice(&connection_id);
        buffer.extend_from_slice(&action);
        buffer.extend_from_slice(&transaction_id);
        buffer.extend_from_slice(&self.info_hash.0);
        buffer.extend_from_slice(&MY_PEER_ID.0);
        buffer.extend_from_slice(&[0; 8]); // downloaded
        buffer.extend_from_slice(&[0; 8]); // left
        buffer.extend_from_slice(&[0; 8]); // uploaded
        buffer.extend_from_slice(&event);
        buffer.extend_from_slice(&[0; 8]); // ip address
        buffer.extend_from_slice(&[0; 4]); // key
        buffer.extend_from_slice(&[0; 4]); // num want
        buffer.extend_from_slice(&[0; 4]); // port

        socket.send_to(&buffer, self.addr)
    }

    fn recv_announce(&self, socket: &mut UdpSocket) -> Result<Vec<SocketAddr>> {
        let mut buffer = vec![0; 1024];
        let len = try!(socket.recv(&mut buffer));
        Ok(parse_peers(&buffer[20..len]))
    }
}

#[derive(Default, Clone)]
pub struct Tracker {
    url: String,
    info_hash: Hash,
}

impl Tracker {
    pub fn new(url: String, info_hash: Hash) -> Tracker {
        Tracker {
            url: url,
            info_hash: info_hash,
        }
    }

    pub fn get_peers_addresses(&self) -> Vec<SocketAddr> {
        if self.url.starts_with("udp") {
            return self.request_udp_peers();
        }
        let client = Client::new();
        let url = format!("{tracker}?info_hash={hash}&peer_id={peer_id}&port=56789&uploaded=0&downloaded=0&left=0&event=started&compact=1",
                    tracker = self.url,
                    hash = self.info_hash.url_encoded(),
                    peer_id = MY_PEER_ID.url_encoded());
        let mut buf = vec![];
        let mut response = client.get(&url).send().unwrap();
        response.read_to_end(&mut buf).unwrap();

        let root = BEncoding::decode(buf).unwrap();
        let peers = root.get_bytes("peers").unwrap();
        if peers.len() <= 6 {
            self.get_default_peers()
        } else {
            parse_peers(&peers)
        }
    }

    fn request_udp_peers(&self) -> Vec<SocketAddr> {
        UDPTrackerProtocol::new(&self.url, self.info_hash.clone()).get_peers_addresses()
    }

    fn get_default_peers(&self) -> Vec<SocketAddr> {
        let addr = "209.141.59.32";
        let port = 51863;
        let ip = IpAddr::from_str(&addr).unwrap();
        vec![SocketAddr::new(ip, port)]
    }
}

impl fmt::Display for Tracker {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.url)
    }
}
