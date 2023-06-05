use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, RwLockReadGuard};

fn user_download_dir() -> PathBuf {
    dirs::download_dir().unwrap_or(PathBuf::from("/"))
}

#[derive(Debug, Clone, Deserialize, Serialize, Eq, PartialEq)]
struct Settings {
    #[serde(default = "user_download_dir")]
    default_download_dir: PathBuf,
    #[serde(default)]
    max_concurrent_downloads: usize,
}

#[derive(Debug, Clone)]
pub struct SettingManager {
    inner: Arc<RwLock<Settings>>,
    settings_path: PathBuf,
}

impl SettingManager {
    const SETTINGS_PATH: &str = "~/.ludownloader/settings.yaml";
    pub async fn load(p: Option<PathBuf>) -> Self {
        let path = p.unwrap_or(PathBuf::from(Self::SETTINGS_PATH));
        let settings = load_settings(&path).await;
        Self {
            inner: Arc::new(RwLock::new(settings)),
            settings_path: path,
        }
    }

    async fn read(&self) -> RwLockReadGuard<Settings> {
        self.inner.read().await
    }

    async fn try_read(&self) -> Option<RwLockReadGuard<Settings>> {
        self.inner.try_read().ok()
    }

    async fn write(&self, settings: Settings) {
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
            default_download_dir: Default::default(),
            max_concurrent_downloads: Default::default(),
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
    tokio::fs::write(&p, settings_str).await.unwrap();
    settings
}

#[cfg(test)]
mod test {
    use super::*;
    use test_log::test;

    #[test]
    fn encode_decode_yaml() {
        let decoded: Settings = serde_yaml::from_str("").unwrap();
        log::info!("decoded: {:?}", decoded);
    }
}
