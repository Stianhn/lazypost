use ratatui::{
    layout::{Constraint, Direction, Layout, Rect},
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Clear, List, ListItem, Paragraph, Wrap},
    Frame,
};
use tui_tree_widget::Tree;

use crate::app::{App, DialogStep, FocusedPane, InputMode};

const FOCUSED_COLOR: Color = Color::Green;
const UNFOCUSED_COLOR: Color = Color::White;

pub fn render(frame: &mut Frame, app: &mut App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Min(0), Constraint::Length(3)])
        .split(frame.area());

    let main_area = chunks[0];
    let status_area = chunks[1];

    render_main_layout(frame, app, main_area);
    render_status_bar(frame, app, status_area);

    // Render workspace indicator in top right at y=0
    render_workspace_indicator(frame, app);

    // Render environment indicator below workspace at y=1
    render_environment_indicator(frame, app);

    // Render dialog overlay if active
    if app.new_request_dialog.is_some() {
        render_new_request_dialog(frame, app);
    }

    // Render saving popup if active
    if app.input_mode == InputMode::Saving {
        render_saving_popup(frame);
    }

    // Render environment selection popup if active
    if app.input_mode == InputMode::EnvironmentSelect {
        render_environment_popup(frame, app);
    }

    // Render variables view popup if active
    if app.input_mode == InputMode::VariablesView {
        render_variables_popup(frame, app);
    }

    // Render workspace selection popup if active
    if app.input_mode == InputMode::WorkspaceSelect {
        render_workspace_popup(frame, app);
    }

    // Render workspace loading popup if loading
    if app.workspace_loading.is_some() {
        render_workspace_loading_popup(frame, app);
    }

    // Render collection loading popup if loading
    if app.collection_loading.is_some() {
        render_collection_loading_popup(frame, app);
    }

    // Render JSON search overlay if in JSON search mode
    if app.input_mode == InputMode::JsonSearch {
        render_json_search_overlay(frame, app);
    }

    // Render execute confirmation popup if active
    if app.input_mode == InputMode::ExecuteConfirm {
        render_execute_confirm_popup(frame, app);
    }
}

fn render_main_layout(frame: &mut Frame, app: &mut App, area: Rect) {
    // Split into left (18%) and right (82%)
    let horizontal = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(18), Constraint::Percentage(82)])
        .split(area);

    let left_area = horizontal[0];
    let right_area = horizontal[1];

    // Split left into top (Collections) and bottom (Requests)
    let left_vertical = Layout::default()
        .direction(Direction::Vertical)
        .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
        .split(left_area);

    let collections_area = left_vertical[0];
    let requests_area = left_vertical[1];

    render_collections_pane(frame, app, collections_area);
    render_requests_pane(frame, app, requests_area);

    // Split right area if we have a response
    if app.response.is_some() {
        let right_vertical = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Percentage(40), Constraint::Percentage(60)])
            .split(right_area);

        render_preview_pane(frame, app, right_vertical[0]);
        render_response_pane(frame, app, right_vertical[1]);
    } else {
        render_preview_pane(frame, app, right_area);
    }
}

fn get_border_color(app: &App, pane: FocusedPane) -> Color {
    if app.focused_pane == pane {
        FOCUSED_COLOR
    } else {
        UNFOCUSED_COLOR
    }
}

fn get_title_with_number(title: &str, number: u8, focused: bool) -> String {
    if focused {
        format!(" [{}] {} ", number, title)
    } else {
        format!(" {} {} ", number, title)
    }
}

fn render_collections_pane(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_pane == FocusedPane::Collections;
    let is_searching = app.input_mode == InputMode::Search
        && !app.search_query.is_empty()
        && is_focused;

    let items: Vec<ListItem> = app
        .flat_collections
        .iter()
        .enumerate()
        .filter(|(i, flat_col)| {
            if is_searching {
                // During search, only show matches (skip Favorites folder header)
                if flat_col.is_favorites_folder {
                    false
                } else {
                    app.search_matches.contains(i)
                }
            } else {
                true
            }
        })
        .map(|(i, flat_col)| {
            let is_favorite = !flat_col.is_favorites_folder && app.config.is_favorite(&flat_col.uid);
            let style = if i == app.selected_collection_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if flat_col.is_favorites_folder {
                Style::default().fg(Color::Yellow)
            } else if is_favorite {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            let indent = "  ".repeat(flat_col.depth);
            let (icon, name) = if flat_col.is_favorites_folder {
                let icon = if app.collections_favorites_expanded { "-" } else { "+" };
                (format!("{} ", icon), flat_col.name.clone())
            } else if is_favorite && flat_col.depth == 1 {
                // Inside Favorites section, no need for * prefix
                ("  ".to_string(), flat_col.name.clone())
            } else if is_favorite {
                // Regular favorite collection outside Favorites section
                ("* ".to_string(), flat_col.name.clone())
            } else {
                ("  ".to_string(), flat_col.name.clone())
            };

            ListItem::new(Line::from(vec![Span::styled(
                format!("{}{}{}", indent, icon, name),
                style,
            )]))
        })
        .collect();

    let title = if is_searching {
        format!("Collections ({} matches)", app.search_matches.len())
    } else {
        "Collections".to_string()
    };

    let border_color = get_border_color(app, FocusedPane::Collections);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(get_title_with_number(&title, 1, is_focused));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_requests_pane(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_pane == FocusedPane::Requests;
    let is_searching = app.input_mode == InputMode::Search
        && !app.search_query.is_empty()
        && is_focused;

    let items: Vec<ListItem> = app
        .flat_items
        .iter()
        .enumerate()
        .filter(|(_, item)| {
            if is_searching {
                // Check if this item's path is in the search matches
                app.search_match_paths.contains(&item.path)
            } else {
                true
            }
        })
        .map(|(i, item)| {
            let is_favorite = app.is_request_favorite(&item.path);
            let has_local_edit = !item.is_folder && app.has_local_edit(&item.path);
            let indent = if is_searching {
                String::new()
            } else {
                "  ".repeat(item.depth)
            };
            let icon = if item.is_folder {
                if item.expanded { "-" } else { "+" }
            } else {
                ">"
            };
            let method_prefix = if !item.is_folder {
                if let Some(req) = &item.request {
                    format!("[{}] ", req.method)
                } else {
                    String::new()
                }
            } else {
                String::new()
            };
            let favorite_prefix = if is_favorite { "* " } else { "" };
            let modified_suffix = if has_local_edit { " ~" } else { "" };

            let style = if i == app.selected_item_index {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if has_local_edit {
                Style::default().fg(Color::Magenta)
            } else if item.is_folder {
                Style::default().fg(Color::Yellow)
            } else if is_favorite {
                Style::default().fg(Color::Yellow)
            } else {
                Style::default()
            };

            ListItem::new(Line::from(vec![Span::styled(
                format!("{}{} {}{}{}{}", indent, icon, favorite_prefix, method_prefix, item.name, modified_suffix),
                style,
            )]))
        })
        .collect();

    let base_title = app
        .current_collection
        .as_ref()
        .map(|c| c.info.name.clone())
        .unwrap_or_else(|| "Requests".to_string());

    let title = if is_searching {
        format!("{} ({} matches)", base_title, app.search_match_paths.len())
    } else {
        base_title
    };

    let border_color = get_border_color(app, FocusedPane::Requests);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(get_title_with_number(&title, 2, is_focused));

    let list = List::new(items).block(block);
    frame.render_widget(list, area);
}

fn render_preview_pane(frame: &mut Frame, app: &App, area: Rect) {
    let is_focused = app.focused_pane == FocusedPane::Preview;
    let border_color = get_border_color(app, FocusedPane::Preview);
    let has_local_edit = app.has_unsaved_edit();

    if let Some(request) = &app.current_request {
        render_request_preview(frame, request, area, border_color, is_focused, has_local_edit);
    } else {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color))
            .title(get_title_with_number("Request", 3, is_focused));
        let paragraph = Paragraph::new("Select a request to see details")
            .block(block)
            .wrap(Wrap { trim: true });
        frame.render_widget(paragraph, area);
    }
}

fn render_response_pane(frame: &mut Frame, app: &mut App, area: Rect) {
    let is_focused = app.focused_pane == FocusedPane::Response;
    let border_color = get_border_color(app, FocusedPane::Response);

    if let Some(response) = app.response.clone() {
        render_response(frame, app, &response, area, border_color, is_focused);
    }
}

fn render_request_preview(frame: &mut Frame, request: &crate::api::Request, area: Rect, border_color: Color, is_focused: bool, has_local_edit: bool) {
    let url = request.url.to_string();
    let headers_text: String = request
        .header
        .iter()
        .map(|h| format!("{}: {}", h.key, h.value))
        .collect::<Vec<_>>()
        .join("\n");

    let body_text = request
        .body
        .as_ref()
        .and_then(|b| b.raw.clone())
        .unwrap_or_else(|| String::from("(no body)"));

    let content = format!(
        "Method: {}\n\nURL: {}\n\nHeaders:\n{}\n\nBody:\n{}",
        request.method,
        url,
        if headers_text.is_empty() {
            "(none)".to_string()
        } else {
            headers_text
        },
        body_text
    );

    let title = if has_local_edit {
        "Request ~ (not synced to Postman)"
    } else {
        "Request"
    };

    let title_style = if has_local_edit {
        Style::default().fg(Color::Magenta)
    } else {
        Style::default()
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(Span::styled(get_title_with_number(title, 3, is_focused), title_style));

    let paragraph = Paragraph::new(content)
        .block(block)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, area);
}

fn render_response(frame: &mut Frame, app: &mut App, response: &crate::api::ExecutedResponse, area: Rect, border_color: Color, is_focused: bool) {
    let status_color = if response.status >= 200 && response.status < 300 {
        Color::Green
    } else if response.status >= 400 {
        Color::Red
    } else {
        Color::Yellow
    };

    let title = if app.loading {
        "Response (loading...)"
    } else if app.json_viewer_state.is_some() {
        "Response (JSON)"
    } else {
        "Response"
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(border_color))
        .title(get_title_with_number(title, 4, is_focused));

    // If we have a JSON viewer state, render the tree
    if let Some(ref mut viewer_state) = app.json_viewer_state {
        // Split the response area to show status at top and tree below
        let chunks = Layout::default()
            .direction(Direction::Vertical)
            .constraints([Constraint::Length(2), Constraint::Min(0)])
            .split(area);

        // Render status line at top
        let status_line = Line::from(vec![
            Span::raw("Status: "),
            Span::styled(
                format!("{}", response.status),
                Style::default().fg(status_color).add_modifier(Modifier::BOLD),
            ),
            Span::raw(format!(" {} | j/k: nav | h/l: collapse/expand | /: search",
                response.status_text.split_whitespace().skip(1).collect::<Vec<_>>().join(" "))),
        ]);

        let status_block = Block::default()
            .borders(Borders::TOP | Borders::LEFT | Borders::RIGHT)
            .border_style(Style::default().fg(border_color))
            .title(get_title_with_number(title, 4, is_focused));

        let status_para = Paragraph::new(status_line).block(status_block);
        frame.render_widget(status_para, chunks[0]);

        // Expand all nodes on first render
        viewer_state.maybe_expand_all();

        // Render the JSON tree
        let tree_items = viewer_state.build_tree_items();
        let tree = Tree::new(&tree_items)
            .expect("valid tree")
            .block(Block::default()
                .borders(Borders::BOTTOM | Borders::LEFT | Borders::RIGHT)
                .border_style(Style::default().fg(border_color)))
            .highlight_style(Style::default().bg(Color::DarkGray));

        frame.render_stateful_widget(tree, chunks[1], &mut viewer_state.tree_state);
    } else {
        // Fall back to plain text display for non-JSON responses
        let headers_text: String = response
            .headers
            .iter()
            .map(|(k, v)| format!("{}: {}", k, v))
            .collect::<Vec<_>>()
            .join("\n");

        let body_preview = if response.body.len() > 2000 {
            format!("{}...\n\n(truncated)", &response.body[..2000])
        } else {
            response.body.clone()
        };

        let content = vec![
            Line::from(vec![
                Span::raw("Status: "),
                Span::styled(
                    format!("{}", response.status),
                    Style::default().fg(status_color).add_modifier(Modifier::BOLD),
                ),
                Span::raw(format!(" {}", response.status_text.split_whitespace().skip(1).collect::<Vec<_>>().join(" "))),
            ]),
            Line::from(""),
            Line::from(Span::styled("Headers:", Style::default().add_modifier(Modifier::BOLD))),
            Line::from(headers_text),
            Line::from(""),
            Line::from(Span::styled("Body:", Style::default().add_modifier(Modifier::BOLD))),
            Line::from(body_preview),
        ];

        let paragraph = Paragraph::new(content)
            .block(block)
            .wrap(Wrap { trim: true });

        frame.render_widget(paragraph, area);
    }
}

fn render_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let status_style = if app.error.is_some() {
        Style::default().fg(Color::Red)
    } else if app.loading {
        Style::default().fg(Color::Yellow)
    } else {
        Style::default().fg(Color::Green)
    };

    let status_text = if let Some(error) = &app.error {
        format!("Error: {}", error)
    } else {
        app.status_message.clone()
    };

    let has_env = app.selected_environment_index.is_some();
    let keybindings = match app.input_mode {
        InputMode::TextInput => "Enter: Next/Submit | Esc: Cancel",
        InputMode::Search => "Enter: Confirm | Esc: Cancel | Type to search",
        InputMode::JsonSearch => "Enter: Confirm | Esc: Cancel | n/N: Next/Prev match | Type to search",
        InputMode::Saving => "Esc: Cancel",
        InputMode::ExecuteConfirm => "y/Enter: Execute | n/Esc: Cancel",
        InputMode::EnvironmentSelect => "j/k: Nav | Enter: Select | Esc: Cancel",
        InputMode::VariablesView => if app.editing_variable.is_some() {
            "Enter: Confirm | Esc: Cancel | Type to edit"
        } else if app.variables_search_active {
            "Enter: Confirm | Esc: Cancel | Type to search"
        } else {
            "j/k: Nav | Enter: Edit | /: Search | s: Save | Esc: Close"
        },
        InputMode::WorkspaceSelect => "j/k: Nav | Enter: Select | Esc: Cancel",
        InputMode::Normal => {
            let has_unsaved = app.has_unsaved_edit();
            match (app.focused_pane, has_env, has_unsaved) {
                (FocusedPane::Collections, true, _) => "1-4: Pane | j/k: Nav | Enter: Load | f: Fav | v: Env | V: Vars | q: Quit",
                (FocusedPane::Collections, false, _) => "1-4: Pane | j/k: Nav | Enter: Load | f: Fav | v: Env | q: Quit",
                (FocusedPane::Requests, true, _) => "1-4: Pane | j/k: Nav | Enter: Select | e: Exec | a: Add | f: Fav | v: Env | V: Vars | q: Quit",
                (FocusedPane::Requests, false, _) => "1-4: Pane | j/k: Nav | Enter: Select | e: Exec | a: Add | f: Fav | v: Env | q: Quit",
                (FocusedPane::Preview, true, true) => "1-4: Pane | e: Exec | E: Edit | S: Save* | v: Env | V: Vars | q: Quit",
                (FocusedPane::Preview, true, false) => "1-4: Pane | e: Exec | E: Edit | v: Env | V: Vars | q: Quit",
                (FocusedPane::Preview, false, true) => "1-4: Pane | e: Exec | E: Edit | S: Save* | v: Env | q: Quit",
                (FocusedPane::Preview, false, false) => "1-4: Pane | e: Exec | E: Edit | v: Env | q: Quit",
                (FocusedPane::Response, true, _) => if app.json_viewer_state.is_some() {
                    "j/k: Nav | h/l: Fold | H/L: Fold All | /: Search | n/N: Match | v: Env | q: Quit"
                } else {
                    "1-4: Pane | v: Env | V: Vars | q: Quit"
                },
                (FocusedPane::Response, false, _) => if app.json_viewer_state.is_some() {
                    "j/k: Nav | h/l: Fold | H/L: Fold All | /: Search | n/N: Match | v: Env | q: Quit"
                } else {
                    "1-4: Pane | v: Env | q: Quit"
                },
            }
        },
    };

    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(50), Constraint::Percentage(50)])
        .split(area);

    let status = Paragraph::new(status_text)
        .style(status_style)
        .block(Block::default().borders(Borders::ALL).title(" Status "));

    let keys = Paragraph::new(keybindings)
        .style(Style::default().fg(Color::Cyan))
        .block(Block::default().borders(Borders::ALL).title(" Keys "));

    frame.render_widget(status, chunks[0]);
    frame.render_widget(keys, chunks[1]);
}

fn render_new_request_dialog(frame: &mut Frame, app: &App) {
    let dialog = match &app.new_request_dialog {
        Some(d) => d,
        None => return,
    };

    let area = frame.area();

    let dialog_width = 60u16;
    let dialog_height = 7u16;
    let x = area.width.saturating_sub(dialog_width) / 2;
    let y = area.height.saturating_sub(dialog_height) / 2;

    let dialog_area = Rect::new(x, y, dialog_width.min(area.width), dialog_height.min(area.height));

    frame.render_widget(Clear, dialog_area);

    let step_indicator = match dialog.step {
        DialogStep::Name => "Step 1/2: Name",
        DialogStep::Url => "Step 2/2: URL",
    };

    let (label, value) = match dialog.step {
        DialogStep::Name => ("Name:", &dialog.name),
        DialogStep::Url => ("URL:", &dialog.url),
    };

    let cursor_pos = dialog.cursor_position;
    let input_with_cursor = if cursor_pos >= value.len() {
        format!("{}_", value)
    } else {
        let (before, after) = value.split_at(cursor_pos);
        format!("{}|{}", before, after)
    };

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled(format!(" {} ", label), Style::default().add_modifier(Modifier::BOLD)),
        ]),
        Line::from(format!(" {}", input_with_cursor)),
        Line::from(""),
        Line::from(Span::styled(
            " Enter: Next | Esc: Cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(format!(" New Request - {} ", step_indicator))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(content).block(block);

    frame.render_widget(paragraph, dialog_area);
}

fn render_saving_popup(frame: &mut Frame) {
    let area = frame.area();

    let popup_width = 40u16;
    let popup_height = 5u16;
    let x = area.width.saturating_sub(popup_width) / 2;
    let y = area.height.saturating_sub(popup_height) / 2;

    let popup_area = Rect::new(x, y, popup_width.min(area.width), popup_height.min(area.height));

    frame.render_widget(Clear, popup_area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            "  Saving changes to Postman...",
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
        Line::from(Span::styled(
            "  Press Esc to cancel",
            Style::default().fg(Color::DarkGray),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Saving ")
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(content).block(block);

    frame.render_widget(paragraph, popup_area);
}

fn render_workspace_indicator(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let ws_name = app.get_current_workspace_name();
    let ws_text = format!(" {} [w] ", ws_name);
    let ws_width = ws_text.len() as u16;

    // Calculate environment indicator width to position workspace to the left of it
    let env_name = app.get_current_environment_name();
    let env_text = format!(" {} [v] ", env_name);
    let env_width = env_text.len() as u16;

    // Position workspace to the left of environment with 2 spaces between
    let spacing = 2u16;
    let x = area.width.saturating_sub(ws_width + env_width + spacing + 1);
    let y = 0;

    let indicator_area = Rect::new(x, y, ws_width.min(area.width), 1);

    let style = Style::default()
        .fg(Color::Black)
        .bg(Color::Magenta);

    let paragraph = Paragraph::new(ws_text).style(style);
    frame.render_widget(paragraph, indicator_area);
}

fn render_environment_indicator(frame: &mut Frame, app: &App) {
    let area = frame.area();
    let env_name = app.get_current_environment_name();
    let display_text = format!(" {} [v] ", env_name);
    let width = display_text.len() as u16;

    // Position in top right corner at y=0
    let x = area.width.saturating_sub(width + 1);
    let y = 0;

    let indicator_area = Rect::new(x, y, width.min(area.width), 1);

    let style = Style::default()
        .fg(Color::Black)
        .bg(Color::Cyan);

    let paragraph = Paragraph::new(display_text).style(style);
    frame.render_widget(paragraph, indicator_area);
}

fn render_environment_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Calculate popup size based on content
    let max_name_len = app.environments.iter()
        .map(|e| e.name.len())
        .max()
        .unwrap_or(10)
        .max("No Environment".len());

    let popup_width = (max_name_len + 6) as u16;
    let popup_height = (app.environments.len() + 4) as u16; // +1 for "No Environment", +3 for borders and padding

    let x = area.width.saturating_sub(popup_width + 2);
    let y = 1u16; // Just below the indicator

    let popup_area = Rect::new(
        x.min(area.width.saturating_sub(popup_width)),
        y,
        popup_width.min(area.width),
        popup_height.min(area.height.saturating_sub(y)),
    );

    frame.render_widget(Clear, popup_area);

    // Build list items
    let mut items: Vec<ListItem> = Vec::new();

    // "No Environment" option at index 0
    let no_env_style = if app.environment_popup_index == 0 {
        Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    items.push(ListItem::new(Line::from(Span::styled("No Environment", no_env_style))));

    // Environment options
    for (i, env) in app.environments.iter().enumerate() {
        let style = if app.environment_popup_index == i + 1 {
            Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        items.push(ListItem::new(Line::from(Span::styled(&env.name, style))));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Environment ")
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    let list = List::new(items).block(block);
    frame.render_widget(list, popup_area);
}

fn render_workspace_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Calculate popup size based on content
    let max_name_len = app.workspaces.iter()
        .map(|w| w.name.len())
        .max()
        .unwrap_or(10)
        .max("All Workspaces".len());

    let popup_width = (max_name_len + 6) as u16;
    let popup_height = (app.workspaces.len() + 4) as u16; // +1 for "All Workspaces", +3 for borders and padding

    let x = area.width.saturating_sub(popup_width + 2);
    let y = 1u16; // Just below the indicator

    let popup_area = Rect::new(
        x.min(area.width.saturating_sub(popup_width)),
        y,
        popup_width.min(area.width),
        popup_height.min(area.height.saturating_sub(y)),
    );

    frame.render_widget(Clear, popup_area);

    // Build list items
    let mut items: Vec<ListItem> = Vec::new();

    // "All Workspaces" option at index 0
    let all_ws_style = if app.workspace_popup_index == 0 {
        Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
    } else {
        Style::default().fg(Color::DarkGray)
    };
    items.push(ListItem::new(Line::from(Span::styled("All Workspaces", all_ws_style))));

    // Workspace options
    for (i, ws) in app.workspaces.iter().enumerate() {
        let style = if app.workspace_popup_index == i + 1 {
            Style::default().bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
        } else {
            Style::default()
        };
        items.push(ListItem::new(Line::from(Span::styled(&ws.name, style))));
    }

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Workspace ")
        .border_style(Style::default().fg(Color::Magenta))
        .style(Style::default().bg(Color::Black));

    let list = List::new(items).block(block);
    frame.render_widget(list, popup_area);
}

fn render_workspace_loading_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let ws_name = app.workspace_loading.as_deref().unwrap_or("workspace");
    let text = format!("Loading workspace: {}", ws_name);
    let popup_width = (text.len() + 6).max(30) as u16;
    let popup_height = 5u16;

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(x, y, popup_width.min(area.width), popup_height.min(area.height));

    frame.render_widget(Clear, popup_area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}  ", text),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Loading ")
        .border_style(Style::default().fg(Color::Magenta))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(content).block(block);

    frame.render_widget(paragraph, popup_area);
}

fn render_collection_loading_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let col_name = app.collection_loading.as_deref().unwrap_or("collection");
    let text = format!("Loading: {}", col_name);
    let popup_width = (text.len() + 6).max(30) as u16;
    let popup_height = 5u16;

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(x, y, popup_width.min(area.width), popup_height.min(area.height));

    frame.render_widget(Clear, popup_area);

    let content = vec![
        Line::from(""),
        Line::from(Span::styled(
            format!("  {}  ", text),
            Style::default().add_modifier(Modifier::BOLD),
        )),
        Line::from(""),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Loading ")
        .border_style(Style::default().fg(Color::Green))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(content).block(block);

    frame.render_widget(paragraph, popup_area);
}

fn render_variables_popup(frame: &mut Frame, app: &App) {
    let area = frame.area();

    let variables = app.get_variables_for_display();

    // Calculate popup size
    let max_key_len = variables.iter()
        .map(|(_, k, _, _)| k.len())
        .max()
        .unwrap_or(10);
    let max_val_len = variables.iter()
        .map(|(_, _, v, _)| v.len())
        .max()
        .unwrap_or(10)
        .min(40); // Cap value display length

    let popup_width = (max_key_len + max_val_len + 10).max(50).min(80) as u16;
    let extra_lines = if app.variables_search_active || !app.variables_search_query.is_empty() { 1 } else { 0 };
    let popup_height = (variables.len() + 5 + extra_lines).max(8).min(20) as u16;

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(
        x,
        y,
        popup_width.min(area.width),
        popup_height.min(area.height),
    );

    frame.render_widget(Clear, popup_area);

    // Build content
    let mut lines: Vec<Line> = Vec::new();

    // Show search input if searching or has query
    if app.variables_search_active || !app.variables_search_query.is_empty() {
        let search_display = if app.variables_search_active {
            format!("/{}_", app.variables_search_query)
        } else {
            format!("/{} ({} matches)", app.variables_search_query, variables.len())
        };
        lines.push(Line::from(Span::styled(
            search_display,
            Style::default().fg(Color::Yellow),
        )));
    }

    if variables.is_empty() {
        if !app.variables_search_query.is_empty() {
            lines.push(Line::from(Span::styled(
                "  No matching variables",
                Style::default().fg(Color::DarkGray),
            )));
        } else {
            lines.push(Line::from(Span::styled(
                "  No variables in environment",
                Style::default().fg(Color::DarkGray),
            )));
        }
    } else {
        for (display_idx, (actual_idx, key, value, enabled)) in variables.iter().enumerate() {
            let is_selected = display_idx == app.variables_popup_index;
            let is_editing = app.editing_variable.as_ref().map(|(idx, _)| *idx == *actual_idx).unwrap_or(false);

            // Determine the display value
            let display_value = if is_editing {
                if let Some((_, edit_val)) = &app.editing_variable {
                    // Show cursor in editing value
                    let cursor_pos = app.variable_cursor_position;
                    if cursor_pos >= edit_val.len() {
                        format!("{}_", edit_val)
                    } else {
                        let (before, after) = edit_val.split_at(cursor_pos);
                        format!("{}|{}", before, after)
                    }
                } else {
                    value.clone()
                }
            } else {
                // Truncate long values
                if value.len() > 40 {
                    format!("{}...", &value[..37])
                } else {
                    value.clone()
                }
            };

            let base_style = if !enabled {
                Style::default().fg(Color::DarkGray)
            } else {
                Style::default()
            };

            let style = if is_selected {
                base_style.bg(Color::DarkGray).fg(Color::White).add_modifier(Modifier::BOLD)
            } else {
                base_style
            };

            let edit_indicator = if is_editing { "> " } else { "  " };
            let line_text = format!("{}{}: {}", edit_indicator, key, display_value);
            lines.push(Line::from(Span::styled(line_text, style)));
        }
    }

    // Add help text at the bottom
    lines.push(Line::from(""));
    let help_text = if app.editing_variable.is_some() {
        "Enter: Confirm | Esc: Cancel"
    } else if app.variables_search_active {
        "Enter: Confirm | Esc: Cancel | Type to search"
    } else if app.variables_modified {
        "Enter: Edit | /: Search | s: Save* | Esc: Close"
    } else {
        "Enter: Edit | /: Search | s: Save | Esc: Close"
    };
    lines.push(Line::from(Span::styled(
        help_text,
        Style::default().fg(Color::DarkGray),
    )));

    let title = if app.variables_modified {
        format!(" Variables ({}) * ", app.get_current_environment_name())
    } else {
        format!(" Variables ({}) ", app.get_current_environment_name())
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(title)
        .border_style(Style::default().fg(Color::Cyan))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(lines).block(block);
    frame.render_widget(paragraph, popup_area);
}

fn render_json_search_overlay(frame: &mut Frame, app: &App) {
    let area = frame.area();

    // Get the search query from the JSON viewer state
    let search_query = app.json_viewer_state
        .as_ref()
        .map(|v| v.search_query.clone())
        .unwrap_or_default();

    let search_status = app.json_viewer_state
        .as_ref()
        .map(|v| v.search_status())
        .unwrap_or_default();

    let popup_width = 50u16.min(area.width.saturating_sub(4));
    let popup_height = 3u16;

    // Position at bottom of screen
    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = area.height.saturating_sub(popup_height + 4); // Above status bar

    let popup_area = Rect::new(x, y, popup_width, popup_height);

    frame.render_widget(Clear, popup_area);

    let display_text = if search_query.is_empty() {
        "/_".to_string()
    } else {
        format!("{}_", search_status)
    };

    let content = vec![
        Line::from(Span::styled(
            display_text,
            Style::default().fg(Color::Yellow),
        )),
    ];

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Search JSON ")
        .border_style(Style::default().fg(Color::Yellow))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(content).block(block);
    frame.render_widget(paragraph, popup_area);
}

fn render_execute_confirm_popup(frame: &mut Frame, app: &App) {
    let pending = match &app.pending_execute {
        Some(p) => p,
        None => return,
    };

    let area = frame.area();

    // Calculate popup size based on content
    let url_display = if pending.url.len() > 50 {
        format!("{}...", &pending.url[..47])
    } else {
        pending.url.clone()
    };

    let popup_width = 60u16.min(area.width.saturating_sub(4));
    let popup_height = 9u16;

    let x = (area.width.saturating_sub(popup_width)) / 2;
    let y = (area.height.saturating_sub(popup_height)) / 2;

    let popup_area = Rect::new(x, y, popup_width.min(area.width), popup_height.min(area.height));

    frame.render_widget(Clear, popup_area);

    // Get method color
    let method_color = match pending.method.as_str() {
        "DELETE" => Color::Red,
        "POST" => Color::Green,
        "PUT" | "PATCH" => Color::Yellow,
        _ => Color::White,
    };

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::raw("  Execute "),
            Span::styled(&pending.method, Style::default().fg(method_color).add_modifier(Modifier::BOLD)),
            Span::raw(" request?"),
        ]),
        Line::from(""),
        Line::from(vec![
            Span::styled("  Name: ", Style::default().fg(Color::DarkGray)),
            Span::raw(&pending.name),
        ]),
        Line::from(vec![
            Span::styled("  URL:  ", Style::default().fg(Color::DarkGray)),
            Span::raw(&url_display),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            "  [y/Enter] Yes   [n/Esc] No",
            Style::default().fg(Color::Cyan),
        )),
    ];

    let title_color = match pending.method.as_str() {
        "DELETE" => Color::Red,
        _ => Color::Yellow,
    };

    let block = Block::default()
        .borders(Borders::ALL)
        .title(" Confirm Request ")
        .border_style(Style::default().fg(title_color))
        .style(Style::default().bg(Color::Black));

    let paragraph = Paragraph::new(content).block(block);

    frame.render_widget(paragraph, popup_area);
}
