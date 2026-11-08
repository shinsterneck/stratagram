// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Color themes: the light look the diagrams are made in, and two dark
//! ones. A theme never changes the document: the colors saved with it are
//! the light ones, and a dark theme translates them when drawing (on screen
//! and in SVG and PNG exports), so a diagram reads the same whichever theme
//! it was made in.
//!
//! Dark themes keep each color's hue: a light fill turns deep, the border
//! and lines drawn from it turn bright, dark text turns light. The slate
//! "ink" of lines and text has fixed counterparts, and cloud providers'
//! brand colors stay as they are.

use serde_json::Value;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Theme {
    #[default]
    Light,
    /// Deep fills with bright borders in the same color.
    Deep,
    /// Near-black fills, saturated outlines and lines that glow.
    Neon,
}

/// What a color is used for, which decides how a dark theme translates it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Role {
    /// A filled area: a device's body, a layer background, a label's box.
    Fill,
    /// A line or outline: links, borders, icons.
    Stroke,
    /// Text.
    Text,
}

/// Colors that are brands (cloud provider tiles): kept in every theme.
const BRANDS: &[&str] = &["#232f3e", "#ff9900", "#0078d4", "#4285f4", "#ff6a00", "#e20074", "#0a4b5a", "#0b6bcb"];

/// The slate "ink" of the light theme (lines, labels, text) and what it
/// becomes in the dark ones.
const INK: &[(&str, &str, &str)] = &[
    // light, deep, neon
    ("#334155", "#cbd5e1", "#67e8f9"),
    ("#475569", "#a5b4c8", "#a5f3fc"),
    ("#64748b", "#94a3b8", "#7dd3fc"),
    ("#0f172a", "#e2e8f0", "#f8fafc"),
];

/// The colors around the diagram itself, for the canvas and exports.
pub struct Palette {
    /// The page of an export.
    pub page: &'static str,
    /// The canvas behind the diagram.
    pub canvas: &'static str,
    /// Grid lines: fine and every fifth.
    pub grid: (&'static str, &'static str),
    /// White in the light theme: label boxes, ports, the gap in double lines.
    pub paper: &'static str,
    /// Lines and text by default.
    pub ink: &'static str,
    /// Secondary text (port names, captions).
    pub muted: &'static str,
    /// Selection and drawing in progress.
    pub accent: &'static str,
    /// A drop target, a new port.
    pub good: &'static str,
    /// Whether lines and outlines glow.
    pub glow: bool,
    /// The interface around the canvas is dark.
    pub dark: bool,
}

impl Theme {
    pub const ALL: [Theme; 3] = [Theme::Light, Theme::Deep, Theme::Neon];

    pub fn as_str(self) -> &'static str {
        match self {
            Theme::Light => "light",
            Theme::Deep => "deep",
            Theme::Neon => "neon",
        }
    }

    /// The name shown in a theme selector.
    pub fn title(self) -> &'static str {
        match self {
            Theme::Light => "Light",
            Theme::Deep => "Dark",
            Theme::Neon => "Neon",
        }
    }

    /// "light", "deep" or "neon" (also "dark" for deep); None otherwise.
    pub fn parse(s: &str) -> Option<Theme> {
        match s.trim().to_ascii_lowercase().as_str() {
            "light" => Some(Theme::Light),
            "deep" | "dark" => Some(Theme::Deep),
            "neon" => Some(Theme::Neon),
            _ => None,
        }
    }

    pub fn is_dark(self) -> bool {
        self != Theme::Light
    }

    pub fn palette(self) -> Palette {
        match self {
            Theme::Light => Palette {
                page: "#ffffff",
                canvas: "#f8fafc",
                grid: ("#eef2f6", "#e1e7ef"),
                paper: "#ffffff",
                ink: "#334155",
                muted: "#64748b",
                accent: "#2563eb",
                good: "#16a34a",
                glow: false,
                dark: false,
            },
            Theme::Deep => Palette {
                page: "#0b1220",
                canvas: "#0b1220",
                grid: ("#131c2f", "#1a253b"),
                paper: "#131c2e",
                ink: "#cbd5e1",
                muted: "#94a3b8",
                accent: "#60a5fa",
                good: "#4ade80",
                glow: false,
                dark: true,
            },
            Theme::Neon => Palette {
                page: "#070b14",
                canvas: "#070b14",
                grid: ("#0d1424", "#131d33"),
                paper: "#0d1424",
                ink: "#67e8f9",
                muted: "#7dd3fc",
                accent: "#38bdf8",
                good: "#4ade80",
                glow: true,
                dark: true,
            },
        }
    }

    /// The palette as JSON, for the canvas.
    pub fn palette_json(self) -> Value {
        let p = self.palette();
        serde_json::json!({
            "name": self.as_str(), "page": p.page, "canvas": p.canvas, "grid": [p.grid.0, p.grid.1],
            "paper": p.paper, "ink": p.ink, "muted": p.muted, "accent": p.accent, "good": p.good,
            "glow": p.glow, "dark": p.dark,
        })
    }

    /// `c` (a "#rrggbb" color as saved) as this theme draws it for `role`.
    /// Anything that is not such a color comes back unchanged.
    pub fn color(self, c: &str, role: Role) -> String {
        if self == Theme::Light {
            return c.to_string();
        }
        let Some(rgb) = parse(c) else { return c.to_string() };
        let lc = c.to_ascii_lowercase();
        if BRANDS.contains(&lc.as_str()) {
            return lc;
        }
        let p = self.palette();
        let neon = self == Theme::Neon;
        if let Some(&(_, deep, neon_ink)) = INK.iter().find(|(l, _, _)| *l == lc) {
            return match role {
                // Ink as a fill (beads, small marks) stays ink.
                _ if neon => neon_ink.to_string(),
                _ => deep.to_string(),
            };
        }
        let (h, l, s) = to_hls(rgb);
        let white = l > 0.97;
        let grey = s < 0.06;
        match role {
            Role::Fill if white => p.paper.to_string(),
            Role::Fill if l >= 0.70 => {
                if grey {
                    p.paper.to_string()
                } else if neon {
                    hls(h, 0.08, 0.5)
                } else {
                    hls(h, 0.24, s.min(0.5))
                }
            }
            // Dark fills are text-dark: they turn light.
            Role::Fill if l < 0.2 => INK[3].1.to_string(),
            Role::Text if l >= 0.5 => lc,
            Role::Text if grey => (if neon { INK[3].2 } else { INK[3].1 }).to_string(),
            Role::Text => hls(h, 0.72, s.max(0.6)),
            // The white gap of a double line, a ring's inside: the page.
            Role::Stroke if white => p.page.to_string(),
            _ if grey => hls(h, if l < 0.35 { 0.80 } else { 0.66 }, s.min(0.2)),
            _ if neon => hls(h, 0.60, 1.0),
            _ => hls(h, 0.68, s.max(0.65)),
        }
    }

    /// The colors of an SVG (as `svg::export` writes it) as this theme
    /// draws them: fills, strokes and text by where they are used. The page
    /// is the first rectangle. With glow, lines and outlines get a soft one.
    pub fn recolor_svg(self, svg: &str) -> String {
        if self == Theme::Light {
            return svg.to_string();
        }
        let mut out = String::with_capacity(svg.len() + 400);
        let mut page_done = false;
        for line in svg.split_inclusive('\n') {
            if !page_done && line.starts_with("<rect ") {
                page_done = true;
                out.push_str(&line.replace(r##"fill="#ffffff""##, &format!(r#"fill="{}""#, self.palette().page)));
                continue;
            }
            out.push_str(&self.recolor_tags(line));
        }
        if self.palette().glow
            && let Some(start) = out.find('>')
        {
            out.insert_str(
                start + 1,
                "\n<defs><filter id=\"glow\" x=\"-20%\" y=\"-20%\" width=\"140%\" height=\"140%\"><feGaussianBlur stdDeviation=\"2.4\" result=\"b\"/><feMerge><feMergeNode in=\"b\"/><feMergeNode in=\"SourceGraphic\"/></feMerge></filter></defs>\n<g filter=\"url(#glow)\">",
            );
            if let Some(end) = out.rfind("</svg>") {
                out.insert_str(end, "</g>\n");
            }
        }
        out
    }

    /// The `fill="#…"` and `stroke="#…"` of the tags in `text`: a text's
    /// fill is text and its stroke (the halo of a number) its device's fill.
    fn recolor_tags(self, text: &str) -> String {
        let mut out = String::with_capacity(text.len());
        let mut rest = text;
        while let Some(i) = rest.find('<') {
            out.push_str(&rest[..i]);
            let tag_end = rest[i..].find('>').map_or(rest.len(), |j| i + j + 1);
            let tag = &rest[i..tag_end];
            let is_text = tag.starts_with("<text");
            let mut t = String::with_capacity(tag.len());
            let mut s = tag;
            loop {
                let next = ["fill=\"#", "stroke=\"#"].iter().filter_map(|k| s.find(k).map(|p| (p, *k))).min();
                let Some((p, key)) = next else { break };
                let at = p + key.len() - 1;
                let end = s[at..].find('"').map_or(s.len(), |e| at + e);
                // Not part of a longer name (stop-color, fill-opacity...).
                let whole = p == 0 || s.as_bytes()[p - 1] == b' ';
                t.push_str(&s[..at]);
                let c = &s[at..end];
                if whole {
                    let role = match (key, is_text) {
                        ("fill=\"#", true) => Role::Text,
                        ("fill=\"#", false) | ("stroke=\"#", true) => Role::Fill,
                        _ => Role::Stroke,
                    };
                    t.push_str(&self.color(c, role));
                } else {
                    t.push_str(c);
                }
                s = &s[end..];
            }
            t.push_str(s);
            out.push_str(&t);
            rest = &rest[tag_end..];
        }
        out.push_str(rest);
        out
    }

    /// The colors in a scene (as the editor sends it to the canvas) as this
    /// theme draws them, by the keys they are under; brand tiles keep theirs.
    pub fn recolor_scene(self, v: &mut Value) {
        if self == Theme::Light {
            return;
        }
        match v {
            Value::Array(a) => a.iter_mut().for_each(|x| self.recolor_scene(x)),
            Value::Object(o) => {
                // A text (in a frame, a note) has its color as text.
                let texty = o.contains_key("lines") || matches!(o.get("t").and_then(Value::as_str), Some("text" | "watermark"));
                for (k, x) in o.iter_mut() {
                    let role = match k.as_str() {
                        "tile" => continue,
                        "fill" => Some(Role::Fill),
                        "stroke" => Some(Role::Stroke),
                        "text" => Some(Role::Text),
                        "color" if texty => Some(Role::Text),
                        "color" => Some(Role::Stroke),
                        _ => None,
                    };
                    match (role, &*x) {
                        (Some(r), Value::String(c)) if parse(c).is_some() => *x = Value::String(self.color(c, r)),
                        _ => self.recolor_scene(x),
                    }
                }
            }
            _ => {}
        }
    }
}

fn parse(c: &str) -> Option<(f64, f64, f64)> {
    let h = c.strip_prefix('#')?;
    if h.len() != 6 || !h.bytes().all(|b| b.is_ascii_hexdigit()) {
        return None;
    }
    let v = |i: usize| u8::from_str_radix(&h[i..i + 2], 16).ok().map(|x| x as f64 / 255.0);
    Some((v(0)?, v(2)?, v(4)?))
}

/// Hue (0..1), lightness and saturation of an RGB color (0..1 each).
fn to_hls((r, g, b): (f64, f64, f64)) -> (f64, f64, f64) {
    let (max, min) = (r.max(g).max(b), r.min(g).min(b));
    let l = (max + min) / 2.0;
    if max == min {
        return (0.0, l, 0.0);
    }
    let d = max - min;
    let s = if l <= 0.5 { d / (max + min) } else { d / (2.0 - max - min) };
    let h = if max == r {
        (g - b) / d + if g < b { 6.0 } else { 0.0 }
    } else if max == g {
        (b - r) / d + 2.0
    } else {
        (r - g) / d + 4.0
    };
    (h / 6.0, l, s)
}

fn hls(h: f64, l: f64, s: f64) -> String {
    let (l, s) = (l.clamp(0.0, 1.0), s.clamp(0.0, 1.0));
    let q = if l < 0.5 { l * (1.0 + s) } else { l + s - l * s };
    let p = 2.0 * l - q;
    let ch = |t: f64| {
        let t = t.rem_euclid(1.0);
        let v = if t < 1.0 / 6.0 {
            p + (q - p) * 6.0 * t
        } else if t < 0.5 {
            q
        } else if t < 2.0 / 3.0 {
            p + (q - p) * (2.0 / 3.0 - t) * 6.0
        } else {
            p
        };
        (v * 255.0).round().clamp(0.0, 255.0) as u8
    };
    format!("#{:02x}{:02x}{:02x}", ch(h + 1.0 / 3.0), ch(h), ch(h - 1.0 / 3.0))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn light_changes_nothing() {
        let svg = r##"<rect fill="#ffffff"/><text fill="#0f172a">a</text>"##;
        assert_eq!(Theme::Light.recolor_svg(svg), svg);
        assert_eq!(Theme::Light.color("#bfdbfe", Role::Fill), "#bfdbfe");
    }

    #[test]
    fn dark_keeps_hues_and_turns_light_fills_deep() {
        for t in [Theme::Deep, Theme::Neon] {
            let fill = parse(&t.color("#bfdbfe", Role::Fill)).unwrap();
            let border = parse(&t.color("#69788c", Role::Stroke)).unwrap();
            let (h, l, _) = to_hls(fill);
            assert!(l < 0.3, "{t:?}: a light switch fill turns deep");
            assert!((h - to_hls(parse("#bfdbfe").unwrap()).0).abs() < 0.02, "same hue");
            assert!(to_hls(border).1 > 0.55, "its border bright");
            assert_eq!(t.color("#ff9900", Role::Fill), "#ff9900", "brands stay");
            assert!(to_hls(parse(&t.color("#0f172a", Role::Text)).unwrap()).1 > 0.85, "dark text turns light");
            assert_eq!(t.color("#ffffff", Role::Text), "#ffffff", "light text stays");
            assert_eq!(t.color("none", Role::Fill), "none");
        }
        assert_eq!(Theme::Deep.color("#334155", Role::Stroke), "#cbd5e1");
        assert_eq!(Theme::Neon.color("#334155", Role::Stroke), "#67e8f9");
    }

    #[test]
    fn svg_is_recolored_by_role() {
        let svg = "<svg>\n<rect x=\"0\" fill=\"#ffffff\"/>\n<rect fill=\"#bfdbfe\" stroke=\"#69788c\"/>\n<text fill=\"#0f172a\" stroke=\"#bfdbfe\">1</text>\n<rect fill-opacity=\"0.5\" fill=\"#ffffff\"/>\n</svg>\n";
        let out = Theme::Deep.recolor_svg(svg);
        let p = Theme::Deep.palette();
        assert!(out.contains(&format!("<rect x=\"0\" fill=\"{}\"/>", p.page)), "{out}");
        let fill = Theme::Deep.color("#bfdbfe", Role::Fill);
        assert!(out.contains(&format!("<text fill=\"#e2e8f0\" stroke=\"{fill}\">")), "a halo is its device's fill: {out}");
        assert!(out.contains(&format!("fill-opacity=\"0.5\" fill=\"{}\"", p.paper)));
        let neon = Theme::Neon.recolor_svg(svg);
        assert!(neon.contains("filter=\"url(#glow)\"") && neon.trim_end().ends_with("</g>\n</svg>"));
    }

    #[test]
    fn scene_is_recolored_by_key() {
        let mut v = serde_json::json!({"devices": [{"fill": "#bfdbfe", "text": "#0f172a", "name": "#bfdbfe",
            "tile": {"bg": "#ffffff", "fg": "#000000"}}], "notes": [{"lines": [], "color": "#0f172a"}],
            "links": [{"color": "#334155"}]});
        Theme::Deep.recolor_scene(&mut v);
        assert_eq!(v["devices"][0]["fill"], Theme::Deep.color("#bfdbfe", Role::Fill));
        assert_eq!(v["devices"][0]["name"], "#bfdbfe", "names are not colors");
        assert_eq!(v["devices"][0]["tile"]["bg"], "#ffffff");
        assert_eq!(v["notes"][0]["color"], "#e2e8f0");
        assert_eq!(v["links"][0]["color"], "#cbd5e1");
    }

    #[test]
    fn names_parse() {
        for t in Theme::ALL {
            assert_eq!(Theme::parse(t.as_str()), Some(t));
        }
        assert_eq!(Theme::parse("Dark"), Some(Theme::Deep));
        assert_eq!(Theme::parse("pink"), None);
    }
}
