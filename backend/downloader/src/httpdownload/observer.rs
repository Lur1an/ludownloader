use std::sync::Arc;

use tokio::sync::{
    mpsc::{Receiver, Sender},
    RwLock,
};

use super::{
    download::{self, DownloadUpdate},
    manager::UpdateConsumer,
};

struct Inner {
    rx: Receiver<Vec<DownloadUpdate>>,
}

#[derive(Clone)]
pub struct DownloadObserver {
    inner: Arc<RwLock<Inner>>,
}

impl DownloadObserver {}

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
        if self.buffer.capacity() > self.buffer.len() {
            let is_running = matches!(update.update_type, download::UpdateType::Running { .. });
            self.buffer.push(update);
            if !is_running {
                self.flush();
            }
        } else {
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
    use super::*;
}
