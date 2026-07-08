use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

use crate::api::{CollectionInfo, EnvironmentInfo, WorkspaceInfo};

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
    /// Request name captured when favorited, so the Favorites pane can show it
    /// before the owning collection has been loaded this session.
    #[serde(default)]
    pub name: String,
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

/// Postman personal API keys are prefixed with `PMAK-` and are 64 characters
/// long. We validate the prefix and a plausible minimum length rather than an
/// exact match, so an obviously wrong/truncated key is rejected up front while
/// staying tolerant of any future length changes.
const API_KEY_PREFIX: &str = "PMAK-";
const API_KEY_MIN_LEN: usize = 40;

/// Validate the shape of a Postman API key. Returns an error message suitable
/// for showing to the user when the key is clearly malformed.
pub fn validate_api_key(key: &str) -> Result<(), String> {
    if key.is_empty() {
        return Err("API key cannot be empty.".to_string());
    }
    if !key.starts_with(API_KEY_PREFIX) {
        return Err(format!(
            "API key should start with \"{API_KEY_PREFIX}\". Copy the full key from \
             https://web.postman.co/settings/me/api-keys"
        ));
    }
    if key.len() < API_KEY_MIN_LEN {
        return Err(format!(
            "API key looks too short ({} chars); expected ~64. It may have been truncated when pasted.",
            key.len()
        ));
    }
    Ok(())
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

        // The config holds the Postman API key, so restrict it to the owner only.
        #[cfg(unix)]
        {
            use std::os::unix::fs::PermissionsExt;
            fs::set_permissions(&path, fs::Permissions::from_mode(0o600))
                .with_context(|| format!("Failed to set permissions on config file: {}", path.display()))?;
        }

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

    pub fn add_favorite_request(&mut self, collection_uid: String, path: Vec<usize>, name: String) {
        if !self.is_request_favorite(&collection_uid, &path) {
            self.favorite_requests.push(FavoriteRequest { collection_uid, path, name });
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

/// Remembered `{{placeholder}}` values entered in the params dialog, keyed by
/// request, so they pre-fill next time (stored in ~/.local/share/lazypost/).
#[derive(Debug, Deserialize, Serialize, Clone)]
pub struct ParamValuesEntry {
    pub collection_uid: String,
    pub path: Vec<usize>,
    #[serde(default)]
    pub values: std::collections::HashMap<String, String>,
}

#[derive(Debug, Deserialize, Serialize, Default)]
pub struct ParamValuesStore {
    #[serde(default)]
    pub entries: Vec<ParamValuesEntry>,
}

impl ParamValuesStore {
    pub fn file_path() -> Result<PathBuf> {
        Ok(LocalEditsStore::data_dir()?.join("param_values.toml"))
    }

    pub fn load() -> Result<Self> {
        let path = Self::file_path()?;
        if !path.exists() {
            return Ok(Self::default());
        }
        let content = fs::read_to_string(&path)
            .with_context(|| format!("Failed to read param values file: {}", path.display()))?;
        let store: ParamValuesStore = toml::from_str(&content)
            .with_context(|| "Failed to parse param values file")?;
        Ok(store)
    }

    pub fn save(&self) -> Result<()> {
        let data_dir = LocalEditsStore::data_dir()?;
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;
        let path = Self::file_path()?;
        let content = toml::to_string_pretty(self)
            .context("Failed to serialize param values")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write param values file: {}", path.display()))?;
        Ok(())
    }

    /// Stored values for a request, if any.
    pub fn get(&self, collection_uid: &str, path: &[usize]) -> Option<&std::collections::HashMap<String, String>> {
        self.entries
            .iter()
            .find(|e| e.collection_uid == collection_uid && e.path == path)
            .map(|e| &e.values)
    }

    /// Replace the stored values for a request. An empty map removes the entry.
    pub fn set(&mut self, collection_uid: String, path: Vec<usize>, values: std::collections::HashMap<String, String>) {
        self.entries
            .retain(|e| !(e.collection_uid == collection_uid && e.path == path));
        if !values.is_empty() {
            self.entries.push(ParamValuesEntry {
                collection_uid,
                path,
                values,
            });
        }
    }
}

/// On-disk cache of the last-seen workspace/collection/environment lists, used
/// to paint the UI instantly at startup while fresh data loads in the
/// background. Only list metadata (names/uids) is cached here — environment
/// *values* and collection details (which may contain secrets) are never
/// written to this file; they are always fetched live.
#[derive(Debug, Deserialize, Serialize, Default)]
pub struct CacheStore {
    #[serde(default)]
    pub workspaces: Vec<WorkspaceInfo>,
    #[serde(default)]
    pub collections: Vec<CollectionInfo>,
    #[serde(default)]
    pub environments: Vec<EnvironmentInfo>,
}

impl CacheStore {
    pub fn cache_path() -> Result<PathBuf> {
        Ok(LocalEditsStore::data_dir()?.join("cache.toml"))
    }

    /// Best-effort load: any error (missing file, parse failure) yields an
    /// empty cache so startup simply falls back to a live fetch.
    pub fn load() -> Self {
        let path = match Self::cache_path() {
            Ok(p) => p,
            Err(_) => return Self::default(),
        };
        match fs::read_to_string(&path) {
            Ok(content) => toml::from_str(&content).unwrap_or_default(),
            Err(_) => Self::default(),
        }
    }

    pub fn save(&self) -> Result<()> {
        let data_dir = LocalEditsStore::data_dir()?;
        fs::create_dir_all(&data_dir)
            .with_context(|| format!("Failed to create data directory: {}", data_dir.display()))?;

        let path = Self::cache_path()?;
        let content = toml::to_string_pretty(self).context("Failed to serialize cache")?;
        fs::write(&path, content)
            .with_context(|| format!("Failed to write cache file: {}", path.display()))?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::validate_api_key;

    #[test]
    fn accepts_well_formed_key() {
        let key = format!("PMAK-{}", "a".repeat(59)); // 64 chars total
        assert!(validate_api_key(&key).is_ok());
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_api_key("").is_err());
    }

    #[test]
    fn rejects_missing_prefix() {
        let key = "a".repeat(64);
        assert!(validate_api_key(&key).is_err());
    }

    #[test]
    fn rejects_truncated_key() {
        // The real-world failure: a 10-char truncated paste.
        assert!(validate_api_key("PMAK-1234").is_err());
    }
}
