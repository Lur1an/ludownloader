use std::{collections::HashMap, sync::Arc};

use data::types::DownloadState;
use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};
use uuid::Uuid;

use super::{
    download::{self, DownloadUpdate},
    manager::UpdateConsumer,
};

struct Inner {
    cache: HashMap<Uuid, DownloadState>,
}

impl Inner {
    pub fn new() -> Self {
        let inner = Self {
            cache: HashMap::new(),
        };
        inner
    }
}

#[derive(Clone)]
/// This struct contains the state of all managed Downloads
/// DownloadState instances in the observer should match 1:1 with HttpDownload instances in the
/// Manager. The Observer wraps the inner state in an Arc<RwLock> for thread-safe access
/// The inner lock gets acquired by a background thread everytime updates to downloads are flushed
pub struct DownloadObserver {
    inner: Arc<RwLock<Inner>>,
    tx: Sender<Vec<DownloadUpdate>>,
}

impl DownloadObserver {
    pub fn new() -> Self {
        let (tx, mut rx) = tokio::sync::mpsc::channel(4);
        let inner = Arc::new(RwLock::new(Inner::new()));
        let inner_clone = inner.clone();
        tokio::task::spawn(async move {
            while let Some(update) = rx.recv().await {
                let mut guard = inner_clone.write().await;
                todo!()
            }
        });
        Self { inner, tx }
    }
    /// Use this channel to send buffered Vec<DownloadUpdate> to the observer
    /// The observer will himself handle the lock of the Inner struct to update state in a
    /// thread-safe manner
    pub fn get_channel(&self) -> Sender<Vec<DownloadUpdate>> {
        self.tx.clone()
    }
}

pub struct BufferedDownloadConsumer {
    tx: Sender<Vec<DownloadUpdate>>,
    buffer: Vec<DownloadUpdate>,
}

impl BufferedDownloadConsumer {
    pub fn new(buffer_size: usize, tx: Sender<Vec<DownloadUpdate>>) -> Self {
        let buffer = Vec::with_capacity(buffer_size);
        Self { tx, buffer }
    }
}

impl UpdateConsumer for BufferedDownloadConsumer {
    fn consume(&mut self, update: DownloadUpdate) {
        let is_running = matches!(update.update_type, download::UpdateType::Running { .. });
        if self.buffer.capacity() > self.buffer.len() {
            self.buffer.push(update);
        } else {
            self.flush();
            self.buffer.push(update);
        }
        if !is_running {
            self.flush();
        }
    }
}

impl BufferedDownloadConsumer {
    fn flush(&mut self) {
        let buffer_size = self.buffer.capacity();
        let new_buffer: Vec<DownloadUpdate> = Vec::with_capacity(buffer_size);
        let buffered_values = std::mem::replace(&mut self.buffer, new_buffer);
        let ch = self.tx.clone();
        tokio::spawn(async move {
            match ch.send(buffered_values).await {
                Err(e) => {
                    log::error!("Failed sending buffered updates to observer: {}", e);
                }
                Ok(_) => log::info!("Sent buffered updates to observer"),
            }
        });
    }
}

#[cfg(test)]
mod test {
    use crate::util::TestResult;

    use super::*;
    use test_log::test;
    #[test(tokio::test)]
    async fn test_flush_on_update() -> TestResult<()> {
        Ok(())
    }
}
