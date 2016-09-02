use std::fmt;
use std::mem;
use rustc_serialize::hex::ToHex;

/// Contains the SHA1 hash of the decoded value.
#[derive(Default, Clone, Copy, PartialEq)]
pub struct Hash(pub [u8; 20]);

impl Hash {
    pub fn url_encoded(&self) -> String {
      self.0.to_hex().as_bytes().chunks(2).map(|h| format!("%{}{}", h[0] as char, h[1] as char) ).collect::<Vec<String>>().join("")
    }

    pub fn from_vec(h: &Vec<u8>) -> Hash {
        let mut bhash: [u8; 20] = [0; 20];
        bhash.copy_from_slice(h);
        Hash(bhash)
    }
}

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

/// Transmutes u32 to byte slice
pub fn usize_to_byte_vec(input: usize) -> Vec<u8> {
    let mut data: [u8; 4] = unsafe { mem::transmute::<u32, [u8; 4]>(input as u32) };
    data.reverse();
    data.to_vec()
}

// Peer ID used in messages (FIXME: simpler init)
pub const MY_PEER_ID: Hash = Hash([b'3', b'1', b'4', b'1', b'5', b'9', b'2', b'6', b'5', b'3', b'5', b'8', b'9', b'7', b'9', b'3', b'2', b'3', b'8', b'5']);

/// Block size of each piece (2^14)
pub const BLOCK_SIZE: usize = 16384;
