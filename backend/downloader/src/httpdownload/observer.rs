use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use api::proto::{
    download_state::State, DownloadComplete, DownloadPaused, DownloadRunning, DownloadState,
    StateBatch,
};
use async_trait::async_trait;
use tokio::{
    sync::{Mutex, MutexGuard, RwLock, RwLockReadGuard},
    time::Instant,
};
use uuid::Uuid;

use crate::util::HALF_SECOND;

use super::{
    download::{DownloadUpdate, UpdateType},
    manager::UpdateConsumer,
    DownloadUpdateBatchSubscriber, Subscribers,
};

/// This struct is responsible for keeping the state of all running downloads
/// It's interal state should subscribe to the SendingUpdateConsumer
/// This struct as other higher-order application components should handle synchronization and
/// threading internally and be safe to Clone and pass around.
#[derive(Clone)]
pub struct DownloadObserver {
    pub state: Arc<RwLock<HashMap<Uuid, State>>>,
}

impl DownloadObserver {
    pub fn new() -> Self {
        Self {
            state: Arc::new(RwLock::new(HashMap::new())),
        }
    }
    pub async fn read_state(&self) -> RwLockReadGuard<HashMap<Uuid, State>> {
        self.state.read().await
    }

    pub async fn get_state_all(&self) -> StateBatch {
        let guard = self.state.read().await;
        let value = guard
            .iter()
            .map(|(id, state)| DownloadState {
                uuid: id.as_bytes().to_vec(),
                state: Some(state.clone()),
            })
            .collect::<Vec<_>>();
        StateBatch { value }
    }

    pub async fn get_state(&self, id: &Uuid) -> DownloadState {
        let guard = self.state.read().await;
        return DownloadState {
            uuid: id.as_bytes().to_vec(),
            state: guard.get(id).cloned(),
        };
    }

    pub async fn track(&self, id: Uuid, state: State) {
        let mut guard = self.state.write().await;
        guard.insert(id, state);
    }
}

#[async_trait]
impl DownloadUpdateBatchSubscriber for DownloadObserver {
    async fn update(&self, updates: &Vec<(Uuid, State)>) {
        log::info!("Updating inner state for DownloadObserver, acquiring lock...");
        let mut guard = self.state.write().await;
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

/// This struct consumes DownloadUpdate and updates an internal state that is aggregating all
/// states of the recorded downloads.
/// Depending on the update type it will immediately notify subscribers of the event.
/// Why does this middle-man exist? The DownloadManager is not responsible for keeping track of the
/// internal state of the downloads, it only forwards events to HttpDownloads;
/// if we tried to just use a DownloadObserver that catches in a background thread
/// all updates from the Manager and shares a State-Map with a Mutex we'd risk filling up the
/// buffer and locking up everything if we read too much from the Observer. This middle man never
/// blocks during the consumption of updates, it creates an Arc<Vec> of the aggregated state that is sent in a
/// non-blocking manner to all subscribers that then will have to consume the updates
pub struct DownloadUpdatePublisher {
    pub subscribers: Subscribers,
    last_flush: Instant,
    cache: HashMap<Uuid, State>,
}

impl DownloadUpdatePublisher {
    pub fn new() -> Self {
        Self {
            subscribers: Arc::new(Mutex::new(Vec::new())),
            cache: HashMap::new(),
            last_flush: Instant::now(),
        }
    }
    pub async fn add_subscriber(
        &self,
        subscriber: impl DownloadUpdateBatchSubscriber + Send + 'static + Sync,
    ) {
        let mut guard = self.subscribers.lock().await;
        guard.push(Arc::new(subscriber));
    }
}

impl From<&DownloadUpdate> for State {
    fn from(value: &DownloadUpdate) -> Self {
        match &value.update_type {
            UpdateType::Complete => State::Complete(DownloadComplete {}),
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

impl UpdateConsumer for DownloadUpdatePublisher {
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
                    tokio::spawn({
                        let updates = updates.clone();
                        async move {
                            log::info!("Sending updates to subscriber!");
                            subscriber.update(&updates).await;
                        }
                    });
                });
            });
        }
    }
}
