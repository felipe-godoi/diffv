# Arquitetura Técnica — CLI Diff Viewer (`diffv`)

## 1. Visão Geral da Stack

A ferramenta foi definida para ser desenvolvida em **Rust**, visando:
- **Startup time instantâneo (< 8ms)**: ideal para acionamento frequente via popup flutuante do tmux (`tmux display-popup`).
- **Renderização por Célula (Double Buffering)**: performance máxima e zero flickering através do `ratatui` + `crossterm`.
- **Baixo consumo de recursos em background**: footprint de memória ~10-15MB e 0% de CPU no modo contínuo de observação (`--watch`).
- **Diffing de precisão**: utilização do algoritmo Myers/Patience com intra-line char diffing de alta performance via crate `similar`.

---

## 2. Dependências e Crates Selecionadas

| Crate | Versão Recomendada | Finalidade | Justificativa |
|---|---|---|---|
| **`ratatui`** | `^0.29` | Framework de Terminal UI (TUI) | Padrão da indústria em Rust; suporte a layouts avançados, widgets customizados e double buffering. |
| **`crossterm`** | `^0.28` | Backend de terminal e eventos | Manipulação de terminal multiplataforma, modo raw, suporte a TrueColor (24-bit) e Kitty keyboard protocol. |
| **`similar`** | `^2.6` | Algoritmos de Diffing | Implementação pura Rust de Myers, Patience e Levenshtein diffing em linhas, palavras e caracteres. |
| **`notify`** + **`notify-debouncer-mini`** | `^6.1` / `^0.4` | File System Watcher | Monitoramento assíncrono de modificações em disco com janela de debounce (~150ms). |
| **`syntect`** | `^5.2` | Syntax Highlighting | Realce de código com temas Sublime/VS Code integrados e suporte a centenas de linguagens. |
| **`clap`** | `^4.5` (derive) | CLI Argument Parsing | Parsing de flags, subcomandos e argumentos com geração automática de help e validação de tipos. |
| **`git2`** ou **`std::process::Command`** (git CLI) | `^0.19` ou CLI wrapper | Provedor de dados Git | Leitura de repositório, branches, working tree status, hunks e staging. |
| **`arboard`** | `^3.4` | Integração com Clipboard | Cópia de hunks/patches diretamente para a área de transferência do SO (macOS/Linux). |
| **`serde`** + **`toml`** | `^1.0` / `^0.8` | Configuração persistente | Leitura e validação do arquivo `~/.config/diffv/config.toml`. |
| **`rayon`** | `^1` | Paralelismo de dados | Alinhamento side-by-side, intraline, untracked e comparação de diretórios em todos os núcleos. |
| **`frizbee`** | `^0.13` | Fuzzy matching | Motor SIMD (Smith-Waterman) usado pelo fff; filtra o picker interno em paralelo, dentro do processo. |

---

## 3. Diagrama Geral de Arquitetura

```mermaid
graph TD
    subgraph CLI Entrypoint
        A[main.rs / clap] --> B[AppConfig]
        A --> C[App Mode: Git / Watch / FilePair / Stdin]
    end

    subgraph Data Layer
        C --> D[GitProvider]
        C --> E[FilePairProvider]
        C --> F[StdinProvider]
        D --> G[DiffEngine]
        E --> G
        F --> G
    end

    subgraph Core Diff Engine
        G --> H[Line-by-line Diff: Myers/Patience]
        H --> I[Side-by-Side Aligner + Fillers]
        I --> J[Intra-line Word/Char Diff: similar]
        J --> K[Syntax Highlighter: syntect]
    end

    subgraph Background Services
        L[FS Watcher: notify] -.->|Eventos Debounced| M[Event Loop / Channel]
        N[Crossterm KeyEvents] --> M
    end

    subgraph UI & Presentation Layer
        M --> O[App State Machine]
        O --> P[Ratatui Renderer]
        P --> Q[File Drawer Component]
        P --> R[Side-by-Side Diff Component]
        P --> S[Unified Diff Component]
        P --> T[Overview Ruler Component]
        P --> U[Status Bar & Popups]
    end

    subgraph Integrations
        O -->|'e' / Enter| V[$EDITOR Launcher - editor do sistema / vi / nano]
        O -->|'c'| W[System Clipboard]
        O -->|'s' / 'd'| X[Git Stage / Discard Worker]
    end
```

---

## 4. Pipeline de Processamento de Diff

A experiência idêntica ao VS Code depende de quatro passos sequenciais no processamento dos arquivos:

### 4.1 Passo 1: Myers / Patience Diff em Linhas
1. Compara a versão **Old** (HEAD ou arquivo original) contra a versão **New** (Working directory ou arquivo modificado).
2. Agrupa alterações em **Hunks**, contendo contexto e números de linha de origem e destino (`old_start`, `old_count`, `new_start`, `new_count`).

### 4.2 Passo 2: Alinhamento Dual-Column (Side-by-Side Aligner)
Para que as colunas da esquerda e da direita fiquem alinhadas horizontalmente:
- Se $N$ linhas foram adicionadas na direita sem remoção correspondente na esquerda, são inseridas $N$ **linhas virtuais (fillers/padding)** na coluna da esquerda.
- Se $M$ linhas foram removidas da esquerda sem adição correspondente na direita, são inseridas $M$ **linhas virtuais** na coluna da direita.
- Se linhas foram alteradas (substituições consecutivas de remoção e adição), elas são emparelhadas linha a linha até o limite do bloco.

### 4.3 Passo 3: Intra-line Character / Word Highlighting
Quando um par de linhas é identificado como uma modificação:
- A engine executa um diff fino (token-level ou char-level) utilizando a crate `similar::capture_diff_slices`.
- A linha recebe uma cor de fundo sutil (ex: vermelho escuro para deleção, verde escuro para adição).
- Os trechos específicos de palavras ou caracteres alterados recebem um realce mais forte com alto contraste (fundo mais claro ou texto brilhante), reproduzindo exatamente o comportamento do VS Code.

### 4.4 Passo 4: Syntax Highlighting com Fallback Gracioso
- As linhas de código recebem estilização sintática prévia via `syntect`.
- As cores de diff (adicionado, removido, intra-line) são mescladas preservando a legibilidade da sintaxe.

---

## 5. Estrutura de Módulos do Código

```
cli-diffviewer/
├── Cargo.toml
├── docs/
│   ├── spec.md                 # Especificação funcional completa
│   ├── roadmap.md              # Checklists de entrega
│   └── architecture.md         # Arquitetura técnica (este documento)
└── src/
    ├── main.rs                 # Inicialização da CLI, crossterm raw mode e panic hook
    ├── cli.rs                  # Estruturas do Clap (argumentos e flags)
    ├── config.rs               # Modelo de configuração TOML e defaults
    │
    ├── core/                   # Regras de negócio e processamento de diff
    │   ├── mod.rs
    │   ├── engine.rs           # Coordenação do pipeline de diff
    │   ├── aligner.rs          # Algoritmo de alinhamento side-by-side e fillers
    │   ├── intraline.rs        # Algoritmo de destaque intra-linha (similar)
    │   ├── models.rs           # Structs: FileDiff, Hunk, AlignedRow, DiffKind
    │   └── syntax.rs           # Wrapper syntect e cache de temas
    │
    ├── git/                    # Integração com o repositório
    │   ├── mod.rs
    │   ├── provider.rs         # Descoberta de arquivos modificados e status
    │   ├── patch.rs            # Extração de hunks e estatísticas (+X, -Y)
    │   └── actions.rs          # Execução de stage, unstage e discard de hunks
    │
    ├── watcher/                # Monitoramento de arquivos (AI Companion)
    │   ├── mod.rs
    │   └── service.rs          # Debounced notify watcher rodando em background thread
    │
    ├── ui/                     # Apresentação e Terminal UI (Ratatui)
    │   ├── mod.rs
    │   ├── app.rs              # App State Machine, navegação e foco
    │   ├── theme.rs            # Definições de estilos e cores (Tokyo Night, VS Code Dark+)
    │   └── components/
    │       ├── header.rs       # Cabeçalho com branch, totais e status
    │       ├── file_tree.rs    # Drawer lateral com lista de arquivos
    │       ├── side_by_side.rs # Renderizador das duas colunas de diff
    │       ├── unified.rs      # Renderizador unified/inline diff
    │       ├── ruler.rs        # Minimap vertical de hunks
    │       ├── status_bar.rs   # Barra de atalhos e alertas
    │       └── help_popup.rs   # Modal de ajuda de teclas (?)
    │
    └── integration/            # Comunicação externa
        ├── mod.rs
        ├── editor.rs           # Invocação do $EDITOR (editor do sistema / vi / nano; nvr opcional)
        ├── clipboard.rs        # Cópia de hunks/patches para o clipboard
        └── tmux.rs             # Utilitários de detecção e redimensionamento do Tmux
```

---

## 6. Ciclo de Vida da Aplicação e Loop de Eventos

A aplicação adota um modelo clássico de **Event Loop Reativo**:

```
                  ┌──────────────────────┐
                  │ Terminal Events      │ (Key, Resize, Mouse)
                  └──────────┬───────────┘
                             │
                             ▼
┌──────────────────┐    ┌─────────┐     ┌──────────────────────┐
│ File Watcher     │--->│ mpsc    │---->│ App::update(event)   │
│ Background Thread│    │ Channel │     └──────────┬───────────┘
└──────────────────┘    └─────────┘                │
                                                   ▼
                                        ┌──────────────────────┐
                                        │ Ratatui::draw(&app)  │
                                        └──────────────────────┘
```

1. **Canal Central MPSC**:
   - Um canal `tokio::sync::mpsc` ou `std::sync::mpsc` unifica eventos do usuário (teclado, resize) e eventos de background (arquivos gravados pelo agente de IA no disco).
2. **Panic Hook Seguro**:
   - Uma closure no `std::panic::set_hook` garante que o modo raw do terminal seja desabilitado e a tela do terminal seja restaurada caso ocorra algum panic inesperado, evitando deixar o terminal em estado corrompido.
3. **Estado Preservado no Reload**:
   - Quando o watcher dispara um evento de reload, o `App` recalcula o diff do arquivo atual mantendo o índice do hunk focado e a porcentagem relativa de scroll.

### 6.1 Performance em repositórios grandes e arquivos pesados

- **Carga paralela**: `GitProvider::load_diffs` roda `git diff --cached`, `git diff`, `git status` e o branch ao mesmo tempo (`std::thread::scope`); untracked são lidos em paralelo (NUL nos primeiros 8 KB = binário) e o alinhamento/intraline usa `rayon` por arquivo e por linha. Worktrees, commits e stashes da inicialização carregam ao lado dos diffs.
- **Reload do watcher fora da UI**: `App::background_reload_job` gera um `ReloadJob` que roda numa thread; o resultado (`ReloadDone`) volta pelo canal e só é aplicado se nenhum reload síncrono (stage, troca de branch, `x`) aconteceu depois (`reload_generation`). No máximo um reload em voo e um na fila.
- **Loop de eventos**: eventos enfileirados são processados antes do próximo frame (até 256), movimento de mouse sem botão não redesenha, e os drains do editor/fzf descartam só entrada, nunca resultados de reload.
- **Watcher**: em macOS/Windows um único watch recursivo na raiz (o notify reinicia o stream do FSEvents a cada `watch`, o que tornava o registro por diretório quadrático); diretórios sempre podados (`node_modules`, `target`, …) são filtrados antes do `git check-ignore`.
- **Arquivo inteiro (`x`)**: só o arquivo visto é carregado com `-U1000000` (`ensure_full_context_file`, marcado em `FileDiff::full_file`); ligar/desligar troca apenas esses arquivos, sem recarregar a árvore.
- **Custo por frame limitado à tela**: linhas acima de 1000 bytes não passam pelo syntect, linhas acima de 2000 bytes não têm intraline (o Patience em uma linha minificada de megabytes travava a carga), scroll horizontal/wrap/mapa de seleção processam só as colunas e linhas visíveis (`scroll_line`, `wrap_line`, `layout_line_clipped`), e `wrapped_row_heights` pula direto para a primeira linha visível. Tokens de sintaxe ficam em cache por linguagem (`Arc<[SyntaxToken]>`, sem cópia), e o syntect é pré-carregado em background durante a carga inicial.
- **Stage de muitos caminhos**: `stage_paths` / `unstage_paths` dividem listas enormes em várias chamadas do git, abaixo do `ARG_MAX`.

---

## 7. Integração Tmux e Editor

### 7.1 Tmux Floating Popup
A ferramenta é compilada para um binário autônomo `diffv`. Para integrá-la ao tmux como um popup instantâneo:
```tmux
# ~/.tmux.conf
# Abre o diffviewer em um popup centralizado de 90% da tela em modo watch
bind-key d display-popup -d "#{pane_current_path}" -w 92% -h 90% -E "diffv --watch"
```

### 7.2 Abrir no Editor (`$EDITOR`, nvr opcional)
Ao pressionar `e` em uma linha da diff:
1. Opcional: se `use_nvr = true`, o `diffv` estiver rodando dentro do Neovim (`$NVIM` definido) e o `nvr` (neovim-remote) estiver instalado:
2. Se sim, instrui o Neovim existente a abrir o arquivo na linha correta sem fechar a sessão.
3. Caso contrário (caminho padrão), suspende temporariamente a TUI do `diffv` e invoca `$EDITOR +<line> <file>`. Ordem de resolução quando o config não define `command`: `$GIT_EDITOR` > `$VISUAL` > `$EDITOR` > editor padrão do sistema (`editor` no `PATH`, a alternativa do Debian/Ubuntu, ou `/usr/bin/editor`) > `vi` > `nano`. Ao fechar o editor, o `diffv` restaura a visualização instantaneamente.

### 7.3 Busca com `fzf` (opcional) e Picker Interno
`Ctrl+p` / `Ctrl+f` geram a lista de candidatos (`App::prepare_fzf`) e o resultado escolhido volta por `handle_fzf_file_result` / `handle_fzf_text_result`:
1. Por padrão (`auto` / `builtin`) abre o picker interno (`ui/components/picker.rs`): o filtro usa o `frizbee` (Smith-Waterman SIMD, o motor de matching do fff) via `Matcher::match_list_parallel`, com smart-case, sintaxe estendida do fzf e desempate por tamanho; em `Ctrl+f` só o conteúdo da linha é comparado, como o `--nth=2` do fzf. Enquanto a query só cresce, `PickerState::refilter` refiltra apenas os matches anteriores. Os candidatos do escopo carregado não são copiados (`FzfQuery::files = None` aponta para `App::files`).
2. Com `fzf` como motor e no `PATH` (`is_fzf_available`, verificado uma vez por sessão), a TUI é suspensa e o `fzf` roda em tela cheia com os candidatos (escritos com buffer no stdin). O item escolhido entra pelos mesmos handlers, então o comportamento após a escolha é idêntico.
3. O motor é configurável em `[search] engine` (`auto` | `fzf` | `builtin`, padrão `auto`) e alternado com `Ctrl+G` fora do picker (`auto` → `fzf` → `builtin`, pulando `fzf` quando não instalado; a escolha é salva no `config.toml`). Com `fzf` escolhido e ausente, o app avisa e usa o picker interno.
