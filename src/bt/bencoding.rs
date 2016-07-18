use std::fs::File;
use std::io::Read;
use std::io::Bytes;
use std::iter::Iterator;
use std::str;
use std::fmt;
use std::iter::Peekable;
use std::collections::BTreeMap;
use std::cmp::PartialEq;

#[derive(Debug)]
pub enum BEncodingParseError {
    Dict,
    List,
    Int,
    Str,
}

impl fmt::Display for BEncodingParseError {
    fn fmt(&self, f:&mut fmt::Formatter) -> fmt::Result {
        match *self {
            BEncodingParseError::Dict => try!(write!(f, "not an dict!")),
            BEncodingParseError::List => try!(write!(f, "not an list!")),
            BEncodingParseError::Int => try!(write!(f, "not an int!")),
            BEncodingParseError::Str => try!(write!(f, "not an str!")),
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
        let f = File::open(&file).unwrap();
        let mut iter = f.bytes().peekable();
        decode_next_type(&mut iter)
    }

    pub fn to_int(&self) -> Result<i64, BEncodingParseError> {
        match *self {
            BEncoding::Int(x) => Ok(x),
            _ => Err(BEncodingParseError::Int),
        }
    }

    pub fn to_dict(&self) -> Result<&BTreeMap<String, BEncoding>, BEncodingParseError> {
        match *self {
            BEncoding::Dict(ref x) => Ok(x),
            _ => Err(BEncodingParseError::Dict),
        }
    }

    pub fn to_list(&self) -> Result<&Vec<BEncoding>, BEncodingParseError> {
        match *self {
            BEncoding::List(ref x) => Ok(x),
            _ => Err(BEncodingParseError::List),
        }
    }

    pub fn to_str(&self) -> Result<String, BEncodingParseError> {
        match *self {
            BEncoding::Str(ref x) => Ok(str::from_utf8(x).unwrap().to_string()),
            _ => Err(BEncodingParseError::Str),
        }
    }

    pub fn get_info_bytes(&self, key: &str) -> Result<Vec<u8>, String> {
        match self {
            &BEncoding::Dict(ref map) => {
                match map.get("info") {
                    Some(&BEncoding::Dict(ref map)) => {
                        match map.get(key) {
                            Some(&BEncoding::Str(ref val)) => Ok(val.clone()),
                            Some(_) => Err("torrent: info should be a dict!".to_string()),
                            None => Err("torrent: info is missing!".to_string()),
                        }
                    }
                    Some(_) => Err("torrent: info should be a dict!".to_string()),
                    None => Err("torrent: info is missing!".to_string()),
                }
            },
            _ => Err("torrent: root type in bencoding should be dictionary!".to_string()),
        }
    }

    pub fn get_hash_string(&self, map: &BTreeMap<String, BEncoding>, key: &str) -> Result<String, String> {
        match map.get(key) {
            Some(&BEncoding::Str(ref value)) => Ok(String::from_utf8_lossy(value).into_owned()),
            Some(_) => Err("bencoding: map value should be a string!".to_string()),
            None => Err(format!("bencoding: map does't have the required key ({})!", key).to_string()),
        }
    }

    pub fn get_hash_int(&self, map: &BTreeMap<String, BEncoding>, key: &str) -> Result<i64, String> {
        match map.get(key) {
            Some(&BEncoding::Int(value)) => Ok(value),
            Some(_) => Err("bencoding: map value should be a string!".to_string()),
            None => Err(format!("bencoding: map does't have the required key ({})!", key).to_string()),
        }
    }

    pub fn get_dict_string(&self, key: &str) -> Result<String, String> {
        match self {
            &BEncoding::Dict(ref map) => self.get_hash_string(map, key),
            _ => Err("bencoding: not a dictionary!".to_string()),
        }
    }

    pub fn get_info_string(&self, key: &str) -> Result<String, String> {
        match self {
            &BEncoding::Dict(ref map) => {
                match map.get("info") {
                    Some(&BEncoding::Dict(ref map)) => self.get_hash_string(map, key),
                    Some(_) => Err("torrent: info should be a dict!".to_string()),
                    None => Err("torrent: info is missing!".to_string()),
                }
            },
            _ => Err("torrent: root type in bencoding should be dictionary!".to_string()),
        }
    }

    pub fn get_info_int(&self, key: &str) -> Result<i64, String> {
        match self {
            &BEncoding::Dict(ref map) => {
                match map.get("info") {
                    Some(&BEncoding::Dict(ref map)) => self.get_hash_int(map, key),
                    Some(_) => Err("torrent: info should be a dict!".to_string()),
                    None => Err("torrent: info is missing!".to_string()),
                }
            },
            _ => Err("torrent: root type in bencoding should be dictionary!".to_string()),
        }
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

fn decode_int(iter: &mut Peekable<Bytes<File>>) -> Option<BEncoding> {
    iter.next();
    let mut num = vec![];
    while let Ok(x) = iter.next().unwrap() {
        if x == TYPE_END {
            break;
        } else {
            num.push(x);
        }
    }
    if num.is_empty() {
        panic!("invalid integer");
    }
    Some(BEncoding::Int(str::from_utf8(&num).unwrap().parse::<i64>().unwrap()))
}

fn decode_list(mut iter: &mut Peekable<Bytes<File>>) -> Option<BEncoding> {
    iter.next();
    let mut list = vec![];
    while let Ok(x) = *iter.peek().unwrap() {
        if x == TYPE_END {
            iter.next();
            break;
        } else {
            match decode_next_type(&mut iter) {
                Some(x) => list.push(x),
                None => panic!("list is invalid!"),
            };
        }
    }
    Some(BEncoding::List(list))
}

fn decode_dict(mut iter: &mut Peekable<Bytes<File>>) -> Option<BEncoding> {
    iter.next();

    let mut map = BTreeMap::new();

    while let Ok(x) = *iter.peek().unwrap() {
        if x == TYPE_END {
            iter.next();
            break;
        } else {
            let key = match decode_str(&mut iter) {
                Some(BEncoding::Str(val)) => str::from_utf8(&val).unwrap().to_string(),
                Some(_) => panic!("Can't have other types as key"),
                None => panic!("Can't have other types as key"),
            };
            let value = decode_next_type(&mut iter).unwrap();
            map.insert(key, value);
        }
    }
    Some(BEncoding::Dict(map))
}

fn decode_str(iter: &mut Peekable<Bytes<File>>) -> Option<BEncoding> {
    let mut len = vec![];
    while let Ok(x) = iter.next().unwrap() {
        if x == b':' {
            break;
        } else {
            len.push(x);
        }
    }
    let mut len = str::from_utf8(&len).unwrap().parse::<u64>().unwrap();
    let mut result = vec![];
    while len > 0 {
        match iter.next().unwrap() {
            Ok(x) => result.push(x),
            Err(_) => panic!("file read failed!"),
        }
        len-=1;
    }
    Some(BEncoding::Str(result))
}

fn decode_next_type(mut iter: &mut Peekable<Bytes<File>>) -> Option<BEncoding> {
    match *iter.peek().unwrap() {
        Ok(DICT_START) => decode_dict(&mut iter),
        Ok(LIST_START) => decode_list(&mut iter),
        Ok(INT_START) => decode_int(&mut iter),
        Ok(_) => decode_str(&mut iter),
        Err(_) => None,
    }
}