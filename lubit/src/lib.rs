use compact_str::CompactString;
use serde::{Deserialize, Serialize};
use serde_bytes::ByteBuf;

#[derive(Debug, Deserialize)]
pub struct Node(String, i64);

#[derive(Debug, Deserialize)]
pub struct File {
    pub path: Vec<CompactString>,
    pub length: i64,
    #[serde(default)]
    pub md5sum: Option<CompactString>,
}

#[derive(Debug, Deserialize)]
pub struct Info {
    pub name: CompactString,
    /// Concatenated SHA-1 hashes of every piece in the torrent
    pub pieces: ByteBuf,
    #[serde(rename = "piece length")]
    pub piece_length: i64,
    #[serde(default)]
    pub private: Option<u8>,
    #[serde(flatten)]
    pub info_spec: InfoSpec,
}

#[derive(Debug, Deserialize)]
pub struct SingleInfo {
    pub md5sum: Option<CompactString>,
    pub length: u32,
}

#[derive(Debug, Deserialize)]
pub struct DictionaryInfo {
    pub files: Vec<File>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum InfoSpec {
    Single(SingleInfo),
    Dictionary(DictionaryInfo),
}

#[derive(Debug, Deserialize)]
pub struct Torrent {
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
    pub announce_list: Vec<Vec<CompactString>>,
    #[serde(rename = "creation date")]
    pub creation_date: Option<u32>,
    #[serde(rename = "comment")]
    pub comment: Option<CompactString>,
    #[serde(rename = "created by")]
    pub created_by: Option<CompactString>,
}

impl Torrent {
    pub fn info(&self) {
        todo!()
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum TrackerResponse {
    Failed(FailedTrackerResponse),
    Success(SuccessTrackerResponse),
}

#[derive(Debug, Deserialize)]
pub struct SuccessTrackerResponse {}

#[derive(Debug, Deserialize)]
pub struct FailedTrackerResponse {
    #[serde(rename = "failure reason")]
    pub failure_reason: CompactString,
}

#[cfg(test)]
mod test {
    use serde_bencode::de;
    use tokio::net::TcpSocket;

    use super::*;

    #[tokio::test]
    async fn test_fetch_torrent() {
        let torrent_file = include_bytes!("../../resources/debian.torrent");
        let parsed_torrent = de::from_bytes::<Torrent>(torrent_file).unwrap();
        println!("{:?}", parsed_torrent);
    }
}
