mod api;
mod app;
mod config;
mod logging;
mod ui;

use std::io::{self, Write};
use std::time::Duration;
use std::process::Command;
use std::fs;
use std::env;

use anyhow::{Context, Result};
use crossterm::{
    event::{self, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen},
};
use ratatui::prelude::*;

use app::{App, EditableRequest, FocusedPane, InputMode};
use config::Config;
use logging::log_error;

#[tokio::main]
async fn main() -> Result<()> {
    let config = get_config()?;

    let mut terminal = setup_terminal()?;
    let result = run_app(&mut terminal, config).await;
    restore_terminal(&mut terminal)?;

    if let Err(err) = result {
        eprintln!("Error: {}", err);
    }

    Ok(())
}

fn get_config() -> Result<Config> {
    if let Some(config) = Config::load()? {
        return Ok(config);
    }

    println!("Welcome to LazyPost!");
    println!();
    println!("No configuration found. Please enter your Postman API key.");
    println!("You can find your API key at: https://web.postman.co/settings/me/api-keys");
    println!();
    print!("API Key: ");
    io::stdout().flush()?;

    let mut api_key = String::new();
    io::stdin().read_line(&mut api_key)?;
    let api_key = api_key.trim().to_string();

    if api_key.is_empty() {
        anyhow::bail!("API key cannot be empty");
    }

    let config = Config::new(api_key);
    config.save()?;

    println!();
    println!("Configuration saved to {:?}", Config::config_path()?);
    println!("Starting LazyPost...");
    println!();

    Ok(config)
}

fn setup_terminal() -> Result<Terminal<CrosstermBackend<io::Stdout>>> {
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let terminal = Terminal::new(backend)?;
    Ok(terminal)
}

fn restore_terminal(terminal: &mut Terminal<CrosstermBackend<io::Stdout>>) -> Result<()> {
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;
    Ok(())
}

async fn perform_save(
    client: api::PostmanClient,
    collection_uid: String,
    collection: Option<api::CollectionDetail>,
    flat_items: Vec<app::FlatItem>,
    pending: app::PendingSave,
) -> Result<(api::CollectionDetail, String)> {
    let collection = collection.context("No collection loaded")?;
    let flat_item = &flat_items[pending.item_index];
    let path = flat_item.path.clone();
    let request_id = flat_item.request_id.clone();

    // Try to use individual request endpoint if we have a request_id
    if let Some(ref req_id) = request_id {
        // Build the request object from the edited data
        let request = api::Request {
            method: pending.edited.method.clone(),
            url: if pending.edited.url.is_empty() {
                api::RequestUrl::Empty
            } else {
                api::RequestUrl::Simple(pending.edited.url.clone())
            },
            header: flat_item.request.as_ref().map(|r| r.header.clone()).unwrap_or_default(),
            body: if pending.edited.body.is_empty() {
                None
            } else {
                Some(api::RequestBody {
                    mode: Some("raw".to_string()),
                    raw: Some(pending.edited.body.clone()),
                })
            },
            description: flat_item.request.as_ref().and_then(|r| r.description.clone()),
        };

        // Use individual endpoint - avoids validation errors from other requests
        client.update_request(&collection_uid, req_id, &pending.edited.name, &request).await?;
    } else {
        // Fall back to bulk update if no request_id available
        let mut items = collection.item.clone();
        update_request_at_path(&mut items, &path, &pending.edited)?;
        client.update_collection(&collection_uid, &collection.info, &items).await?;
    }

    // Reload collection to get updated state
    let updated = client.get_collection(&collection_uid).await?;

    Ok((updated, pending.edited.name))
}

fn update_request_at_path(
    items: &mut Vec<api::Item>,
    path: &[usize],
    edited: &app::EditableRequest,
) -> Result<()> {
    use api::{Item, RequestBody, RequestUrl};

    if path.is_empty() {
        anyhow::bail!("Empty path");
    }

    let index = path[0];
    let remaining_path = &path[1..];

    if index >= items.len() {
        anyhow::bail!("Invalid path index");
    }

    if remaining_path.is_empty() {
        match &mut items[index] {
            Item::Request(req_item) => {
                req_item.name = edited.name.clone();
                req_item.request.method = edited.method.clone();
                req_item.request.url = if edited.url.is_empty() {
                    RequestUrl::Empty
                } else {
                    RequestUrl::Simple(edited.url.clone())
                };
                req_item.request.body = if edited.body.is_empty() {
                    None
                } else {
                    Some(RequestBody {
                        mode: Some("raw".to_string()),
                        raw: Some(edited.body.clone()),
                    })
                };
                Ok(())
            }
            Item::Folder(_) => {
                anyhow::bail!("Path points to a folder, not a request");
            }
        }
    } else {
        match &mut items[index] {
            Item::Folder(folder) => update_request_at_path(&mut folder.item, remaining_path, edited),
            Item::Request(_) => {
                anyhow::bail!("Path traverses through a request");
            }
        }
    }
}

fn edit_request_in_editor(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    request: &EditableRequest,
) -> Result<Option<EditableRequest>> {
    // Create temp file with request data
    let temp_dir = env::temp_dir();
    let temp_file = temp_dir.join("lazypost_edit.toml");

    let content = toml::to_string_pretty(request)
        .context("Failed to serialize request")?;

    fs::write(&temp_file, &content)
        .context("Failed to write temp file")?;

    // Exit TUI mode
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    // Get editor from environment
    let editor = env::var("EDITOR")
        .or_else(|_| env::var("VISUAL"))
        .unwrap_or_else(|_| "vim".to_string());

    // Run editor
    let status = Command::new(&editor)
        .arg(&temp_file)
        .status()
        .context(format!("Failed to run editor: {}", editor))?;

    // Re-enter TUI mode
    enable_raw_mode()?;
    execute!(terminal.backend_mut(), EnterAlternateScreen)?;
    terminal.hide_cursor()?;
    terminal.clear()?;

    if !status.success() {
        fs::remove_file(&temp_file).ok();
        return Ok(None);
    }

    // Read edited content
    let edited_content = fs::read_to_string(&temp_file)
        .context("Failed to read edited file")?;

    fs::remove_file(&temp_file).ok();

    // Check if content changed
    if edited_content == content {
        return Ok(None);
    }

    // Parse edited content
    let edited: EditableRequest = toml::from_str(&edited_content)
        .context("Failed to parse edited request. Check TOML syntax.")?;

    Ok(Some(edited))
}

async fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    config: Config,
) -> Result<()> {
    let mut app = App::new(config);

    app.load_collections().await?;

    // Restore last selected collection and request
    app.restore_last_state().await?;

    loop {
        terminal.draw(|frame| ui::render(frame, &mut app))?;

        if event::poll(Duration::from_millis(100))? {
            if let Event::Key(key) = event::read()? {
                if key.kind != KeyEventKind::Press {
                    continue;
                }

                match app.input_mode {
                    InputMode::Normal => {
                        match key.code {
                            KeyCode::Char('q') => {
                                return Ok(());
                            }
                            // Pane switching with number keys
                            KeyCode::Char('1') => {
                                app.set_focus(FocusedPane::Collections);
                            }
                            KeyCode::Char('2') => {
                                app.set_focus(FocusedPane::Requests);
                            }
                            KeyCode::Char('3') => {
                                app.set_focus(FocusedPane::Preview);
                            }
                            KeyCode::Char('4') => {
                                if app.response.is_some() {
                                    app.set_focus(FocusedPane::Response);
                                }
                            }
                            // Navigation
                            KeyCode::Char('j') | KeyCode::Down => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_viewer_down();
                                } else {
                                    app.move_down();
                                }
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_viewer_up();
                                } else {
                                    app.move_up();
                                }
                            }
                            // JSON viewer collapse/expand (Response pane only)
                            KeyCode::Char('h') | KeyCode::Left => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_viewer_collapse();
                                }
                            }
                            KeyCode::Char('l') | KeyCode::Right => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_viewer_expand();
                                }
                            }
                            // JSON viewer collapse/expand all (Response pane only)
                            KeyCode::Char('H') => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_viewer_collapse_all();
                                }
                            }
                            KeyCode::Char('L') => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_viewer_expand_all();
                                }
                            }
                            // JSON viewer yank (copy) to clipboard
                            KeyCode::Char('y') => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_viewer_yank();
                                }
                            }
                            // Enter key behavior depends on focused pane
                            KeyCode::Enter => match app.focused_pane {
                                FocusedPane::Collections => {
                                    app.start_collection_load();
                                }
                                FocusedPane::Requests => {
                                    if let Some(item) = app.flat_items.get(app.selected_item_index) {
                                        if item.is_folder {
                                            app.toggle_folder();
                                        } else {
                                            app.select_request();
                                        }
                                    }
                                }
                                FocusedPane::Preview => {
                                    // Execute if we have a request
                                    if app.current_request.is_some() {
                                        // Check if method needs confirmation
                                        if !app.start_execute_confirmation() {
                                            // No confirmation needed, execute directly
                                            app.request_executing = true;
                                            terminal.draw(|frame| ui::render(frame, &mut app))?;
                                            app.execute_current_request().await?;
                                        }
                                    }
                                }
                                FocusedPane::Response => {
                                    // Toggle expand/collapse in JSON viewer
                                    if app.json_viewer_state.is_some() {
                                        app.json_viewer_toggle();
                                    }
                                }
                            },
                            // Execute request
                            KeyCode::Char('e') => {
                                if app.current_request.is_some() {
                                    // Check if method needs confirmation
                                    if !app.start_execute_confirmation() {
                                        // No confirmation needed, execute directly
                                        app.request_executing = true;
                                        terminal.draw(|frame| ui::render(frame, &mut app))?;
                                        app.execute_current_request().await?;
                                    }
                                }
                            }
                            // Edit request in external editor
                            KeyCode::Char('E') => {
                                if let Some((request, item_index)) = app.get_current_request_for_edit() {
                                    match edit_request_in_editor(terminal, &request) {
                                        Ok(Some(edited)) => {
                                            // Store edit locally - user can press S to save to Postman
                                            app.store_local_edit(edited, item_index);
                                        }
                                        Ok(None) => {
                                            app.status_message = String::from("Edit cancelled or no changes");
                                        }
                                        Err(e) => {
                                            let error_msg = e.to_string();
                                            log_error("edit_request_in_editor", &error_msg);
                                            app.error = Some(error_msg);
                                            app.status_message = String::from("Edit failed");
                                        }
                                    }
                                }
                            }
                            // Save unsaved edits to Postman
                            KeyCode::Char('S') => {
                                app.start_saving_edit();
                            }
                            // Add new request
                            KeyCode::Char('a') => {
                                app.start_new_request_dialog();
                            }
                            // Toggle favorite
                            KeyCode::Char('f') => {
                                app.toggle_favorite();
                            }
                            // Search
                            KeyCode::Char('/') => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_search_start();
                                } else {
                                    app.start_search();
                                }
                            }
                            KeyCode::Char('n') => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_search_next();
                                } else {
                                    app.next_match();
                                }
                            }
                            KeyCode::Char('N') => {
                                if app.focused_pane == FocusedPane::Response && app.json_viewer_state.is_some() {
                                    app.json_search_prev();
                                } else {
                                    app.prev_match();
                                }
                            }
                            // Tab to cycle through panes
                            KeyCode::Tab => {
                                let next_pane = match app.focused_pane {
                                    FocusedPane::Collections => FocusedPane::Requests,
                                    FocusedPane::Requests => FocusedPane::Preview,
                                    FocusedPane::Preview => {
                                        if app.response.is_some() {
                                            FocusedPane::Response
                                        } else {
                                            FocusedPane::Collections
                                        }
                                    }
                                    FocusedPane::Response => FocusedPane::Collections,
                                };
                                app.set_focus(next_pane);
                            }
                            // Environment selection
                            KeyCode::Char('v') => {
                                app.open_environment_popup();
                            }
                            // Variables view
                            KeyCode::Char('V') => {
                                app.open_variables_popup();
                            }
                            // Workspace selection
                            KeyCode::Char('w') => {
                                app.open_workspace_popup();
                            }
                            _ => {}
                        }
                    }
                    InputMode::TextInput => {
                        match key.code {
                            KeyCode::Esc => {
                                app.cancel_dialog();
                            }
                            KeyCode::Enter => {
                                if app.dialog_next_step() {
                                    app.create_new_request().await?;
                                }
                            }
                            KeyCode::Backspace => {
                                app.dialog_backspace();
                            }
                            KeyCode::Left => {
                                app.dialog_move_cursor_left();
                            }
                            KeyCode::Right => {
                                app.dialog_move_cursor_right();
                            }
                            KeyCode::Char(c) => {
                                app.dialog_input_char(c);
                            }
                            _ => {}
                        }
                    }
                    InputMode::Search => {
                        match key.code {
                            KeyCode::Esc => {
                                app.cancel_search();
                            }
                            KeyCode::Enter => {
                                app.confirm_search();
                            }
                            KeyCode::Backspace => {
                                app.search_backspace();
                            }
                            KeyCode::Char(c) => {
                                app.search_input_char(c);
                            }
                            _ => {}
                        }
                    }
                    InputMode::JsonSearch => {
                        match key.code {
                            KeyCode::Esc => {
                                app.json_search_cancel();
                            }
                            KeyCode::Enter => {
                                app.json_search_confirm();
                            }
                            KeyCode::Backspace => {
                                app.json_search_backspace();
                            }
                            KeyCode::Char(c) => {
                                app.json_search_input(c);
                            }
                            _ => {}
                        }
                    }
                    InputMode::Saving => {
                        // Handled separately below with select!
                    }
                    InputMode::EnvironmentSelect => {
                        match key.code {
                            KeyCode::Esc => {
                                app.close_environment_popup();
                            }
                            KeyCode::Enter => {
                                app.confirm_environment_selection().await;
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.environment_popup_down();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.environment_popup_up();
                            }
                            _ => {}
                        }
                    }
                    InputMode::VariablesView => {
                        if app.editing_variable.is_some() {
                            // Editing mode
                            match key.code {
                                KeyCode::Esc => {
                                    app.cancel_variable_edit();
                                }
                                KeyCode::Enter => {
                                    app.confirm_variable_edit();
                                }
                                KeyCode::Backspace => {
                                    app.variable_backspace();
                                }
                                KeyCode::Left => {
                                    app.variable_cursor_left();
                                }
                                KeyCode::Right => {
                                    app.variable_cursor_right();
                                }
                                KeyCode::Char(c) => {
                                    app.variable_input_char(c);
                                }
                                _ => {}
                            }
                        } else if app.variables_search_active {
                            // Search input mode
                            match key.code {
                                KeyCode::Esc => {
                                    app.cancel_variables_search();
                                }
                                KeyCode::Enter => {
                                    app.confirm_variables_search();
                                }
                                KeyCode::Backspace => {
                                    app.variables_search_backspace();
                                }
                                KeyCode::Char(c) => {
                                    app.variables_search_input_char(c);
                                }
                                _ => {}
                            }
                        } else {
                            // Navigation mode
                            match key.code {
                                KeyCode::Esc => {
                                    if !app.variables_search_query.is_empty() {
                                        // Clear search first
                                        app.cancel_variables_search();
                                    } else {
                                        app.close_variables_popup();
                                    }
                                }
                                KeyCode::Enter => {
                                    app.start_editing_variable();
                                }
                                KeyCode::Char('j') | KeyCode::Down => {
                                    app.variables_popup_down();
                                }
                                KeyCode::Char('k') | KeyCode::Up => {
                                    app.variables_popup_up();
                                }
                                KeyCode::Char('s') => {
                                    app.save_variables_to_postman().await;
                                }
                                KeyCode::Char('/') => {
                                    app.start_variables_search();
                                }
                                _ => {}
                            }
                        }
                    }
                    InputMode::WorkspaceSelect => {
                        match key.code {
                            KeyCode::Esc => {
                                app.close_workspace_popup();
                            }
                            KeyCode::Enter => {
                                app.confirm_workspace_selection();
                            }
                            KeyCode::Char('j') | KeyCode::Down => {
                                app.workspace_popup_down();
                            }
                            KeyCode::Char('k') | KeyCode::Up => {
                                app.workspace_popup_up();
                            }
                            _ => {}
                        }
                    }
                    InputMode::ExecuteConfirm => {
                        match key.code {
                            KeyCode::Char('y') | KeyCode::Char('Y') | KeyCode::Enter => {
                                // User confirmed, execute the request
                                app.pending_execute = None;
                                app.input_mode = InputMode::Normal;
                                app.request_executing = true;
                                terminal.draw(|frame| ui::render(frame, &mut app))?;
                                app.execute_current_request().await?;
                            }
                            KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Esc => {
                                app.cancel_execute_confirmation();
                            }
                            _ => {}
                        }
                    }
                }
            }
        }

        // If workspace is loading, perform the load
        if app.workspace_loading.is_some() {
            // Draw once to show the loading popup
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            // Perform the async load
            app.load_workspace_data().await;
        }

        // If collection is loading, perform the load
        if app.collection_loading.is_some() {
            // Draw once to show the loading popup
            terminal.draw(|frame| ui::render(frame, &mut app))?;
            // Perform the async load
            app.load_collection_data().await;
        }

        // If in saving mode, perform the save with cancellation support
        if app.input_mode == InputMode::Saving {
            if let Some(pending) = app.pending_save.take() {
                // Show the saving popup
                terminal.draw(|frame| ui::render(frame, &mut app))?;

                // Save the path before moving pending (for clearing local edit on success)
                let saved_item_path = app.flat_items.get(pending.item_index)
                    .map(|item| item.path.clone());

                // Spawn the save task
                let client = app.client.clone();
                let collection_uid = app.collections[app.selected_collection_index].uid.clone();
                let collection = app.current_collection.clone();
                let flat_items = app.flat_items.clone();

                let save_handle = tokio::spawn(async move {
                    perform_save(client, collection_uid, collection, flat_items, pending).await
                });

                // Poll for completion while checking for Esc
                let result = loop {
                    // Check for Esc key to cancel
                    if event::poll(Duration::from_millis(50))? {
                        if let Event::Key(key) = event::read()? {
                            if key.kind == KeyEventKind::Press && key.code == KeyCode::Esc {
                                save_handle.abort();
                                app.cancel_saving();
                                break None;
                            }
                        }
                    }

                    // Check if save completed
                    if save_handle.is_finished() {
                        match save_handle.await {
                            Ok(Ok(save_result)) => break Some(Ok(save_result)),
                            Ok(Err(e)) => break Some(Err(e)),
                            Err(_) => break None, // Task was cancelled
                        }
                    }

                    // Redraw UI
                    terminal.draw(|frame| ui::render(frame, &mut app))?;
                };

                // Handle save result
                if let Some(result) = result {
                    match result {
                        Ok((updated_collection, request_name)) => {
                            app.current_collection = Some(updated_collection);
                            app.flatten_items();
                            // Clear the local edit now that it's saved to Postman
                            if let Some(path) = saved_item_path {
                                app.clear_local_edit(&path);
                            }
                            app.status_message = format!("Saved '{}'", request_name);
                            app.update_preview_from_selection();
                        }
                        Err(e) => {
                            let error_msg = e.to_string();
                            log_error("save_request", &error_msg);
                            app.error = Some(error_msg);
                            app.status_message = String::from("Failed to save changes");
                        }
                    }
                    app.input_mode = InputMode::Normal;
                }
            }
        }
    }
}
