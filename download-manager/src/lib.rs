use std::{collections::HashMap, sync::Arc};

use download::httpdownload::{DownloadUpdate, HttpDownload};
use thiserror::Error;
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

#[derive(Debug, Error)]
pub enum Error {}

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

    fn start(&mut self) {
        match &self.download {
            Download::HttpDownload(download) => {
                let (tx, rx) = tokio::sync::oneshot::channel();
                let download_arc = download.clone();
                let (update_sender, _) = mpsc::channel(1000);

                let handle =
                    tokio::spawn(async move { download_arc.start(rx, update_sender).await });
                self.handle = Some((handle, tx));
            }
        }
    }
}

#[derive(Debug)]
struct DownloadManager {
    update_receiver: mpsc::Receiver<DownloadUpdate>,
    update_sender: mpsc::Sender<DownloadUpdate>,
    items: HashMap<Uuid, DownloaderItem>,
}

impl DownloadManager {
    fn new() -> Self {
        let (update_sender, update_receiver) = mpsc::channel::<DownloadUpdate>(1000);
        DownloadManager {
            update_receiver,
            update_sender,
            items: HashMap::new(),
        }
    }

    fn add(&mut self, download: Download) {
        let item = DownloaderItem::new(download);
        self.items.insert(item.download.id(), item);
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
        let mut manager = DownloadManager::new();
        Ok(())
    }
}
