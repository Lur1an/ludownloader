use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use download::httpdownload::{DownloadUpdate, HttpDownload};
use reqwest::Client;
use thiserror::Error;
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Error while trying to access download in map: {0}")]
    DownloadAccess(String),
    #[error("Error occurred while downloading: {0}")]
    DownloadError(#[from] download::Error),
    #[error("JoinError for download: {0}")]
    TokioThreadingError(#[from] tokio::task::JoinError),
}

type Result<T> = std::result::Result<T, Error>;

#[derive(Debug)]
enum Download {
    HttpDownload(Arc<HttpDownload>),
}

impl Download {
    fn id(&self) -> Uuid {
        match self {
            Download::HttpDownload(download) => download.id,
        }
    }
}

#[derive(Debug)]
struct DownloaderItem {
    download: Download,
    handle: Option<(
        JoinHandle<download::Result<u64>>,
        tokio::sync::oneshot::Sender<()>,
    )>,
}

impl DownloaderItem {
    fn new(download: Download) -> Self {
        DownloaderItem {
            download,
            handle: None,
        }
    }

    fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    fn run(&mut self, update_ch: mpsc::Sender<DownloadUpdate>, resume: bool) {
        match &self.download {
            Download::HttpDownload(download) => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let download_arc = download.clone();
                let thread_handle = if resume {
                    tokio::spawn(async move { download_arc.start(rx, update_ch).await })
                } else {
                    tokio::spawn(async move { download_arc.resume(rx, update_ch).await })
                };
                self.handle = Some((thread_handle, tx));
            }
        }
    }

    async fn stop(&mut self) -> Result<u64> {
        todo!()
    }

    async fn complete(&mut self) -> Result<u64> {
        todo!();
        if let Some((handle, tx)) = self.handle.take() {
            let result = handle.await??;
            return Ok(result);
        }
        self.handle = None;
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
        log::trace!("Update: {:?}", update);
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

    fn add(&mut self, download: Download) -> Result<Uuid> {
        let item = DownloaderItem::new(download);
        let id = item.download.id();
        self.items.insert(id, item);
        Ok(id)
    }

    fn start(&mut self, id: Uuid) -> Result<()> {
        if let Some(item) = self.items.get_mut(&id) {
            let update_ch = self.update_ch.clone();
            item.run(update_ch, true);
            Ok(())
        } else {
            Err(Error::DownloadAccess(format!(
                "Download with id {} not found",
                id
            )))
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use reqwest::Url;
    use std::error::Error;
    use tempfile::TempDir;

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.discordapp.net/apps/linux/0.0.26/discord-0.0.26.deb";
    type Test<T> = std::result::Result<T, Box<dyn Error>>;

    #[tokio::test]
    async fn start_download() -> Test<()> {
        let mut manager = DownloadManager::default();
        let tmp_dir = TempDir::new()?;
        let tmp_path = tmp_dir.path();
        let client = Client::new();
        let file_path = tmp_path.join("deez.nuts");
        let download =
            HttpDownload::new(Url::parse(TEST_DOWNLOAD_URL)?, file_path, client, None).await?;
        let download = Download::HttpDownload(Arc::new(download));
        let id = manager.add(download)?;
        manager.start(id)?;
        Ok(())
    }
}
