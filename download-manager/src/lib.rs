use std::error::Error;
use std::sync::Arc;
use std::thread::JoinHandle;
use httpdownload::download::Download;
use httpdownload::download_config::DownloadConfig;

struct DownloadManager {
    downloads: Vec<Arc<Download>>,
}

impl DownloadManager {

    pub async fn add(self: &mut Self, download: Download) -> Result<(), String> {

        Ok(())
    }

    pub async fn start(self: &mut Self, index: u64) -> Result<(), String> {
        Ok(())
    }

    pub async fn status_json() -> String {
        String::new()
    }

}

#[cfg(test)]
mod test {}
