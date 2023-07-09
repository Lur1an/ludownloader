use std::sync::Arc;

use api::proto::{download_state::State, DownloadState};
use async_trait::async_trait;
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
    use api::proto::DownloadPaused;
    use test_log::test;

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";

    #[test(tokio::test)]
    async fn test_download_with_observability() -> TestResult<()> {
        let (manager, observer, _) = init().await;
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let id = manager.add(download).await;
        observer
            .track(
                id,
                State::Paused(DownloadPaused {
                    bytes_downloaded: 0,
                }),
            )
            .await;

        manager.start(id).await?;
        tokio::time::sleep(tokio::time::Duration::from_secs(10)).await;
        manager.stop(id).await?;
        log::info!("Observer state: {:?}", observer.read_state().await);
        Ok(())
    }
}
