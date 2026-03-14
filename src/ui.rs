/// Status bar renderer — pure pixel operations, no external font dependency.
/// Renders into an ARGB8888 byte buffer of size BAR_W × BAR_H.

pub const BAR_W: usize = 292;
pub const BAR_H: usize = 18;

// ── Embedded 3×5 pixel font (digits 0-9) ──────────────────────────────────────
// Each glyph is 5 bytes; bit 7 = left pixel, bit 6 = mid, bit 5 = right.
const DIGITS: [[u8; 5]; 10] = [
    [0xE0, 0xA0, 0xA0, 0xA0, 0xE0], // 0: ███ █·█ █·█ █·█ ███
    [0x40, 0xC0, 0x40, 0x40, 0xE0], // 1: ·█· ██· ·█· ·█· ███
    [0xC0, 0x20, 0x40, 0x80, 0xE0], // 2: ██· ··█ ·█· █·· ███
    [0xE0, 0x20, 0x60, 0x20, 0xE0], // 3: ███ ··█ ·██ ··█ ███
    [0xA0, 0xA0, 0xE0, 0x20, 0x20], // 4: █·█ █·█ ███ ··█ ··█
    [0xE0, 0x80, 0xE0, 0x20, 0xC0], // 5: ███ █·· ███ ··█ ██·
    [0x60, 0x80, 0xE0, 0xA0, 0xE0], // 6: ·██ █·· ███ █·█ ███
    [0xE0, 0x20, 0x40, 0x40, 0x40], // 7: ███ ··█ ·█· ·█· ·█·
    [0xE0, 0xA0, 0xE0, 0xA0, 0xE0], // 8: ███ █·█ ███ █·█ ███
    [0xE0, 0xA0, 0xE0, 0x20, 0xC0], // 9: ███ █·█ ███ ··█ ██·
];

// ── Helpers ────────────────────────────────────────────────────────────────────

#[inline]
fn px(buf: &mut [u8], x: usize, y: usize, r: u8, g: u8, b: u8) {
    if x >= BAR_W || y >= BAR_H { return; }
    let o = (y * BAR_W + x) * 4;
    buf[o] = b; buf[o + 1] = g; buf[o + 2] = r; buf[o + 3] = 255;
}

fn fill_rect(buf: &mut [u8], x: usize, y: usize, w: usize, h: usize, r: u8, g: u8, b: u8) {
    for dy in 0..h {
        for dx in 0..w {
            px(buf, x + dx, y + dy, r, g, b);
        }
    }
}

fn draw_digit_at(buf: &mut [u8], x: usize, y: usize, d: usize, r: u8, g: u8, b: u8) {
    for (row, &bits) in DIGITS[d % 10].iter().enumerate() {
        for col in 0..3usize {
            if bits & (0x80 >> col) != 0 {
                px(buf, x + col, y + row, r, g, b);
            }
        }
    }
}

// ── Button indicator ───────────────────────────────────────────────────────────

fn indicator(buf: &mut [u8], x: usize, y: usize, w: usize, h: usize,
             on: bool, r: u8, g: u8, b: u8) {
    // Lit = full color, unlit = very dark version
    let (dr, dg, db) = if on { (r, g, b) } else { (r / 6, g / 6, b / 6) };
    fill_rect(buf, x, y, w, h, dr, dg, db);
    // Bright 1-pixel highlight on top-left corner when lit
    if on {
        let hr = r.saturating_add(60);
        let hg = g.saturating_add(60);
        let hb = b.saturating_add(60);
        px(buf, x, y, hr, hg, hb);
    }
}

// ── Public render function ─────────────────────────────────────────────────────

/// Fill `buf` (ARGB8888, BAR_W × BAR_H) with the status bar for this frame.
pub fn render_status(buf: &mut [u8], buttons: u8, fps: f32) {
    // ── Background ─────────────────────────────────────────────────────────────
    for i in 0..BAR_W * BAR_H {
        let o = i * 4;
        buf[o] = 22; buf[o + 1] = 16; buf[o + 2] = 16; buf[o + 3] = 255;
    }

    // Top separator line (slightly brighter)
    for x in 0..BAR_W {
        px(buf, x, 0, 80, 60, 60);
    }

    let cy = (BAR_H - 5) / 2 + 1; // vertical center row for 5-tall glyphs

    // ── D-pad (left, 9×9 cross) ────────────────────────────────────────────────
    let dpx = 4usize;
    let dpy = (BAR_H - 9) / 2;

    let up    = buttons & 0x10 != 0;
    let down  = buttons & 0x20 != 0;
    let left  = buttons & 0x40 != 0;
    let right = buttons & 0x80 != 0;

    indicator(buf, dpx + 3, dpy + 0, 3, 3, up,    80, 110, 220);
    indicator(buf, dpx + 3, dpy + 6, 3, 3, down,  80, 110, 220);
    indicator(buf, dpx + 0, dpy + 3, 3, 3, left,  80, 110, 220);
    indicator(buf, dpx + 6, dpy + 3, 3, 3, right, 80, 110, 220);
    fill_rect(buf, dpx + 3, dpy + 3, 3, 3, 45, 45, 70); // center pip (always dim)

    // ── Select / Start (small rectangular pads) ────────────────────────────────
    let sel_on   = buttons & 0x04 != 0;
    let start_on = buttons & 0x08 != 0;

    let ss_x = dpx + 9 + 5;
    let ss_y = cy + 1; // slightly below center for a "sunken" look

    indicator(buf, ss_x,      ss_y, 7, 3, sel_on,   150, 150, 150); // Select (grey)
    indicator(buf, ss_x + 10, ss_y, 7, 3, start_on, 230, 210, 120); // Start  (gold)

    // Labels: two small dots above each pad (classic NES oval style)
    for i in 0..2usize {
        let lx = ss_x + 2 + i * 2;
        let ldx = ss_x + 10 + 2 + i * 2;
        let ly = ss_y.saturating_sub(2);
        if sel_on   { px(buf, lx,  ly, 200, 200, 200); }
        else        { px(buf, lx,  ly,  40,  40,  40); }
        if start_on { px(buf, ldx, ly, 255, 240, 140); }
        else        { px(buf, ldx, ly,  40,  40,  30); }
    }

    // ── FPS counter (center) ───────────────────────────────────────────────────
    let fps_val = (fps.round() as u32).min(999);
    // Right-align 3 digits ending at center
    let fps_end_x = BAR_W / 2 + 6;
    let s = format!("{:3}", fps_val);
    let mut cx = fps_end_x;
    for ch in s.chars().rev() {
        if cx < 4 { break; }
        cx -= 4;
        if let Some(d) = ch.to_digit(10) {
            let (r, g, b) = fps_color(fps_val);
            draw_digit_at(buf, cx, cy, d as usize, r, g, b);
        }
    }
    // Tiny "FPS" suffix using 3×3 abbreviation dots (just two pixels: "Hz")
    // Instead of text, draw a small tick mark to the right of the number
    px(buf, fps_end_x + 1, cy + 4, 90, 130, 60);
    px(buf, fps_end_x + 2, cy + 3, 90, 130, 60);
    px(buf, fps_end_x + 3, cy + 2, 90, 130, 60);

    // ── A / B buttons (right side, circular approximation) ────────────────────
    let a_on = buttons & 0x01 != 0;
    let b_on = buttons & 0x02 != 0;

    let ab_y = (BAR_H - 7) / 2;
    let a_x  = BAR_W - 10;
    let b_x  = BAR_W - 19;

    // Draw 5×5 "circle" for each button (1px border mask)
    draw_circle_button(buf, a_x, ab_y, a_on, 220,  60,  60); // A = red
    draw_circle_button(buf, b_x, ab_y, b_on,  60, 200, 100); // B = green
}

/// Draw a 5×5 rounded button (trimmed corners for a circular look).
fn draw_circle_button(buf: &mut [u8], x: usize, y: usize, on: bool, r: u8, g: u8, b: u8) {
    // 5×5 mask: trim the 4 corners
    const MASK: [[bool; 5]; 5] = [
        [false, true,  true,  true,  false],
        [true,  true,  true,  true,  true ],
        [true,  true,  true,  true,  true ],
        [true,  true,  true,  true,  true ],
        [false, true,  true,  true,  false],
    ];
    let (dr, dg, db) = if on { (r, g, b) } else { (r / 6, g / 6, b / 6) };
    for (dy, row) in MASK.iter().enumerate() {
        for (dx, &filled) in row.iter().enumerate() {
            if filled {
                let mut pr = dr;
                let mut pg = dg;
                let mut pb = db;
                // Highlight top row when lit
                if on && dy == 0 {
                    pr = r.saturating_add(50);
                    pg = g.saturating_add(50);
                    pb = b.saturating_add(50);
                }
                px(buf, x + dx, y + dy, pr, pg, pb);
            }
        }
    }
}

/// Returns an RGB color for the FPS counter based on performance.
fn fps_color(fps: u32) -> (u8, u8, u8) {
    if fps >= 58 { (100, 210, 80)  } // green — good
    else if fps >= 45 { (220, 180, 60) } // yellow — ok
    else { (220, 70, 70) }            // red — slow
}
