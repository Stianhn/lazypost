use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

#[derive(Debug, Deserialize, Serialize)]
pub struct Config {
    pub postman: PostmanConfig,
    #[serde(default)]
    pub favorites: Vec<String>,
    #[serde(default)]
    pub favorite_requests: Vec<FavoriteRequest>,
    #[serde(default)]
    pub last_state: Option<LastState>,
}

#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct FavoriteRequest {
    pub collection_uid: String,
    pub path: Vec<usize>,
}

/// A locally edited request that hasn't been synced to Postman
#[derive(Debug, Deserialize, Serialize, Clone, PartialEq)]
pub struct LocalEdit {
    pub collection_uid: String,
    pub path: Vec<usize>,
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct LastState {
    pub collection_uid: String,
    pub request_path: Vec<usize>,
    #[serde(default)]
    pub environment_uid: Option<String>,
    #[serde(default)]
    pub workspace_id: Option<String>,
}

#[derive(Debug, Deserialize, Serialize)]
pub struct PostmanConfig {
    pub api_key: String,
}

impl Config {
    pub fn config_dir() -> Result<PathBuf> {
        let config_dir = dirs::config_dir()
            .context("Could not determine config directory")?
            .join("lazypost");
        Ok(config_dir)
    }

    pub fn config_path() -> Result<PathBuf> {
        Ok(Self::config_dir()?.join("config.toml"))
    }

    pub fn load() -> Result<Option<Self>> {
        let path = Self::config_path()?;
        if !path.exists() {
            return Ok(None);
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read config file: {}", path.display()))?;

        let config: Config = toml::from_str(&content)
            .with_context(|| "Failed to parse config file")?;

        Ok(Some(config))
    }

    pub fn save(&self) -> Result<()> {
        let config_dir = Self::config_dir()?;
        fs::create_dir_all(&config_dir)
            .with_context(|| format!("Failed to create config directory: {}", config_dir.display()))?;

        let path = Self::config_path()?;
        let content = match toml::to_string_pretty(self) {
            Ok(c) => c,
            Err(e) => {
                anyhow::bail!("Failed to serialize config: {} (last_state: {:?})", e, self.last_state);
            }
        };

        fs::write(&path, content)
            .with_context(|| format!("Failed to write config file: {}", path.display()))?;

        Ok(())
    }

    pub fn new(api_key: String) -> Self {
        Config {
            postman: PostmanConfig { api_key },
            favorites: Vec::new(),
            favorite_requests: Vec::new(),
            last_state: None,
        }
    }

    pub fn add_favorite_request(&mut self, collection_uid: String, path: Vec<usize>) {
        let fav = FavoriteRequest { collection_uid, path };
        if !self.favorite_requests.contains(&fav) {
            self.favorite_requests.push(fav);
        }
    }

    pub fn remove_favorite_request(&mut self, collection_uid: &str, path: &[usize]) {
        self.favorite_requests.retain(|f| !(f.collection_uid == collection_uid && f.path == path));
    }

    pub fn is_request_favorite(&self, collection_uid: &str, path: &[usize]) -> bool {
        self.favorite_requests.iter().any(|f| f.collection_uid == collection_uid && f.path == path)
    }

    pub fn set_last_state(&mut self, collection_uid: String, request_path: Vec<usize>, environment_uid: Option<String>, workspace_id: Option<String>) {
        self.last_state = Some(LastState {
            collection_uid,
            request_path,
            environment_uid,
            workspace_id,
        });
    }

    pub fn set_last_environment(&mut self, environment_uid: Option<String>) {
        if let Some(ref mut state) = self.last_state {
            state.environment_uid = environment_uid;
        } else if let Some(uid) = environment_uid {
            // Create a minimal state just for environment
            self.last_state = Some(LastState {
                collection_uid: String::new(),
                request_path: Vec::new(),
                environment_uid: Some(uid),
                workspace_id: None,
            });
        }
    }

    pub fn set_last_workspace(&mut self, workspace_id: Option<String>) {
        if let Some(ref mut state) = self.last_state {
            state.workspace_id = workspace_id;
            // Sanitize request_path - usize::MAX can't be serialized to TOML
            if state.request_path.contains(&usize::MAX) {
                state.request_path = Vec::new();
            }
        } else if let Some(id) = workspace_id {
            // Create a minimal state just for workspace
            self.last_state = Some(LastState {
                collection_uid: String::new(),
                request_path: Vec::new(),
                environment_uid: None,
                workspace_id: Some(id),
            });
        }
    }

    pub fn add_favorite(&mut self, uid: String) {
        if !self.favorites.contains(&uid) {
            self.favorites.push(uid);
        }
    }

    pub fn remove_favorite(&mut self, uid: &str) {
        self.favorites.retain(|f| f != uid);
    }

    pub fn is_favorite(&self, uid: &str) -> bool {
        self.favorites.contains(&uid.to_string())
    }
}

/// Storage for local edits (stored in ~/.local/share/lazypost/)
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct LocalEditsStore {
    pub edits: Vec<LocalEdit>,
}

impl LocalEditsStore {
    pub fn data_dir() -> Result<PathBuf> {
        let data_dir = dirs::data_local_dir()
            .context("Could not determine local data directory")?
            .join("lazypost");
        Ok(data_dir)
    }

    pub fn file_path() -> Result<PathBuf> {
        Ok(Self::data_dir()?.join("local_edits.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::file_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }

        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read local edits file: {}", path.display()))?;

        let store: LocalEditsStore = toml::from_str(&content)
            .with_context(|| "Failed to parse local edits file")?;

        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        let data_dir = Self::data_dir()?;
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;

        let path = Self::file_path()?;
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize local edits")?;

        fs::write(&path, content)
            .with_context(|| format!("Failed to write local edits file: {}", path.display()))?;

        Ok(())
    }

    /// Add or update a local edit
    pub fn set_edit(&mut self, collection_uid: String, path: Vec<usize>, name: String, method: String, url: String, body: String) {
        self.edits.retain(|e| !(e.collection_uid == collection_uid && e.path == path));
        self.edits.push(LocalEdit {
            collection_uid,
            path,
            name,
            method,
            url,
            body,
        });
    }

    /// Remove a local edit
    pub fn remove_edit(&mut self, collection_uid: &str, path: &[usize]) {
        self.edits.retain(|e| !(e.collection_uid == collection_uid && e.path == path));
    }

    /// Get a local edit
    pub fn get_edit(&self, collection_uid: &str, path: &[usize]) -> Option<&LocalEdit> {
        self.edits.iter().find(|e| e.collection_uid == collection_uid && e.path == path)
    }

    /// Check if a request has a local edit
    pub fn has_edit(&self, collection_uid: &str, path: &[usize]) -> bool {
        self.edits.iter().any(|e| e.collection_uid == collection_uid && e.path == path)
    }
}
