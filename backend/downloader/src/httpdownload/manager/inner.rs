use crate::httpdownload::download::{DownloadUpdate, HttpDownload};

use api::proto::DownloadMetadata;
use std::collections::HashMap;
use std::process::exit;
use tokio::sync::{MutexGuard, RwLockWriteGuard};
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

use super::item::DownloaderItem;
use super::{Error, Result, UpdateConsumer};

impl UpdateConsumer for () {
    fn consume(&mut self, update: DownloadUpdate) {
        log::info!("Update: {:?}", update);
    }
}

#[derive(Debug)]
pub struct Inner {
    pub update_ch: mpsc::Sender<DownloadUpdate>,
    _consumer_thread: JoinHandle<()>,
    pub items: HashMap<Uuid, DownloaderItem>,
}

impl Default for Inner {
    fn default() -> Self {
        Inner::new(())
    }
}

impl Inner {
    pub fn new(mut update_consumer: impl UpdateConsumer + Send + Sync + 'static) -> Self {
        let (update_sender, mut update_recv) = mpsc::channel::<DownloadUpdate>(1000);
        log::info!("Spawning update consumer task");
        let consumer_thread = tokio::task::spawn(async move {
            while let Some(update) = update_recv.recv().await {
                update_consumer.consume(update);
            }
            log::warn!("Update channel closed, last update_sender has been dropped");
            log::error!("Download update consumer thread should live as long as the program, so this should never happen.");
            exit(1);
        });

        Inner {
            _consumer_thread: consumer_thread,
            update_ch: update_sender,
            items: HashMap::new(),
        }
    }

    pub fn add(&mut self, download: HttpDownload) -> Uuid {
        log::info!("Adding download: {:?}", download);
        let id = download.id;
        let item = DownloaderItem::new(download);
        self.items.insert(id, item);
        id
    }

    pub async fn get_metadata_all(&self) -> Vec<DownloadMetadata> {
        let mut result = Vec::with_capacity(self.items.len());
        for item in self.items.values() {
            result.push(item.metadata.clone());
        }
        result
    }

    pub async fn edit(&mut self, id: &Uuid) -> Result<MutexGuard<HttpDownload>> {
        if let Some(item) = self.items.get_mut(id) {
            let guard = item.download.try_lock()?;
            Ok(guard)
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
    }

    pub fn start(&mut self, id: &Uuid) -> Result<()> {
        if let Some(item) = self.items.get_mut(id) {
            let update_ch = self.update_ch.clone();
            if item.is_locked() {
                return Err(Error::Access("Download is already locked, probably running already or locked up by pending operation!".to_owned()));
            }
            item.run(update_ch, false);
            Ok(())
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
    }

    pub fn stop(&mut self, id: &Uuid) -> Result<()> {
        log::info!("Stop action requested for download: {}", id);
        if let Some(item) = self.items.get_mut(id) {
            log::info!("Stopping download {}", id);
            item.stop()
        } else {
            Err(Error::Access(format!("Download with id {} not found", id)))
        }
    }

    pub fn remove(&mut self, id: &Uuid) -> Option<DownloaderItem> {
        log::info!("Removing download: {}", id);
        self.items.remove(id)
    }
}
