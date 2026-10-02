
use std::io::{self, stdout, IsTerminal};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{mpsc, Arc};
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
use diffv::config::{Config, UpdateChannel};
use diffv::git::provider::GitProvider;
use diffv::integration::editor::open_editor;
use diffv::integration::fzf::{is_fzf_available, search_diff_text_fzf, search_files_fzf};
use diffv::ui::app::{App, AppMode, FzfRequest};
use diffv::watcher::service::{WatchEvent, WatchService};

enum AppEvent {
    Input(Event),
    Reload,
    WatcherProgress { scanned: usize, total: Option<usize> },
    Tick,
}

fn main() {
    let args = Cli::parse();
    let is_version = args.targets.len() == 1
        && (args.targets[0] == "version")
        && !Path::new(&args.targets[0]).exists();

    if is_version {
        use clap::CommandFactory;
        let cmd = Cli::command();
        println!("diffv {}", cmd.get_version().unwrap_or(""));
        std::process::exit(0);
    }

    let is_uninstall = args.uninstall
        || (args.targets.len() == 1
            && (args.targets[0] == "uninstall" || args.targets[0] == "remove")
            && !Path::new(&args.targets[0]).exists());

    if is_uninstall {
        if let Err(err) = diffv::uninstall::run() {
            eprintln!("Error: {:#}", err);
            std::process::exit(1);
        }
        std::process::exit(0);
    }
    let wait_on_error = args.wait_on_error;
    let result = match &args.tmux_toggle {
        Some(toggle) => diffv::integration::tmux::toggle_popup(&toggle[0], &toggle[1], Path::new(&toggle[2])),
        None => run(args),
    };
    if let Err(err) = result {
        eprintln!("Error: {:#}", err);
        if wait_on_error {
            wait_for_key();
        }
        std::process::exit(1);
    }
}

fn wait_for_key() {
    eprintln!("\nPress any key to close…");
    if enable_raw_mode().is_err() {
        return;
    }
    while let Ok(evt) = event::read() {
        if matches!(evt, Event::Key(key) if key.kind == KeyEventKind::Press) {
            break;
        }
    }
    let _ = disable_raw_mode();
}

fn run(args: Cli) -> Result<()> {
    let config = Config::load();

    // Determine target update channel:
    // CLI --beta flag > CLI --channel <ch> > DIFFV_BETA / DIFFV_CHANNEL env > config.update.channel
    let channel = if args.beta {
        UpdateChannel::Beta
    } else if let Some(ref ch) = args.channel {
        ch.parse().unwrap_or(UpdateChannel::Stable)
    } else if std::env::var_os("DIFFV_BETA").is_some() {
        UpdateChannel::Beta
    } else if let Ok(ch) = std::env::var("DIFFV_CHANNEL") {
        ch.parse().unwrap_or(UpdateChannel::Stable)
    } else {
        config.update.channel
    };

    // Explicit update request (e.g. diffv --update, diffv update, dv upgrade, dv up)
    let is_explicit_update = args.update
        || (args.targets.len() == 1
            && (args.targets[0] == "update" || args.targets[0] == "upgrade" || args.targets[0] == "up")
            && !Path::new(&args.targets[0]).exists());

    if is_explicit_update {
        match diffv::update::check_and_install_verbose(channel) {
            Ok(Some(path)) => {
                println!("Successfully updated diffv to {} ({})!", path.display(), channel);
            }
            Ok(None) => {
                println!("diffv is already up to date ({})", channel);
            }
            Err(err) => {
                eprintln!("Update failed: {:#}", err);
                std::process::exit(1);
            }
        }
        return Ok(());
    }

    // Determine if automatic update check is enabled on startup:
    // Opt-out priority: CLI -n / --no-update > CLI --auto-update <BOOL> > env DIFFV_NO_UPDATE / DIFFV_AUTO_UPDATE > config.update.auto_update
    let auto_update_enabled = if args.no_update {
        false
    } else if let Some(enabled) = args.auto_update {
        enabled
    } else if std::env::var_os("DIFFV_NO_UPDATE").is_some() {
        false
    } else if let Ok(val) = std::env::var("DIFFV_AUTO_UPDATE") {
        val != "0" && val.to_lowercase() != "false"
    } else {
        config.update.auto_update
    };

    if auto_update_enabled && std::env::var_os("DIFFV_UPDATE_RESTART").is_none() {
        // Offline, rate-limited or no newer release: keep running the installed version.
        if let Ok(Some(path)) = diffv::update::check_and_install(channel) {
            eprintln!("diffv updated. Restarting…");
            #[cfg(unix)] {
                use std::os::unix::process::CommandExt;
                let error = std::process::Command::new(path)
                    .args(std::env::args_os().skip(1))
                    .env("DIFFV_UPDATE_RESTART", "1").exec();
                eprintln!("Could not restart diffv: {}. Reopen to use the update.", error);
            }
        }
    }

    // Setup custom panic hook to restore terminal
    let default_panic = std::panic::take_hook();
    std::panic::set_hook(Box::new(move |panic_info| {
        let _ = disable_raw_mode();
        let _ = execute!(stdout(), LeaveAlternateScreen, event::DisableMouseCapture);
        default_panic(panic_info);
    }));

    // Determine target working directory
    let cwd = args.cwd.as_deref().unwrap_or(Path::new("."));

    // Determine App Mode
    let mode = determine_app_mode(&args, cwd)?;

    // Start TUI
    enable_raw_mode()?;
    let mut stdout = stdout();
    execute!(stdout, EnterAlternateScreen, event::EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    let app_result = run_app(&mut terminal, mode, config, &args, cwd);

    // Teardown TUI
    disable_raw_mode()?;
    execute!(terminal.backend_mut(), LeaveAlternateScreen, event::DisableMouseCapture)?;
    terminal.show_cursor()?;

    app_result
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
    let is_editor_active = Arc::new(AtomicBool::new(false));

    // 1. Keyboard & terminal event listener thread
    let input_tx = tx.clone();
    let editor_flag = is_editor_active.clone();
    thread::spawn(move || {
        loop {
            if editor_flag.load(Ordering::Relaxed) {
                thread::sleep(Duration::from_millis(50));
                continue;
            }

            if event::poll(Duration::from_millis(200)).unwrap_or(false) {
                if editor_flag.load(Ordering::Relaxed) {
                    continue;
                }
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
            thread::sleep(Duration::from_millis(300));
            if tick_tx.send(AppEvent::Tick).is_err() {
                break;
            }
        }
    });

    // 3. Filesystem watcher service
    let (watch_tx, watch_rx) = mpsc::channel();
    let _watcher = if watch_enabled {
        if let AppMode::Git { git_provider, .. } = &app.mode {
            WatchService::start(&git_provider.repo_root, app.config.watcher.debounce_ms, watch_tx).ok()
        } else {
            WatchService::start(cwd, app.config.watcher.debounce_ms, watch_tx).ok()
        }
    } else {
        None
    };

    let watch_event_tx = tx.clone();
    thread::spawn(move || {
        while let Ok(evt) = watch_rx.recv() {
            match evt {
                WatchEvent::ReloadRequested => {
                    if watch_event_tx.send(AppEvent::Reload).is_err() {
                        break;
                    }
                }
                WatchEvent::Scanning { scanned_dirs } => {
                    if watch_event_tx
                        .send(AppEvent::WatcherProgress {
                            scanned: scanned_dirs,
                            total: None,
                        })
                        .is_err()
                    {
                        break;
                    }
                }
                WatchEvent::Ready { total_dirs } => {
                    if watch_event_tx
                        .send(AppEvent::WatcherProgress {
                            scanned: total_dirs,
                            total: Some(total_dirs),
                        })
                        .is_err()
                    {
                        break;
                    }
                }
            }
        }
    });

    // Main event loop with dirty tracking for 0.0% idle CPU
    let mut needs_redraw = true;

    loop {
        if needs_redraw {
            terminal.draw(|f| app.render(f))?;
            needs_redraw = false;
        }

        if app.should_quit {
            break;
        }

        // Check if an external editor request is pending
        if let Some((file_path, line_no)) = app.editor_request.take() {
            // Signal input thread to stop polling/reading stdin
            is_editor_active.store(true, Ordering::SeqCst);

            // Drain any pending input events already queued in rx before editor opens
            while rx.try_recv().is_ok() {}

            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen, event::DisableMouseCapture)?;
            terminal.show_cursor()?;

            let edit_res = open_editor(&file_path, line_no, &app.config.editor);

            enable_raw_mode()?;
            execute!(terminal.backend_mut(), EnterAlternateScreen, event::EnableMouseCapture)?;
            terminal.clear()?;

            // Drain any leftover events in crossterm buffer and in rx channel from editor exit (:q<Enter>)
            while event::poll(Duration::from_millis(40)).unwrap_or(false) {
                let _ = event::read();
            }
            while rx.try_recv().is_ok() {}

            is_editor_active.store(false, Ordering::SeqCst);
            app.editor_request = None;

            if let Err(e) = edit_res {
                app.set_notification(format!("Editor error: {}", e));
            } else {
                app.set_notification("Returned from editor");
                app.reload_diffs();
            }
            needs_redraw = true;
            continue;
        }

        // Check if an interactive fzf search request is pending
        if let Some(fzf_req) = app.fzf_request.take() {
            if !is_fzf_available() {
                let msg = match app.language {
                    diffv::core::models::Language::En => "fzf is not installed or not found on PATH",
                    diffv::core::models::Language::Pt => "fzf não está instalado ou não foi encontrado no PATH",
                };
                app.set_notification(msg);
                needs_redraw = true;
                continue;
            }

            let query = app.prepare_fzf(fzf_req);
            if query.items.is_empty() {
                needs_redraw = true;
                continue;
            }

            is_editor_active.store(true, Ordering::SeqCst);
            while rx.try_recv().is_ok() {}

            disable_raw_mode()?;
            execute!(terminal.backend_mut(), LeaveAlternateScreen, event::DisableMouseCapture)?;
            terminal.show_cursor()?;

            let res = match fzf_req {
                FzfRequest::Files => search_files_fzf(&query.items, &query.header),
                FzfRequest::Text => search_diff_text_fzf(&query.items, &query.header),
            };

            enable_raw_mode()?;
            execute!(terminal.backend_mut(), EnterAlternateScreen, event::EnableMouseCapture)?;
            terminal.clear()?;

            while event::poll(Duration::from_millis(40)).unwrap_or(false) {
                let _ = event::read();
            }
            while rx.try_recv().is_ok() {}

            is_editor_active.store(false, Ordering::SeqCst);

            match res {
                Ok(Some(selected)) => match fzf_req {
                    FzfRequest::Files => app.handle_fzf_file_result(selected),
                    FzfRequest::Text => app.handle_fzf_text_result(selected),
                },
                Ok(None) => {}
                Err(e) => app.set_notification(format!("fzf error: {}", e)),
            }

            needs_redraw = true;
            continue;
        }


        match rx.recv()? {
            AppEvent::Input(Event::Key(key)) => {
                if key.kind == KeyEventKind::Press {
                    app.handle_key(key);
                    needs_redraw = true;
                }
            }
            AppEvent::Input(Event::Mouse(mouse)) => {
                app.handle_mouse(mouse);
                needs_redraw = true;
            }
            AppEvent::Input(Event::Resize(_, _)) => {
                terminal.autoresize()?;
                needs_redraw = true;
            }
            AppEvent::Reload => {
                if app.watch_mode {
                    app.reload_diffs();
                    needs_redraw = true;
                }
            }
            AppEvent::WatcherProgress { scanned, total } => {
                if app.watch_mode {
                    app.update_watcher_progress(scanned, total);
                    needs_redraw = true;
                }
            }
            AppEvent::Tick => {
                let had_notification = app.notification.is_some();
                app.expire_notification();
                let scanning = app.is_watcher_scanning();
                if scanning {
                    app.spinner_idx = (app.spinner_idx + 1) % 10;
                }
                if had_notification || app.pending_key.is_some() || scanning {
                    needs_redraw = true;
                }
            }
            _ => {}
        }
    }

    Ok(())
}
