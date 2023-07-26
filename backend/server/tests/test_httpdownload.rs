use std::time::Duration;

use async_trait::async_trait;
use downloader::httpdownload::{download, DownloadMetadata};
use reqwest::{Client, StatusCode, Url};
use serde::{Deserialize, Serialize};
use server::{api::DownloadData, launch_app};
use test_context::{test_context, AsyncTestContext};
use test_log::test;
use uuid::Uuid;

struct IntegrationTestContext {
    pub client: reqwest::Client,
    pub server_url: Url,
}

#[async_trait]
impl AsyncTestContext for IntegrationTestContext {
    async fn setup() -> Self {
        let listener = std::net::TcpListener::bind("0.0.0.0:0").unwrap();
        let local_addr = listener.local_addr().unwrap();
        let server_url = Url::parse(&format!("http://{}", local_addr)).unwrap();
        log::info!("Local server running on {}", server_url);
        let client = reqwest::Client::builder().build().unwrap();
        tokio::spawn(launch_app(listener));
        IntegrationTestContext { client, server_url }
    }
}

#[derive(Deserialize, Serialize)]
struct ApiError {
    error: String,
}

#[test_context(IntegrationTestContext)]
#[test(tokio::test)]
async fn test_download_crud(ctx: &mut IntegrationTestContext) {
    let body = "https://speed.hetzner.de/1GB.bin".to_owned();
    let resp = ctx
        .client
        .post(ctx.server_url.join("/api/v1/httpdownload").unwrap())
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let metadata: DownloadMetadata = resp.json().await.unwrap();
    assert_eq!(metadata.content_length, 1048576000);
    let incorrect_url = "hgesdg98wq19".to_owned();
    let resp = ctx
        .client
        .post(ctx.server_url.join("/api/v1/httpdownload").unwrap())
        .body(incorrect_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    let body: ApiError = resp.json().await.unwrap();
    assert!(body.error.contains("Invalid URL"));
    let fake_url = "http://ahahahahahaha_wtf.com/something.zip";
    let resp = ctx
        .client
        .post(ctx.server_url.join("/api/v1/httpdownload").unwrap())
        .body(fake_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body: ApiError = resp.json().await.unwrap();
    assert!(body.error.contains("Error creating download"));
}

#[test_context(IntegrationTestContext)]
#[test(tokio::test)]
async fn test_multiple_download_crud(ctx: &mut IntegrationTestContext) {
    let download_url = "https://speed.hetzner.de/1GB.bin";
    for _ in 0..20 {
        let body = download_url.to_owned();
        let resp = ctx
            .client
            .post(ctx.server_url.join("/api/v1/httpdownload").unwrap())
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
    let resp = ctx
        .client
        .get(
            ctx.server_url
                .join("/api/v1/httpdownload/metadata")
                .unwrap(),
        )
        .send()
        .await
        .unwrap();
    let metadata: Vec<DownloadMetadata> = resp.json().await.unwrap();
    assert_eq!(metadata.len(), 20);
    for metadata in metadata.iter() {
        assert_eq!(metadata.url, download_url);
    }
    let resp = ctx
        .client
        .get(ctx.server_url.join("/api/v1/httpdownload/state").unwrap())
        .send()
        .await
        .unwrap();
    let states: Vec<(Uuid, download::State)> = resp.json().await.unwrap();
    for (_id, state) in states.into_iter() {
        assert!(matches!(state, download::State::Paused(_)));
    }
}

#[test_context(IntegrationTestContext)]
#[test(tokio::test)]
async fn test_download_start_stop_resume(ctx: &mut IntegrationTestContext) {
    let body =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb".to_owned();
    let resp = ctx
        .client
        .post(ctx.server_url.join("/api/v1/httpdownload").unwrap())
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let metadata: DownloadMetadata = resp.json().await.unwrap();
    let resp = ctx
        .client
        .get(
            ctx.server_url
                .join(format!("/api/v1/httpdownload/{}/start", metadata.id).as_ref())
                .unwrap(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);

    let update_endpoint = ctx
        .server_url
        .join(format!("/api/v1/httpdownload/{}", metadata.id).as_ref())
        .unwrap();
    async fn new_state(client: &Client, endpoint: &Url) -> download::State {
        let resp = client.get(endpoint.clone()).send().await.unwrap();
        let data: DownloadData = resp.json().await.unwrap();
        data.state
    }
    let mut state = new_state(&ctx.client, &update_endpoint).await;
    while matches!(
        state,
        download::State::Running { .. } | download::State::Paused(_)
    ) {
        tokio::time::sleep(Duration::from_millis(500)).await;
        state = new_state(&ctx.client, &update_endpoint).await;
    }
    state = new_state(&ctx.client, &update_endpoint).await;
    assert!(matches!(state, download::State::Complete));
}
