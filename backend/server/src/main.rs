use server::launch_app;

#[tokio::main]
async fn main() {
    env_logger::init();
    launch_app().await
}
