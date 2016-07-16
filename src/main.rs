mod bt;
use bt::torrent::Torrent;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let file = match args.len() {
        2 => args[1].to_string(),
        _ => {
            println!("usage: {} <torrent file>", args[0]);
            return;
        },
    };
    let torrent = Torrent::new(&file).unwrap();
    println!("torrent(comment) => {}", torrent.comment);
}