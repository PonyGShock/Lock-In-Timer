use lockin_core::{RunState, Snapshot};
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::engine::Engine;
use crate::window;

pub const TRAY_ID: &str = "lockin-tray";

/// Menu entries whose labels change with the timer, kept so they can be
/// updated in place rather than by rebuilding the whole menu each second.
pub struct TrayMenuItems {
    toggle: MenuItem<Wry>,
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    let open = MenuItem::with_id(app, "open", "Open Lock In", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Start focus", true, None::<&str>)?;
    let skip = MenuItem::with_id(app, "skip", "Skip to next phase", true, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "Reset cycle", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Lock In", true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(
        app,
        &[
            &open,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &skip,
            &reset,
            &PredefinedMenuItem::separator(app)?,
            &quit,
        ],
    )?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(Image::from_bytes(include_bytes!("../assets/tray.png"))?)
        .menu(&menu)
        // The left click opens the popover; the menu belongs to the right one.
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event)
        .build(app)?;

    // Lets macOS recolour the glyph for light, dark and highlighted menu bars.
    #[cfg(target_os = "macos")]
    let _ = tray.set_icon_as_template(true);
    let _ = &tray;

    app.manage(TrayMenuItems { toggle });
    Ok(())
}

fn on_menu_event(app: &AppHandle, event: MenuEvent) {
    let engine = app.state::<Engine>();
    match event.id().as_ref() {
        "open" => window::show(app),
        "toggle" => engine.toggle(),
        "skip" => engine.skip(),
        "reset" => engine.reset(),
        "quit" => app.exit(0),
        _ => return,
    }
    let state = engine.state();
    apply_snapshot(app, &state.timer, state.settings.show_clock_in_menu_bar);
    window::broadcast(app, &state);
}

fn on_tray_icon_event(tray: &TrayIcon, event: TrayIconEvent) {
    let app = tray.app_handle();
    // The positioner plugin records the icon's geometry here; without this the
    // popover cannot line itself up under the menu bar item.
    tauri_plugin_positioner::on_tray_event(app, &event);

    if let TrayIconEvent::Click {
        button: MouseButton::Left,
        button_state: MouseButtonState::Up,
        ..
    } = event
    {
        window::toggle(app);
    }
}

/// Reflects the current timer in the menu bar title, tooltip and menu labels.
pub fn apply_snapshot(app: &AppHandle, snapshot: &Snapshot, show_clock: bool) {
    if let Some(items) = app.try_state::<TrayMenuItems>() {
        let _ = items.toggle.set_text(toggle_label(snapshot));
    }

    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    // An idle timer shows the glyph alone. A menu bar that counts at you when
    // you are not in a session is the opposite of calm.
    let title = match (show_clock, snapshot.state) {
        (true, RunState::Running) => Some(snapshot.clock.clone()),
        (true, RunState::Paused) => Some(format!("{} paused", snapshot.clock)),
        _ => None,
    };
    let _ = tray.set_title(title);
    let _ = tray.set_tooltip(Some(format!(
        "{} · {}",
        snapshot.phase_label, snapshot.clock
    )));
}

fn toggle_label(snapshot: &Snapshot) -> String {
    match snapshot.state {
        RunState::Running => format!("Pause {}", snapshot.phase_label.to_lowercase()),
        RunState::Paused => format!("Resume {}", snapshot.phase_label.to_lowercase()),
        RunState::Idle => format!("Start {}", snapshot.phase_label.to_lowercase()),
    }
}
