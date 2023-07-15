use api::proto::{self, DownloadPaused};
use async_trait::async_trait;
use axum::{
    extract::{FromRequestParts, Path, Query, State},
    response::IntoResponse,
    routing::{delete, get, post},
    Router,
};
use downloader::{
    httpdownload::{download::HttpDownload, manager::DownloadManager, observer::DownloadObserver},
    util::{file_size, parse_filename},
};
use prost::Message;
use reqwest::{Client, StatusCode, Url};
use uuid::Uuid;

use crate::settings;

#[derive(Clone)]
pub struct ApplicationState {
    pub manager: DownloadManager,
    pub observer: DownloadObserver,
    pub subscribers: downloader::httpdownload::Subscribers,
    pub setting_manager: settings::SettingManager,
    pub client: Client,
}

async fn delete_download(
    id: Path<Uuid>,
    delete_file: Query<bool>,
    state: State<ApplicationState>,
) -> impl IntoResponse {
    let resp = match state.manager.delete(&id, *delete_file).await {
        Ok(_) => (StatusCode::OK, id.to_string()),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            format!("Couldn't delete Download: {}", e),
        ),
    };
    resp
}

async fn pause_download(state: State<ApplicationState>, id: Path<Uuid>) -> impl IntoResponse {
    let resp = match state.manager.stop(&id).await {
        Ok(_) => (StatusCode::OK, "Success".to_string()),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            format!("Couldn't stop Download: {}", e),
        ),
    };
    resp
}

async fn start_download(state: State<ApplicationState>, id: Path<Uuid>) -> impl IntoResponse {
    match state.manager.start(&id).await {
        Ok(_) => (StatusCode::OK, "Success".to_string()),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            format!("Couldn't start Download: {}", e),
        ),
    }
}

async fn resume_download(state: State<ApplicationState>, id: Path<Uuid>) -> impl IntoResponse {
    match state.manager.resume(&id).await {
        Ok(_) => (StatusCode::OK, "Success".to_string()),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            format!("Couldn't resume Download: {}", e),
        ),
    }
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
    let file_name = if let Some(file_name) = parse_filename(&url) {
        file_name.to_owned()
    } else {
        let error = "Couldn't parse filename from url";
        return (StatusCode::BAD_REQUEST, error.to_owned().into_bytes());
    };
    let download = match HttpDownload::create(
        url,
        download_directory,
        file_name,
        state.client.clone(),
        None,
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            let error = format!("Error creating download: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, error.into_bytes());
        }
    };
    let metadata = download.get_metadata();
    let message = prost::Message::encode_to_vec(&metadata);
    let bytes_downloaded = file_size(&download.file_path()).await;
    let id = state.manager.add(download).await;
    state
        .observer
        .track(
            id,
            proto::download_state::State::Paused(DownloadPaused { bytes_downloaded }),
        )
        .await;
    (StatusCode::CREATED, message)
}

async fn start_all_downloads(state: State<ApplicationState>) {
    state.manager.start_all().await;
}

async fn stop_all_downloads(state: State<ApplicationState>) {
    state.manager.stop_all().await;
}

pub fn routes() -> Router<ApplicationState> {
    let app_router = Router::new()
        .route("/", post(create_download))
        .route("/start_all", get(start_all_downloads))
        .route("/stop_all", get(stop_all_downloads))
        .route("/metadata", get(get_metadata))
        .route("/state", get(get_state))
        .route("/:id", delete(delete_download))
        .route("/:id/start", get(start_download))
        .route("/:id/resume", get(resume_download))
        .route("/:id/pause", get(pause_download));
    app_router
}
