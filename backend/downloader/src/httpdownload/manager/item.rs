use super::download;
use super::download::{DownloadUpdate, HttpDownload};
use crate::httpdownload::manager::{Error, Result};
use crate::httpdownload::DownloadMetadata;
use std::sync::Arc;
use tokio::sync::{mpsc, oneshot, Mutex};

/// Wrapper over HttpDownload to allow multi-threaded managing
/// TODO: add packages to allow batching download commands
#[derive(Debug)]
pub struct DownloaderItem {
    pub download: Arc<Mutex<HttpDownload>>,
    pub metadata: DownloadMetadata,
    /// This sender contains the channel to notify the thread to stop the download function
    /// This is only None if the Download has been stopped, there are edge cases where the function
    /// crashes and the tx value is still Some(tx)
    tx: Option<oneshot::Sender<()>>,
}

impl DownloaderItem {
    pub fn new(download: HttpDownload) -> Self {
        DownloaderItem {
            metadata: download.get_metadata(),
            download: Arc::new(Mutex::new(download)),
            tx: None,
        }
    }

    pub fn is_locked(&self) -> bool {
        self.download.try_lock().is_err()
    }

    pub fn run(&mut self, update_ch: mpsc::Sender<DownloadUpdate>, resume: bool) {
        let (tx, rx) = oneshot::channel();
        let download_arc = self.download.clone();
        tokio::spawn(async move {
            let download = download_arc.lock().await;
            log::info!(
                "Acquired read lock for download: {}, resume: {}",
                download.id,
                resume
            );

            let update_ch_cl = update_ch.clone();
            let download_result = if resume {
                log::info!("Resuming download: {}", download.id);
                download.resume(rx, update_ch).await
            } else {
                log::info!("Starting download: {}", download.id);
                download.start(rx, update_ch).await
            };

            match download_result {
                Ok(downloaded_bytes) => {
                    let update = if downloaded_bytes == download.content_length {
                        download::State::Complete
                    } else {
                        download::State::Paused(downloaded_bytes)
                    };
                    let _ = update_ch_cl
                        .send(DownloadUpdate {
                            id: download.id,
                            state: update,
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
                            state: download::State::Error(format!("{}", e)),
                        })
                        .await;
                }
            }
        });
        self.tx = Some(tx);
    }

    pub fn stop(&mut self) -> Result<()> {
        if let Some(tx) = self.tx.take() {
            let _ = tx.send(());
            Ok(())
        } else {
            Err(Error::DownloadNotRunning)
        }
    }
}
