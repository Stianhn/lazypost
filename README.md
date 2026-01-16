# LazyPost

A terminal UI client for Postman collections, inspired by lazygit.

## Features

- Browse and manage Postman collections
- Execute HTTP requests directly from the terminal
- Three-pane layout with vim-style navigation
- Favorite collections for quick access
- Search collections and requests with `/`
- Edit requests using your preferred text editor

## Installation

```bash
cargo build --release
```

## Configuration

On first run, LazyPost will prompt for your Postman API key. You can find your API key at: https://web.postman.co/settings/me/api-keys

Configuration is stored at `~/.config/lazypost/config.toml`

## Key Bindings

### Navigation
- `1/2/3` - Switch between panes (Collections, Requests, Preview)
- `Tab` - Cycle through panes
- `j/k` or `Up/Down` - Navigate lists
- `Enter` - Load collection / Select request / Execute
- `q` - Quit

### Actions
- `e` - Execute current request
- `E` - Edit request in external editor
- `a` - Add new request (in Requests pane)
- `f` - Toggle favorite (in Collections pane)
- `/` - Search current list
- `n/N` - Next/Previous search match

## Editor Selection

When pressing `E` to edit a request, LazyPost uses your system's configured editor. The editor is selected in the following order:

1. `$EDITOR` environment variable
2. `$VISUAL` environment variable
3. `vim` (fallback)

### Configuring Your Editor

Set your preferred editor by adding one of these to your shell configuration (`.bashrc`, `.zshrc`, etc.):

```bash
# For neovim
export EDITOR=nvim

# For vim
export EDITOR=vim

# For VS Code (will wait for file to close)
export EDITOR="code --wait"

# For nano
export EDITOR=nano
```

### How Editing Works

1. Press `E` on a selected request
2. LazyPost creates a temporary TOML file with the request data
3. Your editor opens with the file
4. Make your changes and save/close the editor
5. LazyPost parses the changes and saves to Postman

The editable fields are:
- `name` - Request name
- `method` - HTTP method (GET, POST, PUT, etc.)
- `url` - Request URL
- `body` - Request body (for POST/PUT requests)

## Error Logging

Errors are logged to `~/.config/lazypost/error.log` with timestamps for debugging.

## License

MIT
