use std::path::PathBuf;
use std::net::SocketAddr;
use std::thread;

use rustc_serialize::hex::FromHex;
use mio::*;
use mio::tcp::*;
use bt::bencoding::*;
use bt::tracker::*;
use bt::utils::*;
use bt::peer::*;

/// Files in a Torrent
pub struct FileItem {
    pub length: i64,
    pub path: String
}

/// Info parsed from a .torrent file
pub struct Torrent {
    pub name: String,
    pub info_hash: Hash,
    pub tracker: Tracker,
    pub piece_length: i64,
    pub pieces_hashes: Vec<Hash>,
    pub files: Vec<FileItem>,

    peer_addresses: Vec<SocketAddr>,
}

impl Torrent {
    pub fn new(file: &str) -> Result<Torrent, BEncodingParseError> {
        let data = BEncoding::decode_file(&file).unwrap();
        let name = try!(data.get_info_string("name"));
        // FIXME: pick up http tracker from announce-list
        let tracker = try!(data.get_dict_string("announce"));
        let piece_length = try!(data.get_info_int("piece length"));
        let pieces = try!(data.get_info_bytes("pieces"));
        // FIXME: calculate info hash(sha1) from info dictionary
        let info_hash = "3c71d81012ceec6adcf8c6009c92fde50878b9cf".from_hex().unwrap(); // FIXME: use proper info_hash

        // Split the pieces into 20 byte sha1 hashes
        let hashes = pieces.chunks(20).map(|chunk| {
            let mut h = Hash([0; 20]); // FIXME: figure out simpler way to do this
            h.0.clone_from_slice(chunk);
            h
        }).collect();

        // Parse files list from the info
        let map = data.to_dict().unwrap();
        let info = map.get("info").unwrap().to_dict().unwrap();
        let mut file_items = vec![];
        if let Some(files) = info.get("files") {
            // Multiple File Mode
            let file_list = files.to_list().unwrap();
            let dir = name.clone();
            for file in file_list {
                let f1 = file.to_dict().unwrap();
                let len = f1.get("length").unwrap().to_int().unwrap();
                let path = f1.get("path").unwrap().to_list().unwrap();
                let mut file_path = PathBuf::from("."); // FIXME: change this to download directory
                file_path.push(dir.clone());
                for part in path {
                    let p = part.to_str().unwrap();
                    file_path.push(p);
                }
                file_items.push(FileItem {
                    path: file_path.to_str().unwrap().into(),
                    length: len,
                });
            }
        } else {
            // Single File Mode
            let file_length = try!(data.get_info_int("length"));
            let file_name = name.clone();
            let mut file_path = PathBuf::from(".");
            file_path.push(file_name);
            file_items.push(FileItem {
                path: file_path.to_str().unwrap().into(),
                length: file_length,
            });
        }

        for file in &file_items {
            println!("torrent: file is {}", file.path);
        }

        Ok(Torrent {
            name: name,
            info_hash: Hash::from_vec(&info_hash),
            tracker: Tracker{url: tracker, info_hash: Hash::from_vec(&info_hash)},
            piece_length: piece_length,
            pieces_hashes: hashes,
            files: file_items,
            peer_addresses: vec![]
        })
    }

    pub fn start(&mut self) {
        self.peer_addresses = self.tracker.get_peers_addresses();
        if self.peer_addresses.is_empty() {
            panic!("torrent: no peers found!");
        }
        let add_peer_address_channel: Sender<_>;
        {
            println!("torrent: creating event loop");
            let address = "0.0.0.0:56789".parse::<SocketAddr>().unwrap();
            let server_socket = TcpListener::bind(&address).unwrap();

            let mut event_loop = EventLoop::new().unwrap();
            add_peer_address_channel = event_loop.channel();
            let mut handler = PeerHandler::new(server_socket, &self.info_hash);

            event_loop.register(&handler.socket, SERVER_TOKEN, EventSet::readable(), PollOpt::edge()).unwrap();
            event_loop.timeout_ms(TIMER_TOKEN, 5000).unwrap();
            thread::spawn(move || {
                event_loop.run(&mut handler).unwrap();
            });
        }

        let mut peers: Vec<Peer> = self.peer_addresses.iter().map(|addr| Peer::from_addr(addr, self.info_hash)).collect();
        for peer in peers {
            add_peer_address_channel.send(NotifyTypes::Peer(peer));
        }
        add_peer_address_channel.send(NotifyTypes::Action(MessageType::UnChoke));
        loop {
            println!("torrent: main loop");
            ::std::thread::sleep_ms(5000);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    #[test]
    fn new() {
        let mut path = PathBuf::from(".");
        path.push("tests");
        path.push("data");
        path.push("99E0511CCB7622664F09B6CD16AAB10AC9DB7104.torrent");
        let file = path.to_str().unwrap();
        let torrent = Torrent::new(file).unwrap();
        assert_eq!("Torrent downloaded from torrent cache at torcache.net", torrent.comment);
    }
}
