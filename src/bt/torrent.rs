use bt::bencoding::*;
use bt::tracker::*;
use bt::utils::*;
use std::path::PathBuf;
use rustc_serialize::hex::FromHex;

/// Files in a Torrent
pub struct FileItem {
    pub length: i64,
    pub path: String
}

/// Info parsed from a .torrent file
pub struct Torrent {
    pub name: String,
    pub tracker: Tracker,
    pub piece_length: i64,
    pub pieces_hashes: Vec<Hash>,
    pub files: Vec<FileItem>,
}

impl Torrent {
    pub fn new(file: &str) -> Result<Torrent, BEncodingParseError> {
        let data = BEncoding::decode_file(&file).unwrap();
        let name = try!(data.get_info_string("name"));
        // TODO: pick up http tracker from announce-list
        let tracker = try!(data.get_dict_string("announce"));
        let piece_length = try!(data.get_info_int("piece length"));
        let pieces = try!(data.get_info_bytes("pieces"));
        // TODO: calculate info hash(sha1) from info dictionary
        let info_hash = "3c71d81012ceec6adcf8c6009c92fde50878b9cf".from_hex().unwrap(); // TODO: use proper info_hash

        // Split the pieces into 20 byte sha1 hashes
        let hashes = pieces.chunks(20).map(|chunk| {
            let mut h = Hash([0; 20]); // TODO: figure out simpler way to do this
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
                let mut file_path = PathBuf::from("."); // TODO: change this to download directory
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
            tracker: Tracker{url: tracker, info_hash: Hash::from_vec(info_hash)},
            piece_length: piece_length,
            pieces_hashes: hashes,
            files: file_items
        })
    }

    pub fn start(&self) {
        let peers = self.tracker.get_peers();
        if peers.is_empty() {
            println!("torrent: no peers found!");
            return;
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
