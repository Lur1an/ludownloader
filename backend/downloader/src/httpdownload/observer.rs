use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use async_trait::async_trait;
use data::types::{download_state::State, DownloadPaused, DownloadRunning, DownloadState};
use tokio::{sync::Mutex, time::Instant};
use uuid::Uuid;

use crate::util::HALF_SECOND;

use super::{
    download::{self, DownloadUpdate, UpdateType},
    manager::UpdateConsumer,
    DownloadSubscriber, Subscribers,
};

/// This struct is responsible for keeping the state of all running downloads
/// It's interal state should subscribe to the SendingUpdateConsumer
/// This struct as other higher-order application components should handle synchronization and
/// threading internally and be safe to Clone and pass around.
/// TODO!
#[derive(Clone)]
pub struct DownloadObserver {
    pub inner: Arc<Mutex<HashMap<Uuid, State>>>,
}

impl DownloadObserver {
    pub fn new() -> Self {
        Self {
            inner: Arc::new(Mutex::new(HashMap::new())),
        }
    }
}

#[async_trait]
impl DownloadSubscriber for DownloadObserver {
    async fn update(&self, updates: Arc<Vec<(Uuid, State)>>) {
        log::info!("Updating inner state for DownloadObserver, acquiring lock...");
        let mut guard = self.inner.lock().await;
        log::info!("Lock acquired, updating {} entries...", updates.len());
        for (id, state) in updates.iter() {
            if !guard.contains_key(id) {
                log::warn!("Received an update for a download whose state is not being tracket by the Observer.");
                continue;
            }
            log::info!("Updating state for download {}", id);
            guard.insert(*id, state.clone());
        }
    }
}

/// This struct consumes DownloadUpdate and updates an internal state
/// Depending on the update type it will immediately notify subscribers of the event.
/// Also it should periodically send the DownloadState to all subscribers.
/// Why does this middle-man exist? The DownloadManager is not responsible for keeping track of the
/// internal state of the downloads, it only forwards actions and returns results to/from
/// HttpDownloads; if we tried to just use a DownloadObserver that catches in a background thread
/// all updates from the Manager and shares a State-Map with a Mutex we'd risk filling up the
/// buffer and locking up everything if we read too much from the Observer. This middle man never
/// blocks on locks the consumption of updates, it creates an Arc<Vec> that is sent in a
/// non-blocking manner to all subscribers that then will have to lock up their mutex to update
/// whatever state they have (or no mutex, maybe it's a Struct that holds a Socket connection)
pub struct SendingUpdateConsumer {
    pub subscribers: Subscribers,
    last_flush: Instant,
    cache: HashMap<Uuid, State>,
}

impl SendingUpdateConsumer {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
            cache: HashMap::new(),
            last_flush: Instant::now(),
        }
    }
    pub async fn add_subscriber(
        &self,
        subscriber: impl DownloadSubscriber + Send + 'static + Sync,
    ) {
        let mut guard = self.subscribers.lock().await;
        guard.push(Arc::new(subscriber));
    }
}

impl From<&DownloadUpdate> for State {
    fn from(value: &DownloadUpdate) -> Self {
        match &value.update_type {
            UpdateType::Complete => State::Complete(true),
            UpdateType::Paused(bytes_downloaded) => State::Paused(DownloadPaused {
                bytes_downloaded: *bytes_downloaded,
            }),
            UpdateType::Running {
                bytes_downloaded,
                bytes_per_second,
            } => State::Running(DownloadRunning {
                bytes_downloaded: *bytes_downloaded,
                bytes_per_second: *bytes_per_second,
            }),
            UpdateType::Error(e) => match e {
                _ => State::Error(e.to_string()),
            },
        }
    }
}

impl UpdateConsumer for SendingUpdateConsumer {
    fn consume(&mut self, update: DownloadUpdate) {
        let flush = self.last_flush.elapsed() > HALF_SECOND
            && !matches!(update.update_type, UpdateType::Running { .. });
        let state = State::from(&update);
        self.cache.insert(update.id, state);
        // If more than HALF_SECOND has elapsed or the download triggered an event
        // The cached state is flushed to all subscribers, this operation shouldn't block the
        // thread that called consume for too long (just the time to create an update array, wrap
        // it in Arc and spawn the tokio task).
        if flush {
            let updates = Arc::new(self.cache.drain().collect::<Vec<_>>());
            let subscribers = self.subscribers.clone();
            tokio::task::spawn(async move {
                log::info!(
                    "Flushing updates from SendingUpdateConsumer to subscribers! Acquiring Lock..."
                );
                let guard = subscribers.lock().await;
                log::info!("Lock on subscribers acquired! Spawning update sender threads...");
                guard.iter().for_each(|subscriber| {
                    let subscriber = subscriber.clone();
                    tokio::task::spawn_local({
                        let updates = updates.clone();
                        async move {
                            log::info!("Sending updates to subscriber!");
                            subscriber.update(updates).await;
                        }
                    });
                });
            });
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
