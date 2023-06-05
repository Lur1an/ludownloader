use std::{collections::HashMap, str::FromStr, sync::Arc};

use crate::data::DownloadState;
use crate::settings::SettingManager;
use async_trait::async_trait;
use axum::{
    routing::{get, post},
    Router,
};
use downloader::httpdownload::{
    download::{DownloadUpdate, HttpDownload},
    manager::{DownloadManager, UpdateConsumer},
};
use uuid::Uuid;

#[derive(Clone)]
pub struct HttpDownloadStateContainer {
    pub downloads: HashMap<Uuid, DownloadState>,
}

#[async_trait]
impl UpdateConsumer for HttpDownloadStateContainer {
    fn consume(&self, update: DownloadUpdate) {
        todo!()
    }
}

impl From<&HttpDownload> for DownloadState {
    fn from(value: &HttpDownload) -> Self {
        todo!()
    }
}

#[derive(Clone)]
pub struct Http {
    pub download_manager: DownloadManager,
    pub state_manager: Arc<HttpDownloadStateContainer>,
}

#[derive(Clone)]
pub struct ApplicationState {
    pub settings: SettingManager,
    pub http: Http,
}

impl ApplicationState {
    pub fn new() -> Self {
        todo!()
    }
}

pub fn routes(state: ApplicationState) -> Router {
    let app_router = Router::new().with_state(state);
    app_router
}
