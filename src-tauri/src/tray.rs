use std::sync::Mutex;

use lockin_core::{tray_glyph, GlyphFill, GlyphStyle, RunState, Snapshot};
use tauri::image::Image;
use tauri::menu::{Menu, MenuEvent, MenuItem, PredefinedMenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIcon, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Wry};

use crate::engine::Engine;
use crate::window;

pub const TRAY_ID: &str = "lockin-tray";

/// macOS recolours a black template glyph to suit the menu bar. Windows and
/// Linux show icons as drawn, so they get the full-colour tile instead.
#[cfg(target_os = "macos")]
const GLYPH: (GlyphStyle, u32) = (GlyphStyle::Template, 36);
#[cfg(not(target_os = "macos"))]
const GLYPH: (GlyphStyle, u32) = (GlyphStyle::Tile, 32);

/// Steps the ring moves in. Fine enough to see it move every half minute or
/// so in a 25-minute session; coarse enough that the icon is not redrawn
/// every second.
const GLYPH_STEPS: f32 = 60.0;

/// What the icon currently shows, so it is only redrawn when that changes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum GlyphKey {
    Mark,
    Progress { step: u8, paused: bool },
}

impl GlyphKey {
    fn of(snapshot: &Snapshot) -> Self {
        let step = (snapshot.progress.clamp(0.0, 1.0) * GLYPH_STEPS).round() as u8;
        match snapshot.state {
            RunState::Idle => GlyphKey::Mark,
            RunState::Running => GlyphKey::Progress {
                step,
                paused: false,
            },
            RunState::Paused => GlyphKey::Progress { step, paused: true },
        }
    }

    fn fill(self) -> GlyphFill {
        match self {
            GlyphKey::Mark => GlyphFill::Mark,
            GlyphKey::Progress { step, paused } => GlyphFill::Progress {
                fraction: f32::from(step) / GLYPH_STEPS,
                paused,
            },
        }
    }
}

fn glyph(key: GlyphKey) -> Image<'static> {
    let (style, size) = GLYPH;
    Image::new_owned(tray_glyph(size, style, key.fill()), size, size)
}

/// Menu entries whose labels change with the timer, kept so they can be
/// updated in place rather than by rebuilding the whole menu each second.
pub struct TrayMenuItems {
    status: MenuItem<Wry>,
    toggle: MenuItem<Wry>,
    glyph: Mutex<GlyphKey>,
}

pub fn build(app: &AppHandle) -> tauri::Result<()> {
    // Information, not an action: shown greyed out at the top of the menu.
    let status = MenuItem::with_id(app, "status", "Focus", false, None::<&str>)?;
    let open = MenuItem::with_id(app, "open", "Open Lock In", true, None::<&str>)?;
    let toggle = MenuItem::with_id(app, "toggle", "Start focus", true, None::<&str>)?;
    let skip = MenuItem::with_id(app, "skip", "Skip to next phase", true, None::<&str>)?;
    let reset = MenuItem::with_id(app, "reset", "Reset cycle", true, None::<&str>)?;
    let quit = MenuItem::with_id(app, "quit", "Quit Lock In", true, Some("CmdOrCtrl+Q"))?;

    let menu = Menu::with_items(
        app,
        &[
            &status,
            &PredefinedMenuItem::separator(app)?,
            &toggle,
            &skip,
            &reset,
            &PredefinedMenuItem::separator(app)?,
            &open,
            &quit,
        ],
    )?;

    let tray = TrayIconBuilder::with_id(TRAY_ID)
        .icon(glyph(GlyphKey::Mark))
        // Lets macOS recolour the glyph for light, dark and highlighted menu
        // bars. Ignored elsewhere.
        .icon_as_template(GLYPH.0 == GlyphStyle::Template)
        .menu(&menu)
        // The left click opens the popover; the menu belongs to the right one.
        .show_menu_on_left_click(false)
        .on_menu_event(on_menu_event)
        .on_tray_icon_event(on_tray_icon_event)
        .build(app)?;
    let _ = tray;

    app.manage(TrayMenuItems {
        status,
        toggle,
        glyph: Mutex::new(GlyphKey::Mark),
    });
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

/// Reflects the current timer in the menu bar icon, title, tooltip and menu.
pub fn apply_snapshot(app: &AppHandle, snapshot: &Snapshot, show_clock: bool) {
    let Some(tray) = app.tray_by_id(TRAY_ID) else {
        return;
    };

    if let Some(items) = app.try_state::<TrayMenuItems>() {
        let _ = items.status.set_text(status_label(snapshot));
        let _ = items.toggle.set_text(toggle_label(snapshot));

        let key = GlyphKey::of(snapshot);
        let mut shown = items.glyph.lock().unwrap_or_else(|e| e.into_inner());
        if *shown != key {
            // Set together: a plain set_icon drops the template flag on macOS,
            // and the glyph would turn black on a dark menu bar.
            let template = GLYPH.0 == GlyphStyle::Template;
            if tray
                .set_icon_with_as_template(Some(glyph(key)), template)
                .is_ok()
            {
                *shown = key;
            }
        }
    }

    // An idle timer shows the glyph alone. A menu bar that counts at you when
    // you are not in a session is the opposite of calm.
    let title = match (show_clock, snapshot.state) {
        (true, RunState::Running) => Some(snapshot.clock.clone()),
        (true, RunState::Paused) => Some(format!("{} paused", snapshot.clock)),
        _ => None,
    };
    let _ = tray.set_title(title);
    let _ = tray.set_tooltip(Some(format!("Lock In · {}", status_label(snapshot))));
}

fn status_label(snapshot: &Snapshot) -> String {
    let when = match snapshot.state {
        RunState::Running => format!("{} left", snapshot.clock),
        RunState::Paused => format!("paused at {}", snapshot.clock),
        RunState::Idle => format!("{} ready", snapshot.clock),
    };
    format!(
        "{} · {} · round {} of {}",
        snapshot.phase_label, when, snapshot.round, snapshot.rounds
    )
}

fn toggle_label(snapshot: &Snapshot) -> String {
    match snapshot.state {
        RunState::Running => format!("Pause {}", snapshot.phase_label.to_lowercase()),
        RunState::Paused => format!("Resume {}", snapshot.phase_label.to_lowercase()),
        RunState::Idle => format!("Start {}", snapshot.phase_label.to_lowercase()),
    }
}
