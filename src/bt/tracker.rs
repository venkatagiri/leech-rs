use hyper::client::Client;
use std::io::Read;
use std::fmt;
use std::string::String;
use std::net::{Ipv4Addr, SocketAddrV4};
use std::mem;

use bt::bencoding::*;
use bt::peer::*;

#[derive(Default, Clone)]
pub struct Tracker(pub String);

impl Tracker {
    pub fn get_peers(&self) -> Vec<Peer>{
        let client = Client::new();
        // let hash = "%99%E0%51%1C%CB%76%22%66%4F%09%B6%CD%16%AA%B1%0A%C9%DB%71%04";
        let hash = "%3c%71%d8%10%12%ce%ec%6a%dc%f8%c6%00%9c%92%fd%e5%08%78%b9%cf";
        let peer_id = "76433642664923430920";
        let url = format!("{tracker}?info_hash={hash}&peer_id={peer_id}&port=56789&uploaded=0&downloaded=0&left=0&event=started&compact=1", tracker=self.0, hash=hash, peer_id=peer_id);
        let mut response = match client.get(&url).send() {
            Ok(response) => response,
            Err(_) => panic!("Whoops."),
        };
        let mut buf = vec![];
        match response.read_to_end(&mut buf) {
            Ok(_) => (),
            Err(_) => panic!("I give up."),
        };
        let benc = BEncoding::decode(buf).unwrap();
        let peers = benc.to_dict().unwrap().get("peers").unwrap().to_bytes().unwrap();
        self.decode_peers(&peers)
    }

    fn decode_peers(&self, peers: &[u8]) -> Vec<Peer>{
        peers.chunks(6).filter_map(|peer| {
            let ip = Ipv4Addr::new(peer[0], peer[1], peer[2], peer[3]);
            let port = unsafe { mem::transmute::<[u8; 2], u16>([peer[5], peer[4]]) };
            if port != 56789 {
              Some(Peer::new(SocketAddrV4::new(ip, port)))
            } else {
              None
            }
        }).collect()
    }
}

impl fmt::Display for Tracker {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        write!(f, "{}", self.0)
    }
}
