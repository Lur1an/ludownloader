mod item;

use crate::httpdownload::download;
use crate::httpdownload::download::{DownloadUpdate, HttpDownload};
use async_trait::async_trait;
use std::collections::HashMap;
use std::sync::Arc;
use thiserror::Error;
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

use self::item::DownloaderItem;

pub type Result<T> = std::result::Result<T, Error>;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error while trying to access download in map: {0}")]
    Access(String),
    #[error("Error occurred while downloading: {0}")]
    HttpDownloadError(#[from] download::Error),
    #[error("JoinError for download: {0}")]
    TokioThreadingError(#[from] tokio::task::JoinError),
    #[error("Download is not running")]
    DownloadNotRunning,
    #[error("Couldn't acquire Lock for Download: {0}")]
    LockError(#[from] tokio::sync::TryLockError),
}

struct DownloadManager {
    inner: Arc<RwLock<Inner>>,
}

#[derive(Debug)]
pub struct Inner {
    update_ch: mpsc::Sender<DownloadUpdate>,
    consumer_thread: JoinHandle<()>,
    items: HashMap<Uuid, DownloaderItem>,
}

#[async_trait]
pub trait UpdateConsumer {
    async fn consume(&self, update: DownloadUpdate);
}

#[derive(Default)]
struct DefaultUpdateConsumer;

#[async_trait]
impl UpdateConsumer for DefaultUpdateConsumer {
    async fn consume(&self, update: DownloadUpdate) {
        log::info!("Update: {:?}", update);
    }
}

impl Default for Inner {
    fn default() -> Self {
        let updater = DefaultUpdateConsumer::default();
        Inner::new(updater)
    }
}

impl Inner {
    pub fn new(update_consumer: impl UpdateConsumer + Send + Sync + 'static) -> Self {
        let (update_sender, mut update_recv) = mpsc::channel::<DownloadUpdate>(1000);
        log::info!("Spawning update consumer task");
        let consumer_thread = tokio::task::spawn(async move {
            while let Some(update) = update_recv.recv().await {
                update_consumer.consume(update).await;
            }
            log::info!("Update channel closed, last update_sender has been dropped");
        });

        Inner {
            consumer_thread,
            update_ch: update_sender,
            items: HashMap::new(),
        }
    }

    pub fn add(&mut self, download: HttpDownload) -> Result<Uuid> {
        log::info!("Adding download: {:?}", download);
        let id = download.id;
        let item = DownloaderItem::new(download);
        self.items.insert(id, item);
        Ok(id)
    }

    pub async fn edit(&mut self, id: Uuid) -> Result<RwLockWriteGuard<HttpDownload>> {
        if let Some(item) = self.items.get_mut(&id) {
            let guard = item.download.try_write()?;
            Ok(guard)
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
    }

    pub fn start(&mut self, id: Uuid) -> Result<()> {
        if let Some(item) = self.items.get_mut(&id) {
            let update_ch = self.update_ch.clone();
            item.run(update_ch, false);
            Ok(())
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
    }

    pub async fn stop(&mut self, id: Uuid) -> Result<()> {
        log::info!("Stop action requested for download: {}", id);
        if let Some(mut item) = self.items.remove(&id) {
            log::info!("Stopping download {}", id);
            item.stop().await
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
    }

    pub async fn complete(&mut self, id: Uuid) -> Result<()> {
        log::info!("Complete action requested for download: {}", id);
        if let Some(mut item) = self.items.remove(&id) {
            log::info!("Running download {} to completion.", id);
            item.complete().await
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
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
    async fn start_download() -> Test<()> {
        let mut manager = DownloadManager::default();
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let download_path = download.file_path.clone();
        let id = manager.add(download)?;
        manager.start(id)?;
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
