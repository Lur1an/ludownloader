pub mod httpdownload;

use axum::{
    extract::{Path, Query, State},
    response::{IntoResponse, Result},
    routing::{delete, get, post},
    Json, Router,
};
use downloader::httpdownload::{download, DownloadMetadata};
use downloader::{
    httpdownload::{download::HttpDownload, manager::DownloadManager, observer::DownloadObserver},
    util::{file_size, parse_filename},
};
use reqwest::{StatusCode, Url};

use serde_json::{json, Value};
use uuid::Uuid;

use crate::{api::DownloadData, ApplicationState};

fn json_error(message: String) -> Json<Value> {
    Json(json!({ "error": message }))
}

async fn delete_download(
    id: Path<Uuid>,
    delete_file: Query<bool>,
    State(manager): State<DownloadManager>,
) -> impl IntoResponse {
    match manager.delete(&id, *delete_file).await {
        Ok(_) => (StatusCode::OK, Json(json!({"id": id.to_string()}))),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            json_error(format!("Couldn't delete Download: {}", e)),
        ),
    }
}

async fn pause_download(
    State(manager): State<DownloadManager>,
    id: Path<Uuid>,
) -> impl IntoResponse {
    match manager.stop(&id).await {
        Ok(_) => (StatusCode::OK, Json(Value::default())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            json_error(format!("Couldn't stop Download: {}", e)),
        ),
    }
}

async fn start_download(manager: State<DownloadManager>, id: Path<Uuid>) -> impl IntoResponse {
    match manager.start(&id).await {
        Ok(_) => (StatusCode::OK, Json(Value::default())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            json_error(format!("Couldn't start Download: {}", e)),
        ),
    }
}

async fn resume_download(
    State(manager): State<DownloadManager>,
    id: Path<Uuid>,
) -> impl IntoResponse {
    match manager.resume(&id).await {
        Ok(_) => (StatusCode::OK, Json(Value::default())),
        Err(e) => (
            StatusCode::BAD_REQUEST,
            json_error(format!("Couldn't resume Download: {}", e)),
        ),
    }
}

async fn get_metadata(state: State<ApplicationState>) -> impl IntoResponse {
    let data = state.download_manager.get_metadata_all().await;
    Json(data)
}

async fn get_state(State(observer): State<DownloadObserver>) -> impl IntoResponse {
    let data = observer.get_state_all().await;
    Json(data)
}

async fn create_download(
    State(ApplicationState {
        download_manager,
        setting_manager,
        observer,
        client,
        ..
    }): State<ApplicationState>,
    url: String,
) -> Result<(StatusCode, Json<DownloadMetadata>), (StatusCode, Json<Value>)> {
    let url = match Url::parse(&url) {
        Ok(u) => u,
        Err(e) => {
            let error = format!("Invalid URL, couldn't parse: {}", e);
            return Err((StatusCode::BAD_REQUEST, json_error(error)));
        }
    };
    let download_directory = setting_manager.read().await.default_download_dir.clone();
    let mut file_name = if let Some(file_name) = parse_filename(&url) {
        file_name.to_owned()
    } else {
        let error = "Couldn't parse filename from url";
        return Err((StatusCode::BAD_REQUEST, json_error(error.to_owned())));
    };

    if tokio::fs::try_exists(download_directory.join(&file_name))
        .await
        .unwrap_or(false)
    {
        file_name = format!("{}-{}", Uuid::new_v4(), file_name);
    }

    let download = match HttpDownload::create(
        url,
        download_directory,
        file_name,
        client.clone(),
        None,
    )
    .await
    {
        Ok(d) => d,
        Err(e) => {
            let error = format!("Error creating download: {}", e);
            return Err((StatusCode::INTERNAL_SERVER_ERROR, json_error(error)));
        }
    };
    let metadata = download.get_metadata();
    let bytes_downloaded = file_size(&download.file_path()).await;
    let id = download_manager.add(download).await;
    observer
        .track(id, download::State::Paused(bytes_downloaded))
        .await;
    Ok((StatusCode::CREATED, Json(metadata)))
}

async fn get_download(
    id: Path<Uuid>,
    state: State<ApplicationState>,
) -> Result<(StatusCode, Json<DownloadData>), (StatusCode, Json<Value>)> {
    let metadata = match state.download_manager.get_metadata(&id).await {
        Ok(value) => value,
        Err(e) => {
            let error = format!("Error getting download_metadata: {}", e);
            return Err((StatusCode::BAD_REQUEST, json_error(error)));
        }
    }
    .into();
    let state = match state.observer.get_state(&id).await {
        Some(v) => v,
        None => {
            let error = format!("Error getting download_state: {}", *id);
            return Err((StatusCode::BAD_REQUEST, json_error(error)));
        }
    }
    .into();
    let message = Json(DownloadData { state, metadata });
    Ok((StatusCode::OK, message))
}

async fn start_all_downloads(State(download_manager): State<DownloadManager>) {
    download_manager.start_all().await;
}

async fn stop_all_downloads(State(download_manager): State<DownloadManager>) {
    download_manager.stop_all().await;
}

pub fn routes() -> Router<ApplicationState> {
    Router::new()
        .route("/", post(create_download))
        .route("/start_all", get(start_all_downloads))
        .route("/stop_all", get(stop_all_downloads))
        .route("/metadata", get(get_metadata))
        .route("/state", get(get_state))
        .route("/:id", delete(delete_download).get(get_download))
        .route("/:id/start", get(start_download))
        .route("/:id/resume", get(resume_download))
        .route("/:id/pause", get(pause_download))
}
