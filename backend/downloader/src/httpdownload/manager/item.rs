use super::download;
use super::download::{DownloadUpdate, HttpDownload};
use crate::httpdownload::manager::{Error, Result};
use api::proto::DownloadMetadata;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

/// Wrapper over HttpDownload to allow multi-threaded managing
#[derive(Debug)]
pub struct DownloaderItem {
    pub download: Arc<RwLock<HttpDownload>>,
    tx: Option<oneshot::Sender<()>>,
}

impl DownloaderItem {
    pub fn new(download: HttpDownload) -> Self {
        DownloaderItem {
            download: Arc::new(RwLock::new(download)),
            tx: None,
        }
    }

    pub async fn get_metadata(&self) -> DownloadMetadata {
        let download = self.download.read().await;
        download.get_metadata()
    }

    pub fn run(&mut self, update_ch: mpsc::Sender<DownloadUpdate>, resume: bool) {
        let (tx, rx) = oneshot::channel();
        let download_arc = self.download.clone();
        tokio::spawn(async move {
            let download = download_arc.read().await;
            log::info!(
                "Acquired read lock for download: {}, resume: {}",
                download.id,
                resume
            );

            let update_ch_cl = update_ch.clone();
            let download_result = if resume {
                download.resume(rx, update_ch).await
            } else {
                download.start(rx, update_ch).await
            };

            match download_result {
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
        self.tx = Some(tx);
    }

    pub async fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
            Ok(())
        } else {
            Err(Error::DownloadNotRunning)
        }
    }
}
