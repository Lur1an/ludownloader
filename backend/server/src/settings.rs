use dirs::{download_dir, home_dir};
use downloader::httpdownload::DownloadMetadata;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::{
    io::AsyncWriteExt,
    sync::{RwLock, RwLockReadGuard},
};

fn user_download_dir() -> PathBuf {
    dirs::download_dir().unwrap_or(PathBuf::from("/"))
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Settings {
    #[serde(default = "user_download_dir")]
    pub default_download_dir: PathBuf,
    #[serde(default)]
    pub max_concurrent_downloads: usize,
    #[serde(default = "Vec::new")]
    pub downloads: Vec<DownloadMetadata>,
}

#[derive(Debug, Clone)]
pub struct SettingManager {
    inner: Arc<RwLock<Settings>>,
    settings_path: PathBuf,
}

fn default_settings_path() -> PathBuf {
    let home_dir = home_dir().unwrap_or_default();
    
    home_dir.join(".ludownloader/settings.yaml")
}

impl SettingManager {
    pub async fn load(p: Option<PathBuf>) -> Self {
        let path = p.unwrap_or_else(default_settings_path);
        if let Some(parent) = path.parent() {
            tokio::fs::create_dir_all(parent).await.unwrap();
        }
        let settings = load_settings(&path).await;
        Self {
            inner: Arc::new(RwLock::new(settings)),
            settings_path: path,
        }
    }

    pub async fn read(&self) -> RwLockReadGuard<Settings> {
        self.inner.read().await
    }

    pub async fn try_read(&self) -> Option<RwLockReadGuard<Settings>> {
        self.inner.try_read().ok()
    }

    pub async fn write(&self, settings: Settings) {
        if let Ok(bytes) = serde_yaml::to_string(&settings) {
            log::info!("Yaml serialization of settings succesful, writing settings to file");
            let file = tokio::fs::write(&self.settings_path, bytes).await;
            match file {
                Ok(_) => log::info!(
                    "Settings file written to {}",
                    self.settings_path.to_string_lossy()
                ),
                Err(e) => log::error!("Error writing settings file: {}", e),
            }
        }
        log::info!("Overwriting inner settings, new value: {:?}", settings);
        let mut guard = self.inner.write().await;
        *guard = settings;
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            default_download_dir: download_dir()
                .map(|p| p.join("ludownloader"))
                .unwrap_or_default(),
            max_concurrent_downloads: 0,
            downloads: Vec::new(),
        }
    }
}

async fn load_settings(p: &PathBuf) -> Settings {
    let file_exists = tokio::fs::try_exists(p).await.unwrap_or(false);
    if file_exists {
        log::info!("Found settings file at {}, reading...", p.to_string_lossy());
        match tokio::fs::read_to_string(p).await {
            Ok(file) => {
                let settings: Settings = serde_yaml::from_str(&file).unwrap();
                log::info!("Settings loaded: {:?}", settings);
                if !tokio::fs::try_exists(&settings.default_download_dir)
                    .await
                    .unwrap()
                {
                    log::info!("default download directory pointed at by settings does not exist, creating...");
                    tokio::fs::create_dir_all(&settings.default_download_dir)
                        .await
                        .unwrap();
                }
                return settings;
            }
            Err(e) => {
                log::error!("Error reading settings file: {}", e);
                return Settings::default();
            }
        }
    }
    log::info!(
        "No settings file found at {}, creating...",
        p.to_string_lossy()
    );
    let settings = Settings::default();
    let settings_str =
        serde_yaml::to_string(&settings).expect("Serialization at this point can't fail!");
    let mut file = tokio::fs::File::create(p).await.unwrap();
    file.write_all(settings_str.as_bytes())
        .await
        .expect("Couldn't write to file");
    settings
}
