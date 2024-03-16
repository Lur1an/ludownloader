use super::DownloadMetadata;
use crate::httpdownload::download::{DownloadUpdate, HttpDownload};
use uuid::Uuid;

pub type Result<T> = anyhow::Result<T>;

/// Trait for a struct that can handle DownloadUpdates.
pub trait UpdateConsumer {
    fn consume(&mut self, update: DownloadUpdate);
}

/// This struct takes care of storing/running/stopping downloads.
/// Internally it uses a RwLock to allow for concurrent access,
/// this exposes a thread-safe interface.
/// This struct is supposed to be cloned as it uses an Arc internally.
#[derive(Clone)]
pub struct DownloadManager {}

impl DownloadManager {
    pub async fn new() -> Self {
        Self {}
    }

    pub async fn start(&self, id: &Uuid) -> Result<()> {
        todo!()
    }

    pub async fn resume(&self, id: &Uuid) -> Result<()> {
        todo!()
    }

    pub async fn stop(&self, id: &Uuid) -> Result<()> {
        todo!()
    }

    pub async fn start_all(&self) {
        todo!()
    }

    pub async fn stop_all(&self) {
        todo!()
    }

    pub async fn get_metadata(&self, id: &Uuid) -> Result<DownloadMetadata> {
        todo!()
    }

    pub async fn get_metadata_all(&self) -> Vec<DownloadMetadata> {
        todo!()
    }

    pub async fn add(&self, download: HttpDownload) -> Uuid {
        todo!()
    }

    pub async fn delete(&self, id: &Uuid, delete_file: bool) -> Result<()> {
        todo!()
    }
}

#[cfg(test)]
mod test {
    use super::*;
    use crate::util::{file_size, test::setup_test_download};
    use test_log::test;
    use tokio::time;

    const TEST_DOWNLOAD_URL: &str =
        "https://dl.google.com/linux/direct/google-chrome-stable_current_amd64.deb";
    type Test<T> = anyhow::Result<T>;

    #[test(tokio::test)]
    async fn start_stop_delete_download() -> Test<()> {
        let manager = DownloadManager::new().await;
        let (download, _tmp_dir) = setup_test_download(TEST_DOWNLOAD_URL).await?;
        let download_path = download.file_path();
        let id = manager.add(download).await;
        manager.start(&id).await?;
        // check metadata as expected
        let metadata = manager.get_metadata_all().await;
        assert_eq!(metadata.len(), 1, "There should be one download");
        let metadata = &metadata[0];
        assert_eq!(metadata.id, id);
        time::sleep(time::Duration::from_secs(1)).await;
        manager.stop(&id).await?;
        // check size of downloaded file
        let downloaded_bytes = file_size(&download_path).await;
        assert_ne!(
            downloaded_bytes, 0,
            "Downloaded bytes should be greater than 0"
        );
        // test deletion
        manager.delete(&id, true).await?;
        let downloaded_bytes = file_size(&download_path).await;
        assert_eq!(
            downloaded_bytes, 0,
            "Download file should not exist on disk anymore"
        );
        Ok(())
    }
}
