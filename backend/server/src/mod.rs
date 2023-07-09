use axum::{
    extract::State,
    http::Response,
    response::IntoResponse,
    routing::{delete, post},
    Router,
};
use downloader::httpdownload::{
    manager::{self, DownloadManager},
    observer::DownloadObserver,
};
use reqwest::{Client, StatusCode};

use crate::settings;

#[derive(Clone)]
pub struct ApplicationState {
    pub manager: DownloadManager,
    pub observer: DownloadObserver,
    pub subscribers: downloader::httpdownload::Subscribers,
    pub setting_manager: settings::SettingManager,
    pub client: Client,
}

#[derive(Debug)]
struct ManagerResponse<T: IntoResponse>(manager::Result<T>);

async fn delete_download() -> ManagerResponse<()> {
    todo!()
}

async fn pause_download() -> ManagerResponse<()> {
    todo!()
}

async fn resume_download() -> ManagerResponse<()> {
    todo!()
}

async fn create_download(url: String) -> ManagerResponse<()> {
    todo!()
}

impl IntoResponse for ManagerResponse<()> {
    fn into_response(self) -> axum::response::Response {
        let mut resp = String::from("ok").into_response();
        *resp.status_mut() = StatusCode::CREATED;
        resp
    }
}

pub fn routes(state: ApplicationState) -> Router {
    let app_router = Router::new()
        .with_state(state)
        .route("/httpdownload", post(create_download))
        .route("/httpdownload/:id", delete(pause_download));
    app_router
}
