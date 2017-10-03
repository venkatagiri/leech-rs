extern crate leech;
use leech::client::Client;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = match args.len() {
        2 => args[1].to_string(),
        _ => {
            println!("leech: usage: {} <torrent file>", args[0]);
            return;
        },
    };
    Client::new(&file).start();
}
