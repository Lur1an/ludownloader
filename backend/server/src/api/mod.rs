use downloader::httpdownload::{download, DownloadMetadata};
use serde::{Deserialize, Serialize};

include!(concat!(env!("OUT_DIR"), "/openapi.rs"));

#[derive(Serialize, Deserialize)]
pub struct DownloadData {
    pub metadata: DownloadMetadata,
    pub state: download::State,
}
