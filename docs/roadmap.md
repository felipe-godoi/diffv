# Roadmap de Implementação — CLI Diff Viewer (`diffv`)

Este roadmap decompõe as especificações do [docs/spec.md](file:///Users/felipegodoi/Documents/repos/cli-diffviewer/docs/spec.md) em marcos e tarefas atômicas acompanháveis via checkboxes.

---

## Fase 1: Fundação do Projeto & Integração Git

- [x] **1.1 Setup Inicial do Repositório**
  - [x] Instalar / verificar toolchain do Rust (`rustup`, `cargo`).
  - [x] Inicializar o projeto com `cargo init --bin`.
  - [x] Configurar `Cargo.toml` com as dependências do `docs/architecture.md`.
  - [x] Configurar `.gitignore` e estrutura inicial de diretórios (`src/`, `docs/`).
  - [x] Definir CLI parser com `clap` para suporte a flags (`--watch`, `--staged`, `--unified`, caminhos de arquivos/diretórios).

- [x] **1.2 Git Provider & Leitura de Mudanças**
  - [x] Detectar raiz do repositório Git atual e branch ativa.
  - [x] Extrair lista de arquivos modificados, adicionados, deletados e untracked.
  - [x] Calcular estatísticas de adições e remoções por arquivo (`+X / -Y`) e totais do repositório.
  - [x] Suporte a leitura de `working tree vs HEAD`, `--staged` e commits específicos (`git diff <ref>`).

- [x] **1.3 Parser de Diff & Estrutura de Dados**
  - [x] Estruturar modelos de dados: `FileDiff`, `Hunk`, `DiffLine` (tipo: adição, remoção, contexto, linha virtual).
  - [x] Parser robusto de unified diff com numeração de linha de origem e destino.

---

## Fase 2: Motor de Diffing & Intra-line Highlighting

- [x] **2.1 Alinhamento Side-by-Side (Dual-Column Alignment)**
  - [x] Algoritmo de emparelhamento de linhas antigas (Old) e novas (New).
  - [x] Inserção de linhas virtuais/fillers ("empty padding lines") para manter alinhamento vertical exato entre as duas colunas.
  - [x] Preservação de números de linha independentes para ambos os lados.

- [x] **2.2 Intra-line Character/Word Highlighting**
  - [x] Algoritmo de diffing fino (Levenshtein ou LCS por tokens/palavras) para pares de linhas substituídas.
  - [x] Geração de spans com destaque de contraste elevado nas palavras/caracteres modificados.

- [x] **2.3 Syntax Highlighting**
  - [x] Identificação da linguagem por extensão ou cabeçalho de arquivo.
  - [x] Aplicação de highlight de sintaxe mantendo a compatibilidade com o destaque de diff.

---

## Fase 3: Interface de Terminal (TUI) & Layout VS Code

- [x] **3.1 Layout Responsivo Principal**
  - [x] Header superior: repositório, branch, estatísticas agregadas (`+N, -M`), modo ativo e ajuda rápida.
  - [x] Painel lateral esquerdo (File Drawer): lista de arquivos com status (`M`, `A`, `D`, `?`) e contadores.
  - [x] Painel central: visualizador de diff side-by-side com divisória vertical e scroll sincronizado.
  - [x] Barra inferior (Status Bar): atalhos contextuais e notificações de ações do sistema.

- [x] **3.2 Navegação e Ergonomia de Teclado**
  - [x] Navegação vertical suave com `j`/`k` ou setas (com suporte a `Ctrl+d`/`Ctrl+u` para meia página).
  - [x] Alternância de foco entre File Tree e Diff View via `Tab`.
  - [x] Navegação rápida entre hunks com `]` / `[` ou `n` / `p`.
  - [x] Modal de ajuda rápida com tabela de atalhos ao pressionar `?`.

- [x] **3.3 Modos de Visualização Alternativos**
  - [x] Toggle para modo **Unified / Inline** (`m` ou `u`) para terminais estreitos.
  - [x] Diff Overview Ruler (minimap vertical de 1 coluna à direita indicando onde estão as mudanças no arquivo).

---

## Fase 4: Live Watcher (Modo Companheiro de Agentes de IA)

- [x] **4.1 Monitoramento de Filesystem**
  - [x] Integração com `fsnotify` para escutar eventos de escrita, criação e remoção no repositório.
  - [x] Mecanismo de debounce configurável (100–200ms) para evitar flickering durante escritas de blocos de IA.

- [x] **4.2 Preservação e Sincronização de Estado**
  - [x] Recarregamento silencioso do diff do arquivo em foco sem resetar a posição de scroll.
  - [x] Atualização automática da lista de arquivos caso o agente crie novos arquivos.
  - [x] Indicador discreto na barra de status informando atualizações em disco (ex: `⚡ [disk updated]`).
  - [x] Toggle para ativar/desativar o modo watch dinamicamente via atalho (`w`).

---

## Fase 5: Interatividade, Staging & Ações de Review

- [x] **5.1 Ações Granulares por Hunk**
  - [x] Fazer stage do hunk selecionado (`s`).
  - [x] Fazer unstage do hunk selecionado (`u`).
  - [x] Descartar/reverter hunk (`d`) com modal de confirmação rápida (`y/n`).

- [x] **5.2 Ações por Arquivo**
  - [x] Stage do arquivo completo (`S`).
  - [x] Unstage do arquivo completo (`U`).
  - [x] Descartar todas as alterações do arquivo (`D`) com confirmação.

- [x] **5.3 Cópia de Contexto para Prompt de IA**
  - [x] Atalho `c` para copiar o hunk atual ou arquivo formatado em Markdown com caminho e linhas para o clipboard do sistema.

- [x] **5.4 Filtro de Arquivos**
  - [x] Abertura de busca fuzzy com `/` no File Tree para filtrar rapidamente arquivos por caminho ou extensão.

---

## Fase 6: Integrações de Terminal (Neovim & Tmux)

- [x] **6.1 Integração Neovim**
  - [x] Atalho `e` ou `Enter` no diff para abrir o editor na linha e arquivo exatos (`$EDITOR +<line> <file>`).
  - [x] Suporte a Neovim Remote (`nvr`) caso o Neovim já esteja ativo em outro pane do tmux.

- [x] **6.2 Integração Tmux**
  - [x] Garantir compatibilidade nativa com `tmux display-popup` (tratamento de dimensões e `SIGWINCH`).
  - [x] Documentar e fornecer script/snippet de configuração para atalhos no `.tmux.conf`:
    ```tmux
    bind-key d display-popup -E -w 90% -h 90% "diffv --watch"
    ```

---

## Fase 7: Modos de Comparação Extras & Polimento

- [x] **7.1 Modos de Comparação Não-Git**
  - [x] Comparação direta entre dois arquivos arbitrários (`diffv arquivo_a arquivo_b`).
  - [x] Comparação direta entre dois diretórios arbitrários (`diffv dir_a dir_b`).
  - [x] Leitura de patch vindo de `stdin` (`git diff | diffv -`).

- [x] **7.2 Temas e Customização Visual**
  - [x] Suporte a arquivo de configuração TOML (`~/.config/diffv/config.toml`).
  - [x] Paletas de cores embutidas: VS Code Dark+, Tokyo Night, Catppuccin, Gruvbox.
  - [x] Suporte a cores TrueColor (24-bit) e fallback para 256 cores.

- [x] **7.3 Qualidade, Testes & Release**
  - [x] Testes unitários para alinhamento side-by-side, intra-line diffing e parsing de patches.
  - [x] Compilação de binário estático único e instrução de instalação no `README.md`.

---

## Fase 8: Histórico de Commits por Arquivo (File Commit History)

- [x] **8.1 Visualização de Histórico de Arquivos**
  - [x] Extração de log de commits para o arquivo ativo via `git log --follow` com hash curto, data relativa, autor e mensagem.
  - [x] Modal interativo de histórico (`H` ou flag CLI `-H` / `--history`) com navegação vertical por setas/`j`/`k`.
  - [x] Pré-visualização do diff exato do commit selecionado ao pressionar `Enter` na lista de commits.
  - [x] Retorno imediato ao diff do working tree ao pressionar `Esc` ou fechar a visualização.

