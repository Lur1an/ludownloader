use std::sync::Arc;

use async_trait::async_trait;
use data::types::{download_state::State, DownloadState};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::httpdownload::observer::DownloadUpdatePublisher;

use self::{manager::DownloadManager, observer::DownloadObserver};

pub mod download;
pub mod manager;
pub mod observer;

/// This trait is used to subscribe to state updates of downloads
#[async_trait]
pub trait DownloadUpdateBatchSubscriber {
    async fn update(&self, updates: &Vec<(Uuid, State)>);
}

type Subscribers = Arc<Mutex<Vec<Arc<dyn DownloadUpdateBatchSubscriber + Send + Sync>>>>;

/// Initializes structs needed for the httpdownload module
pub async fn init() -> (DownloadManager, DownloadObserver) {
    let observer = DownloadObserver::new();
    let update_consumer = DownloadUpdatePublisher::new();
    update_consumer.add_subscriber(observer.clone()).await;
    let manager = DownloadManager::new(update_consumer);
    (manager, observer)
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
        let (manager, _observer) = init().await;
        let (download, _tmp_dir) = crate::util::setup_test_download(TEST_DOWNLOAD_URL).await?;
        let id = manager.add(download).await?;
        manager.start(id).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(5)).await;
        manager.stop(id).await?;
        log::info!("Observer state: {:?}", _observer.get_state().await);
        Ok(())
    }
}
