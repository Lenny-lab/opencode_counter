//! Renders the notification-area icon.
//!
//! macOS has a menu bar wide enough to print a figure as text, so the
//! original app simply writes "1.2M · $0.42" next to its icon. Windows has no
//! such affordance: a tray icon is 16 logical pixels square. Rather than ship
//! an unreadable smear of text, the count is rasterised with a hand-drawn 3x5
//! bitmap font that stays legible when Windows scales the 32x32 buffer down,
//! and the full figures go in the hover tooltip.
//!
//! Keeping the font in-tree means no font-parsing dependency and no asset
//! loading: 13 glyphs is not worth a crate.

use tauri::image::Image;

/// Edge length of the rendered icon, before Windows scales it.
const SIZE: u32 = 32;
/// Pixels per font pixel. Two keeps the 3x5 glyphs on even pixel boundaries
/// once the 32px buffer is halved to the 16px tray slot.
const SCALE: usize = 2;
const GLYPH_W: usize = 3;
const GLYPH_H: usize = 5;
const GAP: usize = 1;
/// Digits we are willing to cram into a 32px square.
const MAX_GLYPHS: usize = 4;

const INK: [u8; 4] = [0xEE, 0xF2, 0xFF, 0xFF];
const PLATE: [u8; 4] = [0x0A, 0x10, 0x20, 0xF0];
const EDGE: [u8; 4] = [0x2A, 0x35, 0x50, 0xFF];

/// 3x5 glyphs, one byte per row, most significant of the low three bits is the
/// leftmost column. Anything unlisted renders as a blank.
fn glyph(c: char) -> [u8; GLYPH_H] {
    match c {
        '0' => [0b111, 0b101, 0b101, 0b101, 0b111],
        '1' => [0b010, 0b110, 0b010, 0b010, 0b111],
        '2' => [0b111, 0b001, 0b111, 0b100, 0b111],
        '3' => [0b111, 0b001, 0b111, 0b001, 0b111],
        '4' => [0b101, 0b101, 0b111, 0b001, 0b001],
        '5' => [0b111, 0b100, 0b111, 0b001, 0b111],
        '6' => [0b111, 0b100, 0b111, 0b101, 0b111],
        '7' => [0b111, 0b001, 0b001, 0b001, 0b001],
        '8' => [0b111, 0b101, 0b111, 0b101, 0b111],
        '9' => [0b111, 0b101, 0b111, 0b001, 0b111],
        '.' => [0b000, 0b000, 0b000, 0b000, 0b010],
        '-' => [0b000, 0b000, 0b111, 0b000, 0b000],
        'K' => [0b110, 0b101, 0b110, 0b101, 0b101],
        // The shoulder row is what separates M from H; a filled middle would
        // read as a heavy H instead.
        'M' => [0b101, 0b111, 0b101, 0b101, 0b101],
        'B' => [0b110, 0b101, 0b110, 0b101, 0b110],
        'T' => [0b111, 0b010, 0b010, 0b010, 0b010],
        _ => [0; GLYPH_H],
    }
}

/// Today's total tokens in the shortest form that still carries a magnitude.
pub fn token_label(tokens: i64) -> String {
    let value = tokens.max(0) as f64;
    if value < 1e3 {
        return format!("{}", value.round() as i64);
    }
    // The largest unit that leaves a mantissa of at least one. The next unit
    // up has already lost, so `scaled` is always under 1000.
    let (divisor, suffix) = if value >= 1e9 {
        (1e9, 'B')
    } else if value >= 1e6 {
        (1e6, 'M')
    } else {
        (1e3, 'K')
    };
    let scaled = value / divisor;
    if scaled < 9.95 {
        return format!("{scaled:.1}{suffix}");
    }
    if scaled < 999.5 {
        return format!("{scaled:.0}{suffix}");
    }
    // The one mantissa that would outgrow the plate: 999.6K is a five-glyph
    // "1000K". Carry into the next unit and let the decimal form say "1.0M".
    let (divisor, suffix) = match suffix {
        'B' => (1e12, 'T'),
        'M' => (1e9, 'B'),
        _ => (1e6, 'M'),
    };
    format!("{:.1}{}", value / divisor, suffix)
}

/// Hover text. Windows caps tooltips around 128 characters, which is far more
/// than this needs.
pub fn tooltip(tokens: i64, cost: f64) -> String {
    format!("OpenCode Stats — today {} tokens · ${cost:.2}", token_label(tokens))
}

/// Paint `text` onto a rounded dark plate and return it as a tray-ready image.
pub fn render(text: &str) -> Image<'static> {
    let mut buf = vec![0u8; (SIZE * SIZE * 4) as usize];
    let n = SIZE as usize;

    for y in 0..n {
        for x in 0..n {
            let inside = rounded_corners(n, x, y);
            let colour = if inside >= 2 {
                EDGE
            } else if inside == 1 {
                PLATE
            } else {
                continue;
            };
            blend(&mut buf, x, y, colour);
        }
    }

    let chars: Vec<char> = text.chars().take(MAX_GLYPHS).collect();
    let text_w = chars.len() * (GLYPH_W * SCALE) + chars.len().saturating_sub(1) * GAP;
    let origin_x = (n.saturating_sub(text_w)) / 2;
    let origin_y = (n.saturating_sub(GLYPH_H * SCALE)) / 2;
    for (index, c) in chars.iter().enumerate() {
        let rows = glyph(*c);
        let left = origin_x + index * (GLYPH_W * SCALE + GAP);
        for (gy, row) in rows.iter().enumerate() {
            for gx in 0..GLYPH_W {
                if row & (0b100 >> gx) == 0 {
                    continue;
                }
                for sy in 0..SCALE {
                    for sx in 0..SCALE {
                        let x = left + gx * SCALE + sx;
                        let y = origin_y + gy * SCALE + sy;
                        if x < n && y < n {
                            blend(&mut buf, x, y, INK);
                        }
                    }
                }
            }
        }
    }

    Image::new_owned(buf, SIZE, SIZE)
}

/// Alpha coverage of a rounded square: 0 outside, 1 on the border ring, 2 in
/// the body. A cheap distance test beats pulling in a rasteriser for 1024
/// pixels.
fn rounded_corners(n: usize, x: usize, y: usize) -> u8 {
    let radius = 7i32;
    let limit = n as i32 - 1;
    let px = x as i32;
    let py = y as i32;
    let cx = px.clamp(radius, limit - radius);
    let cy = py.clamp(radius, limit - radius);
    let dx = px - cx;
    let dy = py - cy;
    if dx * dx + dy * dy > radius * radius {
        0
    } else if px < 1 || py < 1 || px >= limit || py >= limit {
        1
    } else {
        2
    }
}

fn blend(buf: &mut [u8], x: usize, y: usize, colour: [u8; 4]) {
    let index = (y * SIZE as usize + x) * 4;
    let alpha = colour[3] as u32;
    for channel in 0..3 {
        let existing = buf[index + channel] as u32;
        let mixed = (colour[channel] as u32 * alpha + existing * (255 - alpha)) / 255;
        buf[index + channel] = mixed as u8;
    }
    buf[index + 3] = buf[index + 3].max(colour[3]);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn token_labels_stay_within_four_glyphs() {
        for value in [0, 7, 999, 1000, 4321, 12_300, 999_999, 1_000_000, 12_345_678, 890_000_000] {
            let label = token_label(value);
            assert!(label.chars().count() <= MAX_GLYPHS, "{value} -> {label}");
            assert!(label.chars().all(|c| glyph(c) != [0; GLYPH_H]), "{label}");
        }
    }

    #[test]
    fn magnitudes_are_preserved() {
        assert_eq!(token_label(999), "999");
        assert_eq!(token_label(8_900), "8.9K");
        assert_eq!(token_label(12_300), "12K");
        assert_eq!(token_label(890_000), "890K");
        assert_eq!(token_label(999_999), "1.0M");
        assert_eq!(token_label(1_200_000), "1.2M");
        assert_eq!(token_label(890_000_000), "890M");
        assert_eq!(token_label(999_999_999), "1.0B");
        assert_eq!(token_label(2_500_000_000), "2.5B");
    }

    #[test]
    fn negative_totals_render_as_zero() {
        assert_eq!(token_label(-5), "0");
    }

    #[test]
    fn render_produces_an_opaque_plate() {
        let image = render("1.2M");
        assert_eq!(image.width(), SIZE);
        assert_eq!(image.height(), SIZE);
        let pixels = image.rgba();
        assert_eq!(pixels.len(), (SIZE * SIZE * 4) as usize);
        let at = |x: u32, y: u32| (y * SIZE + x) as usize * 4;
        let centre = at(SIZE / 2, SIZE / 2);
        assert_eq!(pixels[centre + 3], 0xFF, "the plate must be opaque");
        assert_eq!(pixels[0 + 3], 0x00, "the corners stay transparent");
    }
}
