use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use diffv::config::{Config, UpdateChannel};
use diffv::core::models::Language;
use diffv::ui::app::{App, AppMode};
use diffv::update::{BuildInfo, UpdateEvent, UpdateFailure};

fn app() -> (tempfile::TempDir, App) {
    let dir = tempfile::tempdir().unwrap();
    let (old, new) = (dir.path().join("old.txt"), dir.path().join("new.txt"));
    std::fs::write(&old, "a\n").unwrap();
    std::fs::write(&new, "b\n").unwrap();
    let app = App::new(
        AppMode::FilePair(old, new),
        Config::default(),
        false,
        false,
        false,
        None,
        false,
        false,
    )
    .unwrap();
    (dir, app)
}

fn screen(app: &mut App) -> String {
    let mut terminal = ratatui::Terminal::new(ratatui::backend::TestBackend::new(120, 32)).unwrap();
    terminal.draw(|frame| app.render(frame)).unwrap();
    terminal
        .backend()
        .buffer()
        .content
        .iter()
        .map(|cell| cell.symbol())
        .collect()
}

fn press(app: &mut App, code: KeyCode) {
    app.handle_key(KeyEvent::new(code, KeyModifiers::NONE));
}

fn nightly() -> BuildInfo {
    BuildInfo {
        channel: UpdateChannel::Nightly,
        tag: "nightly".into(),
        commit: Some("b97105d".into()),
        built_at: Some("2026-10-06 21:21 UTC".into()),
    }
}

#[test]
fn no_popup_when_the_check_reported_nothing() {
    let (_dir, mut app) = app();
    let text = screen(&mut app);
    assert!(app.update_popup.is_none());
    assert!(!text.contains("diffv updated") && !text.contains("update failed"));
}

#[test]
fn popup_appears_after_an_update_and_closes_with_a_key() {
    for key in [KeyCode::Esc, KeyCode::Enter, KeyCode::Char('q')] {
        let (_dir, mut app) = app();
        app.show_update_event(UpdateEvent::Installed(nightly()));
        let text = screen(&mut app);
        assert!(text.contains("diffv updated"));
        assert!(text.contains("nightly · b97105d · 2026-10-06 21:21 UTC"));
        assert!(text.contains("Takes effect the next time you open diffv"));

        // Other keys do not leak to the diff view while the popup is up
        press(&mut app, KeyCode::Char('j'));
        assert!(app.update_popup.is_some());
        press(&mut app, key);
        assert!(app.update_popup.is_none());
        assert!(!app.should_quit);
        assert!(!screen(&mut app).contains("diffv updated"));
    }
}

#[test]
fn result_reopens_a_dismissed_downloading_popup() {
    let (_dir, mut app) = app();
    app.language = Language::Pt;
    app.show_update_event(UpdateEvent::Downloading(nightly()));
    assert!(screen(&mut app).contains("Atualizando o diffv"));
    press(&mut app, KeyCode::Esc);
    app.show_update_event(UpdateEvent::Installed(nightly()));
    let text = screen(&mut app);
    assert!(text.contains("diffv atualizado"));
    assert!(text.contains("Passa a valer na próxima abertura"));
}

#[test]
fn failed_update_shows_the_reason() {
    let (_dir, mut app) = app();
    app.show_update_event(UpdateEvent::Failed {
        channel: UpdateChannel::Nightly,
        failure: UpdateFailure::Locked,
        target: Some(nightly()),
    });
    let text = screen(&mut app);
    assert!(text.contains("diffv update failed"));
    assert!(text.contains("Another diffv is already installing an update"));
    assert!(text.contains("nightly · b97105d"));
}
