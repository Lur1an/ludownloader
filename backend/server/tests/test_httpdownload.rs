use std::{io::Read, time::Duration};

use api::proto::{DownloadMetadata, DownloadPaused, MetadataBatch, StateBatch};
use async_trait::async_trait;
use prost::Message;
use reqwest::{StatusCode, Url};
use server::launch_app;
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
    let metadata: DownloadMetadata = Message::decode(resp.bytes().await.unwrap()).unwrap();
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
    let body = resp.text().await.unwrap();
    assert!(body.contains("Invalid URL"));
    let fake_url = "http://ahahahahahaha_wtf.com/something.zip";
    let resp = ctx
        .client
        .post(ctx.server_url.join("/api/v1/httpdownload").unwrap())
        .body(fake_url)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    let body = resp.text().await.unwrap();
    assert!(body.contains("Error creating download"));
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
    let metadata: MetadataBatch = Message::decode(resp.bytes().await.unwrap()).unwrap();
    assert_eq!(metadata.value.len(), 20);
    for metadata in metadata.value.iter() {
        assert_eq!(metadata.url, download_url);
    }
    let resp = ctx
        .client
        .get(ctx.server_url.join("/api/v1/httpdownload/state").unwrap())
        .send()
        .await
        .unwrap();
    let state: StateBatch = Message::decode(resp.bytes().await.unwrap()).unwrap();
    for state in state.value.iter().cloned() {
        assert!(matches!(
            state.state.unwrap(),
            api::proto::download_state::State::Paused(DownloadPaused {
                bytes_downloaded: 0
            })
        ));
    }
}
#[test_context(IntegrationTestContext)]
#[test(tokio::test)]
async fn test_download_start_stop_resume(ctx: &mut IntegrationTestContext) {
    let body = "https://speed.hetzner.de/1GB.bin".to_owned();
    let resp = ctx
        .client
        .post(ctx.server_url.join("/api/v1/httpdownload").unwrap())
        .body(body)
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::CREATED);
    let metadata: DownloadMetadata = Message::decode(resp.bytes().await.unwrap()).unwrap();
    let id: Uuid = metadata.id();
    let resp = ctx
        .client
        .get(
            ctx.server_url
                .join(format!("/api/v1/httpdownload/{}/start", id).as_ref())
                .unwrap(),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(resp.status(), StatusCode::OK);
    tokio::time::sleep(Duration::from_secs(10)).await;
}
