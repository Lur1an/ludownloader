use std::path::PathBuf;

use async_trait::async_trait;
use uuid::Uuid;

pub mod download;
pub mod manager;

#[derive(Debug, Clone)]
pub struct DownloadMetadata {
    pub id: Uuid,
    pub url: String,
    pub file_path: PathBuf,
    pub download_size: u64,
}

/// This trait is used to subscribe to state updates of downloads
#[async_trait]
pub trait DownloadUpdateSubscriber {
    async fn update(&self, updates: &[(Uuid, download::State)]);
}
