#![allow(dead_code)]

use std::io::{self, stdout, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::mpsc;
use std::thread;
use std::time::Duration;

use anyhow::Result;
use clap::Parser;
use crossterm::event::{self, Event, KeyEventKind};
use crossterm::execute;
use crossterm::terminal::{
    disable_raw_mode, enable_raw_mode, EnterAlternateScreen, LeaveAlternateScreen,
};
use ratatui::backend::CrosstermBackend;
use ratatui::Terminal;

use diffv::cli::Cli;
use diffv::config::Config;
use diffv::git::provider::GitProvider;
use diffv::integration::editor::open_editor;
use diffv::ui::app::{App, AppMode};
use diffv::watcher::service::{WatchEvent, WatchService};

enum AppEvent {
    Input(Event),
    Reload,
    Tick,
}

fn main() -> Result<()> {
    let args = Cli::parse();
    let config = Config::load();

    // Setup custom panic hook to restore terminal
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen);
        default_panic(panic_info);
    }));

    // Determine target working directory
    let cwd = args.cwd.as_deref().unwrap_or(Path::new("."));

    // Determine App Mode
    let mode = determine_app_mode(&args, cwd)?;

    // Start TUI
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_result = run_app(&mut terminal, mode, config, &args, cwd);

    // Teardown TUI
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
    terminal.show_cursor()?;

    if let Err(err) = app_result {
        eprintln!("Error: {:#}", err);
        std::process::exit(1);
    }

    Ok(())
}

fn determine_app_mode(args: &Cli, cwd: &Path) -> Result<AppMode> {
    // Check if stdin is piped or requested as "-"
    if (!io::stdin().is_terminal() && args.targets.is_empty())
        || (args.targets.len() == 1 && args.targets[0] == "-")
    {
        return Ok(AppMode::Stdin);
    }

    if args.targets.len() == 2 {
        let p1 = PathBuf::from(&args.targets[0]);
        let p2 = PathBuf::from(&args.targets[1]);

        if p1.is_dir() && p2.is_dir() {
            return Ok(AppMode::DirPair(p1, p2));
        }
        if p1.is_file() || p2.is_file() {
            return Ok(AppMode::FilePair(p1, p2));
        }
        // If targets are not files or dirs on disk, treat as git revision range (e.g. diffv main feat)
        if let Ok(git_provider) = GitProvider::discover(Some(cwd)) {
            let target_ref = format!("{}..{}", args.targets[0], args.targets[1]);
            return Ok(AppMode::Git {
                target_ref: Some(target_ref),
                git_provider,
            });
        }
        return Ok(AppMode::FilePair(p1, p2));
    }

    let target_ref = if args.targets.len() == 1 {
        Some(args.targets[0].clone())
    } else {
        None
    };

    match GitProvider::discover(Some(cwd)) {
        Ok(git_provider) => Ok(AppMode::Git {
            target_ref,
            git_provider,
        }),
        Err(_) => {
            anyhow::bail!(
                "Not a git repository (or any parent up to mount point).\n\
                 Usage:\n  diffv [git-ref]\n  diffv <file_a> <file_b>\n  diffv <dir_a> <dir_b>\n  git diff | diffv -"
            );
        }
    }
}

fn run_app(
    terminal: &mut Terminal<CrosstermBackend<io::Stdout>>,
    mode: AppMode,
    config: Config,
    args: &Cli,
    cwd: &Path,
) -> Result<()> {
    let watch_enabled = args.watch || (config.watcher.enabled && matches!(mode, AppMode::Git { .. }));

    let mut app = App::new(
        mode,
        config,
        watch_enabled,
        args.staged,
        args.unified,
        args.theme.clone(),
        args.ignore_whitespace,
        args.history,
    )?;

    // Channel for unifying keyboard events, debounced filesystem events and tick timer
    let (tx, rx) = mpsc::channel();

    // 1. Keyboard & terminal event listener thread
    let input_tx = tx.clone();
    thread::spawn(move || {
        loop {
            // Poll with small timeout so thread doesn't hang on exit
            if event::poll(Duration::from_millis(50)).unwrap_or(false) {
                if let Ok(evt) = event::read() {
                    if input_tx.send(AppEvent::Input(evt)).is_err() {
                        break;
                    }
                }
            }
        }
    });

    // 2. Tick event thread for smooth UI refresh and notification expiration
    let tick_tx = tx.clone();
    thread::spawn(move || {
        loop {
            thread::sleep(Duration::from_millis(250));
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    // 3. Filesystem watcher service
    let (watch_tx, watch_rx) = mpsc::channel();
    let _watcher = if let AppMode::Git { git_provider, .. } = &app.mode {
        WatchService::start(&git_provider.repo_root, app.config.watcher.debounce_ms, watch_tx).ok()
    } else {
        WatchService::start(cwd, app.config.watcher.debounce_ms, watch_tx).ok()
    };

    let reload_tx = tx.clone();
    thread::spawn(move || {
        while let Ok(WatchEvent::ReloadRequested) = watch_rx.recv() {
            if reload_tx.send(AppEvent::Reload).is_err() {
                break;
            }
        }
    });

    // Main event loop
    loop {
        terminal.draw(|f| app.render(f))?;

        if app.should_quit {
            break;
        }

        // Check if an external editor request is pending
        if let Some((file_path, line_no)) = app.editor_request.take() {
            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen)?;
            terminal.show_cursor()?;

            let edit_res = open_editor(&file_path, line_no, &app.config.editor);

            enable_raw_mode()?;
            execute!(terminal.backend_mut(), EnterAlternateScreen)?;
            terminal.clear()?;

            if let Err(e) = edit_res {
                app.set_notification(format!("Editor error: {}", e));
            } else {
                app.set_notification("Returned from editor");
                app.reload_diffs();
            }
            continue;
        }

        match rx.recv()? {
            AppEvent::Input(Event::Key(key)) => {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                }
            }
            AppEvent::Input(Event::Resize(_, _)) => {
                terminal.autoresize()?;
            }
            AppEvent::Reload => {
                if app.watch_mode {
                    app.reload_diffs();
                }
            }
            AppEvent::Tick => {
                // Trigger redraw if there's a notification
            }
            _ => {}
        }
    }

    Ok(())
}
