use server::launch_app;

#[tokio::main]
async fn main() {
    env_logger::init();
    let listener = std::net::TcpListener::bind("0.0.0.0:42069").unwrap();
    launch_app(listener).await
}
