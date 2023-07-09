use std::sync::Arc;

use api::proto::CreateDownloadError;
use axum::{
    extract::State,
    http::Response,
    response::IntoResponse,
    routing::{delete, post},
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

async fn resume_download() {
    todo!()
}

async fn create_download(state: State<ApplicationState>, url: String) -> impl IntoResponse {
    let url = match Url::parse(&url) {
        Ok(value) => value,
        Err(e) => {
            let error = CreateDownloadError {
                error: format!("Invalid URL: {}", e),
            };
            let message = prost::Message::encode_to_vec(&error);
            return (StatusCode::BAD_REQUEST, message);
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
        let error = CreateDownloadError {
            error: "Couldn't parse filename from url".to_owned(),
        };
        let message = prost::Message::encode_to_vec(&error);
        return (StatusCode::BAD_REQUEST, message);
    };
    let download = match HttpDownload::create(url, file_path, state.client.clone(), None).await {
        Ok(d) => d,
        Err(e) => {
            let error = CreateDownloadError {
                error: format!("Error creating download: {}", e),
            };
            let message = prost::Message::encode_to_vec(&error);
            return (StatusCode::INTERNAL_SERVER_ERROR, message);
        }
    };
    let metadata = download.get_metadata();
    let body = prost::Message::encode_to_vec(&metadata);
    state.manager.add(download).await;
    (StatusCode::CREATED, body)
}

pub fn routes() -> Router<ApplicationState> {
    let app_router = Router::new()
        .route("/", post(create_download))
        .route("/:id", delete(delete_download));
    app_router
}
