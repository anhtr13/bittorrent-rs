pub mod encoding;
pub mod peer;
pub mod torrent;

use std::{io::Write, net::TcpStream};

use anyhow::Result;
use clap::{Parser, Subcommand};

use crate::bittorent::{
    encoding::Bencoding,
    peer::{Message, MessageId, discover_peers, download_piece, establish_hanshake, new_peer_id},
    torrent::Torrent,
};

#[derive(Debug, Subcommand)]
pub enum Command {
    #[command(name = "decode")]
    Decode { encoded_value: String },

    #[command(name = "info")]
    Info { torrent: String },

    #[command(name = "peers")]
    Peers { torrent: String },

    #[command(name = "handshake")]
    Handshake { torrent: String, addr: String },

    #[command(name = "download_piece")]
    DownloadPiece {
        torrent: String,
        #[arg(short = 'o', long = "output")]
        output: String,
        piece_index: u32,
    },
}

#[derive(Parser)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Command,
}

impl Cli {
    pub fn run(self) -> Result<()> {
        match self.command {
            Command::Decode { encoded_value } => {
                let decoded_value = Bencoding::decode(encoded_value.into_bytes())?;
                println!("{}", decoded_value);
                Ok(())
            }
            Command::Info { torrent } => {
                let torrent = Torrent::from_file(&torrent)?;
                println!("Tracker URL: {}", torrent.announce);
                println!("Length: {}", torrent.info.length);
                println!("Info Hash: {}", hex::encode(torrent.info.hash));
                println!("Piece Length: {}", torrent.info.piece_length);
                println!("Piece Hashes:");
                for piece in torrent.info.pieces {
                    println!("{}", hex::encode(piece));
                }
                Ok(())
            }
            Command::Peers { torrent } => {
                let torrent = Torrent::from_file(&torrent)?;
                let peer_id = new_peer_id();
                let (_, peers) = discover_peers(
                    &torrent.announce,
                    &torrent.info.hash,
                    &peer_id,
                    6881,
                    0,
                    0,
                    torrent.info.length,
                    true,
                )?;
                for peer in peers {
                    println!("{peer}");
                }
                Ok(())
            }
            Command::Handshake { torrent, addr } => {
                let torrent = Torrent::from_file(&torrent)?;
                let peer_id = new_peer_id();
                let mut stream = TcpStream::connect(addr)?;
                let peer_id_back = establish_hanshake(&mut stream, &torrent.info.hash, &peer_id)?;
                println!("Peer ID: {}", hex::encode(peer_id_back));
                Ok(())
            }
            Command::DownloadPiece {
                output,
                torrent,
                piece_index,
            } => {
                let torrent = Torrent::from_file(&torrent)?;
                let peer_id = new_peer_id();
                let (_, peers) = discover_peers(
                    &torrent.announce,
                    &torrent.info.hash,
                    &peer_id,
                    6881,
                    0,
                    0,
                    torrent.info.length,
                    true,
                )?;
                let mut stream = TcpStream::connect(&peers[0])?;

                let _ = establish_hanshake(&mut stream, &torrent.info.hash, &peer_id)?;

                let bitfield = Message::from_stream(&mut stream)?;
                anyhow::ensure!(bitfield.id == MessageId::Bitfield);

                let interested = Message::new(MessageId::Interested, Vec::new());
                stream.write_all(&interested.into_bytes())?;

                let unchoke = Message::from_stream(&mut stream)?;
                anyhow::ensure!(unchoke.id == MessageId::Unchoke);

                download_piece(
                    &mut stream,
                    piece_index,
                    torrent.info.piece_length,
                    torrent.info.length,
                    &output,
                )
            }
        }
    }
}
