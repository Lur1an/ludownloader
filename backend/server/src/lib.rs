mod app;
mod routes;
mod settings;

use std::net::TcpListener;

use axum::Router;
use routes::{routes, ApplicationState};

pub async fn launch_app(listener: TcpListener) {
    // init state
    let setting_manager = crate::settings::SettingManager::load(None).await;
    let (manager, observer, subscribers) = downloader::httpdownload::init().await;
    let state = ApplicationState {
        manager,
        observer,
        subscribers,
        setting_manager,
        client: reqwest::Client::new(),
    };

    let httpdownload_routes = routes().with_state(state);
    let app = Router::new().nest("/api/v1/httpdownload", httpdownload_routes);
    // run it with hyper on localhost:3000

    axum::Server::from_tcp(listener)
        .unwrap()
        .serve(app.into_make_service())
        .await
        .unwrap();
}
