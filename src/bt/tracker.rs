use hyper::client::Client;
use std::io::Read;
use std::fmt;
use std::string::String;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::mem;

use bt::bencoding::*;
use bt::peer::*;
use bt::utils::*;

#[derive(Default, Clone)]
pub struct Tracker {
    pub url: String,
    pub info_hash: Hash
}

impl Tracker {
    pub fn get_peers(&self) -> Vec<Peer>{
        let client = Client::new();
        let url = format!("{tracker}?info_hash={hash}&peer_id={peer_id}&port=56789&uploaded=0&downloaded=0&left=0&event=started&compact=1",
                    tracker = self.url,
                    hash = self.info_hash.url_encoded(),
                    peer_id = MY_PEER_ID.url_encoded());
        let mut buf = vec![];
        let mut response = client.get(&url).send().unwrap();
        response.read_to_end(&mut buf).unwrap();

        let benc = BEncoding::decode(buf).unwrap();
        let peers = benc.to_dict().unwrap().get("peers").unwrap().to_bytes().unwrap();
        self.create_peers(&peers)
    }

    fn create_peers(&self, peers: &[u8]) -> Vec<Peer>{
        peers.chunks(6).filter_map(|peer| {
            let ip = Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3]);
            let port = unsafe { mem::transmute::<[u8; 2], u16>([peer[5], peer[4]]) };
            if port != 56789 {
              Some(Peer::new(SocketAddrV4::new(ip, port), self.info_hash))
            } else {
              None
            }
        }).collect()
    }
}

impl fmt::Display for Tracker {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.url)
    }
}
