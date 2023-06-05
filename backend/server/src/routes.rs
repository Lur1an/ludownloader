use axum::Router;

#[derive(Clone)]
pub struct ApplicationState {}

impl ApplicationState {
    pub fn new() -> Self {
        todo!()
    }
}

pub fn routes(state: ApplicationState) -> Router {
    let app_router = Router::new().with_state(state);
    app_router
}
