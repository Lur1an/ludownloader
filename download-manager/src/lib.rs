use std::error::Error;
use std::sync::Arc;
use std::thread::JoinHandle;
use httpdownload::download::Download;

struct DownloadManager {
    downloads: Vec<Arc<Download>>,
    running_tasks: Vec<Option<JoinHandle<Result<(), Box<dyn Error>>>>>
}

impl DownloadManager {
    pub async fn add(download: Download) -> Result<(), String> {
       Ok(())
    }

    pub async fn start(index: u64) -> Result<(), String> {
       Ok(())
    }

    pub async fn status_json() -> String {
        String::new()
    }
}

#[cfg(test)]
mod test {

}
