use ratatui::style::{Color, Style};
use ratatui::text::{Line, Span};
use serde_json::Value;
use tui_tree_widget::{TreeItem, TreeState};

/// Segment of a path to a JSON node
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub enum JsonPathSegment {
    Root,
    Key(String),
    Index(usize),
}

/// Unique identifier for a JSON tree node
pub type JsonNodeId = Vec<JsonPathSegment>;

/// Color scheme for JSON syntax highlighting
pub struct JsonColors {
    pub key: Color,
    pub string: Color,
    pub number: Color,
    pub boolean: Color,
    pub null: Color,
    pub bracket: Color,
    pub colon: Color,
    pub match_highlight: Color,
}

impl Default for JsonColors {
    fn default() -> Self {
        Self {
            key: Color::Cyan,
            string: Color::Green,
            number: Color::Yellow,
            boolean: Color::Magenta,
            null: Color::Red,
            bracket: Color::White,
            colon: Color::White,
            match_highlight: Color::Black,
        }
    }
}

/// State for the interactive JSON viewer
pub struct JsonViewerState {
    /// Tree widget state for navigation (uses path segments, not full paths)
    pub tree_state: TreeState<JsonPathSegment>,
    /// The parsed JSON value
    pub json: Value,
    /// Search query
    pub search_query: String,
    /// All node IDs that match the search
    pub search_matches: Vec<JsonNodeId>,
    /// Current match index
    pub current_match_index: usize,
    /// Color scheme
    pub colors: JsonColors,
    /// Whether we need to expand all on next render
    needs_expand: bool,
}

impl JsonViewerState {
    /// Create a new JSON viewer state from a JSON string
    pub fn new(json_str: &str) -> Option<Self> {
        let json: Value = serde_json::from_str(json_str).ok()?;

        let state = Self {
            tree_state: TreeState::default(),
            json,
            search_query: String::new(),
            search_matches: Vec::new(),
            current_match_index: 0,
            colors: JsonColors::default(),
            needs_expand: true, // Expand on first render
        };

        Some(state)
    }

    /// Move selection up
    pub fn up(&mut self) {
        self.tree_state.key_up();
    }

    /// Move selection down
    pub fn down(&mut self) {
        self.tree_state.key_down();
    }

    /// Expand the selected node
    pub fn expand(&mut self) {
        self.tree_state.key_right();
    }

    /// Collapse the selected node
    pub fn collapse(&mut self) {
        self.tree_state.key_left();
    }

    /// Toggle expand/collapse of the selected node
    pub fn toggle(&mut self) {
        self.tree_state.toggle_selected();
    }

    /// Expand all nodes in the tree
    pub fn expand_all(&mut self) {
        let mut paths_to_open: Vec<JsonNodeId> = Vec::new();
        self.collect_expandable_paths(&self.json.clone(), vec![JsonPathSegment::Root], &mut paths_to_open);
        for path in paths_to_open {
            // open() expects Vec<JsonPathSegment> - the accumulated path
            self.tree_state.open(path);
        }
    }

    /// Collapse all nodes in the tree
    pub fn collapse_all(&mut self) {
        self.tree_state.close_all();
        // Keep root open
        self.tree_state.open(vec![JsonPathSegment::Root]);
    }

    /// Collect all accumulated paths to expandable nodes (objects and arrays)
    fn collect_expandable_paths(&self, value: &Value, path: JsonNodeId, paths: &mut Vec<JsonNodeId>) {
        match value {
            Value::Object(map) => {
                paths.push(path.clone());
                for (key, val) in map {
                    let mut child_path = path.clone();
                    child_path.push(JsonPathSegment::Key(key.clone()));
                    self.collect_expandable_paths(val, child_path, paths);
                }
            }
            Value::Array(arr) => {
                paths.push(path.clone());
                for (i, val) in arr.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(JsonPathSegment::Index(i));
                    self.collect_expandable_paths(val, child_path, paths);
                }
            }
            _ => {}
        }
    }

    /// Start a new search
    pub fn start_search(&mut self) {
        self.search_query.clear();
        self.search_matches.clear();
        self.current_match_index = 0;
    }

    /// Add a character to the search query
    pub fn search_input(&mut self, c: char) {
        self.search_query.push(c);
        self.update_search_matches();
    }

    /// Remove the last character from the search query
    pub fn search_backspace(&mut self) {
        self.search_query.pop();
        self.update_search_matches();
    }

    /// Update search matches based on current query
    fn update_search_matches(&mut self) {
        self.search_matches.clear();
        self.current_match_index = 0;

        if self.search_query.is_empty() {
            return;
        }

        let query = self.search_query.to_lowercase();
        // Start with Root in the path
        self.find_matches(&self.json.clone(), vec![JsonPathSegment::Root], &query);

        // Jump to first match
        if !self.search_matches.is_empty() {
            self.jump_to_match(0);
        }
    }

    /// Recursively find all nodes matching the search query
    fn find_matches(&mut self, value: &Value, path: JsonNodeId, query: &str) {
        match value {
            Value::Object(map) => {
                for (key, val) in map {
                    let mut child_path = path.clone();
                    child_path.push(JsonPathSegment::Key(key.clone()));

                    // Check if key matches
                    if key.to_lowercase().contains(query) {
                        self.search_matches.push(child_path.clone());
                    } else if let Value::String(s) = val {
                        // Check if string value matches
                        if s.to_lowercase().contains(query) {
                            self.search_matches.push(child_path.clone());
                        }
                    } else if let Value::Number(n) = val {
                        // Check if number matches
                        if n.to_string().contains(query) {
                            self.search_matches.push(child_path.clone());
                        }
                    }

                    self.find_matches(val, child_path, query);
                }
            }
            Value::Array(arr) => {
                for (i, val) in arr.iter().enumerate() {
                    let mut child_path = path.clone();
                    child_path.push(JsonPathSegment::Index(i));

                    if let Value::String(s) = val {
                        if s.to_lowercase().contains(query) {
                            self.search_matches.push(child_path.clone());
                        }
                    } else if let Value::Number(n) = val {
                        if n.to_string().contains(query) {
                            self.search_matches.push(child_path.clone());
                        }
                    }

                    self.find_matches(val, child_path, query);
                }
            }
            _ => {}
        }
    }

    /// Jump to a specific match index
    fn jump_to_match(&mut self, index: usize) {
        if index >= self.search_matches.len() {
            return;
        }

        let path = self.search_matches[index].clone();

        // Expand all parent nodes - open each ancestor path
        for i in 1..path.len() {
            let parent_path = path[..i].to_vec();
            self.tree_state.open(parent_path);
        }

        // Select the node using the accumulated path
        self.tree_state.select(path);
    }

    /// Go to next search match
    pub fn next_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        self.current_match_index = (self.current_match_index + 1) % self.search_matches.len();
        self.jump_to_match(self.current_match_index);
    }

    /// Go to previous search match
    pub fn prev_match(&mut self) {
        if self.search_matches.is_empty() {
            return;
        }
        if self.current_match_index == 0 {
            self.current_match_index = self.search_matches.len() - 1;
        } else {
            self.current_match_index -= 1;
        }
        self.jump_to_match(self.current_match_index);
    }

    /// Get the search status message
    pub fn search_status(&self) -> String {
        if self.search_query.is_empty() {
            String::new()
        } else if self.search_matches.is_empty() {
            format!("/{} (no matches)", self.search_query)
        } else {
            format!(
                "/{} ({}/{})",
                self.search_query,
                self.current_match_index + 1,
                self.search_matches.len()
            )
        }
    }

    /// Build the tree widget items from the JSON
    /// Check if we need to expand all and do it (call before rendering)
    pub fn maybe_expand_all(&mut self) {
        if self.needs_expand {
            self.expand_all();
            self.needs_expand = false;
        }
    }

    pub fn build_tree_items(&self) -> Vec<TreeItem<'static, JsonPathSegment>> {
        self.value_to_tree_items(&self.json, vec![JsonPathSegment::Root], JsonPathSegment::Root, None)
    }

    /// Convert a JSON value to tree items
    /// - `path`: The full accumulated path to this node (for search matching)
    /// - `local_id`: The local identifier for this node (used by TreeItem)
    fn value_to_tree_items(
        &self,
        value: &Value,
        path: JsonNodeId,
        local_id: JsonPathSegment,
        key: Option<&str>,
    ) -> Vec<TreeItem<'static, JsonPathSegment>> {
        let is_match = self.is_search_match(&path);

        match value {
            Value::Object(map) => {
                let children: Vec<TreeItem<'static, JsonPathSegment>> = map
                    .iter()
                    .flat_map(|(k, v)| {
                        let mut child_path = path.clone();
                        let child_local_id = JsonPathSegment::Key(k.clone());
                        child_path.push(child_local_id.clone());
                        self.value_to_tree_items(v, child_path, child_local_id, Some(k))
                    })
                    .collect();

                let label = self.format_object_label(key, map.len(), is_match);
                vec![TreeItem::new(local_id, label, children).expect("valid tree item")]
            }
            Value::Array(arr) => {
                let children: Vec<TreeItem<'static, JsonPathSegment>> = arr
                    .iter()
                    .enumerate()
                    .flat_map(|(i, v)| {
                        let mut child_path = path.clone();
                        let child_local_id = JsonPathSegment::Index(i);
                        child_path.push(child_local_id.clone());
                        self.value_to_tree_items(v, child_path, child_local_id, Some(&format!("[{}]", i)))
                    })
                    .collect();

                let label = self.format_array_label(key, arr.len(), is_match);
                vec![TreeItem::new(local_id, label, children).expect("valid tree item")]
            }
            _ => {
                let label = self.format_value_label(value, key, is_match);
                vec![TreeItem::new(local_id, label, vec![]).expect("valid tree item")]
            }
        }
    }

    /// Check if a path is a search match
    fn is_search_match(&self, path: &JsonNodeId) -> bool {
        !self.search_query.is_empty() && self.search_matches.contains(path)
    }

    /// Format a label for an object node
    fn format_object_label(&self, key: Option<&str>, len: usize, is_match: bool) -> Line<'static> {
        let mut spans = Vec::new();

        if let Some(k) = key {
            if is_match {
                spans.push(Span::styled(
                    k.to_string(),
                    Style::default().fg(self.colors.match_highlight).bg(Color::Yellow),
                ));
            } else {
                spans.push(Span::styled(k.to_string(), Style::default().fg(self.colors.key)));
            }
            spans.push(Span::styled(": ", Style::default().fg(self.colors.colon)));
        }

        spans.push(Span::styled("{", Style::default().fg(self.colors.bracket)));
        spans.push(Span::raw(format!(" {} keys ", len)));
        spans.push(Span::styled("}", Style::default().fg(self.colors.bracket)));

        Line::from(spans)
    }

    /// Format a label for an array node
    fn format_array_label(&self, key: Option<&str>, len: usize, is_match: bool) -> Line<'static> {
        let mut spans = Vec::new();

        if let Some(k) = key {
            if is_match {
                spans.push(Span::styled(
                    k.to_string(),
                    Style::default().fg(self.colors.match_highlight).bg(Color::Yellow),
                ));
            } else {
                spans.push(Span::styled(k.to_string(), Style::default().fg(self.colors.key)));
            }
            spans.push(Span::styled(": ", Style::default().fg(self.colors.colon)));
        }

        spans.push(Span::styled("[", Style::default().fg(self.colors.bracket)));
        spans.push(Span::raw(format!(" {} items ", len)));
        spans.push(Span::styled("]", Style::default().fg(self.colors.bracket)));

        Line::from(spans)
    }

    /// Format a label for a primitive value
    fn format_value_label(&self, value: &Value, key: Option<&str>, is_match: bool) -> Line<'static> {
        let mut spans = Vec::new();

        if let Some(k) = key {
            if is_match {
                spans.push(Span::styled(
                    k.to_string(),
                    Style::default().fg(self.colors.match_highlight).bg(Color::Yellow),
                ));
            } else {
                spans.push(Span::styled(k.to_string(), Style::default().fg(self.colors.key)));
            }
            spans.push(Span::styled(": ", Style::default().fg(self.colors.colon)));
        }

        let (value_str, color) = match value {
            Value::String(s) => {
                let display = if s.len() > 50 {
                    format!("\"{}...\"", &s[..47])
                } else {
                    format!("\"{}\"", s)
                };
                (display, self.colors.string)
            }
            Value::Number(n) => (n.to_string(), self.colors.number),
            Value::Bool(b) => (b.to_string(), self.colors.boolean),
            Value::Null => ("null".to_string(), self.colors.null),
            _ => ("?".to_string(), Color::White),
        };

        if is_match {
            spans.push(Span::styled(
                value_str,
                Style::default().fg(self.colors.match_highlight).bg(Color::Yellow),
            ));
        } else {
            spans.push(Span::styled(value_str, Style::default().fg(color)));
        }

        Line::from(spans)
    }
}
