use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use download::httpdownload::{DownloadUpdate, HttpDownload};
use thiserror::Error;
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Error {
    #[error("Download does not exist")]
    DownloadDoesNotExist,
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
    pub download: Download,
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

    fn start(&mut self, update_ch: mpsc::Sender<DownloadUpdate>) {
        match &self.download {
            Download::HttpDownload(download) => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let download_arc = download.clone();

                let handle = tokio::spawn(async move { download_arc.start(rx, update_ch).await });
                self.handle = Some((handle, tx));
            }
        }
    }
}

#[derive(Debug)]
struct DownloadManager {
    update_ch: mpsc::Sender<DownloadUpdate>,
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
        println!("Update: {:?}", update);
    }
}

impl Default for DownloadManager {
    fn default() -> Self {
        let updater = DefaultUpdateConsumer::default();
        DownloadManager::new(updater)
    }
}

impl DownloadManager {
    fn new(update_consumer: impl UpdateConsumer + Send + Sync + 'static) -> Self {
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
            update_ch: update_sender,
            items: HashMap::new(),
        }
    }

    fn add(&mut self, download: Download) -> Uuid {
        let item = DownloaderItem::new(download);
        let id = item.download.id();
        self.items.insert(id, item);
        id
    }

    fn start(&mut self, id: Uuid) -> Result<()> {
        if let Some(item) = self.items.get_mut(&id) {
            item.start(self.update_ch.clone());
            Ok(())
        } else {
            Err(Error::DownloadDoesNotExist)
        }
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use std::error::Error;

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.discordapp.net/apps/linux/0.0.26/discord-0.0.26.deb";
    type Test<T> = std::result::Result<T, Box<dyn Error>>;

    #[tokio::test]
    async fn add_download() -> Test<()> {
        let mut manager = DownloadManager::default();
        Ok(())
    }
}
