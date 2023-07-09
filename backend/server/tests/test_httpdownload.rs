use api::proto::{CreateDownloadError, DownloadMetadata};
use async_trait::async_trait;
use prost::Message;
use reqwest::{StatusCode, Url};
use server::launch_app;
use test_context::{test_context, AsyncTestContext};
use test_log::test;

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
async fn test_create_download(ctx: &mut IntegrationTestContext) {
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
}

#[test_context(IntegrationTestContext)]
#[test(tokio::test)]
async fn test_create_multiple_downloads(ctx: &mut IntegrationTestContext) {
    for _ in 0..20 {
        let body = "https://speed.hetzner.de/1GB.bin".to_owned();
        let resp = ctx
            .client
            .post(ctx.server_url.join("/api/v1/httpdownload").unwrap())
            .body(body)
            .send()
            .await
            .unwrap();
        assert_eq!(resp.status(), StatusCode::CREATED);
    }
}
