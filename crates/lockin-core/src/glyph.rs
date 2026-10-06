//! The menu bar icon, drawn at runtime.
//!
//! The icon is the app's mark — a cup seen from above — and the crema ring
//! inside it is a live progress ring: it fills as the session runs, so the
//! tray shows how far along you are even where there is no room for a clock.
//!
//! Shapes are signed distance fields: one sample per pixel gives clean,
//! antialiased edges at any size, with no image library and no assets.

/// How the glyph is coloured.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GlyphStyle {
    /// Black on transparency. macOS recolours template images to suit light,
    /// dark and highlighted menu bars.
    Template,
    /// A small app tile in the brand colours, legible on both light and dark
    /// taskbars. Windows and Linux do not recolour tray icons, so a black
    /// glyph would vanish on the dark Windows 11 taskbar.
    Tile,
}

/// What the ring inside the cup shows.
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum GlyphFill {
    /// No session under way: the mark as it appears on the app icon.
    Mark,
    /// A session under way, `fraction` of it done. Paused rings are dimmed.
    Progress { fraction: f32, paused: bool },
}

/// The design grid. Everything below is in these units.
const GRID: f32 = 24.0;

/// The ring's share of a full turn when showing the mark.
const MARK_SWEEP: f32 = 0.75;

const MOCHA_TOP: [f32; 3] = [0x7A as f32, 0x50 as f32, 0x38 as f32];
const MOCHA_BOTTOM: [f32; 3] = [0x4A as f32, 0x30 as f32, 0x22 as f32];
const CREAM: [f32; 3] = [0xFB as f32, 0xF3 as f32, 0xE8 as f32];
const COFFEE: [f32; 3] = [0x3B as f32, 0x23 as f32, 0x16 as f32];
const CREMA: [f32; 3] = [0xE8 as f32, 0xAE as f32, 0x74 as f32];

/// Renders the glyph as `size`×`size` RGBA, rows top to bottom.
pub fn tray_glyph(size: u32, style: GlyphStyle, fill: GlyphFill) -> Vec<u8> {
    let (sweep, ring_alpha) = match fill {
        GlyphFill::Mark => (MARK_SWEEP, 1.0),
        GlyphFill::Progress { fraction, paused } => {
            let fraction = if fraction.is_finite() { fraction } else { 0.0 };
            (fraction.clamp(0.0, 1.0), if paused { 0.45 } else { 1.0 })
        }
    };

    let show_track = matches!(fill, GlyphFill::Progress { .. });
    let scale = size as f32 / GRID;
    let mut pixels = vec![0u8; (size * size * 4) as usize];

    for y in 0..size {
        for x in 0..size {
            let px = (x as f32 + 0.5) / scale;
            let py = (y as f32 + 0.5) / scale;
            let rgba = match style {
                GlyphStyle::Template => {
                    template_pixel(px, py, scale, sweep, ring_alpha, show_track)
                }
                GlyphStyle::Tile => tile_pixel(px, py, scale, sweep, ring_alpha),
            };
            let index = ((y * size + x) * 4) as usize;
            pixels[index..index + 4].copy_from_slice(&rgba);
        }
    }
    pixels
}

// --- the two looks ---------------------------------------------------------

fn template_pixel(
    px: f32,
    py: f32,
    scale: f32,
    sweep: f32,
    ring_alpha: f32,
    show_track: bool,
) -> [u8; 4] {
    // A cup outline with a handle, and the ring inside it.
    let (cx, cy) = (10.0, 12.0);
    let rim = sd_annulus(px, py, cx, cy, 6.6, 0.95);
    let handle = handle(px, py, cx, cy, 7.0, 11.4, 1.35);
    let cup = coverage(rim.min(handle), scale);
    let ring = coverage(sd_arc(px, py, cx, cy, 3.3, 1.15, sweep), scale) * ring_alpha;
    // Behind a session's ring, a faint track shows the whole of it.
    let track = if show_track {
        coverage(sd_annulus(px, py, cx, cy, 3.3, 1.15), scale) * 0.28
    } else {
        0.0
    };

    let alpha = cup.max(ring).max(track);
    [0, 0, 0, to_byte(alpha * 255.0)]
}

fn tile_pixel(px: f32, py: f32, scale: f32, sweep: f32, ring_alpha: f32) -> [u8; 4] {
    let tile = coverage(sd_round_box(px, py, 12.0, 12.0, 12.0, 12.0, 5.4), scale);
    if tile <= 0.0 {
        return [0, 0, 0, 0];
    }

    let (cx, cy) = (11.2, 12.3);
    let mut color = mix(MOCHA_TOP, MOCHA_BOTTOM, (py / GRID).clamp(0.0, 1.0));

    let cup = sd_circle(px, py, cx, cy, 7.9).min(handle(px, py, cx, cy, 7.0, 10.9, 1.6));
    color = mix(color, CREAM, coverage(cup, scale));
    color = mix(
        color,
        COFFEE,
        coverage(sd_circle(px, py, cx, cy, 6.1), scale),
    );

    let track = coverage(sd_annulus(px, py, cx, cy, 3.7, 1.3), scale);
    color = mix(color, CREMA, track * 0.18);
    let ring = coverage(sd_arc(px, py, cx, cy, 3.7, 1.3, sweep), scale) * ring_alpha;
    color = mix(color, CREMA, ring);

    [
        to_byte(color[0]),
        to_byte(color[1]),
        to_byte(color[2]),
        to_byte(tile * 255.0),
    ]
}

/// The handle points out to the right, level, as on the app icon. Tilted, it
/// turned the outline into a magnifying glass.
fn handle(px: f32, py: f32, cx: f32, cy: f32, from: f32, to: f32, radius: f32) -> f32 {
    let (sin, cos) = (0.0f32, 1.0f32);
    sd_capsule(
        px,
        py,
        (cx + cos * from, cy + sin * from),
        (cx + cos * to, cy + sin * to),
        radius,
    )
}

// --- signed distance fields --------------------------------------------------

fn sd_circle(px: f32, py: f32, cx: f32, cy: f32, radius: f32) -> f32 {
    (px - cx).hypot(py - cy) - radius
}

fn sd_annulus(px: f32, py: f32, cx: f32, cy: f32, radius: f32, half_width: f32) -> f32 {
    ((px - cx).hypot(py - cy) - radius).abs() - half_width
}

fn sd_round_box(px: f32, py: f32, cx: f32, cy: f32, hx: f32, hy: f32, r: f32) -> f32 {
    let qx = (px - cx).abs() - hx + r;
    let qy = (py - cy).abs() - hy + r;
    qx.max(0.0).hypot(qy.max(0.0)) + qx.max(qy).min(0.0) - r
}

fn sd_capsule(px: f32, py: f32, a: (f32, f32), b: (f32, f32), r: f32) -> f32 {
    let (pax, pay) = (px - a.0, py - a.1);
    let (bax, bay) = (b.0 - a.0, b.1 - a.1);
    let h = ((pax * bax + pay * bay) / (bax * bax + bay * bay)).clamp(0.0, 1.0);
    (pax - bax * h).hypot(pay - bay * h) - r
}

/// A ring with round ends, starting at twelve o'clock and running clockwise
/// for `sweep` of a turn. A sweep of zero is a single dot at the top: the
/// session has started, and nothing is done yet.
fn sd_arc(px: f32, py: f32, cx: f32, cy: f32, radius: f32, half_width: f32, sweep: f32) -> f32 {
    use std::f32::consts::TAU;

    let (dx, dy) = (px - cx, py - cy);
    if sweep >= 1.0 {
        return (dx.hypot(dy) - radius).abs() - half_width;
    }
    // Clockwise from the top, in screen coordinates where y points down.
    let angle = dx.atan2(-dy).rem_euclid(TAU);
    let end = sweep * TAU;
    if angle <= end {
        return (dx.hypot(dy) - radius).abs() - half_width;
    }
    let cap = |a: f32| {
        let (sin, cos) = a.sin_cos();
        (dx - sin * radius).hypot(dy + cos * radius) - half_width
    };
    cap(0.0).min(cap(end))
}

// --- helpers -----------------------------------------------------------------

/// Distance in grid units to pixel coverage, antialiased across one pixel.
fn coverage(distance: f32, scale: f32) -> f32 {
    (0.5 - distance * scale).clamp(0.0, 1.0)
}

fn mix(a: [f32; 3], b: [f32; 3], t: f32) -> [f32; 3] {
    [
        a[0] + (b[0] - a[0]) * t,
        a[1] + (b[1] - a[1]) * t,
        a[2] + (b[2] - a[2]) * t,
    ]
}

fn to_byte(value: f32) -> u8 {
    value.round().clamp(0.0, 255.0) as u8
}

#[cfg(test)]
mod tests {
    use super::*;

    const SIZE: u32 = 48;

    fn alpha(pixels: &[u8], x: u32, y: u32) -> u8 {
        pixels[((y * SIZE + x) * 4 + 3) as usize]
    }

    /// A pixel on the ring, `turn` of the way round clockwise from the top.
    fn ring_point(turn: f32) -> (u32, u32) {
        let scale = SIZE as f32 / GRID;
        let angle = turn * std::f32::consts::TAU;
        let x = (10.0 + angle.sin() * 3.3) * scale;
        let y = (12.0 - angle.cos() * 3.3) * scale;
        (x as u32, y as u32)
    }

    #[test]
    fn output_is_square_rgba() {
        for style in [GlyphStyle::Template, GlyphStyle::Tile] {
            let pixels = tray_glyph(SIZE, style, GlyphFill::Mark);
            assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);
        }
    }

    #[test]
    fn template_is_pure_black_with_transparent_corners() {
        let pixels = tray_glyph(SIZE, GlyphStyle::Template, GlyphFill::Mark);
        assert!(pixels.chunks(4).all(|p| p[..3] == [0, 0, 0]));
        assert_eq!(alpha(&pixels, 0, 0), 0);
        assert_eq!(alpha(&pixels, SIZE - 1, SIZE - 1), 0);
        // And it draws something.
        assert!(pixels.chunks(4).filter(|p| p[3] == 255).count() > 100);
    }

    #[test]
    fn tile_is_opaque_in_the_middle_and_rounded_at_the_corners() {
        let pixels = tray_glyph(SIZE, GlyphStyle::Tile, GlyphFill::Mark);
        assert_eq!(alpha(&pixels, SIZE / 2, SIZE / 2), 255);
        assert_eq!(alpha(&pixels, 0, 0), 0);
    }

    #[test]
    fn the_ring_fills_clockwise_from_the_top() {
        let quarter = tray_glyph(
            SIZE,
            GlyphStyle::Template,
            GlyphFill::Progress {
                fraction: 0.3,
                paused: false,
            },
        );
        let (x, y) = ring_point(0.15);
        assert!(
            alpha(&quarter, x, y) > 200,
            "a fifth of the way round is drawn"
        );
        let (x, y) = ring_point(0.6);
        assert!(
            alpha(&quarter, x, y) < 100,
            "past the end is only the faint track"
        );
    }

    #[test]
    fn a_paused_ring_is_dimmed() {
        let fill = |paused| GlyphFill::Progress {
            fraction: 0.5,
            paused,
        };
        let running = tray_glyph(SIZE, GlyphStyle::Template, fill(false));
        let paused = tray_glyph(SIZE, GlyphStyle::Template, fill(true));
        let (x, y) = ring_point(0.25);
        assert!(alpha(&paused, x, y) < alpha(&running, x, y));
    }

    #[test]
    fn nonsense_progress_is_clamped_rather_than_drawn() {
        let draw = |fraction| {
            tray_glyph(
                SIZE,
                GlyphStyle::Template,
                GlyphFill::Progress {
                    fraction,
                    paused: false,
                },
            )
        };
        assert_eq!(draw(f32::NAN), draw(0.0));
        assert_eq!(draw(-3.0), draw(0.0));
        assert_eq!(draw(7.0), draw(1.0));
    }
}
