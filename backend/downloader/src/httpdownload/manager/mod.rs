mod inner;
mod item;

use crate::httpdownload::download;
use crate::httpdownload::download::{DownloadUpdate, HttpDownload};
use api::proto::{DownloadMetadata, MetadataBatch};
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

    pub async fn start(&self, id: &Uuid) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.run(id, false)
    }

    pub async fn resume(&self, id: &Uuid) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.run(id, true)
    }

    pub async fn stop(&self, id: &Uuid) -> Result<()> {
        let mut inner = self.inner.write().await;
        inner.stop(id)
    }

    pub async fn start_all(&self) {
        let mut inner = self.inner.write().await;
        inner.start_all()
    }

    pub async fn stop_all(&self) {
        let mut inner = self.inner.write().await;
        inner.stop_all()
    }

    pub async fn get_metadata(&self, id: &Uuid) -> Result<DownloadMetadata> {
        let inner = self.inner.read().await;
        inner.get_metadata(id)
    }

    pub async fn get_metadata_all(&self) -> MetadataBatch {
        let inner = self.inner.read().await;
        MetadataBatch {
            value: inner.get_metadata_all(),
        }
    }

    pub async fn add(&self, download: HttpDownload) -> Uuid {
        let mut inner = self.inner.write().await;
        let id = inner.add(download);
        id
    }

    pub async fn delete(&self, id: &Uuid, delete_file: bool) -> Result<()> {
        let mut inner = self.inner.write().await;
        // Download id does not exist case
        if let Err(Error::Access(e)) = inner.stop(id) {
            return Err(Error::Access(e));
        }
        if let Some(item) = inner.remove(id) {
            if delete_file {
                let file_path = item.metadata.file_path;
                match tokio::fs::remove_file(file_path).await {
                    Err(e) => log::warn!(
                        "Couldn't delete file for httpdownload after removing from manager: {}",
                        e
                    ),
                    _ => (),
                };
            }
        };
        Ok(())
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::{file_size, setup_test_download};
    use std::error::Error;
    use test_log::test;
    use tokio::time;

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";
    type Test<T> = std::result::Result<T, Box<dyn Error>>;

    #[test(tokio::test)]
    async fn start_stop_delete_download() -> Test<()> {
        let manager = DownloadManager::default();
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let download_path = download.file_path();
        let id = manager.add(download).await;
        manager.start(&id).await?;
        // check metadata as expected
        let metadata = manager.get_metadata_all().await.value;
        assert_eq!(metadata.len(), 1, "There should be one download");
        let metadata = &metadata[0];
        assert_eq!(Uuid::from_slice(&metadata.uuid).unwrap(), id);
        time::sleep(time::Duration::from_secs(1)).await;
        manager.stop(&id).await?;
        // check size of downloaded file
        let downloaded_bytes = file_size(&download_path).await;
        assert_ne!(
            downloaded_bytes, 0,
            "Downloaded bytes should be greater than 0"
        );
        // test deletion
        manager.delete(&id, true).await?;
        let downloaded_bytes = file_size(&download_path).await;
        assert_eq!(
            downloaded_bytes, 0,
            "Download file should not exist on disk anymore"
        );
        Ok(())
    }
}
