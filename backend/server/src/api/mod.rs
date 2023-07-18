use downloader::httpdownload::{download, DownloadMetadata};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
pub struct DownloadData {
    pub metadata: DownloadMetadata,
    pub state: download::State,
}
