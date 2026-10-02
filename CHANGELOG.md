# Changelog

## [0.4.0](https://github.com/felipe-godoi/diffv/compare/diffv-v0.3.1...diffv-v0.4.0) (2026-10-02)


### Features

* add --uninstall flag, non-blocking async file watcher and .gitignore filtering ([8c21760](https://github.com/felipe-godoi/diffv/commit/8c217602eec812423ec9d9c1632a09f6c545876c))
* add CLI abbreviations, dv alias, beta channel opt-in and auto-update opt-out ([3b1effe](https://github.com/felipe-godoi/diffv/commit/3b1effeba31eceef253e7b22c4b89e0fb1021b46))
* add cross-platform installer (macOS & Linux) and auto terminal theme adaptation ([1ef08f3](https://github.com/felipe-godoi/diffv/commit/1ef08f3d3260cbfd3b0b91c164355fc30a5e1970))
* add discreet file watcher scan progress indicator ([4ebca99](https://github.com/felipe-godoi/diffv/commit/4ebca99b61a4bb143dbf7e73d5caf7d65f3853ef))
* add file commit history view modal and diff preview ([ea71df8](https://github.com/felipe-godoi/diffv/commit/ea71df8388a8c0527e9b461b11f04273ecaea34e))
* add interactive settings modal and persistent configuration ([c9071ff](https://github.com/felipe-godoi/diffv/commit/c9071ff4c07d26ca8404215576d9c5985d54e21e))
* automatic release updates and refined diff navigation ([af72889](https://github.com/felipe-godoi/diffv/commit/af7288969e39e553422f49066ca669ca5c7b00c5))
* charm-style TUI, full-file context, scoped search and tmux popup toggle ([8c948d5](https://github.com/felipe-godoi/diffv/commit/8c948d59ba3342a5bf31a9edd62eb6415f32c61c))
* complete implementation of diffv (CLI Diff Viewer) ([85e9a6f](https://github.com/felipe-godoi/diffv/commit/85e9a6f769c750dd762d473fbd771f1914a7d61e))
* exact hunk line tracking, visual staging, and path target disambiguation ([c861c7c](https://github.com/felipe-godoi/diffv/commit/c861c7c0e16ec335a1df9d67db15610c5b12093f))
* fix scrolling bounds, add mouse support, pane resizing, and folder/list modes ([6dd89b9](https://github.com/felipe-godoi/diffv/commit/6dd89b948e9ea67cb93ce4680537da45a047483d))
* folder tree view by default, en/pt language toggle, nerd fonts, commits/stashes tabs, worktrees switcher, and pastel theme ([7a41e01](https://github.com/felipe-godoi/diffv/commit/7a41e018266171df122bd3289b7689cff6982282))
* modernize TUI with rounded cards, file icons, pill badges, and refined palettes ([4a9a23e](https://github.com/felipe-godoi/diffv/commit/4a9a23e3c871af6674fa1b1e50aa6fedf33a3085))
* neovim motions in diff, dedicated persistent statusline, rich visual mode, wrapped commit messages & verbose details popups ([c266800](https://github.com/felipe-godoi/diffv/commit/c266800c56ff0c259d6d0c8c6104fb1e6cb7cbb7))
* open commit PRs, faster CI, and production hardening (v0.3.0) ([f30a341](https://github.com/felipe-godoi/diffv/commit/f30a341f9e690554572590b2c73a7e994159be94))
* responsive half-screen layout, sidebar toggle, and extensive file icons ([a6bdc4e](https://github.com/felipe-godoi/diffv/commit/a6bdc4e2a633a90920d35dd6cbfe7a33000861c0))
* split Staged and Changes sections so stage/unstage always apply ([6e5c1a5](https://github.com/felipe-godoi/diffv/commit/6e5c1a52b8f809e6c4e78381ce77ff88b9a485e9))
* standardize release channels to stable, beta, and nightly ([547018b](https://github.com/felipe-godoi/diffv/commit/547018bb576fd51140a46df760e89f7319d6f235))
* tab cycles drawer tabs, hover overlay, mouse drag resizing, worktree creation, fzf search, and esc return ([7b2a8a2](https://github.com/felipe-godoi/diffv/commit/7b2a8a2daff66b3ebf03c9b7e7b8f4d03a0e738c))


### Bug Fixes

* accurate mouse click row mapping and in-line drawer item overflow overlay ([9456122](https://github.com/felipe-godoi/diffv/commit/9456122083389ba3e00d69b8c7364c678a1a022a))
* check for updates on every launch and stay silent on failure ([9e9cc80](https://github.com/felipe-godoi/diffv/commit/9e9cc80b7bdab753d8d711cc12d19e725f6c48bd))
* preserve navigation levels and improve accent contrast ([16742ac](https://github.com/felipe-godoi/diffv/commit/16742ac7d37b1d7c0a5067b7dbd0caa8acec9256))
* prevent Neovim re-open loop on exit; perf: zero-allocation syntax cache and unified diff rendering, 0.0% idle CPU ([2970c5c](https://github.com/felipe-godoi/diffv/commit/2970c5c23200000a379ea8ac7f51c3ce8df1b867))
* remove dv alias and keep binary strictly as diffv ([633dcb1](https://github.com/felipe-godoi/diffv/commit/633dcb19358d495166da1a8e6d944a408127e754))
* test both terminal theme variants and raise light accent contrast ([b313244](https://github.com/felipe-godoi/diffv/commit/b3132443487332b43fb905031d58bd21aec7b64c))


### Performance Improvements

* cache dark mode detection and syntax highlights; feat: full commit inspection and file browsing ([33f9253](https://github.com/felipe-godoi/diffv/commit/33f9253582ab1d62aa51890e36a676932602b464))
