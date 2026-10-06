# CLI Diff Viewer (`diffview` / `cliview`) — Specification

## 1. Visão Geral e Motivação

### 1.1 O Contexto
Com o avanço e popularização de agentes autônomos de IA no terminal (como Antigravity CLI, Claude Code, Aider, Copilot CLI, etc.), o fluxo de desenvolvimento no terminal (especialmente com **tmux** e **Neovim**) mudou drasticamente:
- Agentes de IA realizam alterações volumosas e simultâneas em múltiplos arquivos em background ou em panes adjacentes.
- O desenvolvedor precisa inspecionar, revisar e validar rapidamente essas modificações sem perder o foco ou quebrar o fluxo do terminal.

### 1.2 O Problema com Ferramentas Atuais
- **`git diff` / `delta`**: Excelentes para visualização sequencial no pager, mas são passivos (não interativos), com navegação de múltiplos arquivos pouco ergonômica e sem capacidade de manter uma visualização viva em tempo real enquanto arquivos são alterados.
- **`lazygit`**: Ótimo para gerenciar commits e branches, mas a sua tela de diff é compacta, sem um modo side-by-side de largura total com visual moderno, e não foi pensada especificamente para live-monitoring de alterações de agentes.
- **`vimdiff` / `nvim -d` / `diffview.nvim`**: O `diffview.nvim` é excelente, mas exige estar dentro do Neovim ou abrir instâncias completas do editor, além de não ser tão desacoplado para ser acionado diretamente pela CLI ou em um popup flutuante do tmux.
- **VS Code Diff Editor**: É a referência atual de legibilidade:
  - Visualização side-by-side com sincronização de scroll perfeita.
  - Highlight preciso a nível de palavra/caractere (intra-line diff) com cores suaves.
  - Painel lateral com lista/árvore de arquivos alterados e status claro (`+X / -Y`).
  - Navegação ágil entre alterações (hunks).

### 1.3 Objetivo
Criar uma ferramenta de terminal (TUI) moderna, veloz e focada exclusivamente em fornecer **a melhor experiência de visualização e revisão de diffs para CLI**, combinando a clareza e elegância do VS Code Diff Editor com a velocidade e ergonomia do ambiente `tmux` + `neovim`.

---

## 2. Casos de Uso Principais

1. **Acompanhamento em Tempo Real de Agentes de IA (`Watch Mode`)**:
   - Rodando em um split do tmux ou monitor secundário, o visualizador detecta gravações no disco e atualiza a visualização do diff instantaneamente (debounced), mantendo a posição de scroll relativa e a seleção de arquivo.
2. **Revisão Rápida via Popup do Tmux**:
   - Acionado via atalho global do tmux (ex: `prefix + d`), abrindo um `tmux display-popup` flutuante com a TUI para inspecionar o que o agente acabou de propor ou alterar, permitindo aceitar/reverter ou abrir no Neovim.
3. **Inspeção de Repositório Git (Working Tree / Staged / Commits)**:
   - Execução direta com `diffv` (mostra working tree + staged + untracked), `diffv --staged` ou `diffv <commit|branch>`.
4. **Comparação Arbitrária de Arquivos ou Pastas**:
   - `diffv arquivo_a.py arquivo_b.py` ou `diffv dir_a/ dir_b/`.
5. **Transição Suave para o Editor**:
   - Pressionar uma tecla (`e` ou `Enter`) para abrir instantaneamente o Neovim exatamente no arquivo e na linha onde o cursor da diff está posicionado.

---

## 3. Arquitetura de Interface (TUI Layout)

A interface é dividida em três componentes principais:

```
┌────────────────────────────────────────────────────────────────────────────────────────────────────────┐
│ [REPO: cli-diffviewer]  branch: main*  |  Files: 3 (+42, -15)  |  Mode: SIDE-BY-SIDE [Tab]  |  ? Help │  <-- Header
├─────────────────────┬──────────────────────────────────────────────────────────────────────────────────┤
│ FILES (3)           │ OLD: src/core/engine.rs             │ NEW: src/core/engine.rs           │█│      │  <-- File Tree
│ ------------------- │ ----------------------------------- │ --------------------------------- │ ├──┬───┤      + Diff Panes
│ M src/core/engine.rs│ 12  fn process_diff() {             │ 12  fn process_diff() {           │ │  │   │      + Overview
│   (+30, -5)         │ 13 -    let old_mode = false;       │ 13 +    let old_mode = true;      │ │░ │   │        Ruler
│ M src/ui/render.rs  │ 14      let timeout = 100;          │ 14      let timeout = 100;        │ │  │   │
│   (+10, -8)         │ 15 -    // legacy logic             │ 15 +    // updated by AI agent    │ │░ │   │
│ A tests/engine_test │ 16                                  │ 16 +    init_streaming();         │ │  │   │
│   (+2, -2)          │                                     │ 17                                │ │  │   │
│                     │                                     │                                   │ │  │   │
├─────────────────────┴──────────────────────────────────────────────────────────────────────────────────┤
│ [j/k] Scroll  [n/p] Next/Prev Hunk  [s] Stage Hunk  [d] Discard Hunk  [e] Edit in Nvim  [w] Watch: ON  │  <-- Status Bar
└────────────────────────────────────────────────────────────────────────────────────────────────────────┘
```

### 3.1 Painel Esquerdo: Lista de Arquivos (File Drawer / Tree)
- **Modos de Exibição**:
  - Lista plana (com caminhos relativos).
  - Árvore colapsável de diretórios.
- **Informações por Arquivo**:
  - Status Git: `M` (Modified), `A` (Added), `D` (Deleted), `R` (Renamed), `?` (Untracked).
  - Contador de mudanças por arquivo: `+X / -Y` com cores verde e vermelha suaves.
  - Indicador visual se o arquivo está com alterações staged (`S`), unstaged (`U`) ou ambas.
- **Filtro Rápido**:
  - Pressionar `/` abre fuzzy search para filtrar arquivos instantaneamente por nome ou extensão.

### 3.2 Painel Central: Visualizador de Diff
- **Modo Side-by-Side (Padrão)**:
  - Coluna esquerda: versão original (Base / HEAD / Left).
  - Coluna direita: versão modificada (Working / Staged / Right).
  - Numeração de linha sincronizada (linha antiga vs linha nova).
  - **Sincronização de Scroll**: As duas colunas rolam juntas verticalmente. Onde houver linhas adicionadas ou removidas, o lado oposto exibe linhas virtuais ("fillers" ou linhas tracejadas sutis) para manter o alinhamento visual idêntico ao VS Code.
- **Modo Unified / Inline (Toggle via `Tab` ou `u`)**:
  - Exibição de diff tradicional unificada em uma coluna só, ideal para telas estreitas (como splits verticais pequenos de terminal).
- **Intra-line Word/Token Highlighting**:
  - Não apenas pinta a linha inteira de verde/vermelho: destaca com contraste reforçado exatamente a palavra, identificador ou caractere alterado dentro da linha.
- **Syntax Highlighting**:
  - Realce de sintaxe baseado em linguagens (via Tree-sitter) aplicado a ambas as versões antes da sobreposição das cores de diff.

### 3.3 Barra Lateral Direita: Diff Overview Ruler (Minimap de Hunks)
- Uma barra estreita de 1 caractere no canto direito da tela que mapeia o arquivo inteiro proporcionalmente:
  - Marcações verdes para adições, vermelhas para remoções, amarelas para alterações mistas.
  - Indicador da posição atual do viewport no arquivo.

### 3.4 Header & Status Bar
- **Header**: Nome do repositório, branch atual, contagem agregada de adições/remoções, status do watcher (Live: ON/OFF).
- **Status Bar**: Atalhos de contexto, status da última operação (ex: "Hunk #2 staged", "Changes reloaded from disk").

---

## 4. Funcionalidades Detalhadas

### 4.1 Live File-Watching (AI Companion Mode)
- **Detecção de Mudanças**: Monitoramento do filesystem (`fsnotify` / `notify-rs`) com debounce (~100-200ms) para evitar flickering enquanto o agente de IA está escrevendo blocos de código.
- **Preservação de Estado**:
  - Ao recarregar o diff de um arquivo modificado, o cursor, o índice do hunk atual e a posição relativa do scroll são preservados.
  - Se um novo arquivo for criado pelo agente, ele aparece automaticamente na lista de arquivos sem interromper o review atual.
- **Notificação Suave**: Breve pulso na barra de status indicando que o arquivo foi atualizado em disco pelo agente.

### 4.2 Interatividade e Ações Rápidas (Staging & Revert)
- **Hunk Actions**:
  - `s`: Faz **stage** do hunk atual selecionado.
  - `u`: Faz **unstage** do hunk atual selecionado.
  - `d` ou `x`: Faz **discard** (reverte) o hunk atual para o estado base (com diálogo rápido de confirmação `y/n`).
- **File Actions**:
  - `S`: Faz stage de todo o arquivo atual.
  - `U`: Faz unstage de todo o arquivo atual.
  - `D`: Descarta todas as alterações do arquivo.
- **Line-level Staging (Modo Visual)**:
  - Seleção de linhas específicas com `v` para stage parcial de hunks.

### 4.3 Integração com Neovim e Terminal
- **Jump to Neovim**:
  - Pressionar `e` abre o arquivo no Neovim exatamente na linha e coluna onde o cursor da diff está.
  - Suporte a `$EDITOR` configurável, com detecção de servidores Neovim ativos (`nvr` / Neovim RPC) para abrir em um buffer no Neovim já aberto no pane adjacente do tmux.
- **Tmux Integration**:
  - Suporte out-of-the-box para execução em `tmux display-popup`:
    ```bash
    bind-key d display-popup -E -w 90% -h 90% "diffv --watch"
    ```
  - Redimensionamento limpo de tela (SIGWINCH) garantindo redesenho sem quebras.
- **Busca Fuzzy (`Ctrl+p` / `Ctrl+f`) com `fzf` opcional**:
  - `Ctrl+p` busca arquivos e `Ctrl+f` busca texto do diff no escopo atual (mudanças, commit ou stash).
  - O [`fzf`](https://github.com/junegunn/fzf) é **opcional**: se estiver no `PATH`, a busca abre nele em tela cheia, com a interface e o matching do fzf no terminal.
  - Sem o `fzf`, o `diffv` usa um **picker interno** (popup ratatui com filtro fuzzy/substring, `↑`/`↓`, `Enter`/`Esc`) com os mesmos candidatos e o mesmo resultado; o app avisa que está usando o picker interno.
- **Copy Hunk / Context for Prompt**:
  - Atalho `c` para copiar o diff do hunk atual ou arquivo formatado em markdown com caminho do arquivo para o clipboard do sistema, facilitando colar no prompt da IA para pedir correções.

---

## 5. Mapeamento de Teclas (Keybindings)

Projetado para ergonomia e memória muscular de usuários de Neovim/Vim:

| Tecla | Contexto | Ação |
|---|---|---|
| `j` / `k` ou `↓` / `↑` | Geral | Rolar linhas para baixo / para cima |
| `J` / `K` ou `Ctrl+d` / `Ctrl+u` | Diff | Rolar meia página |
| `]` ou `n` | Diff | Pular para o **próximo hunk** |
| `[` ou `p` | Diff | Pular para o **hunk anterior** |
| `Tab` | Geral | Alternar foco entre File Tree e Diff View |
| `h` / `l` ou `←` / `→` | Geral | Alternar entre coluna Old e New (ou colapsar/expandir na árvore) |
| `Enter` | File Tree | Selecionar e carregar arquivo no diff viewer |
| `e` | Diff | Abrir arquivo no Neovim na linha atual (`nvim +<line> <file>`) |
| `s` | Diff | Fazer stage do hunk atual |
| `u` | Diff | Fazer unstage do hunk atual |
| `d` | Diff | Descartar / reverter o hunk atual |
| `v` | Diff | Entrar no modo de seleção visual de linhas para staging parcial |
| `S` / `U` | Geral | Stage / Unstage de todas as alterações do arquivo |
| `m` | Geral | Alternar modo de visualização (**Side-by-Side** vs **Unified**) |
| `w` | Geral | Ativar/Desativar modo Live Watch |
| `/` | File Tree | Filtrar arquivos (Fuzzy Finder) |
| `c` | Diff | Copiar hunk atual para o clipboard (formato patch/markdown) |
| `?` | Geral | Exibir modal de ajuda com atalhos |
| `q` / `Esc` | Geral | Sair da ferramenta |

---

## 6. Comandos e Invocação CLI

```bash
# Uso padrão no repositório atual (compara Working Tree contra HEAD, staged e unstaged)
diffv

# Modo sentinela otimizado para acompanhar agentes de IA (watch contínuo com live reload)
diffv -w, --watch

# Inspecionar apenas alterações que já estão staged
diffv --staged, --cached

# Comparar branch ou commit específico
diffv HEAD~1
diffv main..feature-branch

# Comparar dois arquivos locais arbitrários (fora do git)
diffv original.rs modificado.rs

# Comparar dois diretórios arbitrários
diffv ./v1/ ./v2/

# Ler diff a partir de stdin (pipe)
git diff | diffv -
```

### Configuração via Arquivo (`~/.config/diffv/config.toml`)
```toml
[ui]
theme = "tokyonight"           # tokyonight, catppuccin, gruvbox, nord, vscode-dark
default_view = "side-by-side"  # side-by-side ou unified
show_line_numbers = true
syntax_highlighting = true
overview_ruler = true
tab_width = 4

[diff]
algorithm = "patience"         # myers, patience, histogram
ignore_whitespace = false
context_lines = 3

[watcher]
enabled = true
debounce_ms = 150
watch_untracked = true

[editor]
command = "nvim"
args = ["+{{line}}", "{{file}}"]
use_nvr = true                 # Usa nvr (neovim-remote) se disponível no tmux
```

---

## 7. Arquitetura Técnica Sugerida

### 7.1 Stack Tecnológica Adotada: **Rust**
Conforme detalhado no [docs/architecture.md](architecture.md), a ferramenta é implementada em **Rust** utilizando:
- **`ratatui` + `crossterm`**: Renderização em TUI de altíssima performance, double buffering por célula e zero flickering em side-by-side.
- **`similar`**: Motor de Myers/Patience diffing e Levenshtein token diffing para realce intra-linha.
- **`notify` + `notify-debouncer-mini`**: File watcher em background thread com debounce para acompanhamento de agentes de IA.
- **`syntect`**: Realce de sintaxe nativo para centenas de linguagens.
- **`clap` (derive)**: CLI parser completo e tipado.

### 7.2 Módulos da Aplicação
```
cli-diffviewer/
├── Cargo.toml
├── src/
│   ├── main.rs                 # CLI entry point e parsing de argumentos (clap)
│   ├── config.rs               # Carregamento e merge de configurações
│   ├── core/
│   │   ├── diff/               # Algoritmo de diff (Myers/Patience, intra-line word diff)
│   │   ├── git.rs              # Integração Git (status, hunks, stage, discard)
│   │   ├── watcher.rs          # Eventos de fs e debounced reload
│   │   └── syntax.rs           # Highlight de código com tree-sitter / temas
│   ├── ui/
│   │   ├── app.rs              # Estado principal da aplicação (App State Machine)
│   │   ├── layout.rs           # Divisão de tela responsiva (Tree, Diff, Status)
│   │   ├── components/
│   │   │   ├── file_tree.rs    # Renderização da lista/árvore de arquivos
│   │   │   ├── side_by_side.rs # Renderização dual-column com sync de scroll
│   │   │   ├── unified.rs      # Renderização unified diff
│   │   │   ├── ruler.rs        # Minimap de hunks
│   │   │   └── status_bar.rs   # Teclas e mensagens informativas
│   │   └── theme.rs            # Esquemas de cores (Dark+, TokyoNight, Catppuccin)
│   └── integration/
│       ├── editor.rs           # Invocação do Neovim / Neovim Remote
│       └── tmux.rs             # Utilitários de detecção e popup de tmux
```

---

## 8. Roteiro de Implementação (Roadmap)

### Fase 1: MVP (Core Engine & Dual Diff Viewer)
- Estruturação do projeto em Rust com `ratatui` e `crossterm`.
- Leitura de diffs do Git local (working tree vs HEAD).
- Visualizador Side-by-Side com rolagem sincronizada e numeração dupla de linhas.
- Intra-line word diffing (destaque de caracteres alterados dentro da linha).
- Lista de arquivos no painel esquerdo com navegação por teclado (`j/k`, `Tab`).

### Fase 2: Watcher & Ergonomia de IA
- Implementação do `notify` watcher com debouncer para recarregar diffs automaticamente quando arquivos forem alterados no disco.
- Navegação entre hunks (`]`, `[` / `n`, `p`).
- Minimap / Overview Ruler no canto direito.
- Integração de abertura direta no Neovim (`e` na linha atual).

### Fase 3: Interatividade Git & Staging
- Ações interativas de stage/unstage por hunk (`s`/`u`).
- Descarte de hunks (`d`) com confirmação.
- Suporte a unified/inline view toggle (`Tab`/`m`).
- Suporte a syntax highlighting com `syntect` ou `tree-sitter`.

### Fase 4: Integrações Avançadas & Tmux
- Suporte completo a pipes de stdin (`git diff | diffv`).
- Configuração TOML e temas visuais (VS Code Dark+, Tokyo Night, Catppuccin).
- Documentação e scripts de atalho para integração como popup do tmux (`tmux display-popup`).
