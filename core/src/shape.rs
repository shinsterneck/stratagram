// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Device outlines by kind: a pill with routing arrows for a router, a box
//! with rows of ports for a switch, a brick wall for a firewall, a cloud, a
//! globe for the internet, and so on. Shapes are path
//! data made of `M`, `L`, `C` and `Z` only, so the canvas can draw them with
//! a few lines of code and the SVG export uses them as they are. They stay
//! inside the device's box, where its ports are.

use crate::model::{Connector, PatchPanel, Rect};
use std::f64::consts::{FRAC_PI_2, PI};
use std::fmt::Write;

/// The device kinds, in the order the panel offers them ("" is a plain box).
#[cfg(test)]
const KINDS: [&str; 29] = [
    "", "router", "switch", "firewall", "load balancer", "server", "cloud", "internet", "client", "access point", "wlan controller", "standalone ap", "managed ap", "aws",
    "aws transit gateway", "aws direct connect gateway", "azure", "google cloud", "alibaba cloud", "telekom cloud", "stackit", "cloud proxy", "zscaler", "ospf router",
    "bgp reflector", "aci l3out",
    "patch panel", "virtual switch", "distributed virtual switch",
];

/// The type as printed under a device's name, shortened where the full
/// type does not fit a device ("AWS DX gateway").
pub fn kind_label(kind: &str) -> &str {
    match kind.trim() {
        "aws direct connect gateway" => "aws dx gateway",
        "distributed virtual switch" => "distributed switch",
        k => k,
    }
}

/// Cloud providers: their tile color, text color and monogram. These are
/// simple stand-ins in the brand colors, not the providers' logos.
const PROVIDERS: [(&str, &str, &str, &str); 7] = [
    ("aws", "#232f3e", "#ff9900", "aws"),
    ("azure", "#0078d4", "#ffffff", "A"),
    ("google cloud", "#4285f4", "#ffffff", "G"),
    ("alibaba cloud", "#ff6a00", "#ffffff", "[-]"),
    ("telekom cloud", "#e20074", "#ffffff", "T"),
    ("stackit", "#0a4b5a", "#ffffff", "S"),
    // Cloud security proxy (SaaS).
    ("zscaler", "#0b6bcb", "#ffffff", "Z"),
];

/// Tile color, text color and monogram for a cloud provider `kind`.
pub fn provider(kind: &str) -> Option<(&'static str, &'static str, &'static str)> {
    let kind = kind.trim().to_lowercase();
    PROVIDERS.iter().find(|p| p.0 == kind).map(|p| (p.1, p.2, p.3))
}

/// A cloud provider's tile: a rounded square at `x`,`y` with side `size`,
/// and its monogram centered in it.
#[derive(Debug, Clone, PartialEq)]
pub struct Tile {
    pub x: f64,
    pub y: f64,
    pub size: f64,
    pub bg: &'static str,
    pub fg: &'static str,
    pub text: &'static str,
    pub font: f64,
}

impl Tile {
    fn new(kind: &str, c: (f64, f64), r: f64) -> Option<Tile> {
        let (bg, fg, text) = provider(kind)?;
        let font = 2.0 * r * [0.62, 0.62, 0.5, 0.4][text.chars().count().min(3)];
        Some(Tile { x: c.0 - r, y: c.1 - r, size: 2.0 * r, bg, fg, text, font })
    }
}

/// A device's shape: the outline to fill and stroke, lines drawn faintly
/// inside it (bricks, globe lines and the like), and a symbol in the
/// outline's color made of lines (`mark`) and filled parts (`solid`). Any
/// but the outline may be empty.
#[derive(Debug, Default, Clone, PartialEq)]
pub struct Shape {
    pub outline: String,
    pub detail: String,
    pub mark: String,
    pub solid: String,
    /// How far the name and type move from the box's center to make room
    /// for the symbol.
    pub label: (f64, f64),
    /// A cloud provider's tile, in place of a symbol.
    pub tile: Option<Tile>,
    /// Small texts in the outline's color (a patch panel's port numbers).
    pub texts: Vec<ShapeText>,
}

/// A text of a shape, centered on `x`,`y`.
#[derive(Debug, Clone, PartialEq)]
pub struct ShapeText {
    pub x: f64,
    pub y: f64,
    pub text: String,
    pub size: f64,
}

// A device's front with sockets: mounting ears at the sides, the name at
// the top, then the sockets, each with its number. A patch panel has rows
// of at most PANEL_ROW_MAX, in groups of PANEL_GROUP. A switch has two
// rows, odd ports on top and even ones below (numbered above and below
// them), in groups of 2 * PANEL_GROUP, and its uplinks in a group of their
// own at the right, numbered on from its last port.
const PANEL_ROW_MAX: u32 = 24;
const PANEL_GROUP: u32 = 6;
const PANEL_GROUP_GAP: f64 = 4.0;
const PANEL_EAR: f64 = 16.0;
const PANEL_TOP: f64 = 34.0;
const PANEL_ROW: f64 = 24.0;
/// Between a switch's ports and its uplinks.
const UPLINK_GAP: f64 = 12.0;
/// A switch's two rows: the top one's numbers, both rows of sockets, the
/// bottom one's numbers.
const SWITCH_ROWS: f64 = 42.0;

/// What a device's front shows: a patch panel's sockets, or a switch's.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Front {
    pub panel: PatchPanel,
    pub switch: bool,
}

/// Whether a device of type `kind` can show its ports as a front, like
/// a switch: switches, routers, firewalls and servers.
pub fn takes_ports(kind: &str) -> bool {
    matches!(kind.trim().to_lowercase().as_str(), "switch" | "router" | "ospf router" | "bgp reflector" | "firewall" | "server")
}

/// The front of a device of type `kind`: a patch panel always has one
/// (24 RJ45 without `panel`); a switch, router, firewall or server only
/// when it has a `panel` with a switch's connectors, RJ45 or SFP (its
/// ports, two rows like a switch's; a patch panel's LC sockets turned
/// into a router leave it a box).
pub fn front_of(kind: &str, panel: Option<&PatchPanel>) -> Option<Front> {
    let kind = kind.trim().to_lowercase();
    match kind.as_str() {
        "patch panel" => Some(Front { panel: panel.copied().unwrap_or_default(), switch: false }),
        k if takes_ports(k) => panel
            .filter(|p| matches!(p.connector, Connector::Rj45 | Connector::Sfp))
            .map(|p| Front { panel: *p, switch: true }),
        _ => None,
    }
}

fn panel_pitch(c: Connector) -> f64 {
    match c {
        Connector::Rj45 | Connector::Sc | Connector::Sfp => 16.0,
        Connector::Lc => 19.0,
    }
}

/// Rows and sockets per row: one row up to PANEL_ROW_MAX ports, else as
/// few rows as hold them, filled evenly.
fn panel_grid(ports: u32) -> (u32, u32) {
    let ports = ports.max(1);
    let rows = ports.div_ceil(PANEL_ROW_MAX);
    (rows, ports.div_ceil(rows))
}

/// Width of a row of `cols` sockets (without the ears).
fn panel_row_w(cols: u32, pitch: f64) -> f64 {
    cols as f64 * pitch + ((cols.max(1) - 1) / PANEL_GROUP) as f64 * PANEL_GROUP_GAP
}

/// A switch's columns of ports and of uplinks (two to a column).
fn switch_cols(p: &PatchPanel) -> (u32, u32) {
    (p.ports.max(1).div_ceil(2), p.uplinks.div_ceil(2))
}

/// Width of a front's sockets (without the ears).
fn sockets_w(f: &Front) -> f64 {
    let p = &f.panel;
    if !f.switch {
        return panel_row_w(panel_grid(p.ports).1, panel_pitch(p.connector));
    }
    let (cols, up) = switch_cols(p);
    let uplinks = if up > 0 { UPLINK_GAP + panel_row_w(up, panel_pitch(Connector::Sfp)) } else { 0.0 };
    panel_row_w(cols, panel_pitch(p.connector)) + uplinks
}

/// The size a front takes: wider with more ports per row, a patch panel
/// taller with more rows.
pub fn panel_size(f: &Front) -> (f64, f64) {
    let w = 2.0 * PANEL_EAR + sockets_w(f);
    let h = if f.switch { PANEL_TOP + SWITCH_ROWS + 2.0 } else { PANEL_TOP + panel_grid(f.panel.ports).0 as f64 * PANEL_ROW + 2.0 };
    (w.max(120.0).ceil(), h)
}

/// Where one socket is drawn: its number `n` (from 1) and where that is
/// written (`num_y`), its center `cx` and top, its type, whether it is
/// on a switch and upside down (a switch's bottom row), and the factor `k`
/// it is shrunk by.
pub struct PanelSocket {
    pub n: u32,
    pub cx: f64,
    pub top: f64,
    pub num_y: f64,
    pub connector: Connector,
    pub switch: bool,
    pub flip: bool,
    pub k: f64,
}

/// The sockets of front `f` drawn in `r`, shrunk if the box was made
/// narrower than they need (and a patch panel's rows left out if it was
/// made lower).
pub fn panel_sockets(f: &Front, r: &Rect) -> Vec<PanelSocket> {
    let p = &f.panel;
    let pitch = panel_pitch(p.connector);
    let all_w = sockets_w(f);
    let k = ((r.w - 2.0 * PANEL_EAR) / all_w).clamp(0.2, 1.0);
    let x0 = r.x + (r.w - all_w * k) / 2.0;
    let at = |col: u32, pitch: f64| (col as f64 * pitch + (col / PANEL_GROUP) as f64 * PANEL_GROUP_GAP + pitch / 2.0) * k;
    let mut out = vec![];
    if !f.switch {
        let (_, cols) = panel_grid(p.ports);
        for i in 0..p.ports.max(1) {
            let (row, col) = (i / cols, i % cols);
            let row_top = r.y + PANEL_TOP + row as f64 * PANEL_ROW;
            if row_top + PANEL_ROW > r.y + r.h + 2.0 {
                break;
            }
            let (top, num_y) = (row_top + 9.0, row_top + 4.0);
            out.push(PanelSocket { n: i + 1, cx: x0 + at(col, pitch), top, num_y, connector: p.connector, switch: false, flip: false, k });
        }
        return out;
    }
    // Odd ports on top, numbered above; even ones below, numbered below.
    let row = |i: u32, n: u32, cx: f64, connector: Connector| {
        let band = r.y + PANEL_TOP;
        let (top, num_y) = if i.is_multiple_of(2) { (band + 9.0, band + 4.0) } else { (band + 21.0, band + 36.0) };
        PanelSocket { n, cx, top, num_y, connector, switch: true, flip: !i.is_multiple_of(2), k }
    };
    for i in 0..p.ports.max(1) {
        out.push(row(i, i + 1, x0 + at(i / 2, pitch), p.connector));
    }
    let up_x = x0 + (panel_row_w(switch_cols(p).0, pitch) + UPLINK_GAP) * k;
    for j in 0..p.uplinks {
        out.push(row(j, p.ports.max(1) + j + 1, up_x + at(j / 2, panel_pitch(Connector::Sfp)), Connector::Sfp));
    }
    out
}

/// A socket's height at full size.
fn socket_h(c: Connector) -> f64 {
    match c {
        Connector::Rj45 => 9.0,
        Connector::Lc | Connector::Sfp => 8.0,
        Connector::Sc => 10.0,
    }
}

/// Where a line plugs into socket `sk`: the middle of its opening (of a
/// duplex adapter, between its two halves).
pub fn socket_center(sk: &PanelSocket) -> (f64, f64) {
    (sk.cx, sk.top + socket_h(sk.connector) * sk.k / 2.0)
}

/// Patch cords plugged into front `f` drawn in `r`: for each socket
/// (numbered from 1) whether its cord leaves upwards, else downwards, and
/// whether it is the socket's back. A cord at the front is a plug in the
/// socket and a cord to the panel's edge, where its line goes on; the back
/// of a socket (a rear port) is a dotted tie from the socket down to the
/// bottom edge, where its line leaves. Added to the shape's solid parts.
pub fn add_cords(s: &mut Shape, f: Option<&Front>, r: &Rect, cords: &[(u32, bool, bool)]) {
    let Some(f) = f else { return };
    if cords.is_empty() {
        return;
    }
    let mut path = Path::new();
    for sk in panel_sockets(f, r) {
        for &(_, up, rear) in cords.iter().filter(|c| c.0 == sk.n) {
            let (hw, hh) = socket_half(sk.connector);
            let (cx, cy) = socket_center(&sk);
            if rear {
                // Behind the panel: dots from the socket to the edge it comes
                // out at.
                let (w, dot, gap) = (0.9 * sk.k.max(0.6), 2.0, 2.2);
                let (from, to) = if up { (r.y, cy - hh * sk.k - 1.5) } else { (cy + hh * sk.k + 1.5, r.y + r.h) };
                let mut y = from + 0.5;
                while y < to - 0.5 {
                    rect_path(&mut path, cx - w, y, cx + w, (y + dot).min(to), false);
                    y += dot + gap;
                }
                continue;
            }
            let (pw, ph, cw) = (hw * 0.8 * sk.k, (hh + 1.0) * sk.k, 1.3 * sk.k.max(0.6));
            rect_path(&mut path, cx - pw, cy - ph, cx + pw, cy + ph, false);
            let (y0, y1) = if up { (r.y, cy) } else { (cy, r.y + r.h) };
            let x = cx + cord_dx(&sk, up);
            rect_path(&mut path, x - cw, y0, x + cw, y1, false);
        }
    }
    if !s.solid.is_empty() {
        s.solid.push(' ');
    }
    s.solid.push_str(&path.done());
}

/// How far beside socket `sk` its cord runs, leaving upwards (`up`) or
/// downwards: past the switch's other socket in its column a little aside,
/// so two cords from one column do not end up on top of each other.
pub fn cord_dx(sk: &PanelSocket, up: bool) -> f64 {
    match (sk.switch, sk.flip, up) {
        (true, false, false) => -3.5 * sk.k,
        (true, true, true) => 3.5 * sk.k,
        _ => 0.0,
    }
}

/// Half the width and height of a socket of type `c` at full size (to
/// hit it with the mouse).
pub fn socket_half(c: Connector) -> (f64, f64) {
    match c {
        Connector::Rj45 => (5.5, 4.5),
        Connector::Lc => (7.5, 4.0),
        Connector::Sc => (5.0, 5.0),
        Connector::Sfp => (6.0, 4.0),
    }
}

/// A rectangle as path data, clockwise (or `ccw`, to cut a hole into a
/// filled one around it).
fn rect_path(p: &mut Path, x0: f64, y0: f64, x1: f64, y1: f64, ccw: bool) {
    if ccw {
        p.m(x0, y0).l(x0, y1).l(x1, y1).l(x1, y0).z();
    } else {
        p.m(x0, y0).l(x1, y0).l(x1, y1).l(x0, y1).z();
    }
}

/// One socket centered on `cx` below `top`, `f` times its full size;
/// `flip`: upside down (a switch's bottom row has its latches on top).
fn socket(p: &mut Path, c: Connector, cx: f64, top: f64, f: f64, flip: bool) {
    let h = socket_h(c) * f;
    // From the socket's own top down, mirrored when upside down.
    let y = |v: f64| if flip { top + h - v } else { top + v };
    let rect = |p: &mut Path, x0: f64, v0: f64, x1: f64, v1: f64, ccw: bool| {
        let (a, b) = (y(v0).min(y(v1)), y(v0).max(y(v1)));
        rect_path(p, x0, a, x1, b, ccw);
    };
    match c {
        Connector::Rj45 => {
            // The jack's opening, with the latch slot at the bottom.
            let (hw, slot, body) = (5.5 * f, 2.6 * f, 6.0 * f);
            let pts = [(-hw, 0.0), (hw, 0.0), (hw, body), (slot, body), (slot, h), (-slot, h), (-slot, body), (-hw, body)];
            // Mirrored, the outline runs the other way round: keep it clockwise.
            let order: Vec<usize> = if flip { (0..pts.len()).rev().collect() } else { (0..pts.len()).collect() };
            for (i, &k) in order.iter().enumerate() {
                let (dx, v) = pts[k];
                if i == 0 { p.m(cx + dx, y(v)) } else { p.l(cx + dx, y(v)) };
            }
            p.z();
        }
        Connector::Lc => {
            // A duplex adapter: a frame with two small square ferrules.
            let (hw, t) = (7.5 * f, 1.2 * f);
            rect(p, cx - hw, 0.0, cx + hw, h, false);
            rect(p, cx - hw + t, t, cx + hw - t, h - t, true);
            let q = 1.8 * f;
            for dx in [-3.4 * f, 3.4 * f] {
                rect(p, cx + dx - q, h / 2.0 - q, cx + dx + q, h / 2.0 + q, false);
            }
        }
        Connector::Sc => {
            // A square adapter with the ferrule in the middle.
            let (hw, t) = (5.0 * f, 1.2 * f);
            rect(p, cx - hw, 0.0, cx + hw, h, false);
            rect(p, cx - hw + t, t, cx + hw - t, h - t, true);
            rect(p, cx - 1.8 * f, 2.6 * f, cx + 1.8 * f, h - 2.6 * f, false);
        }
        Connector::Sfp => {
            // An empty cage, with the latch tab at the bottom.
            let (hw, t) = (6.0 * f, 1.1 * f);
            rect(p, cx - hw, 0.0, cx + hw, h, false);
            rect(p, cx - hw + t, t, cx + hw - t, h - t, true);
            rect(p, cx - 2.4 * f, h - t - 1.6 * f, cx + 2.4 * f, h - t, false);
        }
    }
}

/// Path data being built, in absolute coordinates.
struct Path {
    d: String,
    at: (f64, f64),
}

impl Path {
    fn new() -> Self {
        Path { d: String::new(), at: (0.0, 0.0) }
    }

    fn cmd(&mut self, c: char, pts: &[(f64, f64)]) {
        if !self.d.is_empty() {
            self.d.push(' ');
        }
        self.d.push(c);
        for (x, y) in pts {
            let _ = write!(self.d, " {} {}", num(*x), num(*y));
        }
        if let Some(last) = pts.last() {
            self.at = *last;
        }
    }

    fn m(&mut self, x: f64, y: f64) -> &mut Self {
        self.cmd('M', &[(x, y)]);
        self
    }

    fn l(&mut self, x: f64, y: f64) -> &mut Self {
        self.cmd('L', &[(x, y)]);
        self
    }

    fn c(&mut self, c1: (f64, f64), c2: (f64, f64), to: (f64, f64)) -> &mut Self {
        self.cmd('C', &[c1, c2, to]);
        self
    }

    fn z(&mut self) -> &mut Self {
        self.cmd('Z', &[]);
        self
    }

    /// Elliptical arc around (cx, cy) from angle `a0` to `a1` (radians,
    /// clockwise on screen when `a1 > a0`), continuing from the current
    /// point, as cubic curves of at most a quarter turn each.
    fn arc(&mut self, cx: f64, cy: f64, rx: f64, ry: f64, a0: f64, a1: f64) -> &mut Self {
        let n = ((a1 - a0).abs() / FRAC_PI_2).ceil().max(1.0) as usize;
        let step = (a1 - a0) / n as f64;
        let k = 4.0 / 3.0 * (step / 4.0).tan();
        let pt = |a: f64| (cx + rx * a.cos(), cy + ry * a.sin());
        for i in 0..n {
            let (s, e) = (a0 + step * i as f64, a0 + step * (i + 1) as f64);
            let (p0, p3) = (pt(s), pt(e));
            let c1 = (p0.0 - k * rx * s.sin(), p0.1 + k * ry * s.cos());
            let c2 = (p3.0 + k * rx * e.sin(), p3.1 - k * ry * e.cos());
            self.c(c1, c2, p3);
        }
        self
    }

    fn ellipse(&mut self, cx: f64, cy: f64, rx: f64, ry: f64) -> &mut Self {
        self.m(cx + rx, cy).arc(cx, cy, rx, ry, 0.0, 2.0 * PI).z()
    }

    fn round_rect(&mut self, r: &Rect, radius: f64) -> &mut Self {
        let rad = radius.min(r.w / 2.0).min(r.h / 2.0).max(0.0);
        let (x0, y0, x1, y1) = (r.x, r.y, r.x + r.w, r.y + r.h);
        self.m(x0 + rad, y0).l(x1 - rad, y0);
        self.arc(x1 - rad, y0 + rad, rad, rad, -FRAC_PI_2, 0.0).l(x1, y1 - rad);
        self.arc(x1 - rad, y1 - rad, rad, rad, 0.0, FRAC_PI_2).l(x0 + rad, y1);
        self.arc(x0 + rad, y1 - rad, rad, rad, FRAC_PI_2, PI).l(x0, y0 + rad);
        self.arc(x0 + rad, y0 + rad, rad, rad, PI, 1.5 * PI).z()
    }

    fn cloud(&mut self, r: &Rect) -> &mut Self {
        let p = |u: f64, v: f64| (r.x + u * r.w, r.y + v * r.h);
        let start = p(0.2, 0.95);
        self.m(start.0, start.1);
        self.c(p(0.02, 0.95), p(-0.01, 0.6), p(0.13, 0.54));
        self.c(p(0.07, 0.27), p(0.28, 0.1), p(0.41, 0.2));
        self.c(p(0.47, 0.0), p(0.77, -0.02), p(0.79, 0.24));
        self.c(p(0.97, 0.2), p(1.03, 0.52), p(0.89, 0.6));
        self.c(p(1.01, 0.75), p(0.95, 0.97), p(0.8, 0.95));
        self.z()
    }

    fn line(&mut self, x0: f64, y0: f64, x1: f64, y1: f64) -> &mut Self {
        self.m(x0, y0).l(x1, y1)
    }

    fn done(&mut self) -> String {
        std::mem::take(&mut self.d)
    }
}

fn num(v: f64) -> String {
    let s = format!("{v:.2}");
    let s = s.trim_end_matches('0').trim_end_matches('.');
    if s == "-0" { "0".into() } else { s.into() }
}

/// The shape of a device of `kind` named `name` filling `r`. Containers
/// (devices with others inside) keep just the outline, to leave room for
/// their contents.
pub fn device_shape(kind: &str, name: &str, r: &Rect, container: bool, panel: Option<&PatchPanel>) -> Shape {
    let (x, y, w, h) = (r.x, r.y, r.w, r.h);
    let (mut o, mut d) = (Path::new(), Path::new());
    let mut s = Shape::default();
    // Room for a symbol at the left: its radius, its left edge's distance
    // from the box, and how far the outline's right side cuts in at the
    // height of the name. The name moves right to make room.
    let mut symbol: Option<(f64, f64, f64)> = None;
    // Half the height of a name with its type under it.
    const TEXT_HALF: f64 = 14.0;
    let kind = kind.trim().to_lowercase();
    let front = front_of(&kind, panel);
    match kind.as_str() {
        // Patch panels, and switches with their ports: the front, as in a rack.
        _ if front.is_some() => {
            let fr = front.unwrap();
            o.round_rect(r, 2.0);
            // The ears, each with two mounting holes.
            d.line(x + PANEL_EAR - 3.0, y, x + PANEL_EAR - 3.0, y + h);
            d.line(x + w - PANEL_EAR + 3.0, y, x + w - PANEL_EAR + 3.0, y + h);
            let mut f = Path::new();
            for ex in [x + (PANEL_EAR - 3.0) / 2.0, x + w - (PANEL_EAR - 3.0) / 2.0] {
                for ey in [y + 9.0, y + h - 9.0] {
                    f.ellipse(ex, ey, 1.6, 2.4);
                }
            }
            for sk in panel_sockets(&fr, r) {
                if sk.k >= 0.6 {
                    s.texts.push(ShapeText { x: sk.cx, y: sk.num_y, text: sk.n.to_string(), size: 7.0 * sk.k });
                }
                socket(&mut f, sk.connector, sk.cx, sk.top, sk.k, sk.flip);
            }
            s.solid = f.done();
            // The name and type in the band at the top.
            s.label.1 = 17.0 - h / 2.0;
        }
        "router" | "ospf router" | "bgp reflector" => {
            let rad = w.min(h) / 2.0;
            o.m(x + rad, y).l(x + w - rad, y);
            o.arc(x + w - rad, y + h / 2.0, rad, h / 2.0, -FRAC_PI_2, FRAC_PI_2).l(x + rad, y + h);
            o.arc(x + rad, y + h / 2.0, rad, h / 2.0, FRAC_PI_2, 1.5 * PI).z();
            // In the middle of the round left end.
            let sr = (h * 0.28).min(15.0);
            let cut = |dy: f64| rad - (rad * rad - dy.min(rad).powi(2)).sqrt();
            symbol = Some((sr, cut(sr) + 4.0, cut(TEXT_HALF) + 2.0));
        }
        "switch" => {
            o.round_rect(r, 2.0);
            // Two rows of ports along the bottom, like a switch's front panel;
            // the name moves up out of their way.
            let rows = if h >= 50.0 { 2 } else if h >= 38.0 { 1 } else { 0 };
            let (pw, ph, pitch) = (6.0, 4.5, 8.5);
            let n = (((w - 14.0 + pitch - pw) / pitch).floor() as i32).clamp(0, 24);
            let x0 = x + (w - (n as f64 * pitch - (pitch - pw))) / 2.0;
            let mut ports = Path::new();
            for row in 0..rows {
                let py = y + h - 6.0 - ph - row as f64 * (ph + 2.5);
                for i in 0..n {
                    let px = x0 + i as f64 * pitch;
                    ports.m(px, py).l(px + pw, py).l(px + pw, py + ph).l(px, py + ph).z();
                }
            }
            s.solid = ports.done();
            s.label.1 = -(rows as f64) * 3.5;
        }
        "firewall" => {
            o.round_rect(r, 2.0);
            // A brick wall.
            let (bh, bw) = (12.0, 24.0);
            let mut row = 0;
            let mut top = y;
            while top < y + h - 0.5 {
                let bottom = (top + bh).min(y + h);
                if bottom < y + h - 0.5 {
                    d.line(x, bottom, x + w, bottom);
                }
                let mut jx = x + if row % 2 == 0 { bw } else { bw / 2.0 };
                while jx < x + w - 2.0 {
                    d.line(jx, top, jx, bottom);
                    jx += bw;
                }
                top = bottom;
                row += 1;
            }
        }
        "load balancer" => {
            let i = (h * 0.3).min(w * 0.15);
            o.m(x + i, y).l(x + w - i, y).l(x + w, y + h / 2.0).l(x + w - i, y + h).l(x + i, y + h).l(x, y + h / 2.0).z();
            let sr = (h * 0.24).min(13.0);
            // Clear of the slanted left side at the symbol's top and bottom.
            symbol = Some((sr, i * (1.0 - sr / (h / 2.0)) + 4.0, i * (TEXT_HALF / (h / 2.0)).min(1.0) + 2.0));
        }
        "server" => {
            o.round_rect(r, 3.0);
            // Rack ears on both sides.
            let e = 7.0f64.min(w / 8.0);
            d.line(x + e, y, x + e, y + h).line(x + w - e, y, x + w - e, y + h);
            symbol = Some(((h * 0.24).min(12.0), e + 5.0, e + 3.0));
        }
        "cloud" => {
            o.cloud(r);
        }
        "cloud proxy" => {
            // A cloud security proxy (secure web gateway as a service).
            o.cloud(r);
            symbol = Some(((h * 0.22).min(12.0), w * 0.12 + 4.0, w * 0.1));
        }
        k if provider(k).is_some() => {
            if container {
                // A group box with the provider's tile in its top left corner,
                // like draw.io's "AWS Cloud" groups.
                o.round_rect(r, 8.0);
                s.tile = Tile::new(k, (x + 15.0, y + 15.0), 9.0);
            } else {
                o.cloud(r);
                symbol = Some(((h * 0.22).min(12.0), w * 0.12 + 4.0, w * 0.1));
            }
        }
        "internet" => {
            let (cx, cy, rx, ry) = (x + w / 2.0, y + h / 2.0, w / 2.0, h / 2.0);
            o.ellipse(cx, cy, rx, ry);
            // A globe: equator, two latitudes and three meridians.
            d.line(x, cy, x + w, cy);
            let half = rx * (1.0f64 - 0.25).sqrt();
            d.line(cx - half, cy - ry / 2.0, cx + half, cy - ry / 2.0);
            d.line(cx - half, cy + ry / 2.0, cx + half, cy + ry / 2.0);
            d.line(cx, y, cx, y + h);
            d.ellipse(cx, cy, rx * 0.45, ry);
        }
        "client" => {
            // A monitor on a stand.
            let stand = (h * 0.16).clamp(5.0, 10.0);
            let screen = Rect { h: h - stand, ..*r };
            o.round_rect(&screen, 4.0);
            let cx = x + w / 2.0;
            let foot = (w * 0.18).min(stand * 2.4);
            o.m(cx - foot * 0.45, y + h - stand).l(cx + foot * 0.45, y + h - stand);
            o.l(cx + foot, y + h).l(cx - foot, y + h).z();
        }
        "access point" | "standalone ap" | "managed ap" => {
            o.round_rect(r, w.min(h) * 0.3);
            symbol = Some(((h * 0.24).min(13.0), (w.min(h) * 0.15).max(8.0), 4.0));
        }
        "wlan controller" => {
            o.round_rect(r, 4.0);
            symbol = Some(((h * 0.25).min(13.0), 9.0, 4.0));
        }
        "aws transit gateway" | "aws direct connect gateway" => {
            o.round_rect(r, 6.0);
            symbol = Some(((h * 0.25).min(13.0), 9.0, 4.0));
        }
        "virtual switch" | "distributed virtual switch" => {
            o.round_rect(r, 6.0);
            symbol = Some(((h * 0.25).min(13.0), 9.0, 4.0));
        }
        "aci l3out" => {
            // A logical exit rather than a box in a rack: rounder.
            o.round_rect(r, (h * 0.3).min(14.0));
            symbol = Some(((h * 0.25).min(13.0), 10.0, 4.0));
        }
        _ => {
            o.round_rect(r, 5.0);
        }
    }
    s.outline = o.done();
    if container {
        return Shape { outline: s.outline, tile: s.tile, ..Shape::default() };
    }
    s.detail = d.done();
    if let Some((sr, pad, right)) = symbol {
        // Beside the name if it fits there (estimated from its length),
        // else small above it if the box is tall enough.
        let text_w = (name.chars().count() as f64 * 7.0).max(kind_label(&kind).chars().count() as f64 * 6.0) + 4.0;
        let used = pad + 2.0 * sr + 4.0;
        let (mut m, mut f) = (Path::new(), Path::new());
        let mut place = |c: (f64, f64), sr: f64| {
            draw_symbol(&kind, &mut m, &mut f, c, sr);
            s.tile = Tile::new(&kind, c, sr);
        };
        if h >= 36.0 && w - used - right >= text_w.max(56.0) {
            place((x + pad + sr, y + h / 2.0), sr);
            s.label.0 = (used - right) / 2.0;
        } else if h >= 56.0 {
            let sr = 8.0;
            place((x + w / 2.0, y + 5.0 + sr), sr);
            s.label.1 = sr + 1.0;
        }
        s.mark = m.done();
        s.solid = f.done();
    }
    s
}

/// An arrow from `a` to `b` with a filled head of length `head`.
fn arrow(m: &mut Path, f: &mut Path, a: (f64, f64), b: (f64, f64), head: f64) {
    let (dx, dy) = (b.0 - a.0, b.1 - a.1);
    let len = dx.hypot(dy).max(1e-9);
    let (ux, uy) = (dx / len, dy / len);
    let base = (b.0 - ux * head, b.1 - uy * head);
    m.line(a.0, a.1, base.0, base.1);
    let half = head * 0.6;
    f.m(b.0, b.1).l(base.0 - uy * half, base.1 + ux * half).l(base.0 + uy * half, base.1 - ux * half).z();
}

/// The symbol identifying `kind`, around `c` with radius `r`: lines in `m`
/// (stroked) and filled parts in `f`.
fn draw_symbol(kind: &str, m: &mut Path, f: &mut Path, c: (f64, f64), r: f64) {
    let at = |u: f64, v: f64| (c.0 + u * r, c.1 + v * r);
    match kind {
        "router" => {
            // The classic router symbol: four arrows on the diagonals, two pointing
            // out (up right, down left) and two pointing in.
            let k = std::f64::consts::FRAC_1_SQRT_2;
            let head = 0.42 * r;
            for (u, v, out) in [(1.0, -1.0, true), (-1.0, 1.0, true), (-1.0, -1.0, false), (1.0, 1.0, false)] {
                let (near, far) = (at(u * k * 0.12, v * k * 0.12), at(u * k, v * k));
                if out {
                    arrow(m, f, near, far, head);
                } else {
                    arrow(m, f, far, at(u * k * 0.28, v * k * 0.28), head);
                }
            }
        }
        "ospf router" => {
            // The router's arrows inside a ring: routers sharing an area.
            draw_symbol("router", m, f, c, r * 0.78);
            m.ellipse(c.0, c.1, r, r);
        }
        "bgp reflector" => {
            // A route bouncing off a mirror: in from the top left, back out
            // to the bottom left.
            let hit = at(0.45, 0.0);
            f.m(at(0.55, -0.95).0, at(0.55, -0.95).1).l(at(0.85, -0.95).0, at(0.85, -0.95).1);
            f.l(at(0.85, 0.95).0, at(0.85, 0.95).1).l(at(0.55, 0.95).0, at(0.55, 0.95).1).z();
            m.line(at(-0.95, -0.8).0, at(-0.95, -0.8).1, hit.0, hit.1);
            arrow(m, f, hit, at(-0.95, 0.8), 0.42 * r);
        }
        "load balancer" => {
            // One stream in, spread over three.
            let fork = at(-0.15, 0.0);
            m.line(at(-1.0, 0.0).0, c.1, fork.0, fork.1);
            for v in [-0.75, 0.0, 0.75] {
                arrow(m, f, fork, at(0.95, v), 0.4 * r);
            }
        }
        "server" => {
            // A stack of three rack units, each with a status light.
            let (uh, gap) = (0.52 * r, 0.22 * r);
            for i in 0..3 {
                let top = c.1 - 1.0 * r + i as f64 * (uh + gap);
                m.m(c.0 - r, top).l(c.0 + r, top).l(c.0 + r, top + uh).l(c.0 - r, top + uh).z();
                f.ellipse(c.0 + 0.62 * r, top + uh / 2.0, 0.12 * r, 0.12 * r);
                m.line(c.0 - 0.7 * r, top + uh / 2.0, c.0 + 0.2 * r, top + uh / 2.0);
            }
        }
        "access point" => {
            // Radio waves above a dot.
            let o = at(0.0, 0.7);
            let (a0, a1) = (1.25 * PI, 1.75 * PI);
            for k in [0.55, 1.05, 1.55] {
                let rad = k * r;
                m.m(o.0 + rad * a0.cos(), o.1 + rad * a0.sin()).arc(o.0, o.1, rad, rad, a0, a1);
            }
            f.ellipse(o.0, o.1, 0.16 * r, 0.16 * r);
        }
        "standalone ap" => {
            // Radio waves above a dot in a ring: an access point on its own.
            let o = at(0.0, 0.55);
            let (a0, a1) = (1.25 * PI, 1.75 * PI);
            for k in [0.55, 0.95, 1.35] {
                let rad = k * r;
                m.m(o.0 + rad * a0.cos(), o.1 + rad * a0.sin()).arc(o.0, o.1, rad, rad, a0, a1);
            }
            f.ellipse(o.0, o.1, 0.15 * r, 0.15 * r);
            m.ellipse(o.0, o.1, 0.32 * r, 0.32 * r);
        }
        "cloud proxy" => {
            // A shield with a check mark: traffic inspected on its way.
            let p = |u: f64, v: f64| at(u * 0.85, v * 0.95);
            let (top, l, r, bl, br, tip) = (p(0.0, -1.0), p(-0.8, -0.65), p(0.8, -0.65), p(-0.75, 0.1), p(0.75, 0.1), p(0.0, 1.0));
            m.m(top.0, top.1).l(r.0, r.1).l(br.0, br.1);
            m.c(p(0.7, 0.55), p(0.3, 0.8), tip).c(p(-0.3, 0.8), p(-0.7, 0.55), bl).l(l.0, l.1).z();
            let (c0, c1, c2) = (p(-0.38, 0.02), p(-0.08, 0.32), p(0.42, -0.3));
            m.m(c0.0, c0.1).l(c1.0, c1.1).l(c2.0, c2.1);
        }
        "managed ap" => {
            // Radio waves on a dot tied down to a small controller square:
            // run by a wireless LAN controller.
            let o = at(0.0, 0.1);
            let (a0, a1) = (1.25 * PI, 1.75 * PI);
            for k in [0.45, 0.85, 1.25] {
                let rad = k * r;
                m.m(o.0 + rad * a0.cos(), o.1 + rad * a0.sin()).arc(o.0, o.1, rad, rad, a0, a1);
            }
            f.ellipse(o.0, o.1, 0.15 * r, 0.15 * r);
            m.line(o.0, o.1, o.0, at(0.0, 0.62).1);
            let (sq0, sq1) = (at(-0.22, 0.62), at(0.22, 1.0));
            f.m(sq0.0, sq0.1).l(sq1.0, sq0.1).l(sq1.0, sq1.1).l(sq0.0, sq1.1).z();
        }
        "wlan controller" => {
            // A controller unit with radio waves rising from it.
            let (b0, b1) = (at(-0.9, 0.3), at(0.9, 0.95));
            m.m(b0.0, b0.1).l(b1.0, b0.1).l(b1.0, b1.1).l(b0.0, b1.1).z();
            for u in [0.35, 0.65] {
                f.ellipse(at(u, 0.625).0, at(u, 0.625).1, 0.09 * r, 0.09 * r);
            }
            let (l0, l1) = (at(-0.65, 0.625), at(0.1, 0.625));
            m.line(l0.0, l0.1, l1.0, l1.1);
            let o = at(0.0, 0.3);
            let (a0, a1) = (1.25 * PI, 1.75 * PI);
            for k in [0.45, 0.85, 1.25] {
                let rad = k * r;
                m.m(o.0 + rad * a0.cos(), o.1 + rad * a0.sin()).arc(o.0, o.1, rad, rad, a0, a1);
            }
        }
        "aws transit gateway" => {
            // A hub with spokes to the networks it joins.
            f.ellipse(c.0, c.1, 0.26 * r, 0.26 * r);
            for i in 0..6 {
                let a = PI / 6.0 + i as f64 * PI / 3.0;
                let (u, v) = (a.cos(), a.sin());
                let (from, to) = (at(0.26 * u, 0.26 * v), at(0.72 * u, 0.72 * v));
                m.line(from.0, from.1, to.0, to.1);
                let end = at(0.86 * u, 0.86 * v);
                m.ellipse(end.0, end.1, 0.14 * r, 0.14 * r);
            }
        }
        "aci l3out" => {
            // The fabric (a square) with routes leaving it.
            let (a, b) = (at(-1.0, -0.6), at(0.15, 0.6));
            m.m(a.0, a.1).l(b.0, a.1).l(b.0, b.1).l(a.0, b.1).z();
            f.ellipse(at(-0.42, 0.0).0, c.1, 0.14 * r, 0.14 * r);
            arrow(m, f, at(-0.3, 0.0), at(1.0, 0.0), 0.5 * r);
        }
        "virtual switch" => {
            // A switch's arrows, both ways, in a dashed frame: in software.
            dashed_square(m, c, r);
            arrow(m, f, at(-0.6, -0.3), at(0.6, -0.3), 0.42 * r);
            arrow(m, f, at(0.6, 0.3), at(-0.6, 0.3), 0.42 * r);
        }
        "distributed virtual switch" => {
            // The virtual switch's arrows, spread over three hosts tied
            // together below.
            arrow(m, f, at(-0.7, -0.75), at(0.7, -0.75), 0.4 * r);
            arrow(m, f, at(0.7, -0.25), at(-0.7, -0.25), 0.4 * r);
            m.line(at(-0.75, 0.3).0, at(-0.75, 0.3).1, at(0.75, 0.3).0, at(0.75, 0.3).1);
            for u in [-0.75, 0.0, 0.75] {
                let top = at(u, 0.3);
                m.line(top.0, top.1, top.0, at(0.0, 0.55).1);
                let (a, b) = (at(u - 0.22, 0.55), at(u + 0.22, 0.95));
                f.m(a.0, a.1).l(b.0, a.1).l(b.0, b.1).l(a.0, b.1).z();
            }
        }
        "aws direct connect gateway" => {
            // A dedicated link: traffic both ways.
            arrow(m, f, at(-0.95, -0.38), at(0.95, -0.38), 0.5 * r);
            arrow(m, f, at(0.95, 0.38), at(-0.95, 0.38), 0.5 * r);
        }
        _ => {}
    }
}

/// A square of side 2 `r` around `c` drawn as dashes.
fn dashed_square(m: &mut Path, c: (f64, f64), r: f64) {
    let (dash, gap) = (0.3 * r, 0.2 * r);
    let corners = [(-r, -r), (r, -r), (r, r), (-r, r), (-r, -r)];
    for w in corners.windows(2) {
        let (a, b) = ((c.0 + w[0].0, c.1 + w[0].1), (c.0 + w[1].0, c.1 + w[1].1));
        let len = (b.0 - a.0).hypot(b.1 - a.1);
        let (ux, uy) = ((b.0 - a.0) / len, (b.1 - a.1) / len);
        let mut t = 0.0;
        while t < len - 0.01 {
            let e = (t + dash).min(len);
            m.line(a.0 + ux * t, a.1 + uy * t, a.0 + ux * e, a.1 + uy * e);
            t = e + gap;
        }
    }
}

/// The draw.io style for the outline of a device of `kind` (its nearest
/// built-in shape; inner detail like bricks has no draw.io counterpart).
pub fn drawio_style(kind: &str, w: f64, h: f64) -> String {
    match kind.trim().to_lowercase().as_str() {
        "router" | "ospf router" | "bgp reflector" => "rounded=1;arcSize=50;".into(),
        "switch" | "firewall" | "patch panel" => "rounded=0;".into(),
        "load balancer" => {
            let i = (h * 0.3).min(w * 0.15);
            format!("shape=hexagon;perimeter=hexagonPerimeter2;fixedSize=1;size={};", num(i))
        }
        "server" => format!("shape=process;fixedSize=1;size={};", num(7.0f64.min(w / 8.0))),
        "cloud" | "cloud proxy" => "ellipse;shape=cloud;".into(),
        k if provider(k).is_some() => "ellipse;shape=cloud;".into(),
        "internet" => "ellipse;".into(),
        "access point" | "standalone ap" | "managed ap" => "rounded=1;arcSize=30;".into(),
        "virtual switch" | "distributed virtual switch" => "rounded=1;arcSize=10;dashed=1;".into(),
        _ => "rounded=1;arcSize=8;".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// All x,y pairs in path data.
    fn points(d: &str) -> Vec<(f64, f64)> {
        let nums: Vec<f64> = d.split(' ').filter_map(|t| t.parse().ok()).collect();
        nums.chunks(2).map(|c| (c[0], c[1])).collect()
    }

    #[test]
    fn every_kind_has_a_closed_outline_inside_its_box() {
        let r = Rect { x: 100.0, y: 50.0, w: 120.0, h: 60.0 };
        let mut outlines = vec![];
        for kind in KINDS {
            let s = device_shape(kind, "sw-01", &r, false, None);
            assert!(s.outline.starts_with('M') && s.outline.ends_with('Z'), "{kind}: {}", s.outline);
            for t in [&s.outline, &s.detail, &s.mark, &s.solid].iter().flat_map(|d| d.split(' ')).filter(|t| t.parse::<f64>().is_err()) {
                assert!(["M", "L", "C", "Z", ""].contains(&t), "{kind}: unexpected {t}");
            }
            // Curve control points may bulge out a little; the curve itself stays close.
            let inner = [&s.detail, &s.mark, &s.solid].map(|d| points(d)).concat();
            for (px, py) in points(&s.outline).into_iter().chain(inner) {
                assert!(px >= r.x - 3.0 && px <= r.x + r.w + 4.0 && py >= r.y - 3.0 && py <= r.y + r.h + 0.01, "{kind}: {px},{py}");
            }
            if ["router", "switch", "load balancer", "server", "access point", "ospf router", "bgp reflector", "wlan controller", "standalone ap", "managed ap", "aws transit gateway", "aws direct connect gateway", "aci l3out",
                "virtual switch", "distributed virtual switch"]
                .contains(&kind)
            {
                assert!(!s.solid.is_empty(), "{kind} has a symbol");
            }
            if kind == "cloud proxy" {
                assert!(!s.mark.is_empty(), "the shield");
            }
            assert_eq!(s.tile.is_some(), provider(kind).is_some(), "{kind}");
            outlines.push(s.outline);
        }
        outlines.sort();
        outlines.dedup();
        // Switch, firewall and patch panel share a box, the providers the
        // cloud, the router types the router's pill, the AP modes the AP's
        // box, the cloud proxy the cloud, and the two AWS gateways and
        // the two virtual switches a box.
        assert_eq!(outlines.len(), KINDS.len() - 2 - PROVIDERS.len() - 2 - 2 - 1 - 1 - 2);
    }

    #[test]
    fn symbols_go_beside_the_name_or_above_it() {
        let r = Rect { x: 0.0, y: 0.0, w: 120.0, h: 60.0 };
        let short = device_shape("router", "r1", &r, false, None);
        assert!(short.label.0 > 0.0 && short.label.1 == 0.0);
        // "LOAD BALANCER" is too wide to share the row with the symbol.
        let lb = device_shape("load balancer", "lb", &r, false, None);
        assert!(lb.label.0 == 0.0 && lb.label.1 > 0.0 && !lb.solid.is_empty());
        // No room beside a long name, and too low to fit it above.
        let flat = device_shape("router", "core-router-01", &Rect { h: 40.0, ..r }, false, None);
        assert!(flat.solid.is_empty() && flat.label == (0.0, 0.0));
    }

    #[test]
    fn provider_groups_keep_their_tile_in_the_corner() {
        let r = Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 };
        let group = device_shape("AWS", "prod", &r, true, None);
        let tile = group.tile.unwrap();
        assert_eq!((tile.x, tile.y, tile.text), (6.0, 6.0, "aws"));
        assert_ne!(group.outline, device_shape("aws", "prod", &r, false, None).outline, "a box, not a cloud");
    }

    #[test]
    fn containers_have_no_inner_detail() {
        let r = Rect { x: 0.0, y: 0.0, w: 300.0, h: 200.0 };
        assert!(!device_shape("firewall", "", &r, false, None).detail.is_empty());
        assert!(device_shape("firewall", "", &r, true, None).detail.is_empty());
        let router = device_shape("router", "", &r, true, None);
        assert!(router.mark.is_empty() && router.solid.is_empty() && router.label == (0.0, 0.0));
        assert_eq!(device_shape("Firewall ", "", &r, false, None), device_shape("firewall", "", &r, false, None));
    }

    #[test]
    fn patch_panels_widen_with_their_ports_and_number_them() {
        let p = |ports, connector| PatchPanel { ports, connector, uplinks: 0 };
        let size = |panel: PatchPanel| panel_size(&Front { panel, switch: false });
        let (w12, h12) = size(p(12, Connector::Rj45));
        let (w24, h24) = size(p(24, Connector::Rj45));
        let (w48, h48) = size(p(48, Connector::Rj45));
        assert!(w24 > w12 && h24 == h12, "one row, wider");
        assert!(w48 == w24 && h48 > h24, "48 in two rows of 24");
        assert!(size(p(24, Connector::Lc)).0 > w24, "duplex LC needs more room");
        assert_eq!(size(p(2, Connector::Sc)).0, 120.0, "never narrower than a device");
        let r = Rect { x: 10.0, y: 20.0, w: w48, h: h48 };
        let s = device_shape("patch panel", "pp-01", &r, false, Some(&p(48, Connector::Lc)));
        let numbers: Vec<&str> = s.texts.iter().map(|t| t.text.as_str()).collect();
        assert_eq!(numbers.len(), 48);
        assert_eq!((numbers[0], numbers[47]), ("1", "48"));
        // Port 25 starts the second row, under port 1.
        assert!((s.texts[24].x - s.texts[0].x).abs() < 0.01 && s.texts[24].y > s.texts[0].y);
        for t in &s.texts {
            assert!(t.x > r.x + PANEL_EAR - 4.0 && t.x < r.x + r.w - PANEL_EAR + 4.0 && t.y < r.y + r.h, "{t:?}");
        }
        // Each connector draws its own sockets; the name moves to the top.
        let solid = |c| device_shape("patch panel", "", &r, false, Some(&p(48, c))).solid;
        assert!(solid(Connector::Rj45) != solid(Connector::Lc) && solid(Connector::Lc) != solid(Connector::Sc));
        assert!(s.label.1 < 0.0);
        // Without settings: 24 RJ45.
        assert_eq!(device_shape("patch panel", "", &r, false, None).texts.len(), 24);
    }

    #[test]
    fn switches_show_odd_ports_on_top_and_even_ones_below() {
        let panel = PatchPanel { ports: 48, connector: Connector::Rj45, uplinks: 4 };
        let f = Front { panel, switch: true };
        let (w, h) = panel_size(&f);
        let plain = PatchPanel { uplinks: 0, ..panel };
        assert!(w > panel_size(&Front { panel: plain, switch: true }).0, "room for the uplinks");
        assert!(h < panel_size(&Front { panel, switch: false }).1 + 20.0, "two rows");
        let r = Rect { x: 0.0, y: 0.0, w, h };
        let sk = panel_sockets(&f, &r);
        assert_eq!(sk.len(), 52);
        let at = |n: u32| sk.iter().find(|s| s.n == n).unwrap();
        // 1 above 2, 3 to the right of 1; odd ones on top, numbered above.
        assert!((at(1).cx - at(2).cx).abs() < 0.01 && at(2).top > at(1).top);
        assert!(at(3).cx > at(1).cx && at(3).top == at(1).top && !at(1).flip && at(2).flip);
        assert!(at(1).num_y < at(1).top && at(2).num_y > at(2).top);
        // Ports 13 and up are in the next group of twelve.
        assert!(at(13).cx - at(11).cx > at(11).cx - at(9).cx + 1.0);
        // Uplinks 49-52: SFP, apart at the right, also odd on top.
        assert!(at(49).connector == Connector::Sfp && at(48).connector == Connector::Rj45);
        assert!(at(49).cx - at(47).cx > 10.0 && !at(49).flip && at(50).flip);
        assert!(at(52).cx < r.w - PANEL_EAR);
        // Drawn: the numbers, and the name at the top; a switch without
        // ports is its usual box.
        let s = device_shape("switch", "sw-01", &r, false, Some(&panel));
        assert_eq!(s.texts.len(), 52);
        assert!(s.label.1 < 0.0);
        assert!(front_of("switch", None).is_none() && front_of("load balancer", Some(&panel)).is_none());
        // Routers, firewalls and servers can show their ports too, with a
        // switch's connectors.
        for kind in ["router", "ospf router", "firewall", "server"] {
            assert!(front_of(kind, Some(&panel)).is_some_and(|f| f.switch), "{kind}");
            assert!(front_of(kind, Some(&PatchPanel { connector: Connector::Lc, ..panel })).is_none(), "{kind}");
        }
        assert!(device_shape("switch", "sw-01", &r, false, None).texts.is_empty());
        // Bottom-row jacks are upside down: not the same drawing moved down.
        let one = |ports| device_shape("switch", "", &r, false, Some(&PatchPanel { ports, ..plain })).solid;
        assert_ne!(one(1), one(2));
    }

    #[test]
    fn arcs_land_on_the_circle() {
        let mut p = Path::new();
        p.m(10.0, 0.0).arc(0.0, 0.0, 10.0, 10.0, 0.0, PI);
        let end = points(&p.d).last().copied().unwrap();
        assert!((end.0 + 10.0).abs() < 0.01 && end.1.abs() < 0.01);
    }
}
