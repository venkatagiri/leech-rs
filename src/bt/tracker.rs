use std::io::Read;
use std::fmt;
use std::string::String;
use std::str::FromStr;
use std::net::{IpAddr, SocketAddr};
use std::mem;

use hyper::client::Client;
use bt::bencoding::*;
use bt::peer::*;
use bt::utils::*;

#[derive(Default, Clone)]
pub struct Tracker {
    pub url: String,
    pub info_hash: Hash
}

impl Tracker {
    pub fn get_peers_addresses(&self) -> Vec<SocketAddr>{
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
        if peers.len() <= 6 {
            self.get_default_peers()
        } else {
            self.parse_peers(&peers)
        }
    }

    fn parse_peers(&self, peers: &[u8]) -> Vec<SocketAddr>{
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

    fn get_default_peers(&self) -> Vec<SocketAddr>{
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
