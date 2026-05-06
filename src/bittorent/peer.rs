use std::{
    fs::OpenOptions,
    io::{Read, Write},
    net::TcpStream,
};

use anyhow::{Context, Result};
use rand::{RngExt, distr::Alphanumeric};

use crate::bittorent::encoding::Bencoding;

pub const BLOCK_SIZE: u64 = 16384;

#[allow(clippy::too_many_arguments)]
pub fn discover_peers(
    url: &str,
    info_hash: &[u8],
    peer_id: &str,
    port: u16,
    uploaded: u32,
    downloaded: u32,
    left: u64,
    compact: bool,
) -> Result<(u64, Vec<String>)> {
    let client = reqwest::blocking::Client::new();
    let url = format!(
        "{}?info_hash={}&peer_id={}&port={}&uploaded={}&downloaded={}&left={}&compact={}",
        url,
        url_encode(info_hash).as_str(),
        peer_id,
        port,
        uploaded,
        downloaded,
        left,
        compact as u8
    );
    let res = client.get(&url).send()?.bytes()?.to_vec();
    let data = Bencoding::decode(res)?;
    let Bencoding::Dictionary(dict) = data else {
        anyhow::bail!("tracker response must be a dictionary")
    };
    let Some(Bencoding::Integer(interval)) = dict.get("interval") else {
        anyhow::bail!("failed to parse tracker response");
    };
    let Some(Bencoding::String(peers)) = dict.get("peers") else {
        anyhow::bail!("failed to parse tracker response");
    };
    let interval = *interval as u64;
    let peers: Vec<_> = peers
        .chunks(6)
        .map(|addr| {
            let port = u16::from_be_bytes([addr[4], addr[5]]);
            format!("{}.{}.{}.{}:{}", addr[0], addr[1], addr[2], addr[3], port)
        })
        .collect();
    Ok((interval, peers))
}

pub fn establish_hanshake(
    stream: &mut TcpStream,
    info_hash: &[u8; 20],
    peer_id: &str,
) -> Result<Vec<u8>> {
    let protocol = String::from("BitTorrent protocol");
    let reserved = [0u8; 8];

    let mut buf = Vec::new();
    buf.push(protocol.len() as u8);
    buf.extend(protocol.into_bytes());
    buf.extend(reserved);
    buf.extend(info_hash);
    buf.extend(peer_id.as_bytes());
    stream.write_all(&buf)?;

    let mut buf = [0u8; 68];
    stream.read_exact(&mut buf)?;
    anyhow::ensure!(buf[0] == 19);
    anyhow::ensure!(&buf[1..20] == b"BitTorrent protocol");
    anyhow::ensure!(&buf[28..48] == info_hash);

    Ok(buf[48..].to_owned())
}

pub fn download_piece(
    stream: &mut TcpStream,
    piece_index: u32,
    mut piece_length: u64,
    total_length: u64,
    ouput: &str,
) -> Result<()> {
    let mut file = OpenOptions::new().create(true).append(true).open(ouput)?;

    piece_length = (total_length - piece_length * (piece_index as u64)).min(piece_length);
    let mut offset: u32 = 0;
    while piece_length > 0 {
        let length = piece_length.min(BLOCK_SIZE) as u32;
        piece_length = piece_length.saturating_sub(BLOCK_SIZE);

        let mut payload = Vec::new();
        payload.extend(piece_index.to_be_bytes());
        payload.extend(offset.to_be_bytes());
        payload.extend(length.to_be_bytes());

        let request = Message::new(MessageId::Request, payload);
        stream.write_all(&request.into_bytes())?;

        let piece = Message::from_stream(stream)?;
        anyhow::ensure!(piece.id == MessageId::Piece);
        anyhow::ensure!(piece.payload.len() >= 8);
        anyhow::ensure!(&piece.payload[..4] == piece_index.to_be_bytes());
        anyhow::ensure!(&piece.payload[4..8] == offset.to_be_bytes());

        file.write_all(&piece.payload[8..])?;

        offset += length;
    }
    Ok(())
}

pub fn new_peer_id() -> String {
    rand::rng()
        .sample_iter(&Alphanumeric)
        .take(20)
        .map(char::from)
        .collect()
}

#[derive(PartialEq, Clone, Copy)]
pub enum MessageId {
    Choke = 0,
    Unchoke = 1,
    Interested = 2,
    NotInterested = 3,
    Have = 4,
    Bitfield = 5,
    Request = 6,
    Piece = 7,
    Cancel = 8,
}

pub struct Message {
    pub id: MessageId,
    pub payload: Vec<u8>,
}

impl Message {
    pub fn new(id: MessageId, payload: Vec<u8>) -> Self {
        Self { id, payload }
    }

    pub fn from_stream(stream: &mut TcpStream) -> Result<Self> {
        let mut buf = [0u8; 4];
        stream
            .read_exact(&mut buf)
            .context("failed to read message length")?;

        let length = u32::from_be_bytes(buf);
        anyhow::ensure!(length > 0);

        let mut id = [0u8; 1];
        stream
            .read_exact(&mut id)
            .context("failed to read message id")?;

        let length = length as usize - 1;
        if length == 0 {
            return Ok(Self {
                id: MessageId::try_from(id[0])?,
                payload: Vec::new(),
            });
        }

        let mut payload = vec![0u8; length];
        stream
            .read_exact(&mut payload)
            .context("failed to read message payload")?;

        Ok(Self {
            id: MessageId::try_from(id[0])?,
            payload,
        })
    }

    pub fn into_bytes(self) -> Vec<u8> {
        let length = self.payload.len() as u32 + 1;
        let mut bytes = Vec::new();
        bytes.extend(length.to_be_bytes());
        bytes.push(self.id as u8);
        bytes.extend(self.payload);
        bytes
    }
}

impl TryFrom<u8> for MessageId {
    type Error = anyhow::Error;
    fn try_from(value: u8) -> std::result::Result<Self, Self::Error> {
        match value {
            0 => Ok(Self::Choke),
            1 => Ok(Self::Unchoke),
            2 => Ok(Self::Interested),
            3 => Ok(Self::NotInterested),
            4 => Ok(Self::Have),
            5 => Ok(Self::Bitfield),
            6 => Ok(Self::Request),
            7 => Ok(Self::Piece),
            8 => Ok(Self::Cancel),
            v => anyhow::bail!("Invalid message id: {}", v),
        }
    }
}

fn url_encode(bytes: &[u8]) -> String {
    bytes
        .iter()
        .map(|&b| format!("%{}", hex::encode([b])))
        .collect::<Vec<_>>()
        .join("")
}
