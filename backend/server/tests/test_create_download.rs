use api::proto::DownloadMetadata;
use server::launch_app;
use test_log::test;

#[test(tokio::test)]
async fn test_create_download() {
    tokio::spawn(launch_app());
    let client = reqwest::Client::new();
    let body = "https://speed.hetzner.de/1GB.bin".to_owned();
    let resp = client
        .post("http://localhost:42069/api/v1/httpdownload")
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), reqwest::StatusCode::CREATED);
    let body: DownloadMetadata = prost::Message::decode(resp.bytes().await.unwrap()).unwrap();
    log::info!("metadata: {:?}", body);
}
