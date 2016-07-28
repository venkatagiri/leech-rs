use bt::bencoding::*;
use bt::tracker::*;
use std::path::PathBuf;
use std::fmt;

/// Contains the SHA1 hash of the decoded value.
#[derive(Default, Clone, Copy)]
pub struct Hash(pub [u8; 20]);

impl fmt::Display for Hash {
    /// Prints the SHA1 hash as a string of hexadecimal digits.
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        static HEX_CHARS: &'static [u8; 16] = b"0123456789abcdef";
        let mut buf = [0; 40];

        for (i, &b) in self.0.iter().enumerate() {
            buf[i * 2    ] = HEX_CHARS[(b >> 4) as usize];
            buf[i * 2 + 1] = HEX_CHARS[(b & 0xf) as usize];
        }

        write!(f, "{}", String::from_utf8(buf.to_vec()).unwrap())
    }
}

/// Files in a Torrent
pub struct FileItem {
    pub length: i64,
    pub path: String
}

/// Info parsed from a .torrent file
pub struct Torrent {
    pub comment: String,
    pub tracker: Tracker,
    pub created_by: String,
    pub name: String,
    pub piece_length: i64,
    pub pieces_hashes: Vec<Hash>,
    pub files: Vec<FileItem>,
}

impl Torrent {
    pub fn new(file: &str) -> Result<Torrent, BEncodingParseError> {
        let data = BEncoding::decode_file(&file).unwrap();
        let comment = try!(data.get_dict_string("comment"));
        // TODO: pick up http tracker from announce-list
        let tracker = "http://tracker.trackerfix.com/announce".to_string(); //try!(data.get_dict_string("announce"));
        let created_by = try!(data.get_dict_string("created by"));
        let name = try!(data.get_info_string("name"));
        let piece_length = try!(data.get_info_int("piece length"));
        let pieces = try!(data.get_info_bytes("pieces"));

        // Split the pieces into 20 byte sha1 hashes
        let hashes = pieces.chunks(20).map(|chunk| {
            let mut h = Hash([0; 20]); // TODO: figure out simpler way to do this
            h.0.clone_from_slice(chunk);
            h
        }).collect();

        // Parse files list from the info
        let map = data.to_dict().unwrap();
        let info = map.get("info").unwrap().to_dict().unwrap();
        let files = info.get("files").unwrap().to_list().unwrap();
        let mut file_items = vec![];
        for file in files {
            let f1 = file.to_dict().unwrap();
            let len = f1.get("length").unwrap().to_int().unwrap();
            let path = f1.get("path").unwrap().to_list().unwrap();
            let mut file_path = PathBuf::from(".");
            for part in path {
                let p = part.to_str().unwrap();
                file_path.push(p);
            }
            file_items.push(FileItem {
                path: file_path.to_str().unwrap().into(),
                length: len,
            });
        }

        Ok(Torrent {
            comment: comment,
            tracker: Tracker(tracker),
            created_by: created_by,
            name: name,
            piece_length: piece_length,
            pieces_hashes: hashes,
            files: file_items
        })
    }

    pub fn start(&self) {
        let peers = self.tracker.get_peers();
        for peer in &peers {
            println!("peer address is {}", peer);
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
