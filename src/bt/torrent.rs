use bt::bencoding::*;
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
    pub tracker: String,
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
        let tracker = try!(data.get_dict_string("announce"));
        let created_by = try!(data.get_dict_string("created by"));
        let name = try!(data.get_info_string("name"));
        let piece_length = try!(data.get_info_int("piece length"));
        let pieces = try!(data.get_info_bytes("pieces"));

        // Split the pieces into 20 byte sha1 hashes
        let no_of_pieces = pieces.len()/20;
        let mut hashes = vec![];

        for x in 0..no_of_pieces {
            let start = x * 20;
            let end = (x+1) * 20;
            let mut hash = Hash([0; 20]);
            hash.0.clone_from_slice(&pieces[start..end]);
            hashes.push(hash);
        }

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
            tracker: tracker,
            created_by: created_by,
            name: name,
            piece_length: piece_length,
            pieces_hashes: hashes,
            files: file_items
        })
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
