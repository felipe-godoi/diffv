# diffv ⚡
> High-performance VS Code style CLI diff viewer to use alongside your AI coding agent.

[![CI](https://github.com/felipe-godoi/diffv/actions/workflows/ci.yml/badge.svg)](https://github.com/felipe-godoi/diffv/actions/workflows/ci.yml)
[![Release](https://img.shields.io/github/v/release/felipe-godoi/diffv?color=blue)](https://github.com/felipe-godoi/diffv/releases)
[![License](https://img.shields.io/badge/license-MIT%20OR%20Apache--2.0-blue.svg)](LICENSE-MIT)
[![Rust](https://img.shields.io/badge/rust-1.75%2B-orange.svg)](Cargo.toml)

<p align="center">
  <img src="assets/demo.gif" alt="diffv demo" width="100%" />
</p>

`diffv` brings the visual clarity, dual-column side-by-side layout, and intra-line word/character highlighting of the VS Code Diff Editor straight into your terminal, designed specifically for seamless pairing with **tmux**, your editor (`$EDITOR`), and autonomous AI coding agents (Antigravity CLI, Claude Code, Aider, Copilot, etc.).


---

## ✨ Features

- 🌟 **Side-by-Side Dual-Column Diffing**: Flawless vertical synchronization with filler/padding lines matching the VS Code Diff Editor.
- 🔍 **Intra-Line Character & Word Highlighting**: Subtle background coloring for deleted/added lines with high-contrast highlighted spans for changed tokens.
- ⚡ **AI Companion Mode (`--watch`)**: Listens to disk changes via debounced filesystem events (150ms). Silently reloads diffs in real-time without resetting your scroll position or active file.
- 🎨 **Adaptive Terminal Theme & Built-in Themes**: Adapts naturally to your terminal's active palette, transparency, and dark/light mode by default (`auto`), with optional built-in themes (`vscode-dark`, `tokyonight`, `catppuccin`, `gruvbox`).
- 🌲 **Interactive File Drawer & Quick Filter**: Fast navigation with `j`/`k`, status indicators (`M`, `A`, `D`, `?`, `[S]`, `[U]`), and instant fuzzy search filtering (`/`).
- 🎯 **Granular Staging & Discarding**: Stage (`s`), unstage (`u`), or discard (`d`) individual hunks, or stage (`S`), unstage (`U`), or discard (`D`) entire files with quick confirmation dialogs.
- 📋 **AI Context Copying (`c`)**: One-key copy of the active hunk or file diff formatted in Markdown directly to your system clipboard (`arboard`) to easily paste into AI agent prompts.
- ✏️ **Instant Editor Jump (`e` or `Enter`)**: Opens `$EDITOR +<line> <file>` (`$GIT_EDITOR` and `$VISUAL` take precedence, as in git). With none of them set it uses the system default editor (`editor`, the Debian/Ubuntu `update-alternatives` entry), then `vi`, then `nano`. Optionally, when you run diffv inside Neovim (`$NVIM` set) with `use_nvr = true` and `nvr` (neovim-remote) installed, the file opens in that running instance.
- 📂 **Multi-Mode Comparison**: Works with Git working trees, `--staged` mode, specific Git branches/commits (`HEAD~1`, `main..feat`), arbitrary local files (`diffv a.rs b.rs`), local directories (`diffv dir1 dir2`), and standard input pipes (`git diff | diffv -`).
- 🔀 **Branch Comparison Selector (`B` / `--compare`)**: Interactive branch picker popup (just like VS Code) to compare your current worktree against any local or remote branch (`main`, `origin/main`, etc.), with instant fuzzy search and quick reset to default worktree changes.
- 📜 **File Commit History (`H` / `-H`)**: Inspect commit log entries for the active file with author, relative timestamp, and commit message, and preview individual commit diffs in-place.

---

## 🚀 Installation

### Automated Install (macOS & Linux)

Via `curl`:
```bash
curl -fsSL https://raw.githubusercontent.com/felipe-godoi/diffv/main/install.sh | bash
```

Or from a local cloned repository:
```bash
./install.sh
```

The installer detects your OS and architecture, downloads the latest release binary, verifies its SHA-256 checksum, and installs it to `~/.local/bin/diffv`. Remote installation needs no Rust toolchain or GitHub login. Running the installer from a source checkout builds the local code. If `fzf` is not installed, the script offers it as an **optional** extra with a `(y/N)` prompt — pressing Enter skips it — and in non-interactive runs (`curl … | bash`) it only prints a one-line tip; a missing or failed `fzf` install never fails the diffv installation.

### Manual Install via Cargo

```bash
cargo install --git https://github.com/felipe-godoi/diffv.git
```

### Optional: `fzf`

[`fzf`](https://github.com/junegunn/fzf) is **optional**. When it is on your `PATH`, `Ctrl+p` (find file) and `Ctrl+f` (search diff text) open it full-screen for the fastest fuzzy matching in the terminal. Without it, `diffv` falls back to a **built-in picker** popup with the same candidates (fuzzy/substring filter, `↑`/`↓`, `Enter` to open, `Esc` / `Ctrl+C` / `Ctrl+Q` to cancel like fzf — `Shift+Q` types a "Q" there) — searching keeps working, you just get the simpler in-app list instead of fzf's interface.

```bash
# macOS
brew install fzf
# Debian / Ubuntu
sudo apt install fzf
```

### Uninstall

```bash
diffv --uninstall
```

Or via the installer script:
```bash
./install.sh --uninstall
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

# Compare current worktree against another branch (or press B inside diffv)
diffv --compare main
# or alias:
diffv --branch main

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

When something is staged, the Changes drawer splits into **Staged** (HEAD → index) and **Changes** (index → working tree), like VS Code and lazygit. A file with both kinds of edits appears in both groups, and every hunk shown is exactly what `s` / `u` apply.

| Key | Context | Action |
|---|---|---|
| `j` / `k` or `↓` / `↑` | General | Scroll lines down / up |
| `J` / `K` or `Ctrl+d` / `Ctrl+u` | Diff | Scroll half page down / up |
| `]` or `n` | Diff | Jump to **Next Hunk** |
| `[` or `p` | Diff | Jump to **Previous Hunk** |
| `Tab` | General | Switch focus between **File Tree** and **Diff View** |
| `b` | General | **Toggle Sidebar** (show/hide file tree drawer, great for split/half terminals) |
| `B` | General | Open **Branch Comparison Selector** popup (compare worktree with any branch or reset to Default) |
| `1` / `2` / `3` | General | Switch drawer tabs: **[1] Changes**, **[2] Commits**, **[3] Stashes** |
| `W` | General | Open **Worktrees Switcher** popup |
| `L` | General | Toggle UI language: **English** ↔ **Português** |
| `h` / `l` or `←` / `→` | General | Tree: collapse/expand folder. Diff: switch Old/New column |
| `Space` | File Tree | Toggle collapse/expand on current folder |
| `<` / `>` or `,` / `.` | General | **Resize Panes**: shrink / expand File Tree width |
| `Mouse Wheel` | General | **Scroll the view** of the hovered pane (File Tree or Diff); the cursor stays put and is only dragged along when it would leave the screen |
| `Mouse Click` | General | **Select** file / line, collapse/expand folders, switch focus, or click branch badge to open Branch Selector |
| `Mouse Drag` | General | **Drag vertical divider** to resize File Tree width |
| `Mouse Drag` | Diff | **Select diff text** (unified, side-by-side or commit diff) and copy it to the clipboard on release. `Ctrl+C` copies again, `Esc` clears. Gutters and `+`/`-` markers are left out |
| `t` | File Tree | Toggle **📁 Pastas (Tree)** ↔ **📄 Lista (Flat)** |
| `v` | Diff | Toggle **Visual Mode** for line-level partial staging / unstaging |
| `o` | Commits / History | Open the selected commit's **GitHub pull request** in the browser (falls back to the commit page; uses `gh` when available) |
| `x` | General | **Expand full file** ↔ changes only (keeps file and cursor line) |
| `Ctrl+p` | General | Fuzzy-find files in the current scope (changes, open/selected commit or stash). Uses `fzf` when installed, otherwise the built-in picker |
| `Ctrl+f` | General | Fuzzy-search diff text — only the open file when in the Diff pane, otherwise the current commit/stash/changes. File paths are not matched. Uses `fzf` when installed, otherwise the built-in picker |
| `Enter` | File Tree / Diff | Tree: select file / toggle folder. Diff: open in your editor |
| `e` | Diff | Open file in your editor (`$EDITOR`; system default editor, then `vi`/`nano` when unset) at cursor line (`+line`) |
| `s` | Diff | **Stage** current hunk (or Visual lines) of a file under **Changes** |
| `u` | Diff | **Unstage** current hunk (or Visual lines) of a file under **Staged** |
| `d` | Diff | **Discard** current hunk under **Changes** (with `y/n` confirmation) |
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

To show/hide `diffv` in a centered, floating popup with a single shortcut, add the following to your `~/.tmux.conf`:

```tmux
# Prefix + d shows/hides diffv (live watch mode). While hidden, diffv keeps running
# in a background session per project, so file, cursor and open commit are preserved.
bind-key d run-shell -b "diffv --tmux-toggle '#{client_name}' '#{session_name}' '#{pane_current_path}'"
```

Press `q` inside diffv to close it for real. If diffv cannot start (e.g. the directory is not a git repository), the popup shows the error and waits for a key instead of closing immediately. Use `--wait-on-error` for the same behavior in your own popups or scripts.

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
command = "vi"                 # Default: $GIT_EDITOR > $VISUAL > $EDITOR > system `editor` > vi > nano
args = ["+{{line}}", "{{file}}"]
use_nvr = true                 # Optional: inside Neovim ($NVIM set) with nvr installed, open in that instance
```

---

## 🧪 Running Tests

```bash
cargo test
```

All unit tests for side-by-side alignment, fillers, code tokenization intra-line diffing, unified diff parser, config loading, and Git lifecycle run and pass with zero warnings.

---

## 🤝 Contributing

Contributions are welcome! Please check out [CONTRIBUTING.md](CONTRIBUTING.md) for local development setup, coding guidelines, and our PR workflow. Please read our [Code of Conduct](CODE_OF_CONDUCT.md) before participating.

---

## 📄 License

Dual-licensed under either of:

- Apache License, Version 2.0 ([LICENSE-APACHE](LICENSE-APACHE) or <http://www.apache.org/licenses/LICENSE-2.0>)
- MIT license ([LICENSE-MIT](LICENSE-MIT) or <http://opensource.org/licenses/MIT>)

at your option.

## Automatic updates & Release Channels

At startup, `diffv` checks for updates on GitHub, downloads newer binaries for the current OS/architecture, verifies SHA-256 digests, replaces the executable atomically, and restarts with the same arguments. Offline checks and failed downloads leave the existing installation available.

- **Available Channels:**
  - `stable` (default): Official immutable releases (`/releases/latest`).
  - `beta`: Release Candidate builds from the `beta` branch.
  - `nightly`: Continuous builds from the latest commit on `main`.

Use `diffv --no-update` or `DIFFV_NO_UPDATE=1 diffv` to skip the check. Help and version commands do not make network requests. Installation and updates use public HTTPS downloads; no GitHub account or authentication is required.

For full details on the development lifecycle, branch model, and CI/CD pipelines, see [docs/release-flow.md](docs/release-flow.md).
