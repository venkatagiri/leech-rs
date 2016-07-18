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
    let torrent = match Torrent::new(&file) {
        Ok(val) => val,
        Err(err) => {
            println!("Failure: {}", err);
            return;
        }
    };
    println!("torrent(comment) => {}", torrent.comment);
    println!("torrent(tracker) => {}", torrent.tracker);
    println!("torrent(created_by) => {}", torrent.created_by);
    println!("torrent(name) => {}", torrent.name);
    println!("torrent(piece_length) => {}", torrent.piece_length);
    println!("torrent(pieces_hashes) => {}", torrent.pieces_hashes.len());
    for file in &torrent.files {
        println!("torrent(file) => {}, len => {}", file.path, file.length);
    }
}