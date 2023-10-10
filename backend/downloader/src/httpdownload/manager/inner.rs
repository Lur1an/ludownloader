use crate::httpdownload::download::{DownloadUpdate, HttpDownload};
use crate::httpdownload::DownloadMetadata;

use anyhow::anyhow;
use std::collections::HashMap;
use std::process::exit;
use tokio::sync::MutexGuard;
use tokio::{sync::mpsc, task::JoinHandle};
use uuid::Uuid;

use super::item::DownloaderItem;
use super::{Result, UpdateConsumer};

impl UpdateConsumer for () {
    fn consume(&mut self, update: DownloadUpdate) {
        log::info!("Update: {:?}", update);
    }
}

#[derive(Debug)]
pub struct Inner {
    pub update_ch: mpsc::Sender<DownloadUpdate>,
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
        tokio::task::spawn(async move {
            while let Some(update) = update_recv.recv().await {
                update_consumer.consume(update);
            }
            log::warn!("Update channel closed, last update_sender has been dropped");
            log::error!("Download update consumer thread should live as long as the program, so this should never happen.");
            exit(1);
        });

        Inner {
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

    pub fn get_metadata(&self, id: &Uuid) -> Result<DownloadMetadata> {
        if let Some(item) = self.items.get(id) {
            Ok(item.metadata.clone())
        } else {
            Err(anyhow!("Download with id {} does not exist", id))
        }
    }

    pub fn get_metadata_all(&self) -> Vec<DownloadMetadata> {
        let mut result = Vec::with_capacity(self.items.len());
        for item in self.items.values() {
            result.push(item.metadata.clone());
        }
        result
    }

    pub fn start_all(&mut self) {
        log::info!("Start/Resume all {} downloads", self.items.len());
        for (id, item) in self.items.iter_mut() {
            if item.is_locked() {
                log::info!("HttpDownload: {} is locked, skipping...", id);
                continue;
            }
            log::info!("Starting download: {}", id);
            item.run(self.update_ch.clone(), true);
        }
    }

    pub fn stop_all(&mut self) {
        log::info!("Stopping all {} downloads", self.items.len());
        for (id, item) in self.items.iter_mut() {
            log::info!("Stopping download: {}", id);
            let _ = item.stop();
        }
    }

    #[allow(dead_code)]
    pub async fn edit(&mut self, id: &Uuid) -> Result<MutexGuard<HttpDownload>> {
        if let Some(item) = self.items.get_mut(id) {
            let guard = item.download.try_lock()?;
            Ok(guard)
        } else {
            Err(anyhow!("Download with id {} not found", id))
        }
    }

    pub fn run(&mut self, id: &Uuid, resume: bool) -> Result<()> {
        if let Some(item) = self.items.get_mut(id) {
            let update_ch = self.update_ch.clone();
            if item.is_locked() {
                return Err(anyhow!("Download is already locked, probably running already or locked up by pending operation!"));
            }
            item.run(update_ch, resume);
            Ok(())
        } else {
            Err(anyhow!("Download with id {} not found", id))
        }
    }

    pub fn stop(&mut self, id: &Uuid) -> Result<()> {
        log::info!("Stop action requested for download: {}", id);
        if let Some(item) = self.items.get_mut(id) {
            log::info!("Stopping download {}", id);
            item.stop()
        } else {
            Err(anyhow!("Download with id {} not found", id))
        }
    }

    pub fn remove(&mut self, id: &Uuid) -> Option<DownloaderItem> {
        log::info!("Removing download: {}", id);
        self.items.remove(id)
    }
}
