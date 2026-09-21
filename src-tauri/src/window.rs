use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_positioner::{Position, WindowExt};

use crate::engine::AppState;

pub const MAIN: &str = "main";
/// Pushed whenever the timer or settings move, so the UI is never stale.
pub const STATE_EVENT: &str = "crema://state";

pub fn show(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        return;
    };
    // TrayCenter needs the icon geometry the tray event handler records. Before
    // the first tray event, or on a platform that cannot report it, sit in the
    // top corner rather than appearing in the middle of the screen.
    if window.move_window(Position::TrayCenter).is_err() {
        let _ = window.move_window(Position::TopRight);
    }
    let _ = window.show();
    let _ = window.set_focus();
}

pub fn hide(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MAIN) {
        let _ = window.hide();
    }
}

pub fn toggle(app: &AppHandle) {
    let Some(window) = app.get_webview_window(MAIN) else {
        return;
    };
    if window.is_visible().unwrap_or(false) {
        let _ = window.hide();
    } else {
        show(app);
    }
}

pub fn broadcast(app: &AppHandle, state: &AppState) {
    let _ = app.emit(STATE_EVENT, state);
}
