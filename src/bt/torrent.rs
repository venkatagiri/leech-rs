use bt::bencoding::BEncoding;

pub struct Torrent {
    pub comment: String
}

impl Torrent {
    pub fn new(file: &str) -> Result<Torrent, &str> {
        let data = BEncoding::decode_file(&file);
        let comment = match data {
            Some(BEncoding::Dict(ref map)) => {
                match map.get("comment") {
                    Some(&BEncoding::Str(ref value)) => String::from_utf8_lossy(value).into_owned(),
                    _ => return Err("torrent: comment should be a string!"),
                }
            },
            Some(_) => return Err("torrent: root type in bencoding should be dictionary!"),
            None => return Err("torrent: parsing file failed!"),
        };
        Ok(Torrent {
            comment: comment
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
