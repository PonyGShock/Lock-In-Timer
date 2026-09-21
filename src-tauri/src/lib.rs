mod audio;
mod commands;
mod engine;
mod tray;
mod window;

use std::thread;
use std::time::Duration;

use crema_core::{Phase, RunState, Snapshot, Transition};
use tauri::{Manager, WindowEvent};
use tauri_plugin_notification::NotificationExt;

use crate::audio::Audio;
use crate::engine::Engine;
use crema_core::settings::Settings;

/// How often the clock is advanced. Fine enough that a pause feels immediate,
/// coarse enough to stay invisible in a CPU graph.
const TICK: Duration = Duration::from_millis(200);

pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_positioner::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            None,
        ))
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::timer_toggle,
            commands::timer_start,
            commands::timer_pause,
            commands::timer_reset,
            commands::timer_skip,
            commands::timer_restart_phase,
            commands::set_preset,
            commands::set_custom_preset,
            commands::update_settings,
            commands::preview_chime,
            commands::hide_window,
            commands::quit_app,
        ])
        .setup(|app| {
            // No dock icon and no app switcher entry: Crema lives in the menu
            // bar, and a second place to find it would only be clutter.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();

            let settings_path = handle
                .path()
                .app_config_dir()
                .map(|dir| dir.join("settings.json"))
                .unwrap_or_else(|_| std::path::PathBuf::from("crema-settings.json"));

            let settings = Settings::load(&settings_path);
            let engine = Engine::new(settings, settings_path, Audio::spawn());
            engine.sync_noise();
            app.manage(engine);

            tray::build(&handle)?;

            let state = handle.state::<Engine>().state();
            tray::apply_snapshot(&handle, &state.timer, state.settings.show_clock_in_menu_bar);

            spawn_tick_loop(handle);
            Ok(())
        })
        .on_window_event(|window, event| {
            if let WindowEvent::CloseRequested { api, .. } = event {
                // Closing the popover should put Crema away, not end the
                // session that is still running behind it.
                api.prevent_close();
                let _ = window.hide();
            }

            // While developing, a popover that vanishes the moment devtools
            // take focus is unusable.
            #[cfg(not(debug_assertions))]
            if let WindowEvent::Focused(false) = event {
                let _ = window.hide();
            }
        })
        .run(tauri::generate_context!())
        .expect("crema failed to start");
}

fn spawn_tick_loop(handle: tauri::AppHandle) {
    thread::spawn(move || {
        let mut last: Option<(String, RunState, Phase, u32, u32)> = None;

        loop {
            thread::sleep(TICK);

            let engine = handle.state::<Engine>();
            let transition = engine.tick();
            let state = engine.state();

            // The clock only moves once a second; the ring in between is the
            // stylesheet's job. Emitting every tick would be four wasted
            // round trips out of five.
            let fingerprint = (
                state.timer.clock.clone(),
                state.timer.state,
                state.timer.phase,
                state.timer.round,
                state.timer.completed_focus,
            );
            if last.as_ref() != Some(&fingerprint) {
                last = Some(fingerprint);
                tray::apply_snapshot(&handle, &state.timer, state.settings.show_clock_in_menu_bar);
                window::broadcast(&handle, &state);
            }

            if let Some(transition) = transition {
                if transition.completed && state.settings.notifications_enabled {
                    notify(&handle, &transition, &state.timer);
                }
            }
        }
    });
}

fn notify(handle: &tauri::AppHandle, transition: &Transition, snapshot: &Snapshot) {
    let (title, body) = match transition.ended {
        Phase::Focus => (
            "Focus complete",
            if transition.next == Phase::LongBreak {
                "Take a long break. You have earned it."
            } else if transition.auto_started {
                "Your break has started."
            } else {
                "Time for a break when you are ready."
            },
        ),
        Phase::ShortBreak | Phase::LongBreak => (
            "Break over",
            if transition.auto_started {
                "Back to it. Focus has started."
            } else {
                "Ready when you are."
            },
        ),
    };

    let _ = handle
        .notification()
        .builder()
        .title(title)
        .body(format!(
            "{body}\n{} · {}",
            snapshot.phase_label, snapshot.clock
        ))
        .show();
}
