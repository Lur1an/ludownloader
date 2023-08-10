use std::{path::PathBuf, sync::Arc};

use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::httpdownload::observer::DownloadUpdatePublisher;

use self::{manager::DownloadManager, observer::DownloadObserver};

pub mod download;
pub mod manager;
pub mod observer;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DownloadMetadata {
    pub id: Uuid,
    pub url: String,
    pub file_path: PathBuf,
    pub content_length: u64,
}

/// This trait is used to subscribe to state updates of downloads
#[async_trait]
pub trait DownloadUpdateBatchSubscriber {
    async fn update(&self, updates: &[(Uuid, download::State)]);
}

// Fuck this type, later on just remove the wrapping Arc<Mutex> and instead create a simple channel
// over which new subscribers are sent, whenever the publisher is ready to publish a new batch he
// first checks the channel for new subscribers which will be added to the internal vector.
pub type Subscribers = Arc<Mutex<Vec<Arc<dyn DownloadUpdateBatchSubscriber + Send + Sync>>>>;

/// Initializes structs needed for the httpdownload module
pub async fn init() -> (DownloadManager, DownloadObserver, Subscribers) {
    let observer = DownloadObserver::new();
    let update_consumer = DownloadUpdatePublisher::new();
    let subscribers = update_consumer.subscribers.clone();
    update_consumer.add_subscriber(observer.clone()).await;
    let manager = DownloadManager::new(update_consumer);
    (manager, observer, subscribers)
}

#[cfg(test)]
mod test {
    use crate::util::{setup_test_download, TestResult};

    use super::*;
    use test_log::test;

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";

    #[test(tokio::test)]
    async fn test_download_with_observability() -> TestResult<()> {
        let (manager, observer, _) = init().await;
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let id = manager.add(download).await;
        observer.track(id, download::State::Paused(0)).await;
        manager.start(&id).await?;
        manager.stop(&id).await?;
        let state = observer.read_state().await;
        let download_state = state.get(&id).unwrap();
        assert!(matches!(download_state, download::State::Paused(_)));
        Ok(())
    }
}
