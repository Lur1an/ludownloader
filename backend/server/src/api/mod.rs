use downloader::httpdownload::{self, download};

include!(concat!(env!("OUT_DIR"), "/openapi.rs"));

impl From<httpdownload::DownloadMetadata> for DownloadMetadata {
    fn from(value: httpdownload::DownloadMetadata) -> Self {
        DownloadMetadata {
            id: value.id,
            url: value.url.to_string(),
            file_path: value.file_path.to_string_lossy().into(),
            content_length: value.content_length,
        }
    }
}

impl From<download::State> for DownloadState {
    fn from(value: download::State) -> Self {
        match value {
            download::State::Complete => Self::Complete(Complete {}),
            download::State::Paused(bytes_downloaded) => Self::Paused(Paused { bytes_downloaded }),
            download::State::Running {
                bytes_downloaded,
                bytes_per_second,
            } => Self::Running(Running {
                bytes_downloaded,
                bytes_per_second,
            }),
            download::State::Error(e) => Self::Error(Error { error: e }),
        }
    }
}
