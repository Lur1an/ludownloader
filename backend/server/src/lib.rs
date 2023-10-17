pub mod api;
mod routes;
mod settings;

use std::net::TcpListener;

use axum::extract::FromRef;
use axum::Router;
use downloader::httpdownload::manager::DownloadManager;
use downloader::httpdownload::observer::DownloadObserver;
use reqwest::Client;
use routes::routes;

pub async fn launch_app(listener: TcpListener) {
    // let httpdownload_routes = routes().with_state(state);
    // let app = Router::new().nest("/api/v1/httpdownload", httpdownload_routes);
    todo!()

    // axum::Server::from_tcp(listener)
    //     .unwrap()
    //     .serve(app.into_make_service())
    //     .await
    //     .unwrap();
}

#[derive(Clone, FromRef)]
pub struct ApplicationState {
    pub download_manager: DownloadManager,
    pub setting_manager: settings::SettingManager,
}
