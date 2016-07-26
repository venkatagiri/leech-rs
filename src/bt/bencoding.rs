use std::fs::File;
use std::io::Read;
use std::iter::Iterator;
use std::slice::Iter;
use std::str;
use std::fmt;
use std::iter::Peekable;
use std::collections::BTreeMap;
use std::cmp::PartialEq;

#[derive(Debug)]
pub enum BEncodingParseError {
    NotADict,
    NotAList,
    NotAInt,
    NotAStr,
    MissingKey(String)
}

impl fmt::Display for BEncodingParseError {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        match *self {
            BEncodingParseError::NotADict => try!(write!(f, "not a dict!")),
            BEncodingParseError::NotAList => try!(write!(f, "not a list!")),
            BEncodingParseError::NotAInt => try!(write!(f, "not an int!")),
            BEncodingParseError::NotAStr => try!(write!(f, "not a str!")),
            BEncodingParseError::MissingKey(ref val) => try!(write!(f, "missing key `{}`", val)),
        }
        Ok(())
    }
}

pub enum BEncoding {
    Dict(BTreeMap<String, BEncoding>),
    List(Vec<BEncoding>),
    Int(i64),
    Str(Vec<u8>),
}

impl BEncoding {
    pub fn decode_file(file: &str) -> Option<BEncoding> {
        let mut f = File::open(&file).unwrap();
        let mut buf = vec![];
        f.read_to_end(&mut buf);
        Self::decode(buf)
    }

    pub fn decode(buf: Vec<u8>) -> Option<BEncoding> {
        let mut iter = buf.iter().peekable();
        decode_next_type(&mut iter)
    }

    pub fn to_int(&self) -> Result<i64, BEncodingParseError> {
        match *self {
            BEncoding::Int(val) => Ok(val),
            _ => Err(BEncodingParseError::NotAInt),
        }
    }

    pub fn to_dict(&self) -> Result<&BTreeMap<String, BEncoding>, BEncodingParseError> {
        match *self {
            BEncoding::Dict(ref map) => Ok(map),
            _ => Err(BEncodingParseError::NotADict),
        }
    }

    pub fn to_list(&self) -> Result<&Vec<BEncoding>, BEncodingParseError> {
        match *self {
            BEncoding::List(ref list) => Ok(list),
            _ => Err(BEncodingParseError::NotAList),
        }
    }

    pub fn to_str(&self) -> Result<String, BEncodingParseError> {
        match *self {
            BEncoding::Str(ref val) => Ok(str::from_utf8(val).unwrap().to_string()),
            _ => Err(BEncodingParseError::NotAStr),
        }
    }

    pub fn to_bytes(&self) -> Result<Vec<u8>, BEncodingParseError> {
        match *self {
            BEncoding::Str(ref val) => Ok(val.clone()),
            _ => Err(BEncodingParseError::NotAStr),
        }
    }

    pub fn get_hash_string(&self, map: &BTreeMap<String, BEncoding>, key: &str) -> Result<String, BEncodingParseError> {
        let value = try!(map.get(key).ok_or(BEncodingParseError::MissingKey(key.to_string())));
        value.to_str()
    }

    pub fn get_hash_int(&self, map: &BTreeMap<String, BEncoding>, key: &str) -> Result<i64, BEncodingParseError> {
        let value = try!(map.get(key).ok_or(BEncodingParseError::MissingKey(key.to_string())));
        value.to_int()
    }

    pub fn get_dict_string(&self, key: &str) -> Result<String, BEncodingParseError> {
        let map = try!(self.to_dict());
        self.get_hash_string(map, key)
    }

    pub fn get_info_bytes(&self, key: &str) -> Result<Vec<u8>, BEncodingParseError> {
        let map = try!(self.to_dict());
        let info = try!(map.get("info").ok_or(BEncodingParseError::MissingKey("info".to_string())));
        let info_map = try!(info.to_dict());
        let value = try!(info_map.get(key).ok_or(BEncodingParseError::MissingKey(key.to_string())));
        value.to_bytes()
    }

    pub fn get_info_string(&self, key: &str) -> Result<String, BEncodingParseError> {
        let map = try!(self.to_dict());
        let info = try!(map.get("info").ok_or(BEncodingParseError::MissingKey("info".to_string())));
        let info_map = try!(info.to_dict());
        self.get_hash_string(info_map, key)
    }

    pub fn get_info_int(&self, key: &str) -> Result<i64, BEncodingParseError> {
        let map = try!(self.to_dict());
        let info = try!(map.get("info").ok_or(BEncodingParseError::MissingKey("info".to_string())));
        let info_map = try!(info.to_dict());
        self.get_hash_int(info_map, key)
    }
}

impl fmt::Display for BEncoding {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        match *self {
            BEncoding::Int(value) => {
                try!(write!(f, "{}", value))
            },
            BEncoding::Str(ref value) => {
                let result = String::from_utf8_lossy(value);
                try!(write!(f, "{}", result))
            },
            BEncoding::List(ref list) => {
                let mut start = true;
                try!(write!(f, "["));
                for value in list {
                    if start {
	                    start = false;
                    } else {
                        try!(write!(f, ", "));
                    }
                    try!(write!(f, "{}", value));
                }
                try!(write!(f, "]"))
            },
            BEncoding::Dict(ref map) => {
                let mut start = true;
                try!(write!(f, "{{"));
                for (key, value) in map {
                    if start {
	                    start = false;
                    } else {
                        try!(write!(f, ", "));
                    }
                    try!(write!(f, "{} : ", key));
                    if key == "pieces" {
                        try!(write!(f, "REDACTED"));
                    } else {
                        try!(write!(f, "{}", value));
                    }
                }
                try!(write!(f, "}}"));
            },
        }
        Ok(())
    }
}

impl PartialEq for BEncoding {
    #[allow(unused_variables)]
    fn eq(&self, other: &Self) -> bool {
        true
    }
}

const DICT_START:u8 = b'd';
const LIST_START:u8 = b'l';
const INT_START :u8 = b'i';
const TYPE_END  :u8 = b'e';

fn decode_int(iter: &mut Peekable<Iter<u8>>) -> Option<BEncoding> {
    iter.next();
    let mut num = vec![];
    while TYPE_END != **iter.peek().unwrap() {
        num.push(*iter.next().unwrap());
    }
    iter.next();

    if num.is_empty() {
        panic!("invalid integer");
    }
    Some(BEncoding::Int(str::from_utf8(&num).unwrap().parse::<i64>().unwrap()))
}

fn decode_list(mut iter: &mut Peekable<Iter<u8>>) -> Option<BEncoding> {
    iter.next();
    let mut list = vec![];
    while TYPE_END != **iter.peek().unwrap() {
        match decode_next_type(&mut iter) {
            Some(x) => list.push(x),
            None => panic!("list is invalid!"),
        };
    }
    iter.next();
    Some(BEncoding::List(list))
}

fn decode_dict(mut iter: &mut Peekable<Iter<u8>>) -> Option<BEncoding> {
    iter.next();

    let mut map = BTreeMap::new();

    while TYPE_END != **iter.peek().unwrap() {
        let key = match decode_str(&mut iter) {
            Some(BEncoding::Str(val)) => str::from_utf8(&val).unwrap().to_string(),
            Some(_) => panic!("Can't have other types as key"),
            None => panic!("Can't have other types as key"),
        };
        let value = decode_next_type(&mut iter).unwrap();
        map.insert(key, value);
    }
    iter.next();
    Some(BEncoding::Dict(map))
}

fn decode_str(iter: &mut Peekable<Iter<u8>>) -> Option<BEncoding> {
    let mut len = vec![];
    while b':' != **iter.peek().unwrap() {
        len.push(*iter.next().unwrap());
    }
    iter.next();
    let mut len = str::from_utf8(&len).unwrap().parse::<u64>().unwrap();
    let mut result = vec![];
    while len > 0 {
        result.push(*iter.next().unwrap());
        len-=1;
    }
    Some(BEncoding::Str(result))
}

fn decode_next_type(mut iter: &mut Peekable<Iter<u8>>) -> Option<BEncoding> {
    match **iter.peek().unwrap() {
        DICT_START => decode_dict(&mut iter),
        LIST_START => decode_list(&mut iter),
        INT_START => decode_int(&mut iter),
        _ => decode_str(&mut iter),
    }
}
