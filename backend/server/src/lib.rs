use std::net::TcpListener;

tonic::include_proto!("ludownloader");

pub async fn launch_app(listener: TcpListener) {
    // let httpdownload_routes = routes().with_state(state);
    // let app = Router::new().nest("/api/v1/httpdownload", httpdownload_routes);
    todo!()
}
