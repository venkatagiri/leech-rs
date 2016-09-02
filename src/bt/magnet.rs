#[derive(Default)]
pub struct Magnet {
    pub xt: String,
    pub dn: String,
    pub tr: Vec<String>
}

#[allow(dead_code)]
impl Magnet {
    pub fn new(input: &str) -> Result<Magnet, &str> {
        if !input.starts_with("magnet:?") {
            return Err("Link should start with 'magnet:?'");
        }
        let parts: Vec<&str> = input.split("magnet:?").collect();
        let uri: &str = match parts.get(1) {
            Some(part) => part,
            None => { return Err("Invalid Magnet Link!"); },
        };
        let uri_parts: Vec<&str> = uri.split('&').collect();
        let mut magnet: Magnet = Default::default();
        for part in &uri_parts {
            let key_val: Vec<&str> = part.split('=').collect();
            let key = key_val.get(0).unwrap().clone();
            let value = key_val.get(1).unwrap().clone();

            match key {
                "xt" => magnet.xt = value.to_string(),
                "dn" => magnet.dn = value.to_string(),
                "tr" => magnet.tr.push(value.to_string()),
                _ => {}
            }
        }

        Ok(magnet)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new() {    
        let link = "magnet:?xt=urn:btih:99E0511CCB7622664F09B6CD16AAB10AC9DB7104\
            &dn=Murder+in+the+First+S03E02+HDTV+x264+LOL\
            &tr=udp://tracker.coppersurfer.tk:6969/announce\
            &tr=udp://tracker.leechers-paradise.org:6969\
            &tr=udp://open.demonii.com:1337";
        let magnet = Magnet::new(link).unwrap();
        println!("xt is {}", magnet.xt);
        println!("dn is {}", magnet.dn);
        for tr in &magnet.tr {
            println!("tr is {}", tr);
        }
        assert_eq!("urn:btih:99E0511CCB7622664F09B6CD16AAB10AC9DB7104", magnet.xt);
        assert_eq!("Murder+in+the+First+S03E02+HDTV+x264+LOL", magnet.dn);
        assert_eq!("udp://tracker.coppersurfer.tk:6969/announce", magnet.tr[0]);
        assert_eq!("udp://tracker.leechers-paradise.org:6969", magnet.tr[1]);
        assert_eq!("udp://open.demonii.com:1337", magnet.tr[2]);
    }
}
