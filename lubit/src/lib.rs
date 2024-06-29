use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use serde_bencode::de;
use serde_bytes::ByteBuf;
use std::io::{self, Read};

#[derive(Debug, Deserialize)]
struct Node(String, i64);

#[derive(Debug, Deserialize)]
struct File {
    pub path: Vec<CompactString>,
    pub length: i64,
    #[serde(default)]
    pub md5sum: Option<CompactString>,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct Info {
    pub name: CompactString,
    pub pieces: ByteBuf,
    #[serde(rename = "piece length")]
    pub piece_length: i64,
    #[serde(default)]
    pub md5sum: Option<CompactString>,
    #[serde(default)]
    pub length: Option<i64>,
    #[serde(default)]
    pub files: Option<Vec<File>>,
    #[serde(default)]
    pub private: Option<u8>,
    #[serde(default)]
    pub path: Option<Vec<CompactString>>,
    #[serde(default)]
    #[serde(rename = "root hash")]
    pub root_hash: Option<CompactString>,
}

#[derive(Debug, Deserialize)]
struct Torrent {
    pub info: Info,
    #[serde(default)]
    pub announce: Option<CompactString>,
    #[serde(default)]
    pub nodes: Option<Vec<Node>>,
    #[serde(default)]
    pub encoding: Option<CompactString>,
    #[serde(default)]
    pub httpseeds: Option<Vec<CompactString>>,
    #[serde(default)]
    #[serde(rename = "announce-list")]
    pub announce_list: Option<Vec<Vec<CompactString>>>,
    #[serde(default)]
    #[serde(rename = "creation date")]
    pub creation_date: Option<i64>,
    #[serde(rename = "comment")]
    pub comment: Option<CompactString>,
    #[serde(default)]
    #[serde(rename = "created by")]
    pub created_by: Option<CompactString>,
}

#[cfg(test)]
mod test {
    use super::*;

    #[tokio::test]
    async fn test_fetch_torrent() {
        let torrent_file = include_bytes!("../../resources/debian.torrent");
        let parsed_torrent = de::from_bytes::<Torrent>(torrent_file).unwrap();
        println!("{:?}", parsed_torrent);
    }
}
