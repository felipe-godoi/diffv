use std::path::Path;
use std::sync::mpsc::Sender;
use std::time::Duration;
use notify::RecursiveMode;
use notify_debouncer_mini::{new_debouncer, DebouncedEvent, Debouncer};

pub enum WatchEvent {
    ReloadRequested,
}

pub struct WatchService {
    // Keep debouncer alive
    _debouncer: Debouncer<notify::RecommendedWatcher>,
}

impl WatchService {
    pub fn start(
        watch_path: &Path,
        debounce_ms: u64,
        tx: Sender<WatchEvent>,
    ) -> anyhow::Result<Self> {
        let mut debouncer = new_debouncer(
            Duration::from_millis(debounce_ms),
            move |res: Result<Vec<DebouncedEvent>, _>| match res {
                Ok(events) => {
                    let has_relevant_change = events.iter().any(|e| {
                        let path_str = e.path.to_string_lossy();
                        !path_str.contains("/.git/") && !path_str.contains("/target/")
                    });

                    if has_relevant_change {
                        let _ = tx.send(WatchEvent::ReloadRequested);
                    }
                }
                Err(err) => {
                    eprintln!("Watcher error: {:?}", err);
                }
            },
        )?;

        debouncer.watcher().watch(watch_path, RecursiveMode::Recursive)?;

        Ok(Self {
            _debouncer: debouncer,
        })
    }
}
