use crate::settings::SettingManager;
use axum::{
    routing::{get, post},
    Router,
};

#[derive(Clone, Debug)]
pub struct ApplicationState {
    pub settings: SettingManager,
}

pub fn routes(state: ApplicationState) -> Router {
    let app_router = Router::new()
        .route("/foo", get(|| async {}))
        .with_state(state);
    app_router
}
