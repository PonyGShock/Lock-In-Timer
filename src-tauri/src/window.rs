use tauri::{AppHandle, Emitter};

use crate::engine::AppState;

pub const MAIN: &str = "main";
/// Pushed whenever the timer or settings move, so the UI is never stale.
pub const STATE_EVENT: &str = "lockin://state";

pub fn broadcast(app: &AppHandle, state: &AppState) {
    let _ = app.emit(STATE_EVENT, state);
}

/// Showing, hiding and positioning a popover are desktop concerns. A phone
/// shows one window, always, and the OS decides where it goes.
#[cfg(desktop)]
mod desktop {
    use std::sync::Mutex;
    use std::time::{Duration, Instant};

    use tauri::{AppHandle, Manager, Window};
    use tauri_plugin_positioner::{Position, WindowExt};

    use super::MAIN;

    /// When the popover last hid itself because it lost focus.
    static BLURRED_AT: Mutex<Option<Instant>> = Mutex::new(None);

    /// How recent a blur-hide must be to count as caused by the same click.
    /// A click is well under this; a deliberate second click is well over.
    const SAME_CLICK: Duration = Duration::from_millis(350);

    pub fn show(app: &AppHandle) {
        let Some(window) = app.get_webview_window(MAIN) else {
            return;
        };
        // TrayCenter needs the icon geometry the tray event handler records.
        // Before the first tray event, or on a platform that cannot report it,
        // sit in the top corner rather than in the middle of the screen.
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

    /// Hides the popover because something else took focus.
    pub fn hide_on_blur(window: &Window) {
        // While developing, a popover that vanishes the moment devtools take
        // focus is unusable.
        if cfg!(debug_assertions) {
            return;
        }
        let _ = window.hide();
        *BLURRED_AT.lock().unwrap_or_else(|p| p.into_inner()) = Some(Instant::now());
    }

    /// Opens or closes the popover from the tray icon.
    ///
    /// Clicking the icon to close an open popover delivers two events: the
    /// press takes focus away, which hides the popover, and then the click
    /// arrives and finds it hidden. Toggling naively reopened it, so the icon
    /// could never close what it had opened. A hide that happened a moment
    /// ago is treated as this click's doing, and the popover stays closed.
    pub fn toggle(app: &AppHandle) {
        let Some(window) = app.get_webview_window(MAIN) else {
            return;
        };
        if window.is_visible().unwrap_or(false) {
            let _ = window.hide();
            return;
        }
        let blurred_at = BLURRED_AT.lock().unwrap_or_else(|p| p.into_inner()).take();
        if blurred_at.is_some_and(|at| at.elapsed() < SAME_CLICK) {
            return;
        }
        show(app);
    }
}

#[cfg(desktop)]
pub use desktop::{hide, hide_on_blur, show, toggle};

#[cfg(not(desktop))]
pub fn hide(_app: &AppHandle) {}
