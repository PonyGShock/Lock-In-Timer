use serde_json::{json, Value};
use tauri::{AppHandle, State};

use crate::engine::{AppState, Engine};
use crate::window;

const REPOSITORY: &str = "https://github.com/PonyGShock/Lock-In-Timer";

/// Pushes the latest state at the menu bar after a command has changed it, so
/// the title never waits up to a tick to catch up with a click.
fn refresh(app: &AppHandle, engine: &Engine) -> AppState {
    let state = engine.state();
    #[cfg(desktop)]
    crate::tray::apply_snapshot(app, &state.timer, state.settings.show_clock_in_menu_bar);
    let _ = app;
    state
}

#[tauri::command]
pub fn get_state(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    refresh(&app, &engine)
}

#[tauri::command]
pub fn timer_toggle(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    engine.toggle();
    refresh(&app, &engine)
}

#[tauri::command]
pub fn timer_reset(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    engine.reset();
    refresh(&app, &engine)
}

#[tauri::command]
pub fn timer_skip(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    engine.skip();
    refresh(&app, &engine)
}

#[tauri::command]
pub fn select_preset(app: AppHandle, engine: State<'_, Engine>, id: String) -> AppState {
    engine.select_preset(&id);
    refresh(&app, &engine)
}

/// Applies a partial settings update: only the values named in `patch`.
///
/// Sending the whole settings object instead let a stale copy undo a change
/// made a moment earlier — selecting a preset and then flipping a switch sent
/// the old preset back and reset the timer.
#[tauri::command]
pub fn update_settings(app: AppHandle, engine: State<'_, Engine>, patch: Value) -> AppState {
    let was_enabled = engine.settings().launch_at_login;
    engine.apply_patch(&patch);
    let wants_enabled = engine.settings().launch_at_login;

    if was_enabled != wants_enabled && !set_launch_at_login(&app, wants_enabled) {
        // The system refused. Put the switch back, so it shows what is true
        // rather than what was asked for.
        engine.apply_patch(&json!({ "launchAtLogin": was_enabled }));
    }
    refresh(&app, &engine)
}

#[tauri::command]
pub fn preview_chime(engine: State<'_, Engine>) {
    engine.preview_chime();
}

/// Opens the project page in the user's browser.
///
/// This goes through Rust deliberately. The webview is not allowed to open
/// URLs at all — earlier builds granted it the permission without a scope,
/// which the opener plugin treats as "nothing allowed", so the link silently
/// did nothing — and a single fixed destination does not need that power.
#[tauri::command]
pub fn open_repository(app: AppHandle) {
    use tauri_plugin_opener::OpenerExt;

    if let Err(error) = app.opener().open_url(REPOSITORY, None::<&str>) {
        eprintln!("lock-in: could not open {REPOSITORY}: {error}");
    }
}

#[tauri::command]
pub fn hide_window(app: AppHandle) {
    window::hide(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

/// Asks the system to add or remove the login item. Returns whether it did.
#[cfg(desktop)]
fn set_launch_at_login(app: &AppHandle, enabled: bool) -> bool {
    use tauri_plugin_autostart::ManagerExt;

    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    match result {
        Ok(()) => true,
        Err(error) => {
            eprintln!("lock-in: could not change launch at login: {error}");
            false
        }
    }
}

#[cfg(not(desktop))]
fn set_launch_at_login(_app: &AppHandle, _enabled: bool) -> bool {
    false
}

/// Brings the stored setting in line with the system's actual login items.
/// Someone may have removed Lock In from them in System Settings, or an
/// earlier attempt to add it may have failed; either way the switch should
/// show what is real.
#[cfg(desktop)]
pub fn reconcile_launch_at_login(app: &AppHandle, engine: &Engine) {
    use tauri_plugin_autostart::ManagerExt;

    if let Ok(actual) = app.autolaunch().is_enabled() {
        engine.apply_patch(&json!({ "launchAtLogin": actual }));
    }
}
