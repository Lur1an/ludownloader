use super::download;
use super::download::{DownloadUpdate, HttpDownload};
use crate::httpdownload::manager::Result;
use crate::httpdownload::DownloadMetadata;
use anyhow::anyhow;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, RwLock};

/// Wrapper over HttpDownload to allow multi-threaded managing
/// TODO: add packages to allow batching download commands
#[derive(Debug)]
pub struct DownloaderItem {
    pub(super) download: Arc<RwLock<HttpDownload>>,
    /// This sender contains the channel to notify the thread to stop the download function
    tx: Option<oneshot::Sender<()>>,
}

impl DownloaderItem {
    pub fn new(download: HttpDownload) -> Self {
        DownloaderItem {
            download: Arc::new(RwLock::new(download)),
            tx: None,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.download.try_read().is_err()
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
            let download_task = async {
                if resume {
                    log::info!("Resuming download: {}", download.id);
                    download.resume(update_ch_cl).await
                } else {
                    log::info!("Starting download: {}", download.id);
                    download.start(update_ch_cl).await
                }
            };
            let update = tokio::select! {
                _ = rx => {
                    log::info!("Stopping download: {}", download.id);
                    let downloaded_bytes = download.get_bytes_on_disk().await;
                    DownloadUpdate {
                        id: download.id,
                        state: download::State::Paused(downloaded_bytes),
                    }
                }
                download_result = download_task => {
                    match download_result {
                        Ok(_) => {
                            DownloadUpdate {
                                id: download.id,
                                state: download::State::Complete,
                            }
                        }
                        Err(e) => {
                            log::error!(
                                "Error encountered while downloading {}, Error: {}",
                                download.id,
                                e
                            );
                            DownloadUpdate {
                                id: download.id,
                                state: download::State::Error(format!("{}", e)),
                            }
                        }
                    }
                }
            };
            let _ = update_ch.send(update).await;
        });
        self.tx = Some(tx);
    }

    pub async fn get_metadata(&self) -> DownloadMetadata {
        self.download.read().await.get_metadata()
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.tx.take() {
            tx.send(())
                .map_err(|e| anyhow!("Error sending stop signal: {:?}", e))
        } else {
            Err(anyhow!("Can't stop a download that is not running"))
        }
    }
}
