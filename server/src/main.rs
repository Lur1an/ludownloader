mod app;
mod data;
mod routes;
mod settings;

use routes::{routes, ApplicationState};

#[tokio::main]
async fn main() {
    // init state
    let settings = crate::settings::SettingManager::load(None).await;
    let state = ApplicationState { settings };
    // build our application with a single route
    let app = routes(state);
    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[cfg(test)]
mod test {
    use super::*;
    use data::DownloadMetadata;
    use test_log::test;
    use uuid::Uuid;
    #[test]
    fn encode_decode_foo() {
        let download_metadata = DownloadMetadata {
            url: "https://www.google.com".to_string(),
            file_path: "/tmp/foo".to_string(),
            content_length: 0,
            uuid: Uuid::new_v4().as_bytes().to_vec(),
        };
        let encoded = prost::Message::encode_to_vec(&download_metadata);
        log::info!("encoded: {:?}", encoded);
        let decoded: DownloadMetadata = prost::Message::decode(encoded.as_slice()).unwrap();
        log::info!("decoded: {:?}", decoded);
        assert_eq!(download_metadata, decoded);
    }
}
