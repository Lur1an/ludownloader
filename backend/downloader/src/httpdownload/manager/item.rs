use super::download;
use super::download::{DownloadUpdate, HttpDownload};
use crate::httpdownload::manager::{Error, Result};
use data::types::DownloadMetadata;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};
use tokio::task::JoinHandle;

#[derive(Debug)]
pub struct DownloaderItem {
    pub download: Arc<RwLock<HttpDownload>>,
    pub handle: Option<(JoinHandle<()>, oneshot::Sender<()>)>,
}

impl DownloaderItem {
    pub fn new(download: HttpDownload) -> Self {
        DownloaderItem {
            download: Arc::new(RwLock::new(download)),
            handle: None,
        }
    }

    fn is_running(&self) -> bool {
        self.handle.is_some()
    }

    pub async fn get_metadata(&self) -> DownloadMetadata {
        let download = self.download.read().await;
        DownloadMetadata {
            uuid: download.id.as_bytes().to_vec(),
            url: download.url.to_string(),
            file_path: download.file_path.to_string_lossy().to_string(),
            content_length: download.content_length,
        }
    }

    pub fn run(&mut self, update_ch: mpsc::Sender<DownloadUpdate>, resume: bool) {
        let (tx, rx) = oneshot::channel();
        let download_arc = self.download.clone();
        let thread_handle = tokio::spawn(async move {
            let download = download_arc.read().await;
            log::info!(
                "Acquired read lock for download: {}, resume: {}",
                download.id,
                resume
            );
            let update_ch_cl = update_ch.clone();
            let result = if resume {
                download.resume(rx, update_ch).await
            } else {
                download.start(rx, update_ch).await
            };
            match result {
                Ok(downloaded_bytes) => {
                    let update_type = if downloaded_bytes == download.content_length {
                        download::UpdateType::Complete
                    } else {
                        download::UpdateType::Paused(downloaded_bytes)
                    };
                    let _ = update_ch_cl
                        .send(DownloadUpdate {
                            id: download.id,
                            update_type,
                        })
                        .await;
                }
                Err(e) => {
                    log::error!(
                        "Error encountered while downloading {}, Error: {}",
                        download.id,
                        e
                    );
                    let _ = update_ch_cl
                        .send(DownloadUpdate {
                            id: download.id,
                            update_type: download::UpdateType::Error(e),
                        })
                        .await;
                }
            }
        });
        self.handle = Some((thread_handle, tx));
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some((handle, tx)) = self.handle.take() {
            let _ = tx.send(());
            let result = handle.await?;
            Ok(result)
        } else {
            Err(Error::DownloadNotRunning)
        }
    }

    pub async fn complete(&mut self) -> Result<()> {
        if let Some((handle, _tx)) = self.handle.take() {
            Ok(handle.await?)
        } else {
            return Err(Error::DownloadNotRunning);
        }
    }
}
