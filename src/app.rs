use crate::api::{CollectionDetail, CollectionInfo, EnvironmentDetail, EnvironmentInfo, ExecutedResponse, Item, PostmanClient, Request, RequestItem, RequestUrl, WorkspaceInfo};
use crate::config::{Config, LocalEditsStore};
use crate::logging::log_error;
use crate::ui::JsonViewerState;
use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct EditableRequest {
    pub name: String,
    pub method: String,
    pub url: String,
    #[serde(default)]
    pub body: String,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum FocusedPane {
    Collections,
    Requests,
    Preview,
    Response,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputMode {
    Normal,
    TextInput,
    Search,
    Saving,
    EnvironmentSelect,
    VariablesView,
    WorkspaceSelect,
    JsonSearch,
    ExecuteConfirm,
}

#[derive(Debug, Clone)]
pub struct PendingSave {
    pub edited: EditableRequest,
    pub item_index: usize,
}

#[derive(Debug, Clone)]
pub struct PendingExecute {
    pub method: String,
    pub url: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq)]
pub enum DialogStep {
    Name,
    Url,
}

#[derive(Debug, Clone)]
pub struct NewRequestDialog {
    pub step: DialogStep,
    pub name: String,
    pub url: String,
    pub cursor_position: usize,
    pub target_folder_path: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct FlatItem {
    pub name: String,
    pub depth: usize,
    pub is_folder: bool,
    pub expanded: bool,
    pub request: Option<Request>,
    pub request_id: Option<String>,
    pub path: Vec<usize>,
}

#[derive(Debug, Clone)]
pub struct FlatCollection {
    pub name: String,
    pub uid: String,
    pub is_favorites_folder: bool,
    pub depth: usize,
}

pub struct App {
    pub client: PostmanClient,
    pub config: Config,
    pub local_edits: LocalEditsStore,
    pub focused_pane: FocusedPane,
    pub collections: Vec<CollectionInfo>,
    pub flat_collections: Vec<FlatCollection>,
    pub selected_collection_index: usize,
    pub current_collection: Option<CollectionDetail>,
    pub flat_items: Vec<FlatItem>,
    pub selected_item_index: usize,
    pub expanded_folders: HashSet<Vec<usize>>,
    pub collections_favorites_expanded: bool,
    pub current_request: Option<Request>,
    pub response: Option<ExecutedResponse>,
    pub json_viewer_state: Option<JsonViewerState>,
    pub loading: bool,
    pub error: Option<String>,
    pub status_message: String,
    pub input_mode: InputMode,
    pub new_request_dialog: Option<NewRequestDialog>,
    pub pending_save: Option<PendingSave>,
    // Search state
    pub search_query: String,
    pub search_matches: Vec<usize>,
    pub search_match_paths: Vec<Vec<usize>>, // Paths for deep search in requests
    pub current_match_index: usize,
    pub pre_search_index: usize,
    // Environment state
    pub environments: Vec<EnvironmentInfo>,
    pub selected_environment_index: Option<usize>,
    pub current_environment: Option<EnvironmentDetail>,
    pub variables: HashMap<String, String>,
    pub environment_popup_index: usize,
    // Variables view state
    pub variables_popup_index: usize,
    pub editing_variable: Option<(usize, String)>, // (index, new value being edited)
    pub variable_cursor_position: usize,
    pub variables_modified: bool,
    pub variables_search_query: String,
    pub variables_search_active: bool,
    pub variables_filtered_indices: Vec<usize>, // Indices of variables matching search
    // Workspace state
    pub workspaces: Vec<WorkspaceInfo>,
    pub selected_workspace_index: Option<usize>, // None = "All Workspaces"
    pub workspace_popup_index: usize,
    pub workspace_loading: Option<String>, // Name of workspace being loaded
    pub collection_loading: Option<String>, // Name of collection being loaded
    // Unsaved edit state
    pub unsaved_edit: Option<(EditableRequest, usize)>, // (edited request, item_index)
    // Execute confirmation state
    pub pending_execute: Option<PendingExecute>,
    // Request execution state
    pub request_executing: bool,
}

impl App {
    pub fn new(config: Config) -> Self {
        let api_key = config.postman.api_key.clone();
        let local_edits = LocalEditsStore::load().unwrap_or_default();
        App {
            client: PostmanClient::new(api_key),
            config,
            local_edits,
            focused_pane: FocusedPane::Collections,
            collections: Vec::new(),
            flat_collections: Vec::new(),
            selected_collection_index: 0,
            current_collection: Option::None,
            flat_items: Vec::new(),
            selected_item_index: 0,
            expanded_folders: HashSet::new(),
            collections_favorites_expanded: true,
            current_request: None,
            response: None,
            json_viewer_state: None,
            loading: false,
            error: None,
            status_message: String::from("1/2/3: Switch pane | j/k: Navigate | Enter: Select | f: Favorite | q: Quit"),
            input_mode: InputMode::Normal,
            new_request_dialog: None,
            pending_save: None,
            search_query: String::new(),
            search_matches: Vec::new(),
            search_match_paths: Vec::new(),
            current_match_index: 0,
            pre_search_index: 0,
            environments: Vec::new(),
            selected_environment_index: None,
            current_environment: None,
            variables: HashMap::new(),
            environment_popup_index: 0,
            variables_popup_index: 0,
            editing_variable: None,
            variable_cursor_position: 0,
            variables_modified: false,
            variables_search_query: String::new(),
            variables_search_active: false,
            variables_filtered_indices: Vec::new(),
            workspaces: Vec::new(),
            selected_workspace_index: None,
            workspace_popup_index: 0,
            workspace_loading: None,
            collection_loading: None,
            unsaved_edit: None,
            pending_execute: None,
            request_executing: false,
        }
    }

    pub fn toggle_favorite(&mut self) {
        match self.focused_pane {
            FocusedPane::Collections => {
                if self.flat_collections.is_empty() {
                    return;
                }
                let flat_collection = &self.flat_collections[self.selected_collection_index];
                // Don't allow favoriting the Favorites folder itself
                if flat_collection.is_favorites_folder {
                    return;
                }
                let uid = flat_collection.uid.clone();

                if self.config.is_favorite(&uid) {
                    self.config.remove_favorite(&uid);
                    self.status_message = String::from("Removed from favorites");
                } else {
                    self.config.add_favorite(uid.clone());
                    self.status_message = String::from("Added to favorites");
                }

                // Re-flatten collections
                self.flatten_collections();
                // Find and select the item again by uid
                if let Some(new_index) = self.flat_collections.iter().position(|c| c.uid == uid) {
                    self.selected_collection_index = new_index;
                }
            }
            FocusedPane::Requests => {
                if self.flat_items.is_empty() || self.collections.is_empty() {
                    return;
                }
                let item = &self.flat_items[self.selected_item_index];
                // Don't allow favoriting the Favorites folder itself
                if item.path == vec![usize::MAX] {
                    return;
                }
                let collection_uid = self.collections[self.selected_collection_index].uid.clone();
                let path = item.path.clone();

                if self.config.is_request_favorite(&collection_uid, &path) {
                    self.config.remove_favorite_request(&collection_uid, &path);
                    self.status_message = String::from("Removed from favorites");
                } else {
                    self.config.add_favorite_request(collection_uid, path.clone());
                    self.status_message = String::from("Added to favorites");
                }

                // Re-flatten to update Favorites section
                self.flatten_items();
                // Find and select the item again by path
                if let Some(new_index) = self.flat_items.iter().position(|item| item.path == path) {
                    self.selected_item_index = new_index;
                }
            }
            FocusedPane::Preview | FocusedPane::Response => {
                return;
            }
        }

        // Save config
        if let Err(e) = self.config.save() {
            let error_msg = format!("Failed to save favorites: {}", e);
            log_error("toggle_favorite", &error_msg);
            self.error = Some(error_msg);
        }
    }

    pub fn is_request_favorite(&self, path: &[usize]) -> bool {
        if self.collections.is_empty() {
            return false;
        }
        let collection_uid = &self.collections[self.selected_collection_index].uid;
        self.config.is_request_favorite(collection_uid, path)
    }

    fn sort_collections(&mut self) {
        // Sort alphabetically (favorites are handled separately in flatten)
        self.collections.sort_by(|a, b| {
            a.name.to_lowercase().cmp(&b.name.to_lowercase())
        });
    }

    pub fn flatten_collections(&mut self) {
        self.flat_collections.clear();

        let favorite_uids: Vec<&String> = self.collections
            .iter()
            .filter(|c| self.config.is_favorite(&c.uid))
            .map(|c| &c.uid)
            .collect();

        // Add Favorites section if there are any
        if !favorite_uids.is_empty() {
            self.flat_collections.push(FlatCollection {
                name: String::from("Favorites"),
                uid: String::new(),
                is_favorites_folder: true,
                depth: 0,
            });

            if self.collections_favorites_expanded {
                for collection in &self.collections {
                    if self.config.is_favorite(&collection.uid) {
                        self.flat_collections.push(FlatCollection {
                            name: collection.name.clone(),
                            uid: collection.uid.clone(),
                            is_favorites_folder: false,
                            depth: 1,
                        });
                    }
                }
            }
        }

        // Add all collections
        for collection in &self.collections {
            self.flat_collections.push(FlatCollection {
                name: collection.name.clone(),
                uid: collection.uid.clone(),
                is_favorites_folder: false,
                depth: 0,
            });
        }
    }

    pub fn toggle_collections_favorites_folder(&mut self) {
        self.collections_favorites_expanded = !self.collections_favorites_expanded;
        let selected_uid = self.flat_collections.get(self.selected_collection_index)
            .map(|c| c.uid.clone());
        self.flatten_collections();
        // Try to restore selection
        if let Some(uid) = selected_uid {
            if let Some(pos) = self.flat_collections.iter().position(|c| c.uid == uid) {
                self.selected_collection_index = pos;
            }
        }
    }

    pub fn set_focus(&mut self, pane: FocusedPane) {
        self.focused_pane = pane;
        self.update_status_for_pane();
    }

    fn update_status_for_pane(&mut self) {
        self.status_message = match self.focused_pane {
            FocusedPane::Collections => String::from("1-4: Switch pane | j/k: Navigate | Enter: Load | /: Search | q: Quit"),
            FocusedPane::Requests => String::from("1-4: Switch pane | j/k: Navigate | Enter: Select | e: Execute | a: Add | /: Search | q: Quit"),
            FocusedPane::Preview => String::from("1-4: Switch pane | e: Execute | q: Quit"),
            FocusedPane::Response => String::from("1-4: Switch pane | q: Quit"),
        };
    }

    pub async fn load_collections(&mut self) -> Result<()> {
        self.loading = true;
        self.error = None;
        self.status_message = String::from("Loading...");

        // Load workspaces first
        match self.client.list_workspaces().await {
            Ok(workspaces) => {
                self.workspaces = workspaces;

                // Restore saved workspace selection before loading collections
                if let Some(ref state) = self.config.last_state {
                    log_error("restore_workspace", &format!(
                        "last_state.workspace_id={:?}, workspaces.len={}",
                        state.workspace_id,
                        self.workspaces.len()
                    ));
                    if let Some(ref saved_ws_id) = state.workspace_id {
                        if let Some(ws_index) = self.workspaces.iter().position(|w| &w.id == saved_ws_id) {
                            self.selected_workspace_index = Some(ws_index);
                            log_error("restore_workspace", &format!("Restored to index {}", ws_index));
                        } else {
                            log_error("restore_workspace", &format!(
                                "saved workspace_id '{}' not found in {} workspaces",
                                saved_ws_id,
                                self.workspaces.len()
                            ));
                        }
                    }
                } else {
                    log_error("restore_workspace", "No last_state in config");
                }
            }
            Err(e) => {
                log_error("load_workspaces", &e.to_string());
                // Don't fail - workspaces are optional
            }
        }

        // Get the workspace ID for filtering
        let workspace_id = self.get_selected_workspace_id();

        // Load collections (filtered by workspace if selected)
        match self.client.list_collections(workspace_id.as_deref()).await {
            Ok(collections) => {
                self.collections = collections;
                self.sort_collections();
                self.flatten_collections();
                self.loading = false;
                self.status_message = format!("Loaded {} collections", self.collections.len());
            }
            Err(e) => {
                self.loading = false;
                let error_msg = e.to_string();
                log_error("load_collections", &error_msg);
                self.error = Some(error_msg);
                self.status_message = String::from("Failed to load collections");
            }
        }

        // Also load environments (filtered by workspace if selected)
        match self.client.list_environments(workspace_id.as_deref()).await {
            Ok(environments) => {
                self.environments = environments;
                // Don't auto-select here - restore_last_state will handle it
            }
            Err(e) => {
                log_error("load_environments", &e.to_string());
                // Don't fail - environments are optional
            }
        }
        Ok(())
    }

    fn get_selected_workspace_id(&self) -> Option<String> {
        self.selected_workspace_index
            .and_then(|idx| self.workspaces.get(idx))
            .map(|ws| ws.id.clone())
    }

    pub async fn load_selected_environment(&mut self) {
        if let Some(idx) = self.selected_environment_index {
            if let Some(env_info) = self.environments.get(idx) {
                let env_uid = env_info.uid.clone();
                match self.client.get_environment(&env_uid).await {
                    Ok(env_detail) => {
                        self.current_environment = Some(env_detail);
                        self.rebuild_variables();
                    }
                    Err(e) => {
                        log_error("load_environment", &e.to_string());
                    }
                }
            }
        } else {
            self.current_environment = None;
            self.rebuild_variables();
        }
    }

    pub fn rebuild_variables(&mut self) {
        self.variables.clear();

        // First add collection variables (lower priority)
        if let Some(collection) = &self.current_collection {
            for var in &collection.variable {
                if var.enabled.unwrap_or(true) {
                    self.variables.insert(var.key.clone(), var.value.clone());
                }
            }
        }

        // Then add environment variables (higher priority, overwrites collection vars)
        if let Some(env) = &self.current_environment {
            for var in &env.values {
                if var.enabled.unwrap_or(true) {
                    self.variables.insert(var.key.clone(), var.value.clone());
                }
            }
        }
    }

    pub fn substitute_variables(&self, text: &str) -> String {
        let mut result = text.to_string();
        for (key, value) in &self.variables {
            let pattern = format!("{{{{{}}}}}", key);
            result = result.replace(&pattern, value);
        }
        result
    }

    pub fn open_environment_popup(&mut self) {
        // Set the popup index to current selection, or 0 for "No Environment"
        self.environment_popup_index = self.selected_environment_index
            .map(|i| i + 1) // +1 because index 0 is "No Environment"
            .unwrap_or(0);
        self.input_mode = InputMode::EnvironmentSelect;
    }

    pub fn close_environment_popup(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub async fn confirm_environment_selection(&mut self) {
        if self.environment_popup_index == 0 {
            // "No Environment" selected
            self.selected_environment_index = None;
            self.current_environment = None;
            self.rebuild_variables();
        } else {
            // Actual environment selected
            self.selected_environment_index = Some(self.environment_popup_index - 1);
            self.load_selected_environment().await;
        }
        self.input_mode = InputMode::Normal;

        // Save environment selection
        self.save_environment_state();

        let env_name = self.get_current_environment_name();
        self.status_message = format!("Environment: {}", env_name);
    }

    pub fn environment_popup_up(&mut self) {
        if self.environment_popup_index > 0 {
            self.environment_popup_index -= 1;
        }
    }

    pub fn environment_popup_down(&mut self) {
        // +1 for "No Environment" option
        let max_index = self.environments.len();
        if self.environment_popup_index < max_index {
            self.environment_popup_index += 1;
        }
    }

    pub fn get_current_environment_name(&self) -> String {
        match self.selected_environment_index {
            Some(idx) => self.environments.get(idx)
                .map(|e| e.name.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            None => "No Environment".to_string(),
        }
    }

    // Workspace methods
    pub fn open_workspace_popup(&mut self) {
        // Set the popup index to current selection, or 0 for "All Workspaces"
        self.workspace_popup_index = self.selected_workspace_index
            .map(|i| i + 1) // +1 because index 0 is "All Workspaces"
            .unwrap_or(0);
        self.input_mode = InputMode::WorkspaceSelect;
    }

    pub fn close_workspace_popup(&mut self) {
        self.input_mode = InputMode::Normal;
    }

    pub fn confirm_workspace_selection(&mut self) {
        let old_workspace_id = self.get_selected_workspace_id();

        if self.workspace_popup_index == 0 {
            // "All Workspaces" selected
            self.selected_workspace_index = None;
        } else {
            // Actual workspace selected
            self.selected_workspace_index = Some(self.workspace_popup_index - 1);
        }

        let new_workspace_id = self.get_selected_workspace_id();
        self.input_mode = InputMode::Normal;

        // Save workspace selection
        self.save_workspace_state();

        // If workspace changed, set up for async loading
        if old_workspace_id != new_workspace_id {
            let ws_name = self.get_current_workspace_name();
            self.workspace_loading = Some(ws_name);

            // Clear current collection view
            self.current_collection = None;
            self.flat_items.clear();
            self.current_request = None;
            self.response = None;
            self.selected_collection_index = 0;
            self.selected_item_index = 0;

            // Clear environment since we're changing workspaces
            self.selected_environment_index = None;
            self.current_environment = None;
            self.rebuild_variables();
        } else {
            let ws_name = self.get_current_workspace_name();
            self.status_message = format!("Workspace: {}", ws_name);
        }
    }

    pub async fn load_workspace_data(&mut self) {
        let workspace_id = self.get_selected_workspace_id();

        match self.client.list_collections(workspace_id.as_deref()).await {
            Ok(collections) => {
                self.collections = collections;
                self.sort_collections();
                self.flatten_collections();
            }
            Err(e) => {
                log_error("load_collections", &e.to_string());
                self.error = Some(e.to_string());
                self.status_message = String::from("Failed to load collections");
                self.workspace_loading = None;
                return;
            }
        }

        match self.client.list_environments(workspace_id.as_deref()).await {
            Ok(environments) => {
                self.environments = environments;
            }
            Err(e) => {
                log_error("load_environments", &e.to_string());
            }
        }

        let ws_name = self.get_current_workspace_name();
        self.status_message = format!("Workspace: {} ({} collections)", ws_name, self.collections.len());
        self.workspace_loading = None;
    }

    pub fn workspace_popup_up(&mut self) {
        if self.workspace_popup_index > 0 {
            self.workspace_popup_index -= 1;
        }
    }

    pub fn workspace_popup_down(&mut self) {
        // +1 for "All Workspaces" option
        let max_index = self.workspaces.len();
        if self.workspace_popup_index < max_index {
            self.workspace_popup_index += 1;
        }
    }

    pub fn get_current_workspace_name(&self) -> String {
        match self.selected_workspace_index {
            Some(idx) => self.workspaces.get(idx)
                .map(|w| w.name.clone())
                .unwrap_or_else(|| "Unknown".to_string()),
            None => "All Workspaces".to_string(),
        }
    }

    pub fn save_workspace_state(&mut self) {
        let workspace_id = self.get_selected_workspace_id();
        log_error("save_workspace_state", &format!(
            "Saving workspace: index={:?}, id={:?}, workspaces.len={}",
            self.selected_workspace_index,
            workspace_id,
            self.workspaces.len()
        ));
        self.config.set_last_workspace(workspace_id.clone());
        if let Err(e) = self.config.save() {
            log_error("save_workspace_state", &format!("{:#}", e));
        }
    }

    // Variables view methods
    pub fn open_variables_popup(&mut self) {
        if self.current_environment.is_none() {
            self.status_message = String::from("No environment selected");
            return;
        }
        self.variables_popup_index = 0;
        self.editing_variable = None;
        self.variable_cursor_position = 0;
        self.variables_modified = false;
        self.variables_search_query.clear();
        self.variables_search_active = false;
        self.variables_filtered_indices.clear();
        self.input_mode = InputMode::VariablesView;
    }

    pub fn close_variables_popup(&mut self) {
        self.editing_variable = None;
        self.input_mode = InputMode::Normal;
        if self.variables_modified {
            self.status_message = String::from("Unsaved changes discarded");
        }
        self.variables_modified = false;
    }

    pub fn variables_popup_up(&mut self) {
        if self.editing_variable.is_some() || self.variables_search_active {
            return; // Don't navigate while editing or searching
        }
        if self.variables_popup_index > 0 {
            self.variables_popup_index -= 1;
        }
    }

    pub fn variables_popup_down(&mut self) {
        if self.editing_variable.is_some() || self.variables_search_active {
            return; // Don't navigate while editing or searching
        }
        let max_index = if !self.variables_search_query.is_empty() {
            self.variables_filtered_indices.len().saturating_sub(1)
        } else if let Some(env) = &self.current_environment {
            env.values.len().saturating_sub(1)
        } else {
            0
        };
        if self.variables_popup_index < max_index {
            self.variables_popup_index += 1;
        }
    }

    pub fn start_editing_variable(&mut self) {
        if self.variables_search_active {
            return; // Can't edit while searching
        }
        let actual_index = self.get_actual_variable_index();
        if let Some(env) = &self.current_environment {
            if let Some(var) = env.values.get(actual_index) {
                let current_value = var.value.clone();
                self.variable_cursor_position = current_value.len();
                self.editing_variable = Some((actual_index, current_value));
            }
        }
    }

    fn get_actual_variable_index(&self) -> usize {
        if !self.variables_search_query.is_empty() && !self.variables_filtered_indices.is_empty() {
            self.variables_filtered_indices.get(self.variables_popup_index)
                .copied()
                .unwrap_or(0)
        } else {
            self.variables_popup_index
        }
    }

    // Variables search methods
    pub fn start_variables_search(&mut self) {
        if self.editing_variable.is_some() {
            return; // Can't search while editing
        }
        self.variables_search_active = true;
        self.variables_search_query.clear();
        self.variables_filtered_indices.clear();
    }

    pub fn variables_search_input_char(&mut self, c: char) {
        self.variables_search_query.push(c);
        self.update_variables_search_matches();
    }

    pub fn variables_search_backspace(&mut self) {
        self.variables_search_query.pop();
        self.update_variables_search_matches();
    }

    fn update_variables_search_matches(&mut self) {
        self.variables_filtered_indices.clear();
        self.variables_popup_index = 0;

        if self.variables_search_query.is_empty() {
            return;
        }

        let query = self.variables_search_query.to_lowercase();
        if let Some(env) = &self.current_environment {
            for (i, var) in env.values.iter().enumerate() {
                if var.key.to_lowercase().contains(&query)
                    || var.value.to_lowercase().contains(&query)
                {
                    self.variables_filtered_indices.push(i);
                }
            }
        }
    }

    pub fn confirm_variables_search(&mut self) {
        self.variables_search_active = false;
        // Keep the filtered results but allow navigation/editing
    }

    pub fn cancel_variables_search(&mut self) {
        self.variables_search_active = false;
        self.variables_search_query.clear();
        self.variables_filtered_indices.clear();
        self.variables_popup_index = 0;
    }

    pub fn variable_input_char(&mut self, c: char) {
        if let Some((_, ref mut value)) = self.editing_variable {
            value.insert(self.variable_cursor_position, c);
            self.variable_cursor_position += 1;
        }
    }

    pub fn variable_backspace(&mut self) {
        if let Some((_, ref mut value)) = self.editing_variable {
            if self.variable_cursor_position > 0 {
                self.variable_cursor_position -= 1;
                value.remove(self.variable_cursor_position);
            }
        }
    }

    pub fn variable_cursor_left(&mut self) {
        if self.variable_cursor_position > 0 {
            self.variable_cursor_position -= 1;
        }
    }

    pub fn variable_cursor_right(&mut self) {
        if let Some((_, ref value)) = self.editing_variable {
            if self.variable_cursor_position < value.len() {
                self.variable_cursor_position += 1;
            }
        }
    }

    pub fn confirm_variable_edit(&mut self) {
        if let Some((index, new_value)) = self.editing_variable.take() {
            if let Some(env) = &mut self.current_environment {
                if let Some(var) = env.values.get_mut(index) {
                    log_error("confirm_variable_edit", &format!(
                        "index={}, key={}, old_value={}, new_value={}, changed={}",
                        index, var.key, var.value, new_value, var.value != new_value
                    ));
                    if var.value != new_value {
                        var.value = new_value;
                        self.variables_modified = true;
                        self.rebuild_variables();
                    }
                }
            }
        }
        self.variable_cursor_position = 0;
    }

    pub fn cancel_variable_edit(&mut self) {
        self.editing_variable = None;
        self.variable_cursor_position = 0;
    }

    pub async fn save_variables_to_postman(&mut self) {
        log_error("save_variables_to_postman", &format!(
            "called, variables_modified={}",
            self.variables_modified
        ));
        if !self.variables_modified {
            self.status_message = String::from("No changes to save");
            return;
        }

        let env_idx = match self.selected_environment_index {
            Some(idx) => idx,
            None => {
                self.status_message = String::from("No environment selected");
                return;
            }
        };

        let env_info = match self.environments.get(env_idx) {
            Some(info) => info.clone(),
            None => {
                self.status_message = String::from("Environment not found");
                return;
            }
        };

        let env_detail = match &self.current_environment {
            Some(detail) => detail.clone(),
            None => {
                self.status_message = String::from("No environment loaded");
                return;
            }
        };

        self.status_message = String::from("Saving variables...");

        // Replace empty keys with "NOT_SET" to avoid API rejection
        let values: Vec<_> = env_detail.values.iter()
            .map(|v| {
                if v.key.trim().is_empty() {
                    crate::api::Variable {
                        key: "NOT_SET".to_string(),
                        value: v.value.clone(),
                        enabled: v.enabled,
                    }
                } else {
                    v.clone()
                }
            })
            .collect();

        match self.client.update_environment(&env_info.uid, &env_info.name, &values).await {
            Ok(()) => {
                self.variables_modified = false;
                self.status_message = String::from("Variables saved successfully");
            }
            Err(e) => {
                let error_msg = e.to_string();
                log_error("save_variables", &error_msg);
                self.error = Some(error_msg);
                self.status_message = String::from("Failed to save variables");
            }
        }
    }

    pub fn get_variables_for_display(&self) -> Vec<(usize, String, String, bool)> {
        match &self.current_environment {
            Some(env) => {
                if !self.variables_search_query.is_empty() {
                    // Return only filtered variables with their original indices
                    self.variables_filtered_indices.iter()
                        .filter_map(|&i| env.values.get(i).map(|v| (i, v.key.clone(), v.value.clone(), v.enabled.unwrap_or(true))))
                        .collect()
                } else {
                    // Return all variables with indices
                    env.values.iter()
                        .enumerate()
                        .map(|(i, v)| (i, v.key.clone(), v.value.clone(), v.enabled.unwrap_or(true)))
                        .collect()
                }
            }
            None => Vec::new(),
        }
    }

    pub fn start_collection_load(&mut self) -> bool {
        if self.flat_collections.is_empty() {
            return false;
        }

        let flat_collection = &self.flat_collections[self.selected_collection_index];

        // If it's the favorites folder, toggle it instead of loading
        if flat_collection.is_favorites_folder {
            self.toggle_collections_favorites_folder();
            return false;
        }

        let collection_name = flat_collection.name.clone();
        self.collection_loading = Some(collection_name);
        true
    }

    pub async fn load_collection_data(&mut self) {
        let (collection_uid, collection_name) = {
            let flat_collection = &self.flat_collections[self.selected_collection_index];
            (flat_collection.uid.clone(), flat_collection.name.clone())
        };

        match self.client.get_collection(&collection_uid).await {
            Ok(detail) => {
                self.current_collection = Some(detail);
                self.rebuild_variables();
                self.expanded_folders.clear();
                self.flatten_items();
                self.selected_item_index = 0;
                self.current_request = None;
                self.response = None;
                self.set_focus(FocusedPane::Requests);
                self.save_last_state();
                self.status_message = format!("Loaded {}", collection_name);
            }
            Err(e) => {
                let error_msg = e.to_string();
                log_error("load_collection_data", &error_msg);
                self.error = Some(error_msg);
                self.status_message = String::from("Failed to load collection");
            }
        }
        self.collection_loading = None;
    }

    // Keep the old method for restore_last_state which needs synchronous behavior
    pub async fn load_collection_detail(&mut self) -> Result<()> {
        if self.flat_collections.is_empty() {
            return Ok(());
        }

        let flat_collection = &self.flat_collections[self.selected_collection_index];

        // If it's the favorites folder, toggle it instead of loading
        if flat_collection.is_favorites_folder {
            self.toggle_collections_favorites_folder();
            return Ok(());
        }

        let collection_uid = flat_collection.uid.clone();
        let collection_name = flat_collection.name.clone();
        self.loading = true;
        self.status_message = format!("Loading {}...", collection_name);

        match self.client.get_collection(&collection_uid).await {
            Ok(detail) => {
                self.current_collection = Some(detail);
                self.rebuild_variables(); // Rebuild variables with new collection
                self.expanded_folders.clear();
                self.flatten_items();
                self.selected_item_index = 0;
                self.current_request = None;
                self.response = None;
                self.loading = false;
                self.set_focus(FocusedPane::Requests);
                self.save_last_state();
            }
            Err(e) => {
                self.loading = false;
                let error_msg = e.to_string();
                log_error("load_collection_detail", &error_msg);
                self.error = Some(error_msg);
                self.status_message = String::from("Failed to load collection");
            }
        }
        Ok(())
    }

    pub fn flatten_items(&mut self) {
        self.flat_items.clear();
        if let Some(collection) = &self.current_collection {
            let items = collection.item.clone();
            let expanded = self.expanded_folders.clone();
            let collection_uid = if self.collections.is_empty() {
                String::new()
            } else {
                self.collections[self.selected_collection_index].uid.clone()
            };
            let favorite_paths: Vec<Vec<usize>> = self.config.favorite_requests
                .iter()
                .filter(|f| f.collection_uid == collection_uid)
                .map(|f| f.path.clone())
                .collect();

            // Add Favorites section at the top if there are any
            if !favorite_paths.is_empty() {
                // Favorites folder is expanded by default (only collapsed if explicitly toggled)
                let favorites_expanded = !self.expanded_folders.contains(&vec![usize::MAX]);
                self.flat_items.push(FlatItem {
                    name: String::from("Favorites"),
                    depth: 0,
                    is_folder: true,
                    expanded: favorites_expanded,
                    request: None,
                    request_id: None,
                    path: vec![usize::MAX], // Special path for favorites folder
                });

                if favorites_expanded {
                    // Add favorite items
                    for fav_path in &favorite_paths {
                        if let Some((name, request, request_id)) = get_item_at_path(&items, fav_path) {
                            self.flat_items.push(FlatItem {
                                name,
                                depth: 1,
                                is_folder: false,
                                expanded: false,
                                request,
                                request_id,
                                path: fav_path.clone(),
                            });
                        }
                    }
                }
            }

            // Then add the normal tree
            flatten_recursive(&items, 0, vec![], &expanded, &mut self.flat_items);
        }
    }

    pub fn toggle_folder(&mut self) {
        if self.flat_items.is_empty() {
            return;
        }

        let item = &self.flat_items[self.selected_item_index];
        if !item.is_folder {
            return;
        }

        let path = item.path.clone();
        if self.expanded_folders.contains(&path) {
            self.expanded_folders.remove(&path);
        } else {
            self.expanded_folders.insert(path);
        }
        self.flatten_items();
    }

    pub fn select_request(&mut self) {
        if self.flat_items.is_empty() {
            return;
        }

        let item = &self.flat_items[self.selected_item_index];
        let item_path = item.path.clone();

        if let Some(request) = &item.request {
            // Check if there's a local edit for this request
            if let Some(local_edit) = self.get_local_edit(&item_path) {
                // Apply the local edit
                self.current_request = Some(Request {
                    method: local_edit.method.clone(),
                    url: if local_edit.url.is_empty() {
                        RequestUrl::Empty
                    } else {
                        RequestUrl::Simple(local_edit.url.clone())
                    },
                    header: request.header.clone(),
                    body: if local_edit.body.is_empty() {
                        None
                    } else {
                        Some(crate::api::RequestBody {
                            mode: Some("raw".to_string()),
                            raw: Some(local_edit.body.clone()),
                        })
                    },
                    description: request.description.clone(),
                });
                // Also set unsaved_edit so the UI knows it's modified
                self.unsaved_edit = Some((local_edit, self.selected_item_index));
            } else {
                self.current_request = Some(request.clone());
                self.unsaved_edit = None;
            }
            self.response = None;
            self.json_viewer_state = None;
            self.set_focus(FocusedPane::Preview);

            // Save state for persistence
            self.save_last_state();
        }
    }

    pub fn save_last_state(&mut self) {
        if self.flat_collections.is_empty() {
            return;
        }

        let flat_col = &self.flat_collections[self.selected_collection_index];
        // Don't save state if on Favorites folder
        if flat_col.is_favorites_folder || flat_col.uid.is_empty() {
            return;
        }
        let collection_uid = flat_col.uid.clone();
        let request_path = if self.flat_items.is_empty() {
            vec![]
        } else {
            let path = self.flat_items[self.selected_item_index].path.clone();
            // Don't save if path contains usize::MAX (Favorites marker) - TOML can't serialize it
            if path.contains(&usize::MAX) {
                vec![]
            } else {
                path
            }
        };

        let environment_uid = self.selected_environment_index
            .and_then(|idx| self.environments.get(idx))
            .map(|env| env.uid.clone());

        let workspace_id = self.get_selected_workspace_id();

        self.config.set_last_state(collection_uid, request_path, environment_uid, workspace_id);
        let _ = self.config.save(); // Ignore errors for state saving
    }

    pub fn save_environment_state(&mut self) {
        let environment_uid = self.selected_environment_index
            .and_then(|idx| self.environments.get(idx))
            .map(|env| env.uid.clone());

        self.config.set_last_environment(environment_uid);
        let _ = self.config.save();
    }

    pub async fn restore_last_state(&mut self) -> Result<()> {
        let last_state = match &self.config.last_state {
            Some(state) => state.clone(),
            None => return Ok(()),
        };

        // Workspace is already restored in load_collections()

        // Restore environment
        if let Some(env_uid) = &last_state.environment_uid {
            if let Some(env_index) = self.environments.iter().position(|e| &e.uid == env_uid) {
                self.selected_environment_index = Some(env_index);
                self.load_selected_environment().await;
            }
        }

        // Skip collection restore if no collection was saved
        if last_state.collection_uid.is_empty() {
            return Ok(());
        }

        // Find the collection in flat_collections by UID
        let collection_index = self.flat_collections
            .iter()
            .position(|c| !c.is_favorites_folder && c.uid == last_state.collection_uid);

        let collection_index = match collection_index {
            Some(idx) => idx,
            None => return Ok(()), // Collection no longer exists
        };

        self.selected_collection_index = collection_index;

        // Load the collection
        self.load_collection_detail().await?;

        // Find the request by path
        if let Some(item_index) = self.flat_items
            .iter()
            .position(|item| item.path == last_state.request_path)
        {
            self.selected_item_index = item_index;
            self.update_preview_from_selection();
        } else {
            // Path doesn't exist, try to expand folders to find it
            self.expand_to_path(&last_state.request_path);
            if let Some(item_index) = self.flat_items
                .iter()
                .position(|item| item.path == last_state.request_path)
            {
                self.selected_item_index = item_index;
                self.update_preview_from_selection();
            }
        }

        Ok(())
    }

    fn expand_to_path(&mut self, target_path: &[usize]) {
        // Expand all parent folders leading to the target
        for i in 1..target_path.len() {
            let parent_path: Vec<usize> = target_path[..i].to_vec();
            self.expanded_folders.insert(parent_path);
        }
        self.flatten_items();
    }

    /// Check if the HTTP method modifies data and requires confirmation
    pub fn is_destructive_method(method: &str) -> bool {
        matches!(
            method.to_uppercase().as_str(),
            "POST" | "PUT" | "DELETE" | "PATCH"
        )
    }

    /// Start the execute confirmation dialog for destructive requests
    pub fn start_execute_confirmation(&mut self) -> bool {
        if let Some(request) = &self.current_request {
            let method = request.method.to_uppercase();
            if Self::is_destructive_method(&method) {
                // Get the request name from flat_items
                let name = self.flat_items.get(self.selected_item_index)
                    .map(|item| item.name.clone())
                    .unwrap_or_else(|| "Unknown".to_string());

                self.pending_execute = Some(PendingExecute {
                    method: method.clone(),
                    url: self.substitute_variables(&request.url.to_string()),
                    name,
                });
                self.input_mode = InputMode::ExecuteConfirm;
                self.status_message = format!("Confirm {} request? (y/n)", method);
                return true; // Needs confirmation
            }
        }
        false // No confirmation needed (GET, HEAD, OPTIONS, etc.)
    }

    /// Cancel the execute confirmation
    pub fn cancel_execute_confirmation(&mut self) {
        self.pending_execute = None;
        self.input_mode = InputMode::Normal;
        self.status_message = String::from("Request cancelled");
    }

    pub async fn execute_current_request(&mut self) -> Result<()> {
        if let Some(request) = &self.current_request {
            self.loading = true;
            self.status_message = String::from("Executing request...");

            // Create a copy of the request with variables substituted
            let mut resolved_request = request.clone();

            // Substitute variables in URL
            let url_str = resolved_request.url.to_string();
            let resolved_url = self.substitute_variables(&url_str);
            resolved_request.url = RequestUrl::Simple(resolved_url);

            // Substitute variables in headers
            for header in &mut resolved_request.header {
                header.key = self.substitute_variables(&header.key);
                header.value = self.substitute_variables(&header.value);
            }

            // Substitute variables in body
            if let Some(body) = &mut resolved_request.body {
                if let Some(raw) = &body.raw {
                    body.raw = Some(self.substitute_variables(raw));
                }
            }

            match self.client.execute_request(&resolved_request).await {
                Ok(response) => {
                    // Try to parse response body as JSON for the viewer
                    self.json_viewer_state = JsonViewerState::new(&response.body);
                    self.response = Some(response);
                    self.loading = false;
                    self.request_executing = false;
                    self.status_message = String::from("Request completed");
                }
                Err(e) => {
                    self.loading = false;
                    self.request_executing = false;
                    // Include root cause in error message
                    let error_msg = format!("{:#}", e);
                    log_error("execute_request", &error_msg);
                    self.error = Some(error_msg);
                    self.status_message = String::from("Request failed");
                }
            }
        }
        Ok(())
    }

    pub fn store_local_edit(&mut self, edited: EditableRequest, item_index: usize) {
        // Update the preview with the edited request
        self.current_request = Some(Request {
            method: edited.method.clone(),
            url: if edited.url.is_empty() {
                RequestUrl::Empty
            } else {
                RequestUrl::Simple(edited.url.clone())
            },
            header: self.current_request.as_ref().map(|r| r.header.clone()).unwrap_or_default(),
            body: if edited.body.is_empty() {
                None
            } else {
                Some(crate::api::RequestBody {
                    mode: Some("raw".to_string()),
                    raw: Some(edited.body.clone()),
                })
            },
            description: self.current_request.as_ref().and_then(|r| r.description.clone()),
        });

        // Persist to local storage
        if let Some(collection_uid) = self.get_current_collection_uid() {
            let path = self.flat_items[item_index].path.clone();
            self.local_edits.set_edit(
                collection_uid,
                path,
                edited.name.clone(),
                edited.method.clone(),
                edited.url.clone(),
                edited.body.clone(),
            );
            if let Err(e) = self.local_edits.save() {
                log_error("save_local_edit", &e.to_string());
            }
        }

        self.unsaved_edit = Some((edited, item_index));
        self.status_message = String::from("Changes stored locally. Press S to save to Postman.");
    }

    pub fn has_unsaved_edit(&self) -> bool {
        self.unsaved_edit.is_some()
    }

    /// Get the UID of the currently selected collection
    pub fn get_current_collection_uid(&self) -> Option<String> {
        if self.selected_collection_index < self.collections.len() {
            Some(self.collections[self.selected_collection_index].uid.clone())
        } else {
            None
        }
    }

    /// Check if a request at the given path has a local edit
    pub fn has_local_edit(&self, path: &[usize]) -> bool {
        if let Some(collection_uid) = self.get_current_collection_uid() {
            self.local_edits.has_edit(&collection_uid, path)
        } else {
            false
        }
    }

    /// Get local edit for a request if it exists
    pub fn get_local_edit(&self, path: &[usize]) -> Option<EditableRequest> {
        let collection_uid = self.get_current_collection_uid()?;
        let edit = self.local_edits.get_edit(&collection_uid, path)?;
        Some(EditableRequest {
            name: edit.name.clone(),
            method: edit.method.clone(),
            url: edit.url.clone(),
            body: edit.body.clone(),
        })
    }

    /// Remove local edit after successful save to Postman
    pub fn clear_local_edit(&mut self, path: &[usize]) {
        if let Some(collection_uid) = self.get_current_collection_uid() {
            self.local_edits.remove_edit(&collection_uid, path);
            if let Err(e) = self.local_edits.save() {
                log_error("clear_local_edit", &e.to_string());
            }
        }
    }

    pub fn start_saving_edit(&mut self) {
        if let Some((edited, item_index)) = self.unsaved_edit.take() {
            self.pending_save = Some(PendingSave { edited, item_index });
            self.input_mode = InputMode::Saving;
            self.status_message = String::from("Saving changes to Postman...");
        } else {
            self.status_message = String::from("No unsaved changes");
        }
    }

    pub fn cancel_saving(&mut self) {
        self.pending_save = None;
        self.input_mode = InputMode::Normal;
        self.status_message = String::from("Save cancelled");
    }

    pub fn get_current_request_for_edit(&self) -> Option<(EditableRequest, usize)> {
        let item = self.flat_items.get(self.selected_item_index)?;
        let request = item.request.as_ref()?;

        let editable = EditableRequest {
            name: item.name.clone(),
            method: request.method.clone(),
            url: request.url.to_string(),
            body: request.body.as_ref()
                .and_then(|b| b.raw.clone())
                .unwrap_or_default(),
        };

        Some((editable, self.selected_item_index))
    }

    pub fn move_up(&mut self) {
        match self.focused_pane {
            FocusedPane::Collections => {
                if self.selected_collection_index > 0 {
                    self.selected_collection_index -= 1;
                }
            }
            FocusedPane::Requests => {
                if self.selected_item_index > 0 {
                    self.selected_item_index -= 1;
                    self.update_preview_from_selection();
                    self.save_last_state();
                }
            }
            FocusedPane::Preview | FocusedPane::Response => {}
        }
    }

    pub fn move_down(&mut self) {
        match self.focused_pane {
            FocusedPane::Collections => {
                if !self.flat_collections.is_empty()
                    && self.selected_collection_index < self.flat_collections.len() - 1
                {
                    self.selected_collection_index += 1;
                }
            }
            FocusedPane::Requests => {
                if !self.flat_items.is_empty()
                    && self.selected_item_index < self.flat_items.len() - 1
                {
                    self.selected_item_index += 1;
                    self.update_preview_from_selection();
                    self.save_last_state();
                }
            }
            FocusedPane::Preview | FocusedPane::Response => {}
        }
    }

    pub fn update_preview_from_selection(&mut self) {
        if let Some(item) = self.flat_items.get(self.selected_item_index) {
            if let Some(request) = &item.request {
                self.current_request = Some(request.clone());
                self.response = None;
            }
        }
    }

    pub fn start_new_request_dialog(&mut self) {
        if self.current_collection.is_none() {
            return;
        }

        let target_folder_path = self.get_current_folder_path();

        self.new_request_dialog = Some(NewRequestDialog {
            step: DialogStep::Name,
            name: String::new(),
            url: String::new(),
            cursor_position: 0,
            target_folder_path,
        });
        self.input_mode = InputMode::TextInput;
        self.status_message = String::from("Enter request name");
    }

    fn get_current_folder_path(&self) -> Vec<usize> {
        if self.flat_items.is_empty() {
            return vec![];
        }

        let item = &self.flat_items[self.selected_item_index];

        if item.is_folder {
            // If cursor is on a folder, use that folder
            item.path.clone()
        } else {
            // If cursor is on a request, use parent folder
            let mut path = item.path.clone();
            path.pop(); // Remove the request's index to get parent folder path
            path
        }
    }

    pub fn dialog_input_char(&mut self, c: char) {
        if let Some(dialog) = &mut self.new_request_dialog {
            let input = match dialog.step {
                DialogStep::Name => &mut dialog.name,
                DialogStep::Url => &mut dialog.url,
            };
            input.insert(dialog.cursor_position, c);
            dialog.cursor_position += 1;
        }
    }

    pub fn dialog_backspace(&mut self) {
        if let Some(dialog) = &mut self.new_request_dialog {
            let input = match dialog.step {
                DialogStep::Name => &mut dialog.name,
                DialogStep::Url => &mut dialog.url,
            };
            if dialog.cursor_position > 0 {
                dialog.cursor_position -= 1;
                input.remove(dialog.cursor_position);
            }
        }
    }

    pub fn dialog_move_cursor_left(&mut self) {
        if let Some(dialog) = &mut self.new_request_dialog {
            if dialog.cursor_position > 0 {
                dialog.cursor_position -= 1;
            }
        }
    }

    pub fn dialog_move_cursor_right(&mut self) {
        if let Some(dialog) = &mut self.new_request_dialog {
            let len = match dialog.step {
                DialogStep::Name => dialog.name.len(),
                DialogStep::Url => dialog.url.len(),
            };
            if dialog.cursor_position < len {
                dialog.cursor_position += 1;
            }
        }
    }

    pub fn dialog_next_step(&mut self) -> bool {
        if let Some(dialog) = &mut self.new_request_dialog {
            match dialog.step {
                DialogStep::Name => {
                    if dialog.name.trim().is_empty() {
                        self.status_message = String::from("Name cannot be empty");
                        return false;
                    }
                    dialog.step = DialogStep::Url;
                    dialog.cursor_position = 0;
                    self.status_message = String::from("Enter request URL (or leave empty)");
                    false
                }
                DialogStep::Url => {
                    // Ready to create request
                    true
                }
            }
        } else {
            false
        }
    }

    pub fn cancel_dialog(&mut self) {
        self.new_request_dialog = None;
        self.input_mode = InputMode::Normal;
        self.status_message = String::from("Use j/k to navigate, Enter to select, e to execute, a to add request");
    }

    pub async fn create_new_request(&mut self) -> Result<()> {
        let dialog = match self.new_request_dialog.take() {
            Some(d) => d,
            None => return Ok(()),
        };

        self.input_mode = InputMode::Normal;
        self.loading = true;
        self.status_message = String::from("Creating request...");

        let collection = match &self.current_collection {
            Some(c) => c.clone(),
            None => {
                self.loading = false;
                self.status_message = String::from("No collection loaded");
                return Ok(());
            }
        };

        let collection_uid = self.collections[self.selected_collection_index].uid.clone();

        // Create the new request item
        let new_request = Item::Request(RequestItem {
            id: None,
            name: dialog.name.clone(),
            request: Request {
                method: String::from("GET"),
                url: if dialog.url.is_empty() {
                    RequestUrl::Empty
                } else {
                    RequestUrl::Simple(dialog.url)
                },
                header: vec![],
                body: None,
                description: None,
            },
            response: vec![],
        });

        // Clone and modify the collection's items
        let mut items = collection.item.clone();
        insert_item_at_path(&mut items, &dialog.target_folder_path, new_request);

        // Update the collection via API
        match self.client.update_collection(&collection_uid, &collection.info, &items).await {
            Ok(()) => {
                // Reload collection to get updated state
                match self.client.get_collection(&collection_uid).await {
                    Ok(detail) => {
                        self.current_collection = Some(detail);
                        self.flatten_items();
                        self.loading = false;
                        self.status_message = format!("Created request '{}'", dialog.name);
                    }
                    Err(e) => {
                        self.loading = false;
                        let error_msg = e.to_string();
                        log_error("create_new_request:refresh", &error_msg);
                        self.error = Some(error_msg);
                        self.status_message = String::from("Request created but failed to refresh");
                    }
                }
            }
            Err(e) => {
                self.loading = false;
                let error_msg = e.to_string();
                log_error("create_new_request:save", &error_msg);
                self.error = Some(error_msg);
                self.status_message = String::from("Failed to create request");
            }
        }

        Ok(())
    }

    // Search methods
    pub fn start_search(&mut self) {
        if self.focused_pane == FocusedPane::Preview {
            return;
        }
        self.search_query.clear();
        self.search_matches.clear();
        self.current_match_index = 0;
        self.pre_search_index = match self.focused_pane {
            FocusedPane::Collections => self.selected_collection_index,
            FocusedPane::Requests => self.selected_item_index,
            FocusedPane::Preview | FocusedPane::Response => 0,
        };
        self.input_mode = InputMode::Search;
        self.status_message = String::from("Type to search, Enter to confirm, Esc to cancel");
    }

    pub fn search_input_char(&mut self, c: char) {
        self.search_query.push(c);
        self.update_search_matches();
        let has_matches = if self.focused_pane == FocusedPane::Requests {
            !self.search_match_paths.is_empty()
        } else {
            !self.search_matches.is_empty()
        };
        if has_matches {
            self.current_match_index = 0;
            self.jump_to_current_match();
        }
    }

    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.update_search_matches();
        let has_matches = if self.focused_pane == FocusedPane::Requests {
            !self.search_match_paths.is_empty()
        } else {
            !self.search_matches.is_empty()
        };
        if has_matches {
            self.current_match_index = 0;
            self.jump_to_current_match();
        }
    }

    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        self.search_match_paths.clear();
        if self.search_query.is_empty() {
            return;
        }

        let query = self.search_query.to_lowercase();

        match self.focused_pane {
            FocusedPane::Collections => {
                for (i, collection) in self.collections.iter().enumerate() {
                    if collection.name.to_lowercase().contains(&query) {
                        self.search_matches.push(i);
                    }
                }
            }
            FocusedPane::Requests => {
                // Search through entire collection tree, not just visible items
                if let Some(collection) = &self.current_collection {
                    let items = collection.item.clone();
                    search_items_recursive(&items, &query, vec![], &mut self.search_match_paths);
                }
            }
            FocusedPane::Preview | FocusedPane::Response => {}
        }

        self.update_search_status();
    }

    fn update_search_status(&mut self) {
        let match_count = if self.focused_pane == FocusedPane::Requests {
            self.search_match_paths.len()
        } else {
            self.search_matches.len()
        };

        if match_count == 0 {
            if self.search_query.is_empty() {
                self.status_message = String::from("Type to search, Enter to confirm, Esc to cancel");
            } else {
                self.status_message = format!("/{} - No matches", self.search_query);
            }
        } else {
            self.status_message = format!(
                "/{} - Match {}/{}",
                self.search_query,
                self.current_match_index + 1,
                match_count
            );
        }
    }

    fn jump_to_current_match(&mut self) {
        match self.focused_pane {
            FocusedPane::Collections => {
                if let Some(&index) = self.search_matches.get(self.current_match_index) {
                    self.selected_collection_index = index;
                }
            }
            FocusedPane::Requests => {
                if let Some(path) = self.search_match_paths.get(self.current_match_index).cloned() {
                    // Expand all parent folders to make the item visible
                    self.expand_to_path(&path);
                    // Find the item in flat_items by path
                    if let Some(item_index) = self.flat_items.iter().position(|item| item.path == path) {
                        self.selected_item_index = item_index;
                        self.update_preview_from_selection();
                    }
                }
            }
            FocusedPane::Preview | FocusedPane::Response => {}
        }
    }

    pub fn confirm_search(&mut self) {
        self.input_mode = InputMode::Normal;
        self.update_status_for_pane();
        // Save state after search selection
        if self.focused_pane == FocusedPane::Requests {
            self.save_last_state();
        }
    }

    pub fn cancel_search(&mut self) {
        // Restore original selection
        match self.focused_pane {
            FocusedPane::Collections => self.selected_collection_index = self.pre_search_index,
            FocusedPane::Requests => self.selected_item_index = self.pre_search_index,
            FocusedPane::Preview | FocusedPane::Response => {}
        }
        self.search_query.clear();
        self.search_matches.clear();
        self.input_mode = InputMode::Normal;
        self.update_status_for_pane();
    }

    pub fn next_match(&mut self) {
        let match_count = if self.focused_pane == FocusedPane::Requests {
            self.search_match_paths.len()
        } else {
            self.search_matches.len()
        };
        if match_count == 0 {
            return;
        }
        self.current_match_index = (self.current_match_index + 1) % match_count;
        self.jump_to_current_match();
        self.update_search_status();
    }

    pub fn prev_match(&mut self) {
        let match_count = if self.focused_pane == FocusedPane::Requests {
            self.search_match_paths.len()
        } else {
            self.search_matches.len()
        };
        if match_count == 0 {
            return;
        }
        if self.current_match_index == 0 {
            self.current_match_index = match_count - 1;
        } else {
            self.current_match_index -= 1;
        }
        self.jump_to_current_match();
        self.update_search_status();
    }

    // JSON viewer methods
    pub fn json_viewer_up(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.up();
        }
    }

    pub fn json_viewer_down(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.down();
        }
    }

    pub fn json_viewer_expand(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.expand();
        }
    }

    pub fn json_viewer_collapse(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.collapse();
        }
    }

    pub fn json_viewer_toggle(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.toggle();
        }
    }

    pub fn json_viewer_expand_all(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.expand_all();
        }
    }

    pub fn json_viewer_collapse_all(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.collapse_all();
        }
    }

    pub fn json_search_start(&mut self) {
        if self.json_viewer_state.is_some() {
            if let Some(ref mut viewer) = self.json_viewer_state {
                viewer.start_search();
            }
            self.input_mode = InputMode::JsonSearch;
            self.status_message = String::from("Type to search JSON, Enter to confirm, Esc to cancel");
        }
    }

    pub fn json_search_input(&mut self, c: char) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.search_input(c);
            self.status_message = viewer.search_status();
        }
    }

    pub fn json_search_backspace(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.search_backspace();
            self.status_message = viewer.search_status();
        }
    }

    pub fn json_search_confirm(&mut self) {
        self.input_mode = InputMode::Normal;
        self.update_status_for_pane();
    }

    pub fn json_search_cancel(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.search_query.clear();
            viewer.search_matches.clear();
        }
        self.input_mode = InputMode::Normal;
        self.update_status_for_pane();
    }

    pub fn json_search_next(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.next_match();
            self.status_message = viewer.search_status();
        }
    }

    pub fn json_search_prev(&mut self) {
        if let Some(ref mut viewer) = self.json_viewer_state {
            viewer.prev_match();
            self.status_message = viewer.search_status();
        }
    }
}

fn flatten_recursive(
    items: &[Item],
    depth: usize,
    path: Vec<usize>,
    expanded_folders: &HashSet<Vec<usize>>,
    flat_items: &mut Vec<FlatItem>,
) {
    for (i, item) in items.iter().enumerate() {
        let mut current_path = path.clone();
        current_path.push(i);

        match item {
            Item::Folder(folder) => {
                let is_expanded = expanded_folders.contains(&current_path);
                flat_items.push(FlatItem {
                    name: folder.name.clone(),
                    depth,
                    is_folder: true,
                    expanded: is_expanded,
                    request: None,
                    request_id: None,
                    path: current_path.clone(),
                });
                if is_expanded {
                    flatten_recursive(&folder.item, depth + 1, current_path, expanded_folders, flat_items);
                }
            }
            Item::Request(req_item) => {
                flat_items.push(FlatItem {
                    name: req_item.name.clone(),
                    depth,
                    is_folder: false,
                    expanded: false,
                    request: Some(req_item.request.clone()),
                    request_id: req_item.id.clone(),
                    path: current_path,
                });
            }
        }
    }
}

fn get_item_at_path(items: &[Item], path: &[usize]) -> Option<(String, Option<Request>, Option<String>)> {
    if path.is_empty() {
        return None;
    }

    let index = path[0];
    if index >= items.len() {
        return None;
    }

    let remaining_path = &path[1..];

    match &items[index] {
        Item::Folder(folder) => {
            if remaining_path.is_empty() {
                Some((folder.name.clone(), None, None))
            } else {
                get_item_at_path(&folder.item, remaining_path)
            }
        }
        Item::Request(req_item) => {
            if remaining_path.is_empty() {
                Some((req_item.name.clone(), Some(req_item.request.clone()), req_item.id.clone()))
            } else {
                None
            }
        }
    }
}

fn insert_item_at_path(items: &mut Vec<Item>, path: &[usize], new_item: Item) {
    insert_item_recursive(items, path, new_item);
}

fn insert_item_recursive(items: &mut Vec<Item>, path: &[usize], new_item: Item) {
    if path.is_empty() {
        items.push(new_item);
        return;
    }

    let index = path[0];
    let remaining_path = &path[1..];

    if index >= items.len() {
        // Index out of bounds, add to current level
        items.push(new_item);
        return;
    }

    match &mut items[index] {
        Item::Folder(folder) => {
            insert_item_recursive(&mut folder.item, remaining_path, new_item);
        }
        Item::Request(_) => {
            // Path points to a request, add to current level
            items.push(new_item);
        }
    }
}

fn search_items_recursive(
    items: &[Item],
    query: &str,
    path: Vec<usize>,
    matches: &mut Vec<Vec<usize>>,
) {
    for (i, item) in items.iter().enumerate() {
        let mut current_path = path.clone();
        current_path.push(i);

        match item {
            Item::Folder(folder) => {
                // Check if folder name matches
                if folder.name.to_lowercase().contains(query) {
                    matches.push(current_path.clone());
                }
                // Recurse into folder
                search_items_recursive(&folder.item, query, current_path, matches);
            }
            Item::Request(req_item) => {
                // Check if request name matches
                if req_item.name.to_lowercase().contains(query) {
                    matches.push(current_path);
                }
            }
        }
    }
}
