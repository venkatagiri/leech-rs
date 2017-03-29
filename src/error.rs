use std::result;
use std::net;
use std::io;
use hyper;

use bencoding;

#[derive(Debug)]
pub enum Error {
    AddrParseError(net::AddrParseError),
    Io(io::Error),
    Hyper(hyper::Error),
    BEncoding(bencoding::Error),
}

impl From<net::AddrParseError> for Error {
    fn from(other: net::AddrParseError) -> Self {
        Error::AddrParseError(other)
    }
}

impl From<io::Error> for Error {
    fn from(other: io::Error) -> Self {
        Error::Io(other)
    }
}

impl From<hyper::Error> for Error {
    fn from(other: hyper::Error) -> Self {
        Error::Hyper(other)
    }
}

impl From<bencoding::Error> for Error {
    fn from(other: bencoding::Error) -> Self {
        Error::BEncoding(other)
    }
}

pub type Result<T> = result::Result<T, Error>;
