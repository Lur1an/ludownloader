use std::sync::Arc;

use async_trait::async_trait;
use data::types::{download_state::State, DownloadState};
use tokio::sync::Mutex;
use uuid::Uuid;

use crate::httpdownload::observer::SendingUpdateConsumer;

use self::{manager::DownloadManager, observer::DownloadObserver};

pub mod download;
pub mod manager;
pub mod observer;

/// This trait is used to subscribe to state updates of downloads
#[async_trait]
pub trait DownloadSubscriber {
    async fn update(&self, updates: Arc<Vec<(Uuid, State)>>);
}
type Subscribers = Arc<Mutex<Vec<Arc<dyn DownloadSubscriber + Send + Sync>>>>;

/// Initializes structs needed for the httpdownload module
pub fn init() -> (DownloadManager, DownloadObserver, Subscribers) {
    let update_consumer = SendingUpdateConsumer::new();
    let subscribers = update_consumer.subscribers.clone();
    let manager = DownloadManager::new(update_consumer);
    let observer = DownloadObserver::new();
    todo!();
}
