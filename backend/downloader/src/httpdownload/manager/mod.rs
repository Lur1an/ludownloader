mod inner;
mod item;

use crate::httpdownload::download;
use crate::httpdownload::download::{DownloadUpdate, HttpDownload};
use api::proto::DownloadMetadata;
use std::ops::Deref;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::RwLock;
use uuid::Uuid;

use self::inner::Inner;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error while trying to access download in map: {0}")]
    Access(String),
    #[error("Error occurred while downloading: {0}")]
    HttpDownloadError(#[from] download::Error),
    #[error("JoinError for download: {0}")]
    TokioJoinError(#[from] tokio::task::JoinError),
    #[error("Download is not running")]
    DownloadNotRunning,
    #[error("Couldn't acquire Lock for Download: {0}")]
    LockError(#[from] tokio::sync::TryLockError),
}

/// Trait for a struct that can handle DownloadUpdates.
pub trait UpdateConsumer {
    fn consume(&mut self, update: DownloadUpdate);
}

/// This struct takes care of storing/running/stopping downloads.
/// Internally it uses a RwLock to allow for concurrent access,
/// this exposes a thread-safe interface.
/// TODO: Add a way to time out if lock-acquisition takes too long (not expected as it's 1-2 user
/// only anyways)
/// This struct is supposed to be cloned as it uses an Arc internally.
#[derive(Clone, Default)]
pub struct DownloadManager {
    inner: Arc<RwLock<Inner>>,
}

impl DownloadManager {
    /// The update_consumer is placed in a separate thread and will receive all updates from all downloads.
    pub fn new(update_consumer: impl UpdateConsumer + Send + Sync + 'static) -> Self {
        let inner = Arc::new(RwLock::new(Inner::new(update_consumer)));
        let manager = Self { inner };
        manager
    }

    pub async fn start(&self, id: Uuid) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.start(id)
    }

    pub async fn stop(&self, id: Uuid) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.stop(id).await
    }

    pub async fn get_metadata_all(&self) -> Vec<DownloadMetadata> {
        let inner = self.inner.read().await;
        inner.get_metadata_all().await
    }

    pub async fn add(&self, download: HttpDownload) -> Uuid {
        let mut inner = self.inner.write().await;
        let id = inner.add(download);
        id
    }

    pub async fn delete(&self, id: Uuid) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::{file_size, setup_test_download};
    use std::error::Error;
    use test_log::test;
    use tokio::{sync::mpsc::Sender, time};

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";
    type Test<T> = std::result::Result<T, Box<dyn Error>>;

    #[test(tokio::test)]
    async fn start_and_stop_download() -> Test<()> {
        let manager = DownloadManager::default();
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let download_path = download.file_path.clone();
        let id = manager.add(download).await;
        manager.start(id).await?;
        time::sleep(time::Duration::from_secs(1)).await;
        manager.stop(id).await?;
        let downloaded_bytes = file_size(&download_path).await;
        assert_ne!(
            downloaded_bytes, 0,
            "Downloaded bytes should be greater than 0"
        );

        Ok(())
    }
}
