use std::sync::Arc;

use tokio::sync::Mutex;

use crate::httpdownload::observer::SendingUpdateConsumer;

use self::{manager::DownloadManager, observer::DownloadObserver};

pub mod download;
pub mod manager;
pub mod observer;

pub trait DownloadSubscriber {}
type Subscribers = Arc<Mutex<Vec<Box<dyn DownloadSubscriber + Send>>>>;

/// Initializes structs needed for the httpdownload module
pub fn init() -> (DownloadManager, DownloadObserver, Subscribers) {
    let update_consumer = SendingUpdateConsumer::new();
    let subscribers = update_consumer.subscribers.clone();
    let manager = DownloadManager::new(update_consumer);
    todo!();
}
