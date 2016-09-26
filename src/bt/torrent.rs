use std::path::{PathBuf, Path};
use std::net::SocketAddr;
use std::collections::HashMap;
use std::fs;
use std::cmp;
use std::io::{
    self,
    Seek,
    SeekFrom,
    Write,
    Read
};

use bt::bencoding::*;
use bt::tracker::*;
use bt::utils::*;
use bt::peer::Peer;

/// Files in a Torrent
#[derive(Clone)]
pub struct FileItem {
    pub length: usize,
    pub offset: usize,
    pub path: String,
}

/// Info parsed from a .torrent file
#[derive(Clone)]
pub struct Torrent {
    pub name: String,
    pub info_hash: Hash,
    pub tracker: Tracker,
    pub piece_size: usize,
    pub pieces_hashes: Vec<Hash>,
    pub files: Vec<FileItem>,
    pub no_of_pieces: usize,
    pub is_piece_downloaded: Vec<bool>,
    pub is_block_downloaded: Vec<Vec<bool>>,
    pub peers: HashMap<SocketAddr, Peer>,
    pub seeders: Vec<SocketAddr>,
}

impl Torrent {
    pub fn new(file: &str) -> Result<Torrent, BEncodingParseError> {
        let root = BEncoding::decode_file(&file).unwrap();
        println!("torrent: contents {}", root);
        let info = try!(root.get_dict("info"));
        let info_hash = {
            let data = BEncoding::encode(&info);
            sha1(&data)
        };

        let info = try!(root.get_dict("info"));
        let name = try!(info.get_str("name"));

        let mut tracker_list = vec![];
        tracker_list.push(try!(root.get_str("announce")));
        if let Ok(announce_list) = root.get_list("announce-list") {
            for list in announce_list {
                for tracker in try!(list.to_list()) {
                    tracker_list.push(try!(tracker.to_str()));
                }
            }
        }

        let piece_size = try!(info.get_int("piece length")) as usize;
        let pieces = try!(info.get_bytes("pieces"));

        // Split the pieces into 20 byte sha1 hashes
        let hashes: Vec<Hash> =
            pieces
            .chunks(20)
            .map(|chunk| { Hash::from_slice(chunk) })
            .collect();
        let no_of_pieces = hashes.len();

        // Parse files list from the info
        let mut file_items = vec![];
        if let Ok(files) = info.get_list("files") {
            // Multiple File Mode
            let dir = name.clone();
            let mut offset = 0;
            for file in files {
                let len = try!(file.get_int("length")) as usize;
                let path = try!(file.get_list("path"));
                let mut file_path = PathBuf::from("."); // FIXME: change this to download directory
                file_path.push(dir.clone());
                for part in path {
                    let p = part.to_str().unwrap();
                    file_path.push(p);
                }
                file_items.push(FileItem {
                    path: file_path.to_str().unwrap().into(),
                    length: len,
                    offset: offset,
                });
                offset += len;
            }
        } else {
            // Single File Mode
            let file_length = try!(info.get_int("length")) as usize;
            let file_name = name.clone();
            let mut file_path = PathBuf::from(".");
            file_path.push(file_name);
            file_items.push(FileItem {
                path: file_path.to_str().unwrap().into(),
                length: file_length,
                offset: 0,
            });
        }

        for file in &file_items {
            println!("torrent: file is {}", file.path);
        }

        let mut t = Torrent {
            name: name,
            info_hash: Hash::from_slice(&info_hash),
            tracker: Tracker::new(tracker_list, Hash::from_slice(&info_hash)),
            piece_size: piece_size,
            pieces_hashes: hashes,
            files: file_items,
            no_of_pieces: no_of_pieces,
            is_piece_downloaded: vec![false; no_of_pieces as usize],
            is_block_downloaded: vec![],
            peers: HashMap::new(),
            seeders: vec![],
        };
        for piece in 0..t.no_of_pieces {
            let block_count = t.get_block_count(piece);
            t.is_block_downloaded.push(vec![false; block_count]);
            t.verify_piece(piece);
        }
        Ok(t)
    }

    pub fn write_block(&mut self, piece: usize, block: usize, data: Vec<u8>) {
        self.write(piece * self.piece_size + block * BLOCK_SIZE, data);
        self.is_block_downloaded[piece][block] = true;

        let block_count = self.get_block_count(piece);
        let completed_block_count = self.get_completed_block_count(piece);
        if block_count == completed_block_count {
            self.verify_piece(piece);
        }
    }

    fn write(&self, start: usize, data: Vec<u8>) {
        let end = start + data.len();
        for file in &self.files {
            if (start < file.offset && end < file.offset) || (start > file.offset + file.length && end > file.offset + file.length) {
                continue;
            }
            let fstart = cmp::max(0, start - file.offset);
            let fend = cmp::min(end - file.offset, file.length);
            let flength = fend - fstart;
            let bstart = cmp::max(0, file.offset as i64 - start as i64) as usize;
            let bend = bstart + flength;

            // Create directories in the file path if they don't exist
            if let Some(dirs) = Path::new(&file.path).parent() {
                if let Err(err) = fs::create_dir_all(dirs) {
                    println!("torrent: create dir({:?}) failed with error {}", dirs, err);
                    return;
                };
            }

            let mut f = fs::OpenOptions::new().read(true).write(true).create(true).open(&file.path).unwrap();
            f.seek(SeekFrom::Start(fstart as u64)).unwrap();
            f.write(&data[bstart..bend]).unwrap();
        }
    }

    fn verify_piece(&mut self, piece: usize) {
        let data = match self.read_piece(piece) {
            Ok(data) => data,
            Err(_) => return,
        };
        let sha1 = sha1(&data);
        let hash = Hash::from_slice(&sha1);
        if self.pieces_hashes.get(piece) == Some(&hash) {
            self.is_piece_downloaded[piece] = true;
            self.is_block_downloaded[piece] = vec![true; self.get_block_count(piece)];
            if self.is_complete() {
                println!("client: torrent download is complete");
            }
            // send Have messages when a piece is verified
            for peer in self.peers.values_mut() {
                peer.send_have(piece);
            }
        } else {
            self.is_piece_downloaded[piece] = false;
            self.is_block_downloaded[piece] = vec![false; self.get_block_count(piece)];
        }
    }

    pub fn is_block_requested(&self, piece: usize, block: usize) -> bool {
        for peer in self.peers.values() {
            if peer.is_block_requested[piece][block] {
                return true;
            }
        }
        false
    }

    pub fn is_complete(&self) -> bool {
        self.get_completed_piece_count() == self.no_of_pieces
    }

    fn read_piece(&self, piece: usize) -> io::Result<Vec<u8>> {
        let start = piece * self.piece_size;
        let end = cmp::min(self.get_total_size(), (piece + 1) * self.piece_size);
        let mut data = vec![];

        for file in &self.files {
            if (start < file.offset && end < file.offset) || (start > file.offset + file.length && end > file.offset + file.length) {
                continue;
            }

            let fstart = cmp::max(0, start - file.offset);
            let fend = cmp::min(end - file.offset, file.length);
            let flength = fend - fstart;
            let mut buffer = vec![0; flength];

            let mut f = try!(fs::OpenOptions::new().read(true).open(&file.path));
            try!(f.seek(SeekFrom::Start(fstart as u64)));
            try!(f.read_exact(&mut buffer));

            data.extend_from_slice(&buffer);
        }

        Ok(data)
    }

    pub fn get_block_count(&self, piece: usize) -> usize {
        (self.get_piece_size(piece) as f32 / BLOCK_SIZE as f32).ceil() as usize
    }

    fn get_completed_block_count(&self, piece: usize) -> usize {
        self.is_block_downloaded[piece].iter().map(|b| { if *b == true { 1 } else { 0 } }).fold(0, |sum, x| sum + x)
    }

    fn get_completed_piece_count(&self) -> usize {
        self.is_piece_downloaded.iter().map(|b| { if *b == true { 1 } else { 0 } }).fold(0, |sum, x| sum + x)
    }

    pub fn get_block_size(&self, piece: usize, block: usize) -> usize {
        if block == self.get_block_count(piece) - 1 {
            let size = self.get_piece_size(piece) % BLOCK_SIZE;
            if size != 0 {
                return size;
            }
        }
        BLOCK_SIZE
    }

    pub fn get_piece_size(&self, piece: usize) -> usize {
        if piece == self.no_of_pieces - 1 {
            let size = self.get_total_size() % self.piece_size;
            if size != 0 {
                return size;
            }
        }
        self.piece_size
    }

    pub fn get_total_size(&self) -> usize {
        self.files.iter().fold(0, |sum, ref f| sum + f.length)
    }

}
