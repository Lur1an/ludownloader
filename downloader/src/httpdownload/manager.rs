use std::{collections::HashMap, sync::Arc};

use crate::httpdownload::download::{DownloadUpdate, HttpDownload};
use async_trait::async_trait;
use reqwest::Client;
use thiserror::Error;
use tokio::{
    sync::{mpsc, RwLock},
    task::JoinHandle,
};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error while trying to access download in map: {0}")]
    Access(String),
    #[error("Error occurred while downloading: {0}")]
    HttpDownloadError(#[from] crate::httpdownload::download::Error),
    #[error("JoinError for download: {0}")]
    TokioThreadingError(#[from] tokio::task::JoinError),
    #[error("Download is not running")]
    DownloadNotRunning,
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
struct DownloaderItem {
    download: Arc<RwLock<HttpDownload>>,
    handle: Option<(
        JoinHandle<crate::httpdownload::download::Result<u64>>,
        tokio::sync::oneshot::Sender<()>,
    )>,
}

async fn run_download_locked(
    download: Arc<RwLock<HttpDownload>>,
    rx: tokio::sync::oneshot::Receiver<()>,
    update_ch: mpsc::Sender<DownloadUpdate>,
    resume: bool,
) -> super::download::Result<u64> {
    let download = download.read().await;
    let update_ch_cl = update_ch.clone();
    let result = if resume {
        download.resume(rx, update_ch).await
    } else {
        download.start(rx, update_ch).await
    };
    if let Err(e) = &result {
        match e {
            crate::httpdownload::download::Error::Io(_) => todo!(),
            crate::httpdownload::download::Error::Request(_) => todo!(),
            crate::httpdownload::download::Error::MissingContentLength(_) => todo!(),
            crate::httpdownload::download::Error::ChannelDrop(_, _) => todo!(),
            crate::httpdownload::download::Error::DownloadComplete(_) => todo!(),
            crate::httpdownload::download::Error::DownloadNotOk(_, _) => todo!(),
            crate::httpdownload::download::Error::StreamEndedBeforeCompletion(_) => todo!(),
        }
    }
    let _ = update_ch_cl
        .send(DownloadUpdate {
            id: download.id,
            update_type: crate::httpdownload::download::UpdateType::Completed,
        })
        .await;
    result
}

impl DownloaderItem {
    fn new(download: HttpDownload) -> Self {
        DownloaderItem {
            download: Arc::new(RwLock::new(download)),
            handle: None,
        }
    }

    fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    fn run(&mut self, update_ch: mpsc::Sender<DownloadUpdate>, resume: bool) {
        let (tx, rx) = tokio::sync::oneshot::channel();
        let download_arc = self.download.clone();
        let thread_handle =
            tokio::spawn(
                async move { run_download_locked(download_arc, rx, update_ch, resume).await },
            );
        self.handle = Some((thread_handle, tx));
    }

    async fn stop(&mut self) -> Result<u64> {
        if let Some((handle, tx)) = self.handle.take() {
            let _ = tx.send(());
            let result = handle.await??;
            Ok(result)
        } else {
            Err(Error::DownloadNotRunning)
        }
    }

    async fn complete(&mut self) -> Result<u64> {
        if let Some((handle, _tx)) = self.handle.take() {
            let result = handle.await??;
            return Ok(result);
        } else {
            return Err(Error::DownloadNotRunning);
        }
    }
}

#[derive(Debug)]
struct DownloadManager {
    update_ch: mpsc::Sender<DownloadUpdate>,
    client: Client,
    consumer_thread: JoinHandle<()>,
    items: HashMap<Uuid, DownloaderItem>,
}

#[async_trait]
trait UpdateConsumer {
    async fn consume(&self, update: DownloadUpdate);
}

#[derive(Default)]
struct DefaultUpdateConsumer {}

#[async_trait]
impl UpdateConsumer for DefaultUpdateConsumer {
    async fn consume(&self, update: DownloadUpdate) {
        log::info!("Update: {:?}", update);
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        let updater = DefaultUpdateConsumer::default();
        let client = Client::new();
        DownloadManager::new(updater, client)
    }
}

impl DownloadManager {
    pub fn new(
        update_consumer: impl UpdateConsumer + Send + Sync + 'static,
        client: Client,
    ) -> Self {
        let (update_sender, mut update_recv) = mpsc::channel::<DownloadUpdate>(1000);
        log::info!("Spawning update consumer task");
        let consumer_thread = tokio::task::spawn(async move {
            while let Some(update) = update_recv.recv().await {
                update_consumer.consume(update).await;
            }
            log::info!("Update channel closed, last update_sender has been dropped");
        });

        DownloadManager {
            consumer_thread,
            client,
            update_ch: update_sender,
            items: HashMap::new(),
        }
    }

    fn add(&mut self, download: HttpDownload) -> Result<Uuid> {
        log::info!("Adding download: {:?}", download);
        let id = download.id;
        let item = DownloaderItem::new(download);
        self.items.insert(id, item);
        Ok(id)
    }

    fn start(&mut self, id: Uuid) -> Result<()> {
        if let Some(item) = self.items.get_mut(&id) {
            let update_ch = self.update_ch.clone();
            item.run(update_ch, false);
            Ok(())
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
    }

    async fn stop(&mut self, id: Uuid) -> Result<u64> {
        log::info!("Stop action requested for download: {}", id);
        if let Some(mut item) = self.items.remove(&id) {
            log::info!("Stopping download {}", id);
            item.stop().await
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
    }

    async fn complete(&mut self, id: Uuid) -> Result<u64> {
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
    use crate::util::setup_test_download;

    use super::*;
    use reqwest::Url;
    use std::error::Error;
    use tempfile::TempDir;
    use test_log::test;

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";
    type Test<T> = std::result::Result<T, Box<dyn Error>>;

    #[test(tokio::test)]
    async fn start_download() -> Test<()> {
        let mut manager = DownloadManager::default();
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let id = manager.add(download)?;
        manager.start(id)?;
        tokio::time::sleep(tokio::time::Duration::from_secs(1)).await;
        let downloaded_bytes = manager.stop(id).await?;
        assert_ne!(
            downloaded_bytes, 0,
            "Downloaded bytes should be greater than 0"
        );

        Ok(())
    }
}
