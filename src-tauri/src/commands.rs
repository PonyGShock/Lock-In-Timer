use tauri::{AppHandle, State};

use crate::engine::{AppState, Engine};
use crate::tray;
use crate::window;
use crema_core::settings::Settings;

/// Pushes the latest state at the menu bar after a command has changed it, so
/// the title never waits up to a tick to catch up with a click.
fn refresh(app: &AppHandle, engine: &Engine) -> AppState {
    let state = engine.state();
    tray::apply_snapshot(app, &state.timer, state.settings.show_clock_in_menu_bar);
    state
}

#[tauri::command]
pub fn get_state(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    engine.roll_over_day();
    refresh(&app, &engine)
}

#[tauri::command]
pub fn timer_toggle(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    engine.toggle();
    refresh(&app, &engine)
}

#[tauri::command]
pub fn timer_start(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    engine.start();
    refresh(&app, &engine)
}

#[tauri::command]
pub fn timer_pause(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    engine.pause();
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
pub fn timer_restart_phase(app: AppHandle, engine: State<'_, Engine>) -> AppState {
    engine.restart_phase();
    refresh(&app, &engine)
}

#[tauri::command]
pub fn set_preset(app: AppHandle, engine: State<'_, Engine>, id: String) -> AppState {
    engine.set_preset(&id);
    refresh(&app, &engine)
}

#[tauri::command]
pub fn set_custom_preset(
    app: AppHandle,
    engine: State<'_, Engine>,
    focus_secs: u32,
    short_break_secs: u32,
    long_break_secs: u32,
    rounds: u32,
) -> AppState {
    engine.set_custom_preset(focus_secs, short_break_secs, long_break_secs, rounds);
    refresh(&app, &engine)
}

#[tauri::command]
pub fn update_settings(app: AppHandle, engine: State<'_, Engine>, settings: Settings) -> AppState {
    let was_enabled = engine.settings().launch_at_login;
    let wants_enabled = settings.launch_at_login;
    engine.update_settings(settings);
    if was_enabled != wants_enabled {
        apply_launch_at_login(&app, wants_enabled);
    }
    refresh(&app, &engine)
}

#[tauri::command]
pub fn preview_chime(engine: State<'_, Engine>) {
    engine.preview_chime();
}

#[tauri::command]
pub fn hide_window(app: AppHandle) {
    window::hide(&app);
}

#[tauri::command]
pub fn quit_app(app: AppHandle) {
    app.exit(0);
}

#[cfg(desktop)]
fn apply_launch_at_login(app: &AppHandle, enabled: bool) {
    use tauri_plugin_autostart::ManagerExt;

    let manager = app.autolaunch();
    let result = if enabled {
        manager.enable()
    } else {
        manager.disable()
    };
    if let Err(error) = result {
        eprintln!("crema: could not change launch at login: {error}");
    }
}

#[cfg(not(desktop))]
fn apply_launch_at_login(_app: &AppHandle, _enabled: bool) {}
