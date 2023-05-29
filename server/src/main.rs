mod app;
mod data;
mod routes;

use axum::{routing::get, Router};

use data::Foo;
use downloader::httpdownload;

#[tokio::main]
async fn main() {
    // build our application with a single route
    let app = Router::new().route("/", get(|| async { "Hello, World!" }));
    let manager = httpdownload::manager::DownloadManager::default();

    let foo = Foo {
        bar: "Hello, World!".to_string(),
    };

    // run it with hyper on localhost:3000
    axum::Server::bind(&"0.0.0.0:3000".parse().unwrap())
        .serve(app.into_make_service())
        .await
        .unwrap();
}

#[cfg(test)]
mod test {
    use super::*;
    use test_log::test;
    #[test]
    fn encode_decode_foo() {
        let foo = Foo {
            bar: "Hello, I'm a protobuf Foo!".to_string(),
        };
        let encoded = prost::Message::encode_to_vec(&foo);
        log::info!("encoded: {:?}", encoded);
        let decoded: Foo = prost::Message::decode(encoded.as_slice()).unwrap();
        log::info!("decoded: {:?}", decoded);
        assert_eq!(foo, decoded);
    }
}
