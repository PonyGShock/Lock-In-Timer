#!/usr/bin/env python3
"""Draw Lock In's icons from scratch.

Everything is described as signed distance fields, so one sample per pixel is
enough for clean edges and the whole thing runs on a stock Python with no
imaging library installed. Run it after changing the mark:

    python3 scripts/make_icons.py

It writes the menu bar template glyph and the 1024px source image that
`npm run icons` turns into the platform icon set.
"""

import math
import struct
import zlib
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent

ESPRESSO = (0x3A, 0x2B, 0x21)
CARAMEL = (0xC0, 0x85, 0x52)
CREAM_TOP = (0xFF, 0xFC, 0xF7)
CREAM_BOTTOM = (0xF2, 0xE6, 0xD6)

# The mark is designed on a 24x24 grid and scaled from there.
GRID = 24.0


# --- signed distance fields -------------------------------------------------


def sd_round_box(px, py, cx, cy, hx, hy, r):
    qx = abs(px - cx) - hx + r
    qy = abs(py - cy) - hy + r
    outside = math.hypot(max(qx, 0.0), max(qy, 0.0))
    return outside + min(max(qx, qy), 0.0) - r


def sd_annulus(px, py, cx, cy, radius, half_width):
    return abs(math.hypot(px - cx, py - cy) - radius) - half_width


def sd_capsule(px, py, ax, ay, bx, by, r):
    pax, pay = px - ax, py - ay
    bax, bay = bx - ax, by - ay
    denom = bax * bax + bay * bay
    h = 0.0 if denom == 0 else max(0.0, min(1.0, (pax * bax + pay * bay) / denom))
    return math.hypot(pax - bax * h, pay - bay * h) - r


def union(*distances):
    return min(distances)


def intersect(a, b):
    return max(a, b)


def coverage(distance, scale):
    """Distance in grid units to pixel coverage, antialiased over one pixel."""
    return max(0.0, min(1.0, 0.5 - distance * scale))


# --- the mark ---------------------------------------------------------------


def mug(px, py):
    """A mug with a handle, sitting on a saucer."""
    body = sd_round_box(px, py, 10.8, 12.0, 5.25, 6.0, 2.4)
    handle = intersect(
        sd_annulus(px, py, 16.9, 11.4, 3.1, 0.85),
        # Trim the half of the ring that would sit inside the mug.
        -(px - 15.6),
    )
    saucer = sd_round_box(px, py, 12.0, 19.75, 9.0, 0.85, 0.85)
    return union(body, handle, saucer)


def steam(px, py):
    # All three lean the same way, tallest in the middle. Mixed directions
    # read as a chevron rather than as steam.
    wisps = (
        (8.6, 4.9, 9.5, 2.6, 0.68),
        (11.3, 4.6, 12.3, 1.3, 0.68),
        (14.0, 4.9, 15.0, 2.6, 0.68),
    )
    return union(*(sd_capsule(px, py, *wisp) for wisp in wisps))


# --- output -----------------------------------------------------------------


def write_png(path, width, height, pixels):
    """pixels: a flat bytearray of RGBA rows."""
    stride = width * 4
    raw = bytearray()
    for y in range(height):
        raw.append(0)  # no per-scanline filter
        raw.extend(pixels[y * stride : (y + 1) * stride])

    def chunk(tag, data):
        body = tag + data
        return struct.pack(">I", len(data)) + body + struct.pack(">I", zlib.crc32(body))

    header = struct.pack(">IIBBBBB", width, height, 8, 6, 0, 0, 0)
    png = (
        b"\x89PNG\r\n\x1a\n"
        + chunk(b"IHDR", header)
        + chunk(b"IDAT", zlib.compress(bytes(raw), 9))
        + chunk(b"IEND", b"")
    )
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_bytes(png)
    print(f"wrote {path.relative_to(ROOT)} ({width}x{height})")


def blend(base, layer, alpha):
    return tuple(round(b + (l - b) * alpha) for b, l in zip(base, layer))


def render_tray(size=44):
    """Monochrome glyph on transparency, as macOS template images require."""
    pixels = bytearray(size * size * 4)
    # Inset slightly so the mark does not touch the menu bar edges.
    scale = size / GRID * 0.92
    offset = (size - GRID * scale) / 2

    for y in range(size):
        for x in range(size):
            px = (x + 0.5 - offset) / scale
            py = (y + 0.5 - offset) / scale
            alpha = coverage(mug(px, py), scale)
            index = (y * size + x) * 4
            pixels[index : index + 4] = bytes((0, 0, 0, round(alpha * 255)))

    write_png(ROOT / "src-tauri" / "assets" / "tray.png", size, size, pixels)


def render_app_icon(size=1024):
    pixels = bytearray(size * size * 4)
    # Apple's squircle is close to a 22.37% corner radius on a full-bleed tile.
    radius = size * 0.2237
    half = size / 2.0

    glyph_scale = size / GRID * 0.60
    glyph_offset_x = (size - GRID * glyph_scale) / 2
    # Nudge down so the steam has room and the mass sits optically centred.
    glyph_offset_y = glyph_offset_x + size * 0.035

    for y in range(size):
        row_mix = y / (size - 1)
        background = tuple(
            round(a + (b - a) * row_mix) for a, b in zip(CREAM_TOP, CREAM_BOTTOM)
        )
        for x in range(size):
            px, py = x + 0.5, y + 0.5
            tile = coverage(sd_round_box(px, py, half, half, half, half, radius), 1.0)
            if tile <= 0.0:
                continue

            gx = (px - glyph_offset_x) / glyph_scale
            gy = (py - glyph_offset_y) / glyph_scale

            color = background
            steam_alpha = coverage(steam(gx, gy), glyph_scale)
            if steam_alpha > 0.0:
                color = blend(color, CARAMEL, steam_alpha)
            mug_alpha = coverage(mug(gx, gy), glyph_scale)
            if mug_alpha > 0.0:
                color = blend(color, ESPRESSO, mug_alpha)

            index = (y * size + x) * 4
            pixels[index : index + 4] = bytes((*color, round(tile * 255)))

    write_png(ROOT / "app-icon.png", size, size, pixels)


if __name__ == "__main__":
    render_tray()
    render_app_icon()
