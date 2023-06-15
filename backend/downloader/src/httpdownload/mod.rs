use std::sync::Arc;

use async_trait::async_trait;
use data::types::{download_state::State, DownloadState};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::httpdownload::observer::SendingUpdateConsumer;

use self::{manager::DownloadManager, observer::DownloadObserver};

pub mod download;
pub mod manager;
pub mod observer;

/// This trait is used to subscribe to state updates of downloads
#[async_trait]
pub trait DownloadSubscriber {
    async fn update(&self, updates: Arc<Vec<(Uuid, State)>>);
}
type Subscribers = Arc<Mutex<Vec<Arc<dyn DownloadSubscriber + Send + Sync>>>>;

/// Initializes structs needed for the httpdownload module
pub fn init() -> (DownloadManager, DownloadObserver, Subscribers) {
    let update_consumer = SendingUpdateConsumer::new();
    let subscribers = update_consumer.subscribers.clone();
    let manager = DownloadManager::new(update_consumer);
    let observer = DownloadObserver::new();
    (manager, observer, subscribers)
}

#[cfg(test)]
mod test {
    use crate::util::TestResult;

    use super::*;
    use test_log::test;

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";
    #[test(tokio::test)]
    async fn test_download_with_observability() -> TestResult<()> {
        let (manager, observer, _subscribers) = init();
        let (download, _tmp_dir) = crate::util::setup_test_download(TEST_DOWNLOAD_URL).await?;
        let id = manager.add(download).await?;
        manager.start(id).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        Ok(())
    }
}
