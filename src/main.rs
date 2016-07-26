extern crate hyper;

mod bt;
use bt::torrent::Torrent;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = match args.len() {
        2 => args[1].to_string(),
        _ => {
            println!("leecher: usage: {} <torrent file>", args[0]);
            return;
        },
    };
    let torrent = match Torrent::new(&file) {
        Ok(val) => val,
        Err(err) => {
            println!("leecher: failure: {}", err);
            return;
        }
    };
    torrent.start();
}
