mod audio;
mod commands;
mod engine;
#[cfg(desktop)]
mod tray;
mod window;

use std::thread;
use std::time::Duration;

use lockin_core::{Fingerprint, Persister, Phase, Settings, Snapshot, Transition};
use tauri::{Manager, RunEvent};
use tauri_plugin_notification::NotificationExt;

use crate::audio::Audio;
use crate::engine::Engine;

/// How often the clock is advanced. Fine enough that a pause feels immediate,
/// coarse enough to stay invisible in a CPU graph.
const TICK: Duration = Duration::from_millis(200);

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_opener::init());

    // A tray, a login item and a window that positions itself under one are
    // all desktop ideas. On a phone the app is simply the app.
    #[cfg(desktop)]
    let builder =
        builder
            .plugin(tauri_plugin_positioner::init())
            .plugin(tauri_plugin_autostart::init(
                tauri_plugin_autostart::MacosLauncher::LaunchAgent,
                None,
            ));

    let app = builder
        .invoke_handler(tauri::generate_handler![
            commands::get_state,
            commands::timer_toggle,
            commands::timer_reset,
            commands::timer_skip,
            commands::select_preset,
            commands::update_settings,
            commands::preview_chime,
            commands::open_repository,
            commands::hide_window,
            commands::quit_app,
        ])
        .setup(|app| {
            // No dock icon and no app switcher entry: Lock In lives in the menu
            // bar, and a second place to find it would only be clutter.
            #[cfg(target_os = "macos")]
            app.set_activation_policy(tauri::ActivationPolicy::Accessory);

            let handle = app.handle().clone();

            let settings_path = handle
                .path()
                .app_config_dir()
                .map(|dir| dir.join("settings.json"))
                .unwrap_or_else(|_| std::path::PathBuf::from("lockin-settings.json"));

            let settings = Settings::load(&settings_path);
            let engine = Engine::new(settings, Persister::spawn(settings_path), Audio::spawn());
            app.manage(engine);

            #[cfg(desktop)]
            {
                commands::reconcile_launch_at_login(&handle, &handle.state::<Engine>());
                tray::build(&handle)?;
                let state = handle.state::<Engine>().state();
                tray::apply_snapshot(&handle, &state.timer, state.settings.show_clock_in_menu_bar);
            }

            spawn_tick_loop(handle);
            Ok(())
        })
        .on_window_event(|_window, _event| {
            #[cfg(desktop)]
            {
                use tauri::WindowEvent;

                if let WindowEvent::CloseRequested { api, .. } = _event {
                    // Closing the popover should put Lock In away, not end the
                    // session that is still running behind it.
                    api.prevent_close();
                    let _ = _window.hide();
                }

                if let WindowEvent::Focused(false) = _event {
                    window::hide_on_blur(_window);
                }
            }
        })
        .build(tauri::generate_context!())
        .expect("Lock In failed to start");

    app.run(|handle, event| {
        // Settings are written a moment after they change, so a slider drag is
        // one write and not a hundred. Quitting inside that moment must not
        // lose the change.
        if let RunEvent::Exit = event {
            if let Some(engine) = handle.try_state::<Engine>() {
                engine.flush();
            }
        }
    });
}

fn spawn_tick_loop(handle: tauri::AppHandle) {
    thread::spawn(move || {
        let mut last: Option<Fingerprint> = None;

        loop {
            thread::sleep(TICK);

            let engine = handle.state::<Engine>();
            let transition = engine.tick();

            // The clock face changes once a second; the ring in between is the
            // stylesheet's job. Comparing a copy-only fingerprint is free, so
            // the full state — settings, presets, strings — is only built on
            // the one tick in five that has something new to show.
            let fingerprint = engine.fingerprint();
            if last == Some(fingerprint) && transition.is_none() {
                continue;
            }
            last = Some(fingerprint);

            let state = engine.state();
            #[cfg(desktop)]
            tray::apply_snapshot(&handle, &state.timer, state.settings.show_clock_in_menu_bar);
            window::broadcast(&handle, &state);

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
