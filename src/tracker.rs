use std::io::Read;
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
use std::time::Duration;
use hyper::client::Client;

use bencoding::*;
use utils::*;
use error::Result;

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

/// HTTP Tracker
struct HTTPTracker {}

impl HTTPTracker {
    fn get_peers_addresses(url: &String, info_hash: &Hash) -> Result<Vec<SocketAddr>> {
        let client = Client::new();
        let url = format!("{tracker}?info_hash={hash}&peer_id={peer_id}&port=56789&uploaded=0&downloaded=0&left=0&event=started&compact=1",
                    tracker = url,
                    hash = info_hash.url_encoded(),
                    peer_id = MY_PEER_ID.url_encoded());
        let mut buf = vec![];
        let mut response = try!(client.get(&url).send());
        try!(response.read_to_end(&mut buf));

        let root = try!(BEncoding::decode(buf).ok_or(Error::DecodeError));
        let peers = try!(root.get_bytes("peers"));
        Ok(parse_peers(&peers))
    }
}

/// UDP Tracker
struct UDPTracker {}

impl UDPTracker {
    fn get_addr_from_url(url: &String) -> Result<SocketAddr> {
        let parts: Vec<_> = url.split("/").collect();
        let addr = parts[2];
        let addrs: Vec<_> = try!(addr.to_socket_addrs()).collect();
        Ok(addrs[0])
    }

    fn get_peers_addresses(url: &String, info_hash: &Hash) -> Result<Vec<SocketAddr>> {
        let addr = try!(Self::get_addr_from_url(url));
        let mut socket = try!(UdpSocket::bind("0.0.0.0:55555"));
        try!(socket.connect(addr));

        try!(socket.set_read_timeout(Some(Duration::from_secs(1))));
        try!(socket.set_write_timeout(Some(Duration::from_secs(1))));

        let _ = try!(Self::send_connect(&mut socket));
        let connection_id = try!(Self::recv_connect(&mut socket));
        let _ = try!(Self::send_announce(&mut socket, connection_id, info_hash));
        let peers = try!(Self::recv_announce(&mut socket));
        Ok(peers)
    }

    fn send_connect(socket: &mut UdpSocket) -> Result<usize> {
        let connection_id = u64_to_byte_slice(0x41727101980);
        let action = u32_to_byte_slice(0);
        let transaction_id = u32_to_byte_slice(0x1337);

        let mut buffer = vec![];
        buffer.extend_from_slice(&connection_id);
        buffer.extend_from_slice(&action);
        buffer.extend_from_slice(&transaction_id);

        let len = try!(socket.send(&buffer));
        Ok(len)
    }

    fn recv_connect(socket: &mut UdpSocket) -> Result<u64> {
        let mut buffer = [0; 1024];
        let _ = try!(socket.recv(&mut buffer));
        let _ = byte_slice_to_u32(&buffer[4..8]); // FIXME: validate len, transaction_id, action
        let connection_id = byte_slice_to_u64(&buffer[8..16]);
        Ok(connection_id)
    }

    fn send_announce(socket: &mut UdpSocket, connection_id: u64, info_hash: &Hash) -> Result<usize> {
        let connection_id = u64_to_byte_slice(connection_id);
        let action = u32_to_byte_slice(1);
        let transaction_id = u32_to_byte_slice(0x1337);
        let event = u32_to_byte_slice(2);

        let mut buffer = vec![];
        buffer.extend_from_slice(&connection_id);
        buffer.extend_from_slice(&action);
        buffer.extend_from_slice(&transaction_id);
        buffer.extend_from_slice(&info_hash.0);
        buffer.extend_from_slice(&MY_PEER_ID.0);
        buffer.extend_from_slice(&[0; 8]); // downloaded
        buffer.extend_from_slice(&[0; 8]); // left
        buffer.extend_from_slice(&[0; 8]); // uploaded
        buffer.extend_from_slice(&event);
        buffer.extend_from_slice(&[0; 8]); // ip address
        buffer.extend_from_slice(&[0; 4]); // key
        buffer.extend_from_slice(&[0; 4]); // num want
        buffer.extend_from_slice(&[0; 4]); // port

        let len = try!(socket.send(&buffer));
        Ok(len)
    }

    fn recv_announce(socket: &mut UdpSocket) -> Result<Vec<SocketAddr>> {
        let mut buffer = vec![0; 1024];
        let len = try!(socket.recv(&mut buffer));
        Ok(parse_peers(&buffer[20..len]))
    }
}

#[derive(Default, Clone)]
pub struct Tracker {
    urls: Vec<String>,
    info_hash: Hash,
}

impl Tracker {
    pub fn new(urls: Vec<String>, info_hash: Hash) -> Tracker {
        Tracker {
            urls: urls,
            info_hash: info_hash,
        }
    }

    pub fn get_peers_addresses(&self) -> Vec<SocketAddr> {
        for url in &self.urls {
            let result = if url.starts_with("udp") {
                UDPTracker::get_peers_addresses(url, &self.info_hash)
            } else {
                HTTPTracker::get_peers_addresses(url, &self.info_hash)
            };
            match result {
                Ok(ref list) if list.len() > 0 => {
                    return list.clone();
                },
                Ok(_) => {
                    println!("tracker: no peers found from url({})", url);
                    continue;
                },
                Err(err) => {
                    println!("tracker: error while requesting url({}): {:?}", url, err);
                    continue;
                }
            };
        }
        vec![]
    }
}

impl fmt::Display for Tracker {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{:?}", self.urls)
    }
}
