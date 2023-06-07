use crate::httpdownload::download::{DownloadUpdate, HttpDownload};

use async_trait::async_trait;
use std::collections::HashMap;
use tokio::sync::{RwLock, RwLockWriteGuard};
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

use super::item::DownloaderItem;
use super::{Error, Result, UpdateConsumer};

#[derive(Default)]
struct DefaultUpdateConsumer;

impl UpdateConsumer for DefaultUpdateConsumer {
    fn consume(&mut self, update: DownloadUpdate) {
        log::info!("Update: {:?}", update);
    }
}

impl UpdateConsumer for () {
    fn consume(&mut self, update: DownloadUpdate) {
        todo!()
    }
}

#[derive(Debug)]
pub struct Inner {
    update_ch: mpsc::Sender<DownloadUpdate>,
    _consumer_thread: JoinHandle<()>,
    items: HashMap<Uuid, DownloaderItem>,
}

impl Default for Inner {
    fn default() -> Self {
        let updater = DefaultUpdateConsumer::default();
        Inner::new(updater)
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
            log::error!("Update channel closed, last update_sender has been dropped");
            log::error!(
                "This thread should live as long as the program, so this should never happen."
            );
        });

        Inner {
            _consumer_thread: consumer_thread,
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
