use std::sync::Arc;

use api::proto::{self, create_download_response::Response, DownloadPaused};
use axum::{
    extract::State,
    response::IntoResponse,
    routing::{delete, get, post},
    Json, Router,
};
use downloader::{
    httpdownload::{
        download::HttpDownload,
        manager::{self, DownloadManager},
        observer::DownloadObserver,
    },
    util::parse_filename,
};
use prost::Message;
use reqwest::{Client, StatusCode, Url};
use serde_json::json;

use crate::settings;

#[derive(Clone)]
pub struct ApplicationState {
    pub manager: DownloadManager,
    pub observer: DownloadObserver,
    pub subscribers: downloader::httpdownload::Subscribers,
    pub setting_manager: settings::SettingManager,
    pub client: Client,
}

async fn delete_download() {
    todo!()
}

async fn pause_download() {
    todo!()
}

async fn start_download() {
    todo!()
}

async fn resume_download() {
    todo!()
}

async fn get_metadata(state: State<ApplicationState>) -> impl IntoResponse {
    let data = state.manager.get_metadata_all().await;
    let message = Message::encode_to_vec(&data);
    message
}

async fn get_state(state: State<ApplicationState>) -> impl IntoResponse {
    let data = state.observer.get_state_all().await;
    let message = Message::encode_to_vec(&data);
    message
}

async fn create_download(state: State<ApplicationState>, url: String) -> impl IntoResponse {
    let url = match Url::parse(&url) {
        Ok(value) => value,
        Err(e) => {
            let error = format!("Invalid URL: {}", e);
            return (StatusCode::BAD_REQUEST, error.into_bytes());
        }
    };
    let download_directory = state
        .setting_manager
        .read()
        .await
        .default_download_dir
        .clone();
    let file_path = if let Some(file_name) = parse_filename(&url) {
        download_directory.join(file_name)
    } else {
        let error = "Couldn't parse filename from url";
        return (StatusCode::BAD_REQUEST, error.to_owned().into_bytes());
    };
    let download = match HttpDownload::create(url, file_path, state.client.clone(), None).await {
        Ok(d) => d,
        Err(e) => {
            let error = format!("Error creating download: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, error.into_bytes());
        }
    };
    let metadata = download.get_metadata();
    let message = prost::Message::encode_to_vec(&metadata);
    let id = state.manager.add(download).await;
    state
        .observer
        .track(
            id,
            proto::download_state::State::Paused(DownloadPaused {
                bytes_downloaded: 0,
            }),
        )
        .await;
    (StatusCode::CREATED, message)
}

pub fn routes() -> Router<ApplicationState> {
    let app_router = Router::new()
        .route("/", post(create_download))
        .route("/metadata", get(get_metadata))
        .route("/state", get(get_state))
        .route("/:id", delete(delete_download))
        .route("/:id/start", get(start_download))
        .route("/:id/resume", get(resume_download))
        .route("/:id/pause", get(pause_download));
    app_router
}
