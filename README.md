# diffv ⚡
> High-performance VS Code style CLI diff viewer and companion for AI coding agents.

`diffv` brings the visual clarity, dual-column side-by-side layout, and intra-line word/character highlighting of the VS Code Diff Editor straight into your terminal, designed specifically for seamless pairing with **tmux**, **Neovim**, and autonomous AI coding agents (Antigravity CLI, Claude Code, Aider, Copilot, etc.).

---

## ✨ Features

- 🌟 **Side-by-Side Dual-Column Diffing**: Flawless vertical synchronization with filler/padding lines matching the VS Code Diff Editor.
- 🔍 **Intra-Line Character & Word Highlighting**: Subtle background coloring for deleted/added lines with high-contrast highlighted spans for changed tokens.
- ⚡ **AI Companion Mode (`--watch`)**: Listens to disk changes via debounced filesystem events (150ms). Silently reloads diffs in real-time without resetting your scroll position or active file.
- 🎨 **Adaptive Terminal Theme & Built-in Themes**: Adapts naturally to your terminal's active palette, transparency, and dark/light mode by default (`auto`), with optional built-in themes (`vscode-dark`, `tokyonight`, `catppuccin`, `gruvbox`).
- 🌲 **Interactive File Drawer & Quick Filter**: Fast navigation with `j`/`k`, status indicators (`M`, `A`, `D`, `?`, `[S]`, `[U]`), and instant fuzzy search filtering (`/`).
- 🎯 **Granular Staging & Discarding**: Stage (`s`), unstage (`u`), or discard (`d`) individual hunks, or stage (`S`), unstage (`U`), or discard (`D`) entire files with quick confirmation dialogs.
- 📋 **AI Context Copying (`c`)**: One-key copy of the active hunk or file diff formatted in Markdown directly to your system clipboard (`arboard`) to easily paste into AI agent prompts.
- ✏️ **Instant Neovim / $EDITOR Jump (`e` or `Enter`)**: Opens `$EDITOR +<line> <file>` or routes to an active Neovim session via `nvr` (neovim-remote).
- 📂 **Multi-Mode Comparison**: Works with Git working trees, `--staged` mode, specific Git branches/commits (`HEAD~1`, `main..feat`), arbitrary local files (`diffv a.rs b.rs`), local directories (`diffv dir1 dir2`), and standard input pipes (`git diff | diffv -`).
- 📜 **File Commit History (`H` / `-H`)**: Inspect commit log entries for the active file with author, relative timestamp, and commit message, and preview individual commit diffs in-place.

---

## 🚀 Installation

### Automated Installer (macOS & Linux)

You can install `diffv` with a single command:

```bash
# Via curl
curl -fsSL https://raw.githubusercontent.com/felipegodoi/cli-diffviewer/main/install.sh | bash

# Or from a cloned repository
./install.sh
```

The installer detects your OS and architecture, compiles an optimized release build, copies the executable to `~/.local/bin/diffv`, and ensures it is available in your `$PATH`.

To uninstall:
```bash
./install.sh --uninstall
```

### Manual Install via Cargo

```bash
cargo install --git https://github.com/felipegodoi/cli-diffviewer.git
```

---

## 📖 Usage

```bash
# Open in current Git repository (working tree vs HEAD)
diffv

# AI Companion Watch Mode (continuous live reload on disk changes)
diffv --watch
# or shortcut:
diffv -w

# View only staged / cached changes
diffv --staged

# View a specific commit or branch diff
diffv HEAD~1
diffv main..feature-branch

# Compare two arbitrary files
diffv file_original.py file_modified.py

# Compare two directories recursively
diffv ./v1/ ./v2/

# Read patch from stdin
git diff | diffv -

# Start directly in Unified / Inline mode
diffv --unified

# Choose a specific theme
diffv --theme tokyonight
```

---

## ⌨️ Keybindings

| Key | Context | Action |
|---|---|---|
| `j` / `k` or `↓` / `↑` | General | Scroll lines down / up |
| `J` / `K` or `Ctrl+d` / `Ctrl+u` | Diff | Scroll half page down / up |
| `]` or `n` | Diff | Jump to **Next Hunk** |
| `[` or `p` | Diff | Jump to **Previous Hunk** |
| `Tab` | General | Switch focus between **File Tree** and **Diff View** |
| `h` / `l` or `←` / `→` | General | Tree: collapse/expand folder. Diff: switch Old/New column |
| `t` | File Tree | Toggle **Flat List** ↔ **Collapsible Directory Tree** |
| `v` | Diff | Toggle **Visual Mode** for line-level partial staging |
| `Enter` | File Tree / Diff | Tree: select file / toggle folder. Diff: open in Neovim |
| `e` | Diff | Open file in Neovim / `$EDITOR` at cursor line (`+line`) |
| `s` | Diff | **Stage** current hunk (or selected lines in Visual Mode) |
| `u` | Diff | **Unstage** current hunk |
| `d` | Diff | **Discard** current hunk (with `y/n` confirmation) |
| `S` | General | **Stage** entire file |
| `U` | General | **Unstage** entire file |
| `D` | General | **Discard** entire file (with `y/n` confirmation) |
| `c` | Diff | **Copy** hunk to clipboard as Markdown |
| `H` | General | Open **File Commit History** modal & diff preview |
| `m` | General | Toggle view mode (**Side-by-Side** ↔ **Unified**) |
| `w` | General | Toggle **Live Watch Mode** |
| `/` | File Tree | Filter files by name/extension (fuzzy search) |
| `?` | General | Toggle **Help Modal** with full shortcut list |
| `q` / `Esc` | General | Quit `diffv` (or exit Visual/Filter mode) |

---

## 🪟 Tmux Integration

To open `diffv` in a centered, floating popup at the touch of a shortcut in Tmux, add the following to your `~/.tmux.conf`:

```tmux
# Press Prefix + d to open diffv floating popup in watch mode
bind-key d display-popup -d "#{pane_current_path}" -w 92% -h 90% -E "diffv --watch"
```

Reload tmux configuration:
```bash
tmux source-file ~/.tmux.conf
```

---

## ⚙️ Configuration

Configuration is loaded from `~/.config/diffv/config.toml`. A sample configuration file:

```toml
[ui]
theme = "vscode-dark"          # vscode-dark, tokyonight, catppuccin, gruvbox
default_view = "side-by-side"  # side-by-side or unified
show_line_numbers = true
syntax_highlighting = true
overview_ruler = true
tab_width = 4

[diff]
algorithm = "patience"         # patience, myers, lcs
ignore_whitespace = false
context_lines = 3

[watcher]
enabled = true
debounce_ms = 150
watch_untracked = true

[editor]
command = "nvim"
args = ["+{{line}}", "{{file}}"]
use_nvr = true                 # Uses nvr (neovim-remote) if running inside tmux/nvim
```

---

## 🧪 Running Tests

```bash
cargo test
```

All unit tests for side-by-side alignment, fillers, code tokenization intra-line diffing, unified diff parser, config loading, and Git lifecycle run and pass with zero warnings.

---

## 📄 License

MIT OR Apache-2.0
