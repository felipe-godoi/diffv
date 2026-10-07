# Changelog

## [0.6.0](https://github.com/felipe-godoi/diffv/compare/v0.5.0...v0.6.0) (2026-10-07)


### Features

* **git:** stage_path / unstage_path for a file or a whole directory ([56cdc21](https://github.com/felipe-godoi/diffv/commit/56cdc21bbeda631d458932ef5e0697d0b2e4ce52))
* **install:** offer fzf as an optional, unchecked-by-default extra ([549a445](https://github.com/felipe-godoi/diffv/commit/549a445649fc8bec9c89f13123f41419452614d7))
* melhorias anotadas — mouse, scroll, loading, fzf, editor e tagline ([6c2bb00](https://github.com/felipe-godoi/diffv/commit/6c2bb00a7f40c0ee4e40943c83c9e6c6291e806d))
* **search:** built-in picker fallback when fzf is not installed ([400f5f0](https://github.com/felipe-godoi/diffv/commit/400f5f012f20962584cb55590ad4de9200f9079f))
* **search:** Ctrl+G dentro do filtro, preview no picker interno e prompt do fzf no instalador ([3d0219e](https://github.com/felipe-godoi/diffv/commit/3d0219e27eb213440152b78772a47e47810fdcf9))
* **search:** Ctrl+G switches between fzf and the built-in picker ([e739727](https://github.com/felipe-godoi/diffv/commit/e7397270160dd15987e57da4a047176a65ba3408))
* **search:** preview pane in the built-in picker ([35b2963](https://github.com/felipe-godoi/diffv/commit/35b2963049ab3636f5e0fb499f4b2348f356f836))
* **search:** switch engine with Ctrl+G from inside the filter, carrying the query ([e06e74d](https://github.com/felipe-godoi/diffv/commit/e06e74dc244fedb159ea2514126e7be3188883bb))
* **ui:** mouse wheel scrolls the view and drags the cursor only at the edges ([d150ebc](https://github.com/felipe-godoi/diffv/commit/d150ebc8e63cfff7ea1015e0895d1f31790b8f3d))
* **ui:** popup avisando o resultado da atualização na inicialização ([2ff0797](https://github.com/felipe-godoi/diffv/commit/2ff0797234821fbb326e9dcfb8b475bed23ab8ef))
* **ui:** s / u in the Changes list stage the selected file, folder or section ([4a894af](https://github.com/felipe-godoi/diffv/commit/4a894af2ebaf5efcf81e6fdebcea6e39ad412b8b))
* **ui:** s/u no painel de Changes agem no arquivo, pasta ou seção inteiros ([724d55f](https://github.com/felipe-godoi/diffv/commit/724d55fde73ee42f107d9ccff514373de867014e))
* **ui:** select and copy diff text with the mouse ([7016f39](https://github.com/felipe-godoi/diffv/commit/7016f39415344e7eb2fd307aa606590609c22ee2))


### Bug Fixes

* **ci:** tag e release rolantes (nightly/beta) voltam a acompanhar o commit do push ([8732b2a](https://github.com/felipe-godoi/diffv/commit/8732b2a00ac6dbf93971c6d711f6f1e9d4b63315))
* **editor:** resolve $GIT_EDITOR/$VISUAL/$EDITOR, then the system default, then vi/nano ([8c3c846](https://github.com/felipe-godoi/diffv/commit/8c3c8469810431389a21323d121f679f057a916e))
* **install:** ask about the optional fzf even when the script is piped ([c08efbd](https://github.com/felipe-godoi/diffv/commit/c08efbd7784f4eeaa78dd593cfd3dc7826d4ce3c))
* preserve Changes file count in narrow panes ([abfc80f](https://github.com/felipe-godoi/diffv/commit/abfc80f37deec4d94f88b5bbda033155ac3a8662))
* **search:** make the built-in picker exit keys match fzf ([398cdae](https://github.com/felipe-godoi/diffv/commit/398cdaee71dd4a1c33a275bad62fd95626faebe6))
* **ui:** cabeçalho de Changes quebra linha antes de perder texto ([b97105d](https://github.com/felipe-godoi/diffv/commit/b97105db2e0855ef15e6c8c03d36d3e72031d86e))
* **ui:** contagem de arquivos do painel de Changes não é mais cortada (leva o [#11](https://github.com/felipe-godoi/diffv/issues/11) para a main) ([69d60e1](https://github.com/felipe-godoi/diffv/commit/69d60e1046b7a2e6d104edb1eb61c686ec2a5234))
* **ui:** wrap the Changes header instead of dropping its text ([734ef57](https://github.com/felipe-godoi/diffv/commit/734ef574c127832201f67af417534fa7107ba5e7))
* **update:** libera o lock da atualização explicitamente (destrava a CI e a release) ([7a4e6a8](https://github.com/felipe-godoi/diffv/commit/7a4e6a81aa1a25c15e3b7ae4e0d42a46b18eb74d))
* **update:** show the startup update result in a popup ([62820ca](https://github.com/felipe-godoi/diffv/commit/62820cacd3a671edbe97e5e9f3a2f990762e94eb))


### Performance Improvements

* **startup:** keep the auto-update check off the first frame ([579d5b9](https://github.com/felipe-godoi/diffv/commit/579d5b9a6580495005b005c940f04643c9a687bb))
* **startup:** show a loading indicator while the initial diff loads ([3621471](https://github.com/felipe-godoi/diffv/commit/3621471a33f4abe0216ede2a59e6f34024616d7a))

## [0.5.0](https://github.com/felipe-godoi/diffv/compare/v0.4.0...v0.5.0) (2026-10-02)


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
