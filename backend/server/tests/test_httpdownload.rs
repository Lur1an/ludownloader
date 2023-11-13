use std::time::Duration;

use async_trait::async_trait;
use downloader::httpdownload::{download, DownloadMetadata};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use server::launch_app;
use test_context::{test_context, AsyncTestContext};
use test_log::test;
use uuid::Uuid;

struct Ctx {
    pub client: reqwest::Client,
    pub server_url: Url,
}

#[async_trait]
impl AsyncTestContext for Ctx {
    async fn setup() -> Self {
        let listener = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
        let local_addr = listener.local_addr().unwrap();
        let server_url = Url::parse(&format!("http://{}", local_addr)).unwrap();
        log::info!("Local server running on {}", server_url);
        let client = reqwest::Client::builder().build().unwrap();
        tokio::spawn(launch_app(listener));
        Ctx { client, server_url }
    }
}

#[derive(Deserialize, Serialize)]
struct ApiError {
    error: String,
}

#[test_context(Ctx)]
#[test(tokio::test)]
async fn test_download_crud(Ctx { client, server_url }: &mut Ctx) {
    let body = "https://speed.hetzner.de/1GB.bin".to_owned();
    let resp = client
        .post(server_url.join("/api/v1/httpdownload").unwrap())
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let metadata: DownloadMetadata = resp.json().await.unwrap();
    assert_eq!(metadata.download_size, 1048576000);
    let incorrect_url = "hgesdg98wq19".to_owned();
    let resp = client
        .post(server_url.join("/api/v1/httpdownload").unwrap())
        .body(incorrect_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: ApiError = resp.json().await.unwrap();
    assert!(body.error.contains("Invalid URL"));
    let fake_url = "http://ahahahahahaha_wtf.com/something.zip";
    let resp = client
        .post(server_url.join("/api/v1/httpdownload").unwrap())
        .body(fake_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body: ApiError = resp.json().await.unwrap();
    assert!(body.error.contains("Error creating download"));
}

#[test_context(Ctx)]
#[test(tokio::test)]
async fn test_multiple_download_crud(Ctx { client, server_url }: &mut Ctx) {
    let download_url = "https://speed.hetzner.de/1GB.bin";
    for _ in 0..20 {
        let body = download_url.to_owned();
        let resp = client
            .post(server_url.join("/api/v1/httpdownload").unwrap())
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
    let resp = client
        .get(server_url.join("/api/v1/httpdownload/metadata").unwrap())
        .send()
        .await
        .unwrap();
    let metadata: Vec<DownloadMetadata> = resp.json().await.unwrap();
    assert_eq!(metadata.len(), 20);
    for metadata in metadata.iter() {
        assert_eq!(metadata.url, download_url);
    }
    let resp = client
        .get(server_url.join("/api/v1/httpdownload/state").unwrap())
        .send()
        .await
        .unwrap();
    let states: Vec<(Uuid, download::State)> = resp.json().await.unwrap();
    for (_id, state) in states.into_iter() {
        assert!(matches!(state, download::State::Paused(_)));
    }
}

#[test_context(Ctx)]
#[test(tokio::test)]
async fn test_download_start_stop_resume(Ctx { client, server_url }: &mut Ctx) {
    let body =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb".to_owned();
    let resp = client
        .post(server_url.join("/api/v1/httpdownload").unwrap())
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let metadata: DownloadMetadata = resp.json().await.unwrap();
    let resp = client
        .get(
            server_url
                .join(format!("/api/v1/httpdownload/{}/start", metadata.id).as_ref())
                .unwrap(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let update_endpoint = server_url
        .join(format!("/api/v1/httpdownload/{}", metadata.id).as_ref())
        .unwrap();

    async fn fetch_state(client: &Client, endpoint: &Url) -> DownloadState {
        let resp = client.get(endpoint.clone()).send().await.unwrap();
        let data: DownloadData = resp.json().await.unwrap();
        data.state
    }

    let mut state = fetch_state(&client, &update_endpoint).await;
    while matches!(
        state,
        DownloadState::Running { .. } | DownloadState::Paused(_)
    ) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        state = fetch_state(&client, &update_endpoint).await;
    }

    state = fetch_state(&client, &update_endpoint).await;
    assert!(matches!(state, DownloadState::Complete(_)));
}
