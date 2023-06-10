use std::{collections::HashMap, sync::Arc};

use data::types::{download_state::State, DownloadState};
use tokio::sync::{
    mpsc::{Receiver, Sender},
    Mutex, RwLock,
};
use uuid::Uuid;

use super::{
    download::{self, DownloadUpdate, UpdateType},
    manager::UpdateConsumer,
    DownloadSubscriber, Subscribers,
};

/// This struct is responsible for keeping the state of all running downloads
pub struct DownloadObserver {}

/// This struct consumes DownloadUpdate and updates an internal state
/// Depending on the update type it will immediately notify subscribers of the event.
/// Also it should periodically send the DownloadState to all subscribers.
pub struct SendingUpdateConsumer {
    pub subscribers: Subscribers,
    cache: HashMap<Uuid, State>,
}

impl SendingUpdateConsumer {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
            cache: HashMap::new(),
        }
    }
    pub async fn add_subscriber(&self, subscriber: Box<dyn DownloadSubscriber + Send>) {
        let mut guard = self.subscribers.lock().await;
        guard.push(subscriber);
    }
}

impl UpdateConsumer for SendingUpdateConsumer {
    fn consume(&mut self, update: DownloadUpdate) {
        match update.update_type {
            UpdateType::Complete => todo!(),
            UpdateType::Paused => todo!(),
            UpdateType::Running {
                bytes_downloaded,
                bytes_per_second,
            } => {}
            UpdateType::Error(_) => todo!(),
        }
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
