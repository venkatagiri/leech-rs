use std::path::PathBuf;
use std::net::SocketAddr;
use std::collections::HashMap;
use std::fs::OpenOptions;
use std::cmp;
use std::io::Seek;
use std::io::SeekFrom;
use std::io::Write;

use rustc_serialize::hex::FromHex;
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
    pub is_block_downloaded: HashMap<usize, Vec<bool>>,
    pub peers: HashMap<SocketAddr, Peer>,
    pub seeders: HashMap<SocketAddr, Peer>,
}

impl Torrent {
    pub fn new(file: &str) -> Result<Torrent, BEncodingParseError> {
        let data = BEncoding::decode_file(&file).unwrap();
        let name = try!(data.get_info_string("name"));
        // FIXME: pick up http tracker from announce-list
        let tracker = try!(data.get_dict_string("announce"));
        let piece_size = try!(data.get_info_int("piece length")) as usize;
        let pieces = try!(data.get_info_bytes("pieces"));
        // FIXME: calculate info hash(sha1) from info dictionary
        let info_hash = "3c71d81012ceec6adcf8c6009c92fde50878b9cf".from_hex().unwrap(); // FIXME: use proper info_hash

        // Split the pieces into 20 byte sha1 hashes
        let hashes: Vec<Hash> = pieces.chunks(20).map(|chunk| {
            let mut h = Hash([0; 20]); // FIXME: figure out simpler way to do this
            h.0.clone_from_slice(chunk);
            h
        }).collect();
        let no_of_pieces = hashes.len();

        // Parse files list from the info
        let map = data.to_dict().unwrap();
        let info = map.get("info").unwrap().to_dict().unwrap();
        let mut file_items = vec![];
        if let Some(files) = info.get("files") {
            // Multiple File Mode
            let file_list = files.to_list().unwrap();
            let dir = name.clone();
            let mut offset = 0;
            for file in file_list {
                let f1 = file.to_dict().unwrap();
                let len = f1.get("length").unwrap().to_int().unwrap() as usize;
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
                    offset: offset,
                });
                offset += len;
            }
        } else {
            // Single File Mode
            let file_length = try!(data.get_info_int("length")) as usize;
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
            info_hash: Hash::from_vec(&info_hash),
            tracker: Tracker{url: tracker, info_hash: Hash::from_vec(&info_hash)},
            piece_size: piece_size,
            pieces_hashes: hashes,
            files: file_items,
            no_of_pieces: no_of_pieces,
            is_piece_downloaded: vec![false; no_of_pieces as usize],
            is_block_downloaded: HashMap::new(),
            peers: HashMap::new(),
            seeders: HashMap::new(),
        };
        for piece in 0..t.no_of_pieces {
            let block_count = t.get_block_count(piece);
            t.is_block_downloaded.insert(piece, vec![false; block_count]);
        }
        Ok(t)
    }

    pub fn write_block(&mut self, piece: usize, block: usize, data: Vec<u8>) {
        self.write(piece * self.piece_size + block * BLOCK_SIZE, data);
        let block_count = self.get_block_count(piece);
        let b_blocks = self.is_block_downloaded.get_mut(&piece).unwrap();
        b_blocks[block] = true;
        let completed_block_count = b_blocks.iter().map(|b| { if *b == true { 1 } else { 0 } }).fold(0, |sum, x| sum + x);
        if block_count == completed_block_count {
            // FIXME: Verify Piece before marking as downloaded
            self.is_piece_downloaded[piece] = true;
        }
    }

    fn write(&self, start: usize, data: Vec<u8>) {
        let end = start + data.len();
        println!("torrent: start {} end {}", start, end);
        for file in &self.files {
            if (start < file.offset && end < file.offset) || (start > file.offset + file.length && end > file.offset + file.length) {
                continue;
            }
            let fstart = cmp::max(0, start - file.offset);
            let fend = cmp::min(end - file.offset, file.length);
            let flength = fend - fstart;
            let bstart = cmp::max(0, (file.offset as i32 - start as i32).abs()) as usize;
            let bend = bstart + flength;

            let mut f = OpenOptions::new().read(true).write(true).create(true).open(&file.path).unwrap();
            f.seek(SeekFrom::Start(fstart as u64)).unwrap();
            f.write(&data[bstart..bend]).unwrap();
        }
    }

    pub fn get_block_count(&self, piece: usize) -> usize {
        (self.get_piece_size(piece) / BLOCK_SIZE) + 1
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
