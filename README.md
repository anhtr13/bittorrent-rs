# The ["Build Your Own BitTorrent" Challenge](https://app.codecrafters.io/courses/bittorrent/overview)

![progress-banner](https://backend.codecrafters.io/progress/bittorrent/cd5b0695-a581-4bb1-ac92-1fb5839d7850)

A BitTorrent client that's capable of parsing a .torrent file and downloading a file from a peer.
Build to learn about how torrent files are structured, HTTP trackers, BitTorrent’s Peer Protocol, pipelining, etc.

## Build & Run

```bash
# Release build
cargo build --release

# Download from a torrent file
./target/release/my-bittorrent download [sample.torrent] -o [output]

# Download from a magnet link
./target/release/my-bittorrent magnet_download [magnet_link] -o [output]
```
