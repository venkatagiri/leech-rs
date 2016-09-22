use std::fmt;
use std::mem;

use sha1;
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
pub fn u32_to_byte_slice(input: u32) -> Vec<u8> {
    let data: [u8; 4] = unsafe { mem::transmute::<u32, [u8; 4]>(input.to_be()) };
    data.to_vec()
}

/// Transmutes u64 to byte slice
pub fn u64_to_byte_slice(input: u64) -> Vec<u8> {
    let data: [u8; 8] = unsafe { mem::transmute::<u64, [u8; 8]>(input.to_be()) };
    data.to_vec()
}

/// Transmutes byte slice to u64
pub fn byte_slice_to_u64(input: &[u8]) -> u64 {
    let mut val: [u8; 8] = [0; 8];
    val.copy_from_slice(input);
    let num = unsafe { mem::transmute::<[u8; 8], u64>(val) };
    u64::from_be(num)
}

/// Transmutes byte slice to usize
pub fn byte_slice_to_u32(input: &[u8]) -> u32 {
    let mut val: [u8; 4] = [0; 4];
    val.copy_from_slice(input);
    let num = unsafe { mem::transmute::<[u8; 4], u32>(val) };
    u32::from_be(num)
}

/// Calculate sha1 of a vector
pub fn sha1(data: &Vec<u8>) -> Vec<u8> {
    let mut m = sha1::Sha1::new();
    m.update(&data[..]);
    m.digest().bytes().to_vec()
}

/// Get bits from byte slice
pub fn to_bits(list: &[u8]) -> Vec<u8> {
    fn get_bits(n: &u8) -> Vec<u8> {
        let mask: u8 = 0b1000_0000;
        let mut bits = vec![0; 8];
        for shift in 0..8 {
            let bit = n & mask.rotate_right(shift);
            bits[shift as usize] = bit.count_ones() as u8;
        }
        bits
    }

    let mut bits = vec![];
    for byte in list {
        bits.extend_from_slice(&get_bits(byte));
    }
    bits
}

// Peer ID used in messages (FIXME: simpler init)
pub const MY_PEER_ID: Hash = Hash([b'3', b'1', b'4', b'1', b'5', b'9', b'2', b'6', b'5', b'3', b'5', b'8', b'9', b'7', b'9', b'3', b'2', b'3', b'8', b'5']);

/// Block size of each piece (2^14)
pub const BLOCK_SIZE: usize = 16384;
