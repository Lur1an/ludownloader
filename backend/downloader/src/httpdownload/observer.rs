use async_trait::async_trait;

use super::{
    download::{DownloadUpdate, HttpDownload},
    manager::UpdateConsumer,
};

pub struct Inner {}

pub struct DownloadObserver {}

#[async_trait]
impl UpdateConsumer for DownloadObserver {
    async fn consume(&self, update: DownloadUpdate) {
        todo!()
    }
}
