// Copyright 2026 The tokenwhy contributors <tokenwhy@dryaides.com>
// SPDX-License-Identifier: Apache-2.0
// Redistributions must keep the NOTICE file and credit tokenwhy.

//! SVG text never wraps by itself, so text is measured with the embedded font and wrapped or shrunk here.

use crate::cards::{Card, Viz};
use std::fmt::Write;
use std::sync::OnceLock;

pub const W: f32 = 1080.0;
pub const H: f32 = 1350.0;
const LEFT: f32 = 120.0;
const TEXT_W: f32 = 840.0;

#[derive(Debug, Clone, PartialEq)]
pub struct Palette {
    pub plane: String,
    pub card: String,
    pub fg: String,
    pub fg2: String,
    pub muted: String,
    pub rule: String,
    pub accent: String,
    pub accent_soft: String,
    pub accent_ink: String,
    pub series: [String; 7],
}

impl Palette {
    fn of(v: [&str; 16]) -> Palette {
        let s = |i: usize| v[i].to_string();
        Palette {
            plane: s(0),
            card: s(1),
            fg: s(2),
            fg2: s(3),
            muted: s(4),
            rule: s(5),
            accent: s(6),
            accent_soft: s(7),
            accent_ink: s(8),
            series: std::array::from_fn(|i| s(9 + i)),
        }
    }

    /// theme.css `:root`.
    pub fn light() -> Palette {
        Palette::of([
            "#f4f4f2", "#ffffff", "#16161a", "#4d4d55", "#6a6a72", "#e6e6e2", "#256bbf", "#e6effb",
            "#ffffff", "#2a78d6", "#eb6834", "#1baf7a", "#eda100", "#e87ba4", "#6250d6", "#008300",
        ])
    }

    /// theme.css dark block.
    pub fn dark() -> Palette {
        Palette::of([
            "#0f0f11", "#18181b", "#f0f0ee", "#c4c4c8", "#8d8d96", "#26262b", "#4f95ea", "#172538",
            "#0b1322", "#3987e5", "#d95926", "#199e70", "#c98500", "#d55181", "#9085e9", "#008300",
        ])
    }
}

struct Fonts {
    regular: ttf_parser::Face<'static>,
    heavy: ttf_parser::Face<'static>,
}

fn fonts() -> &'static Fonts {
    static F: OnceLock<Fonts> = OnceLock::new();
    F.get_or_init(|| Fonts {
        regular: ttf_parser::Face::parse(crate::FONT_REGULAR, 0).expect("embedded font"),
        heavy: ttf_parser::Face::parse(crate::FONT_HEAVY, 0).expect("embedded font"),
    })
}

pub fn width(s: &str, size: f32, heavy: bool) -> f32 {
    let f = fonts();
    let face = if heavy { &f.heavy } else { &f.regular };
    let units: u32 = s
        .chars()
        .map(|c| {
            face.glyph_index(c)
                .and_then(|g| face.glyph_hor_advance(g))
                .map_or(0, u32::from)
        })
        .sum();
    units as f32 * size / f32::from(face.units_per_em())
}

pub fn wrap(s: &str, size: f32, max: f32) -> Vec<String> {
    let mut lines: Vec<String> = vec![];
    for para in s.split('\n') {
        let first = lines.len();
        for word in para.split(' ') {
            let open = lines.len() > first;
            match lines.last_mut() {
                Some(l) if open && width(&format!("{l} {word}"), size, false) <= max => {
                    l.push(' ');
                    l.push_str(word);
                }
                _ => lines.push(word.to_string()),
            }
        }
    }
    lines
}

fn fit(s: &str, start: f32, max: f32, heavy: bool) -> f32 {
    let w = width(s, start, heavy);
    if w <= max {
        start
    } else {
        (start * max / w).floor()
    }
}

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
}

struct Svg(String);

impl Svg {
    #[allow(clippy::too_many_arguments)]
    fn text(
        &mut self,
        id: &str,
        x: f32,
        y: f32,
        size: f32,
        heavy: bool,
        fill: &str,
        anchor: &str,
        s: &str,
    ) {
        let weight = if heavy { 700 } else { 400 };
        let _ = write!(
            self.0,
            r#"<text id="{id}" x="{x:.1}" y="{y:.1}" font-family="twcard" font-weight="{weight}" font-size="{size:.1}" fill="{fill}" text-anchor="{anchor}">{}</text>"#,
            esc(s)
        );
    }
    fn rect(&mut self, x: f32, y: f32, w: f32, h: f32, r: f32, fill: &str) {
        let _ = write!(
            self.0,
            r#"<rect x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" rx="{r:.1}" fill="{fill}"/>"#
        );
    }
}

/// The unit is a tspan of the same text so the gap holds whatever font the viewer draws with.
fn figure(s: &mut Svg, id: &str, y: f32, start: f32, big: &str, unit: &str, p: &Palette) -> f32 {
    const UNIT_SIZE: f32 = 56.0;
    const GAP: f32 = 24.0;
    let unit_w = if unit.is_empty() {
        0.0
    } else {
        width(unit, UNIT_SIZE, false) + GAP
    };
    let inline = unit_w > 0.0 && unit_w < TEXT_W * 0.35;
    // 8 % slack: the webview's text rendering runs a little wider than the font's advances, and a figure that ends
    // flush with the text column can overflow it.
    let size = fit(big, start, (TEXT_W - if inline { unit_w } else { 0.0 }) * 0.92, true);
    if inline {
        let _ = write!(
            s.0,
            r#"<text id="big{id}" x="{LEFT:.1}" y="{y:.1}" font-family="twcard" font-weight="700" font-size="{size:.1}" fill="{}">{}<tspan id="unit{id}" dx="{GAP:.1}" font-weight="400" font-size="{UNIT_SIZE:.1}" fill="{}"> {}</tspan></text>"#,
            p.fg,
            esc(big),
            p.fg2,
            esc(unit)
        );
        return y;
    }
    s.text(
        &format!("big{id}"),
        LEFT,
        y,
        size,
        true,
        &p.fg,
        "start",
        big,
    );
    if unit.is_empty() {
        return y;
    }
    let y = y + 80.0;
    s.text(
        &format!("unit{id}"),
        LEFT,
        y,
        UNIT_SIZE,
        false,
        &p.fg2,
        "start",
        unit,
    );
    y
}

fn lines(s: &mut Svg, id: &str, mut y: f32, text: &str, p: &Palette) -> f32 {
    const SIZE: f32 = 50.0;
    for (i, l) in wrap(text, SIZE, TEXT_W * 0.92).iter().enumerate() {
        s.text(&format!("{id}{i}"), LEFT, y, SIZE, false, &p.fg, "start", l);
        y += 66.0;
    }
    y
}

pub fn render(card: &Card, n: usize, total: usize, p: &Palette, variant: Option<&str>) -> String {
    let mut s = Svg(String::new());
    let _ = write!(
        s.0,
        r#"<svg xmlns="http://www.w3.org/2000/svg" width="{W}" height="{H}" viewBox="0 0 {W} {H}" role="img"><title>{}</title>"#,
        esc(&card.text())
    );
    s.rect(0.0, 0.0, W, H, 0.0, &p.plane);
    s.rect(48.0, 48.0, W - 96.0, H - 96.0, 56.0, &p.card);
    s.text(
        "kicker",
        LEFT,
        200.0,
        40.0,
        false,
        &p.fg2,
        "start",
        &card.kicker,
    );
    s.text(
        "page",
        W - LEFT,
        200.0,
        34.0,
        false,
        &p.muted,
        "end",
        &format!("{n} / {total}"),
    );

    let poster = matches!(card.viz, Viz::Poster(_));
    let pair = matches!(card.viz, Viz::Figure(..));
    let (big_y, big_start) = if poster {
        (340.0, 120.0)
    } else if pair {
        (440.0, 170.0)
    } else if card.viz != Viz::None {
        (440.0, 240.0)
    } else {
        (600.0, 240.0)
    };
    let mut y = figure(&mut s, "", big_y, big_start, &card.big, &card.unit, p);
    y = lines(&mut s, "line", y + 110.0, &card.line, p);
    if let Viz::Figure(big, unit, line) = &card.viz {
        y = figure(&mut s, "2", y + 170.0, big_start, big, unit, p);
        y = lines(&mut s, "line2_", y + 110.0, line, p);
    }

    let foot_size = 34.0;
    let foot = wrap(&card.foot, foot_size, TEXT_W * 0.92);
    let foot_top = 1150.0 - 46.0 * foot.len().saturating_sub(1) as f32;
    for (i, l) in foot.iter().enumerate() {
        let fy = foot_top + 46.0 * i as f32;
        s.text(
            &format!("foot{i}"),
            LEFT,
            fy,
            foot_size,
            false,
            &p.muted,
            "start",
            l,
        );
    }

    let (top, bottom) = (y + 10.0, foot_top - 90.0);
    match &card.viz {
        Viz::None | Viz::Figure(..) => {}
        Viz::Clock(hours, week) => clock(&mut s, hours, week, top, bottom, p),
        Viz::Bars(rows) => bars(&mut s, rows, top, bottom, p),
        Viz::Poster(tiles) => poster_grid(&mut s, tiles, top, bottom, p),
    }

    let _ = write!(
        s.0,
        r#"<image id="mark" x="{LEFT:.1}" y="{:.1}" width="{MARK:.1}" height="{MARK:.1}" href="data:image/png;base64,{}"/>"#,
        1248.0 - 36.0,
        mark_b64(variant)
    );
    s.text(
        "footer",
        LEFT + MARK + 14.0,
        1248.0,
        36.0,
        false,
        &p.fg2,
        "start",
        "tokenwhy",
    );
    s.0.push_str("</svg>");
    s.0
}

const MARK: f32 = 46.0;

macro_rules! marks {
    ($($v:literal),*) => {
        [$(($v, include_bytes!(concat!("../assets/marks/", $v, ".png")) as &[u8])),*]
    };
}

static MARKS: [(&str, &[u8]); 13] = marks!(
    "light",
    "dark",
    "neu",
    "neu-dark",
    "clay",
    "clay-dark",
    "glass",
    "green",
    "green-dark",
    "orange",
    "orange-dark",
    "pastel",
    "pastel-dark"
);

fn mark_b64(variant: Option<&str>) -> &'static str {
    static CLI: OnceLock<String> = OnceLock::new();
    static B64: [OnceLock<String>; 13] = [const { OnceLock::new() }; 13];
    match variant.and_then(|v| MARKS.iter().position(|(id, _)| *id == v)) {
        Some(i) => B64[i].get_or_init(|| base64(MARKS[i].1)),
        None => CLI.get_or_init(|| base64(include_bytes!("../assets/footer-mark.png"))),
    }
}

fn base64(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = c
            .iter()
            .enumerate()
            .fold(0u32, |n, (i, b)| n | u32::from(*b) << (16 - 8 * i));
        for i in 0..4 {
            out.push(if i <= c.len() {
                A[(n >> (18 - 6 * i) & 63) as usize] as char
            } else {
                '='
            });
        }
    }
    out
}

fn clock(s: &mut Svg, hours: &[u32; 24], week: &[f64; 7], top: f32, bottom: f32, p: &Palette) {
    let max = f32::max(1.0, *hours.iter().max().unwrap_or(&1) as f32);
    let slot = TEXT_W / 24.0;
    let base = bottom - 150.0;
    let height = (base - top - 10.0).max(60.0);
    for (h, v) in hours.iter().enumerate() {
        let bh = (*v as f32 / max * height).max(4.0);
        s.rect(
            LEFT + h as f32 * slot + 3.0,
            base - bh,
            slot - 6.0,
            bh,
            6.0,
            &p.accent,
        );
    }
    for h in [0usize, 6, 12, 18] {
        s.text(
            &format!("hour{h}"),
            LEFT + h as f32 * slot + slot / 2.0,
            base + 40.0,
            28.0,
            false,
            &p.muted,
            "middle",
            &format!("{h}h"),
        );
    }
    let wmax = week.iter().copied().fold(0.0, f64::max).max(1e-9);
    let cell = TEXT_W / 7.0;
    for (i, (d, v)) in ["M", "T", "W", "T", "F", "S", "S"]
        .iter()
        .zip(week)
        .enumerate()
    {
        let x = LEFT + i as f32 * cell;
        let side = 18.0 + 34.0 * (*v / wmax) as f32;
        let cx = x + cell / 2.0;
        s.rect(
            cx - side / 2.0,
            base + 90.0 - side / 2.0,
            side,
            side,
            8.0,
            &p.series[1],
        );
        s.text(
            &format!("day{i}"),
            cx,
            base + 145.0,
            28.0,
            false,
            &p.muted,
            "middle",
            d,
        );
    }
}

fn bars(s: &mut Svg, rows: &[(String, f64)], top: f32, bottom: f32, p: &Palette) {
    let n = rows.len().max(1) as f32;
    let step = ((bottom - top) / n).clamp(60.0, 110.0);
    let label_w = 300.0;
    let bar_w = TEXT_W - label_w - 120.0;
    let max = rows.iter().map(|r| r.1).fold(0.0, f64::max).max(1e-9);
    for (i, (name, share)) in rows.iter().enumerate() {
        let y = top + step * (i as f32 + 0.6);
        let size = fit(name, 38.0, label_w - 20.0, false);
        s.text(
            &format!("bar{i}"),
            LEFT,
            y + 12.0,
            size,
            false,
            &p.fg,
            "start",
            name,
        );
        let w = (bar_w * (*share / max) as f32).max(6.0);
        s.rect(LEFT + label_w, y - 18.0, w, 36.0, 10.0, &p.series[i % 7]);
        s.text(
            &format!("pct{i}"),
            LEFT + label_w + w + 16.0,
            y + 12.0,
            32.0,
            false,
            &p.fg2,
            "start",
            &format!("{:.0}%", share * 100.0),
        );
    }
}

fn poster_grid(s: &mut Svg, tiles: &[(String, String)], top: f32, bottom: f32, p: &Palette) {
    let cols = 2.0;
    let gap = 24.0;
    let tw = (TEXT_W - gap) / cols;
    let rows = (tiles.len() as f32 / cols).ceil().max(1.0);
    let th = (((bottom - top) - gap * (rows - 1.0)) / rows).min(190.0);
    for (i, (label, value)) in tiles.iter().enumerate() {
        let (c, r) = ((i % 2) as f32, (i / 2) as f32);
        let x = LEFT + c * (tw + gap);
        let y = top + r * (th + gap);
        s.rect(x, y, tw, th, 28.0, &p.accent_soft);
        s.text(
            &format!("tile{i}k"),
            x + 28.0,
            y + 44.0,
            30.0,
            false,
            &p.fg2,
            "start",
            label,
        );
        let size = fit(value, 72.0, tw - 56.0, true);
        let size = size.min((th - 24.0 - 44.0 - 16.0) / 0.75).max(28.0);
        s.text(
            &format!("tile{i}v"),
            x + 28.0,
            (y + th - 24.0).max(y + 44.0 + 16.0 + size * 0.75),
            size,
            true,
            &p.fg,
            "start",
            value,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_matches_rfc4648_vectors() {
        let cases = [
            ("", ""),
            ("f", "Zg=="),
            ("fo", "Zm8="),
            ("foo", "Zm9v"),
            ("foobar", "Zm9vYmFy"),
        ];
        for (raw, enc) in cases {
            assert_eq!(base64(raw.as_bytes()), enc);
        }
    }

    fn contrast(a: &str, b: &str) -> f64 {
        let lum = |h: &str| {
            let c = |i: usize| {
                let v = f64::from(u8::from_str_radix(&h[i..i + 2], 16).unwrap()) / 255.0;
                if v <= 0.03928 {
                    v / 12.92
                } else {
                    ((v + 0.055) / 1.055).powf(2.4)
                }
            };
            0.2126 * c(1) + 0.7152 * c(3) + 0.0722 * c(5)
        };
        let (x, y) = (lum(a), lum(b));
        (x.max(y) + 0.05) / (x.min(y) + 0.05)
    }

    const THEME_CSS: &str = "app/web/src/styles/theme.css";

    #[test]
    fn cli_palettes_match_theme_css() {
        let css = std::fs::read_to_string(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../..")
                .join(THEME_CSS),
        )
        .unwrap();
        let dark_at = css.find("prefers-color-scheme: dark").unwrap();
        let token = |block: &str, name: &str| -> String {
            let k = format!("--{name}:");
            let i = block.find(&k).unwrap_or_else(|| panic!("{name}")) + k.len();
            block[i..].trim_start()[..7].to_string()
        };
        for (pal, block) in [
            (Palette::light(), &css[..dark_at]),
            (Palette::dark(), &css[dark_at..]),
        ] {
            let names = [
                "plane",
                "card",
                "fg",
                "fg-2",
                "muted",
                "rule",
                "accent",
                "accent-soft",
                "accent-ink",
            ];
            let got = [
                &pal.plane,
                &pal.card,
                &pal.fg,
                &pal.fg2,
                &pal.muted,
                &pal.rule,
                &pal.accent,
                &pal.accent_soft,
                &pal.accent_ink,
            ];
            for (n, g) in names.iter().zip(got) {
                assert_eq!(&token(block, n), g, "{n}");
            }
            for i in 0..7 {
                assert_eq!(token(block, &format!("series-{}", i + 1)), pal.series[i]);
            }
        }
    }

    #[test]
    fn text_colours_reach_4_5_to_1() {
        for p in [Palette::light(), Palette::dark()] {
            for fg in [&p.fg, &p.fg2, &p.muted] {
                assert!(contrast(fg, &p.card) >= 4.5, "{fg} on {}", p.card);
            }
            for fg in [&p.fg, &p.fg2] {
                assert!(
                    contrast(fg, &p.accent_soft) >= 4.5,
                    "{fg} on {}",
                    p.accent_soft
                );
            }
        }
    }
}
