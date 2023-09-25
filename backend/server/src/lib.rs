pub mod api;
mod routes;
mod settings;

use std::net::TcpListener;

use axum::Router;
use axum::extract::FromRef;
use downloader::httpdownload::manager::DownloadManager;
use downloader::httpdownload::observer::DownloadObserver;
use reqwest::Client;
use routes::routes;

pub async fn launch_app(listener: TcpListener) {
    // init state
    let setting_manager = crate::settings::SettingManager::load(None).await;
    let (manager, observer, subscribers) = downloader::httpdownload::init().await;
    let state = ApplicationState {
        download_manager: manager,
        observer,
        subscribers,
        setting_manager,
        client: reqwest::Client::new(),
    };

    let httpdownload_routes = routes().with_state(state);
    let app = Router::new().nest("/api/v1/httpdownload", httpdownload_routes);

    axum::Server::from_tcp(listener)
        .unwrap()
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[derive(Clone, FromRef)]
pub struct ApplicationState {
    pub download_manager: DownloadManager,
    pub observer: DownloadObserver,
    pub subscribers: downloader::httpdownload::Subscribers,
    pub setting_manager: settings::SettingManager,
    pub client: Client,
}
