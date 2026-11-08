// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Diagram data model: devices with ports, links between ports, free text
//! notes, layers, and the on-disk JSON format.
//!
//! Every device, link and note belongs to one layer and has a stacking
//! position `z` that is global: layers only group items (and show or hide
//! them), while `z` alone decides what is drawn in front.

use std::collections::HashSet;
use serde::{Deserialize, Serialize};

pub type Id = u32;

pub const GRID: f64 = 10.0;
pub const MIN_W: f64 = 40.0;
pub const MIN_H: f64 = 30.0;

// Basic geometry

#[derive(Clone, Copy, Debug, PartialEq, Default)]
pub struct Pt {
    pub x: f64,
    pub y: f64,
}

impl Pt {
    pub const fn new(x: f64, y: f64) -> Self {
        Pt { x, y }
    }
    #[allow(clippy::should_implement_trait)]
    pub fn add(self, o: Pt) -> Pt {
        Pt::new(self.x + o.x, self.y + o.y)
    }
    #[allow(clippy::should_implement_trait)]
    pub fn sub(self, o: Pt) -> Pt {
        Pt::new(self.x - o.x, self.y - o.y)
    }
    pub fn scale(self, k: f64) -> Pt {
        Pt::new(self.x * k, self.y * k)
    }
    pub fn len(self) -> f64 {
        self.x.hypot(self.y)
    }
    pub fn dist(self, o: Pt) -> f64 {
        self.sub(o).len()
    }
    pub fn norm(self) -> Pt {
        let l = self.len();
        if l < 1e-9 { Pt::new(1.0, 0.0) } else { self.scale(1.0 / l) }
    }
    /// Perpendicular (rotated 90 degrees counter-clockwise in screen coordinates).
    pub fn perp(self) -> Pt {
        Pt::new(-self.y, self.x)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Rect {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Rect {
    pub fn from_points(a: Pt, b: Pt) -> Rect {
        Rect {
            x: a.x.min(b.x),
            y: a.y.min(b.y),
            w: (a.x - b.x).abs(),
            h: (a.y - b.y).abs(),
        }
    }
    pub fn contains(&self, p: Pt) -> bool {
        p.x >= self.x && p.x <= self.x + self.w && p.y >= self.y && p.y <= self.y + self.h
    }
    pub fn contains_rect(&self, r: &Rect) -> bool {
        r.x >= self.x && r.y >= self.y && r.x + r.w <= self.x + self.w && r.y + r.h <= self.y + self.h
    }
    pub fn inflate(&self, d: f64) -> Rect {
        Rect { x: self.x - d, y: self.y - d, w: self.w + 2.0 * d, h: self.h + 2.0 * d }
    }
    pub fn union(&self, o: &Rect) -> Rect {
        let x0 = self.x.min(o.x);
        let y0 = self.y.min(o.y);
        let x1 = (self.x + self.w).max(o.x + o.w);
        let y1 = (self.y + self.h).max(o.y + o.h);
        Rect { x: x0, y: y0, w: x1 - x0, h: y1 - y0 }
    }
}

#[cfg(test)]
pub fn snap(v: f64) -> f64 {
    snap_to(v, GRID)
}

/// Snap to a grid of `step` (the grid of a deeper zoom level is finer).
pub fn snap_to(v: f64, step: f64) -> f64 {
    if step > 0.0 { (v / step).round() * step } else { v }
}

pub fn dist_to_segment(p: Pt, a: Pt, b: Pt) -> f64 {
    let ab = b.sub(a);
    let l2 = ab.x * ab.x + ab.y * ab.y;
    if l2 < 1e-12 {
        return p.dist(a);
    }
    let t = (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / l2).clamp(0.0, 1.0);
    p.dist(a.add(ab.scale(t)))
}

pub fn dist_to_polyline(p: Pt, pts: &[Pt]) -> f64 {
    pts.windows(2)
        .map(|w| dist_to_segment(p, w[0], w[1]))
        .fold(f64::INFINITY, f64::min)
}

// Enums

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(rename_all = "lowercase")]
pub enum Side {
    Top,
    Right,
    Bottom,
    Left,
}

impl Side {
    pub const ALL: [Side; 4] = [Side::Top, Side::Right, Side::Bottom, Side::Left];

    /// Outward unit normal.
    pub fn normal(self) -> Pt {
        match self {
            Side::Top => Pt::new(0.0, -1.0),
            Side::Right => Pt::new(1.0, 0.0),
            Side::Bottom => Pt::new(0.0, 1.0),
            Side::Left => Pt::new(-1.0, 0.0),
        }
    }
    pub fn is_horizontal(self) -> bool {
        matches!(self, Side::Left | Side::Right)
    }
    pub fn as_str(self) -> &'static str {
        match self {
            Side::Top => "top",
            Side::Right => "right",
            Side::Bottom => "bottom",
            Side::Left => "left",
        }
    }
    pub fn parse(s: &str) -> Option<Side> {
        Side::ALL.into_iter().find(|v| v.as_str() == s)
    }
}

macro_rules! str_enum {
    ($name:ident { $($var:ident => $s:literal),+ $(,)? }) => {
        impl $name {
            pub const ALL: &'static [$name] = &[$($name::$var),+];
            pub fn as_str(self) -> &'static str {
                match self { $($name::$var => $s),+ }
            }
            pub fn parse(s: &str) -> Option<$name> {
                Self::ALL.iter().copied().find(|v| v.as_str() == s)
            }
        }
    };
}

/// Line end decoration.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Head {
    #[default]
    None,
    Arrow,
    Open,
    Diamond,
    Circle,
}
str_enum!(Head { None => "none", Arrow => "arrow", Open => "open", Diamond => "diamond", Circle => "circle" });

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Routing {
    #[default]
    Straight,
    Orthogonal,
    /// Orthogonal, finding a way around devices and texts in between.
    Around,
}
str_enum!(Routing { Straight => "straight", Orthogonal => "orthogonal", Around => "around" });

/// What a link carries, shown by how its line is drawn (a double line for
/// a trunk, a tunnel for IPsec and VXLAN) and by a tag on it.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LinkKind {
    #[default]
    Plain,
    /// 802.1Q trunk.
    Trunk,
    Ipsec,
    Vxlan,
    Sdwan,
    Mpls,
    /// Routed (layer 3) link.
    L3,
    /// Routed subinterfaces over an 802.1Q trunk.
    L3Sub,
    /// Tunnel between an access point and its wireless LAN controller.
    Capwap,
    /// Switched (layer 2) link.
    L2,
}
str_enum!(LinkKind {
    Plain => "plain", Trunk => "trunk", Ipsec => "ipsec", Vxlan => "vxlan", Sdwan => "sdwan", Mpls => "mpls", L3 => "l3",
    L3Sub => "l3sub", Capwap => "capwap", L2 => "l2"
});

impl LinkKind {
    /// The tag shown on the line ("" for a plain link).
    pub fn tag(self) -> &'static str {
        match self {
            LinkKind::Plain => "",
            LinkKind::Trunk => "802.1Q",
            LinkKind::Ipsec => "IPsec",
            LinkKind::Vxlan => "VXLAN",
            LinkKind::Sdwan => "SD-WAN",
            LinkKind::Mpls => "MPLS",
            LinkKind::L3 => "L3",
            LinkKind::L3Sub => "L3 subif",
            LinkKind::Capwap => "CAPWAP",
            LinkKind::L2 => "L2",
        }
    }

    /// Drawn as a double line (802.1Q).
    pub fn is_double(self) -> bool {
        matches!(self, LinkKind::Trunk | LinkKind::L3Sub)
    }

    /// Routed: a small open circle near each end marks the IP interfaces.
    pub fn is_routed(self) -> bool {
        matches!(self, LinkKind::L3 | LinkKind::L3Sub)
    }

    /// Marked near each end: open circles when routed, filled squares
    /// (switch ports) when switched.
    pub fn has_end_marks(self) -> bool {
        self.is_routed() || self == LinkKind::L2
    }

    /// Drawn as a tunnel: a band around the line.
    pub fn is_tunnel(self) -> bool {
        matches!(self, LinkKind::Ipsec | LinkKind::Vxlan | LinkKind::Capwap)
    }

    fn is_plain(&self) -> bool {
        *self == LinkKind::Plain
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum LineStyle {
    #[default]
    Solid,
    Dashed,
    Dotted,
}
str_enum!(LineStyle { Solid => "solid", Dashed => "dashed", Dotted => "dotted" });

// Devices, ports, links

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Port {
    pub id: Id,
    #[serde(default)]
    pub name: String,
    #[serde(default = "default_side")]
    pub side: Side,
    /// Position along the side, 0.0 = start (left/top), 1.0 = end.
    #[serde(default = "half")]
    pub t: f64,
    /// Its own layer, to show or hide it (and its name) apart from its
    /// device; 0 for its device's layer.
    #[serde(default, skip_serializing_if = "is_no_layer")]
    pub layer: Id,
    /// What it stands for elsewhere, e.g. "dcim.device:42" for a NetBox
    /// device ("" when drawn by hand). Refreshing from there matches by it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// The socket it is plugged into on a patch panel (from 1; 0 for a
    /// port on the border). It sits there, `side` only says which way its
    /// lines leave.
    #[serde(default, skip_serializing_if = "is_no_socket")]
    pub socket: u32,
    /// On a patch panel: the connection at the back of its socket (a rear
    /// port), on the panel's edge right above or below the socket (by its
    /// side: top, else bottom).
    #[serde(default, skip_serializing_if = "is_false")]
    pub rear: bool,
}

fn is_no_socket(n: &u32) -> bool {
    *n == 0
}

/// The color of a plain device. A device written without a color (by a
/// script, say) gets its type's color when read (see `repair`).
const DEVICE_COLOR: &str = "#dbeafe";

fn default_device_color() -> String {
    String::new()
}

// Defaults for files written by hand or by scripts: positions and sizes
// can be left to auto layout, ports to face their lines.
fn default_side() -> Side {
    Side::Right
}

fn half() -> f64 {
    0.5
}

fn default_device_w() -> f64 {
    120.0
}

fn default_device_h() -> f64 {
    60.0
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Device {
    pub id: Id,
    #[serde(default)]
    pub name: String,
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub x: f64,
    #[serde(default)]
    pub y: f64,
    #[serde(default = "default_device_w")]
    pub w: f64,
    #[serde(default = "default_device_h")]
    pub h: f64,
    #[serde(default = "default_device_color")]
    pub color: String,
    #[serde(default)]
    pub ports: Vec<Port>,
    #[serde(default)]
    pub layer: Id,
    #[serde(default)]
    pub z: i64,
    /// Drawing scale: 1 at the top level, smaller for items added while
    /// zoomed into another item (fonts, borders, ports and lines shrink
    /// with it, and the item only shows once zoomed in far enough).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f64,
    /// What it stands for elsewhere, e.g. "dcim.device:42" for a NetBox
    /// device ("" when drawn by hand). Refreshing from there matches by it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
    /// The sockets on its front: a patch panel's (none: 24 RJ45), or a
    /// switch's ports (none: drawn as a box).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub panel: Option<PatchPanel>,
}

/// The sockets on the front of a patch panel or a switch.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct PatchPanel {
    pub ports: u32,
    #[serde(default)]
    pub connector: Connector,
    /// A switch's uplink ports (SFP), after its others.
    #[serde(default, skip_serializing_if = "is_no_socket")]
    pub uplinks: u32,
}

impl Default for PatchPanel {
    fn default() -> Self {
        PatchPanel { ports: 24, connector: Connector::Rj45, uplinks: 0 }
    }
}

/// Most ports a patch panel or switch may have.
pub const PANEL_MAX_PORTS: u32 = 288;
/// Most uplinks a switch may have.
pub const MAX_UPLINKS: u32 = 48;

/// A patch panel's socket type.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum Connector {
    /// Copper (8P8C).
    #[default]
    Rj45,
    /// Fiber, duplex LC.
    Lc,
    /// Fiber, SC.
    Sc,
    /// A transceiver cage (SFP, SFP+, QSFP...).
    Sfp,
}

impl Connector {
    pub fn as_str(self) -> &'static str {
        match self {
            Connector::Rj45 => "rj45",
            Connector::Lc => "lc",
            Connector::Sc => "sc",
            Connector::Sfp => "sfp",
        }
    }

    /// From its name, or NetBox's port and interface types ("8p8c",
    /// "lc-apc", "sc-upc", "1000base-t", "10gbase-x-sfpp"...).
    pub fn parse(s: &str) -> Option<Connector> {
        let s = s.trim().to_lowercase();
        let cage = ["sfp", "xfp", "gbic", "cfp", "cxp", "cpak", "x2", "xenpak"];
        match s.as_str() {
            _ if s.ends_with("base-t") || s.ends_with("base-tx") => Some(Connector::Rj45),
            _ if cage.iter().any(|c| s.contains(c)) => Some(Connector::Sfp),
            "rj45" | "rj-45" | "8p8c" | "8p6c" | "8p4c" | "8p2c" | "cat5" | "cat5e" | "cat6" | "cat6a" | "cat7" | "cat8" | "copper" => {
                Some(Connector::Rj45)
            }
            _ if s.starts_with("lc") || s.starts_with("mrj21") || s == "mpo" => Some(Connector::Lc),
            _ if s.starts_with("sc") || s.starts_with("st") || s.starts_with("fc") => Some(Connector::Sc),
            _ => None,
        }
    }
}

impl Device {
    pub fn is_patch_panel(&self) -> bool {
        self.kind.trim().eq_ignore_ascii_case("patch panel")
    }

    /// Its front with sockets: a patch panel's, or a switch's with ports.
    pub fn front(&self) -> Option<crate::shape::Front> {
        crate::shape::front_of(&self.kind, self.panel.as_ref())
    }

    /// Size a patch panel, or a switch with ports, to its sockets (other
    /// devices stay as they are).
    pub fn fit_panel(&mut self) {
        if let Some(f) = self.front() {
            let (w, h) = crate::shape::panel_size(&f);
            (self.w, self.h) = (w * self.scale, h * self.scale);
        }
    }

    pub fn rect(&self) -> Rect {
        Rect { x: self.x, y: self.y, w: self.w, h: self.h }
    }

    pub fn port(&self, id: Id) -> Option<&Port> {
        self.ports.iter().find(|p| p.id == id)
    }

    pub fn point_on_side(&self, side: Side, t: f64) -> Pt {
        match side {
            Side::Top => Pt::new(self.x + self.w * t, self.y),
            Side::Bottom => Pt::new(self.x + self.w * t, self.y + self.h),
            Side::Left => Pt::new(self.x, self.y + self.h * t),
            Side::Right => Pt::new(self.x + self.w, self.y + self.h * t),
        }
    }

    pub fn port_pos(&self, port: &Port) -> Pt {
        if port.rear
            && let Some(p) = self.rear_pos(port.socket, port.side == Side::Top)
        {
            return p;
        }
        self.socket_pos(port.socket).unwrap_or_else(|| self.point_on_side(port.side, port.t))
    }

    /// Where the back of socket `n` of a patch panel comes out: on its top
    /// (`top`) or bottom edge, right above or under the socket.
    pub fn rear_pos(&self, n: u32, top: bool) -> Option<Pt> {
        if n == 0 || !self.is_patch_panel() {
            return None;
        }
        let (sockets, k) = self.sockets();
        let sk = sockets.into_iter().find(|s| s.n == n)?;
        Some(Pt::new(self.x + sk.cx * k, if top { self.y } else { self.y + self.h }))
    }

    /// The way the back of socket `n` comes out, if it is cabled there (a
    /// rear port): up or down.
    pub fn rear_side(&self, n: u32) -> Option<Side> {
        if n == 0 || self.rear_pos(n, false).is_none() {
            return None;
        }
        self.ports.iter().find(|q| q.rear && q.socket == n).map(|q| if q.side == Side::Top { Side::Top } else { Side::Bottom })
    }

    /// Whether socket `n` has a connection at its back (a rear port).
    pub fn has_rear(&self, n: u32) -> bool {
        self.rear_side(n).is_some()
    }

    /// The socket right above or under the spot `t` on the top or bottom
    /// edge of a patch panel (to take a rear port there), if any.
    pub fn rear_socket_at(&self, side: Side, t: f64) -> Option<u32> {
        if !matches!(side, Side::Top | Side::Bottom) || !self.is_patch_panel() {
            return None;
        }
        let x = self.x + self.w * t;
        let (sockets, k) = self.sockets();
        sockets
            .iter()
            .filter(|sk| ((self.x + sk.cx * k) - x).abs() <= crate::shape::socket_half(sk.connector).0 * sk.k * k)
            // The row nearest to that edge.
            .max_by(|a, b| if side == Side::Top { b.top.total_cmp(&a.top) } else { a.top.total_cmp(&b.top) })
            .map(|sk| sk.n)
    }

    /// Where the lines of `port` start: a socket's at the panel's edge
    /// straight above or below it (its cord runs from the socket there),
    /// others' at the port.
    pub fn port_exit(&self, port: &Port) -> Pt {
        if port.rear
            && let Some(p) = self.rear_pos(port.socket, port.side == Side::Top)
        {
            return p;
        }
        let (sockets, k) = self.sockets();
        let Some(sk) = sockets.iter().find(|s| s.n == port.socket && port.socket > 0) else {
            return self.point_on_side(port.side, port.t);
        };
        let up = self.port_side(port) == Side::Top;
        let x = self.x + (sk.cx + crate::shape::cord_dx(sk, up)) * k;
        Pt::new(x, if up { self.y } else { self.y + self.h })
    }

    /// Whether the name of `port` is drawn: not in a socket of a patch
    /// panel or switch, where its number stands for it (and names would
    /// not fit beside each other).
    pub fn name_shown(&self, port: &Port) -> bool {
        !port.name.is_empty() && self.socket_pos(port.socket).is_none()
    }

    /// Which way lines leave `port`: from a socket up or down (not across
    /// the other sockets), else away from its side.
    pub fn port_side(&self, port: &Port) -> Side {
        // At the back of a socket: up or down; in a socket cabled at its back
        // too: the other way, clear of that.
        if port.rear && self.rear_pos(port.socket, false).is_some() {
            return if port.side == Side::Top { Side::Top } else { Side::Bottom };
        }
        if !port.rear
            && self.socket_pos(port.socket).is_some()
            && let Some(back) = self.rear_side(port.socket)
        {
            return if back == Side::Top { Side::Bottom } else { Side::Top };
        }
        match port.side {
            Side::Left | Side::Right if self.socket_pos(port.socket).is_some() => Side::Bottom,
            side => side,
        }
    }

    /// Where socket `n` (from 1) of a patch panel or switch takes its line.
    pub fn socket_pos(&self, n: u32) -> Option<Pt> {
        if n == 0 {
            return None;
        }
        let (sockets, k) = self.sockets();
        let sk = sockets.into_iter().find(|s| s.n == n)?;
        let (x, y) = crate::shape::socket_center(&sk);
        Some(self.to_canvas(Pt::new(x, y), k))
    }

    /// The socket of a patch panel or switch at `p`, if any.
    pub fn socket_at(&self, p: Pt, tol: f64) -> Option<u32> {
        let (sockets, k) = self.sockets();
        sockets.iter().find_map(|sk| {
            let (hw, hh) = crate::shape::socket_half(sk.connector);
            let (x, y) = crate::shape::socket_center(sk);
            let c = self.to_canvas(Pt::new(x, y), k);
            let f = sk.k * k;
            ((p.x - c.x).abs() <= hw * f + tol && (p.y - c.y).abs() <= hh * f + tol).then_some(sk.n)
        })
    }

    /// Which way a line from socket `n` best leaves the panel towards
    /// `to`: up or down, never across the face of the panel.
    pub fn socket_side(&self, n: u32, to: Pt) -> Side {
        if let Some(back) = self.rear_side(n) {
            return if back == Side::Top { Side::Bottom } else { Side::Top };
        }
        let at = self.socket_pos(n).unwrap_or(Pt::new(self.x + self.w / 2.0, self.y + self.h));
        if to.y < at.y - self.h / 2.0 { Side::Top } else { Side::Bottom }
    }

    /// The sockets as drawn: in a box of the panel's size unscaled (the
    /// shape's sizes are at scale 1), with the scale to map them back.
    /// None without a front.
    pub(crate) fn sockets(&self) -> (Vec<crate::shape::PanelSocket>, f64) {
        let k = if self.scale > 0.0 { self.scale } else { 1.0 };
        let Some(f) = self.front() else { return (vec![], k) };
        let r = Rect { x: 0.0, y: 0.0, w: self.w / k, h: self.h / k };
        (crate::shape::panel_sockets(&f, &r), k)
    }

    fn to_canvas(&self, p: Pt, k: f64) -> Pt {
        Pt::new(self.x + p.x * k, self.y + p.y * k)
    }

    /// Nearest side to `p` (by distance to the actual border segment) and
    /// the clamped position along it.
    pub fn nearest_side(&self, p: Pt) -> (Side, f64) {
        let (x0, y0, x1, y1) = (self.x, self.y, self.x + self.w, self.y + self.h);
        let d = [
            (Side::Top, dist_to_segment(p, Pt::new(x0, y0), Pt::new(x1, y0))),
            (Side::Right, dist_to_segment(p, Pt::new(x1, y0), Pt::new(x1, y1))),
            (Side::Bottom, dist_to_segment(p, Pt::new(x0, y1), Pt::new(x1, y1))),
            (Side::Left, dist_to_segment(p, Pt::new(x0, y0), Pt::new(x0, y1))),
        ];
        let side = d.iter().min_by(|a, b| a.1.total_cmp(&b.1)).unwrap().0;
        let t = if side.is_horizontal() {
            (p.y - self.y) / self.h
        } else {
            (p.x - self.x) / self.w
        };
        (side, clamp_t(t))
    }

    /// Position in the largest free gap on a side (used when adding a port
    /// from the properties panel).
    pub fn free_spot(&self, side: Side) -> f64 {
        let mut ts: Vec<f64> = self.ports.iter().filter(|p| p.side == side).map(|p| p.t).collect();
        if ts.is_empty() {
            return 0.5;
        }
        ts.push(0.0);
        ts.push(1.0);
        ts.sort_by(f64::total_cmp);
        let (a, b) = ts
            .windows(2)
            .map(|w| (w[0], w[1]))
            .max_by(|x, y| (x.1 - x.0).total_cmp(&(y.1 - y.0)))
            .unwrap();
        clamp_t((a + b) / 2.0)
    }
}

pub fn clamp_t(t: f64) -> f64 {
    t.clamp(0.05, 0.95)
}

/// One end of a link: a port on a device, or a free connection point
/// (then `device` is the point's id and `port` is 0). Saved as
/// `{"device": d, "port": p}` or `{"point": id}`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[serde(from = "EndpointRepr", into = "EndpointRepr")]
pub struct Endpoint {
    pub device: Id,
    pub port: Id,
}

impl Endpoint {
    pub fn point(id: Id) -> Endpoint {
        Endpoint { device: id, port: 0 }
    }
    pub fn is_point(self) -> bool {
        self.port == 0
    }
    /// The device or point this end is attached to.
    pub fn owner(self) -> Id {
        self.device
    }
}

#[derive(Serialize, Deserialize)]
struct EndpointRepr {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    device: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    port: Option<Id>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    point: Option<Id>,
}

impl From<EndpointRepr> for Endpoint {
    fn from(r: EndpointRepr) -> Self {
        match r.point {
            Some(id) => Endpoint::point(id),
            // A missing port makes the link dangle; it is dropped on load.
            None => Endpoint { device: r.device.unwrap_or(0), port: r.port.unwrap_or(u32::MAX) },
        }
    }
}

impl From<Endpoint> for EndpointRepr {
    fn from(e: Endpoint) -> Self {
        if e.is_point() {
            EndpointRepr { device: None, port: None, point: Some(e.device) }
        } else {
            EndpointRepr { device: Some(e.device), port: Some(e.port), point: None }
        }
    }
}

fn default_link_color() -> String {
    "#334155".into()
}
fn default_width() -> f64 {
    2.0
}

/// Free line kept on each side of a link's label by auto layout.
pub const DEFAULT_LABEL_MARGIN: f64 = 24.0;

fn default_label_margin() -> f64 {
    DEFAULT_LABEL_MARGIN
}

fn is_default_label_margin(v: &f64) -> bool {
    *v == DEFAULT_LABEL_MARGIN
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LinkStyle {
    #[serde(default = "default_link_color")]
    pub color: String,
    #[serde(default = "default_width")]
    pub width: f64,
    #[serde(default)]
    pub style: LineStyle,
    #[serde(default)]
    pub head_start: Head,
    #[serde(default)]
    pub head_end: Head,
    #[serde(default)]
    pub routing: Routing,
    #[serde(default, skip_serializing_if = "LinkKind::is_plain")]
    pub kind: LinkKind,
    /// Free line on each side of the label (and its tag) that auto layout
    /// keeps, so labels do not crowd the devices or the interface names.
    #[serde(default = "default_label_margin", skip_serializing_if = "is_default_label_margin")]
    pub label_margin: f64,
    /// What the line is carried over, innermost first: VXLAN over IPsec is
    /// `kind` vxlan with `over` [ipsec]. Each is drawn as a band around
    /// what it carries.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub over: Vec<LinkKind>,
}

impl LinkStyle {
    /// The tag shown on the line: its type and what carries it, e.g.
    /// "VXLAN over IPsec" ("" for a plain line carried by nothing).
    pub fn full_tag(&self) -> String {
        let tags: Vec<&str> = std::iter::once(self.kind.tag()).chain(self.over.iter().map(|k| k.tag())).filter(|t| !t.is_empty()).collect();
        tags.join(" over ")
    }

    /// The bands drawn around the line, outermost first: each carrier, and
    /// the line's own type if it is a tunnel. With each how far its edge is
    /// from the line (beyond half the line's width), at scale 1.
    pub fn bands(&self) -> Vec<(LinkKind, f64)> {
        let inner = self.kind.is_tunnel().then_some(self.kind);
        let mut out: Vec<(LinkKind, f64)> =
            inner.into_iter().chain(self.over.iter().copied()).enumerate().map(|(i, k)| (k, 6.0 + 7.0 * i as f64)).collect();
        out.reverse();
        out
    }
}

impl Default for LinkStyle {
    fn default() -> Self {
        LinkStyle {
            color: default_link_color(),
            width: default_width(),
            style: LineStyle::Solid,
            head_start: Head::None,
            head_end: Head::None,
            routing: Routing::Straight,
            kind: LinkKind::Plain,
            label_margin: DEFAULT_LABEL_MARGIN,
            over: vec![],
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Link {
    pub id: Id,
    pub from: Endpoint,
    pub to: Endpoint,
    #[serde(flatten)]
    pub style: LinkStyle,
    #[serde(default)]
    pub label: String,
    /// Where the label sits along the line, 0 (start) to 1 (end), when it
    /// was dragged there; else it is placed automatically.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub label_at: Option<f64>,
    /// The label (and type tag) not shown, on screen or in exports; the
    /// text and type are kept.
    #[serde(default, skip_serializing_if = "is_false")]
    pub label_hidden: bool,
    #[serde(default)]
    pub layer: Id,
    #[serde(default)]
    pub z: i64,
    /// Drawing scale: 1 at the top level, smaller for items added while
    /// zoomed into another item (fonts, borders, ports and lines shrink
    /// with it, and the item only shows once zoomed in far enough).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f64,
    /// What it stands for elsewhere, e.g. "dcim.device:42" for a NetBox
    /// device ("" when drawn by hand). Refreshing from there matches by it.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

// Notes (free text annotations)

pub const NOTE_MIN_SIZE: f64 = 6.0;
pub const NOTE_MAX_SIZE: f64 = 96.0;
/// Narrowest box a note can be given.
pub const NOTE_MIN_BOX: f64 = 30.0;
/// Line height as a multiple of the font size.
pub const NOTE_LINE_HEIGHT: f64 = 1.3;

fn default_note_size() -> f64 {
    14.0
}
fn default_note_color() -> String {
    "#334155".into()
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Note {
    pub id: Id,
    /// May span several lines (`\n`).
    pub text: String,
    /// Top left corner of the text.
    pub x: f64,
    pub y: f64,
    /// Font size in pixels.
    #[serde(default = "default_note_size")]
    pub size: f64,
    #[serde(default = "default_note_color")]
    pub color: String,
    #[serde(default)]
    pub bold: bool,
    #[serde(default)]
    pub layer: Id,
    #[serde(default)]
    pub z: i64,
    /// Drawing scale: 1 at the top level, smaller for items added while
    /// zoomed into another item (fonts, borders, ports and lines shrink
    /// with it, and the item only shows once zoomed in far enough).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f64,
    /// A fixed box (at the note's own scale, like `size`), or 0 to fit the
    /// text. In a box the text wraps at its width and is cut at its height.
    #[serde(default, skip_serializing_if = "is_zero")]
    pub w: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub h: f64,
}

impl Note {
    pub fn lines(&self) -> impl Iterator<Item = &str> {
        self.text.split('\n')
    }

    /// Has a fixed box (see `w` and `h`).
    pub fn boxed(&self) -> bool {
        self.w > 0.0 && self.h > 0.0
    }

    /// Average glyph width, for estimating text widths (Rust has no font
    /// metrics; the canvas measures properly).
    fn glyph(&self) -> f64 {
        self.font_size() * if self.bold { 0.64 } else { 0.58 }
    }

    /// The lines as shown: in a box, wrapped at its width and cut at its
    /// height (the last line shown then ends with "…").
    pub fn shown_lines(&self) -> Vec<String> {
        if !self.boxed() {
            return self.lines().map(String::from).collect();
        }
        let per_line = ((self.w * self.scale / self.glyph()).floor() as usize).max(1);
        let mut out: Vec<String> = vec![];
        for para in self.lines() {
            let mut line = String::new();
            for word in para.split(' ') {
                let mut word: String = word.into();
                // Words longer than a line are broken.
                while word.chars().count() > per_line {
                    if !line.is_empty() {
                        out.push(std::mem::take(&mut line));
                    }
                    let rest: String = word.chars().skip(per_line).collect();
                    out.push(word.chars().take(per_line).collect());
                    word = rest;
                }
                let len = line.chars().count() + usize::from(!line.is_empty()) + word.chars().count();
                if !line.is_empty() && len > per_line {
                    out.push(std::mem::replace(&mut line, word));
                } else if line.is_empty() {
                    line = word;
                } else {
                    line = format!("{line} {word}");
                }
            }
            out.push(line);
        }
        let fit = ((self.h * self.scale / (self.font_size() * NOTE_LINE_HEIGHT)).floor() as usize).max(1);
        if out.len() > fit {
            out.truncate(fit);
            if let Some(last) = out.last_mut() {
                let keep = last.chars().count().min(per_line.saturating_sub(1));
                *last = last.chars().take(keep).collect::<String>() + "…";
            }
        }
        out
    }

    /// The note's box: its fixed one, or estimated around the text.
    pub fn rect(&self) -> Rect {
        if self.boxed() {
            return Rect { x: self.x, y: self.y, w: self.w * self.scale, h: self.h * self.scale };
        }
        let size = self.font_size();
        let chars = self.lines().map(|l| l.chars().count()).max().unwrap_or(0).max(1);
        let lines = self.lines().count().max(1);
        Rect { x: self.x, y: self.y, w: chars as f64 * self.glyph(), h: lines as f64 * size * NOTE_LINE_HEIGHT }
    }

    /// Font size as drawn: the nominal size at the note's scale.
    pub fn font_size(&self) -> f64 {
        self.size * self.scale
    }
}

// Connection points

/// A free x,y point links can attach to, so a line can end at nothing in
/// particular (an arrow pointing out of the drawing). Shown as a small
/// handle in the editor, never in exports.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Point {
    pub id: Id,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub layer: Id,
    #[serde(default)]
    pub z: i64,
    /// Drawing scale: 1 at the top level, smaller for items added while
    /// zoomed into another item (fonts, borders, ports and lines shrink
    /// with it, and the item only shows once zoomed in far enough).
    #[serde(default = "one", skip_serializing_if = "is_one")]
    pub scale: f64,
    /// Text beside the point, beyond the end of its line (e.g. what an
    /// arrow points at).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub label: String,
}

impl Point {
    pub fn pos(&self) -> Pt {
        Pt::new(self.x, self.y)
    }
}

/// Side of a point at `at` that faces `toward`, used to route links to and
/// from connection points.
pub fn facing_side(at: Pt, toward: Pt) -> Side {
    let d = toward.sub(at);
    if d.x.abs() >= d.y.abs() {
        if d.x >= 0.0 { Side::Right } else { Side::Left }
    } else if d.y >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    }
}

// Layers and stacking order

fn yes() -> bool {
    true
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Layer {
    pub id: Id,
    pub name: String,
    #[serde(default = "yes")]
    pub visible: bool,
    /// Read only: its items are shown but cannot be selected or changed.
    #[serde(default, skip_serializing_if = "is_false")]
    pub locked: bool,
    /// Show the layer's name in the lower right corner of its devices.
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_name: bool,
    /// A frame drawn around the whole diagram, under everything else.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<Frame>,
    /// A rectangle behind everything on the layer, grouping it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub background: Option<LayerBackground>,
    /// The layer it is a sub-layer of (0: none). A hidden or locked layer
    /// hides or locks its sub-layers too.
    #[serde(default, skip_serializing_if = "is_no_layer")]
    pub parent: Id,
    /// Its sub-layers listed under it (else folded away).
    #[serde(default, skip_serializing_if = "is_false")]
    pub expanded: bool,
    /// What it stands for elsewhere, like a device's `source` (a NetBox
    /// device's own layer: "dcim.device:42"); "" if nothing.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub source: String,
}

impl Layer {
    pub fn new(id: Id, name: &str) -> Layer {
        Layer {
            id,
            name: name.into(),
            visible: true,
            locked: false,
            show_name: false,
            frame: None,
            background: None,
            parent: 0,
            expanded: false,
            source: String::new(),
        }
    }
}

/// How a layer background's border is drawn.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum BorderStyle {
    None,
    #[default]
    Solid,
    Dashed,
    Dotted,
}
str_enum!(BorderStyle { None => "none", Solid => "solid", Dashed => "dashed", Dotted => "dotted" });

fn default_bg_margin() -> f64 {
    30.0
}
fn default_bg_fill() -> String {
    "#e0f2fe".into()
}
fn default_bg_opacity() -> f64 {
    50.0
}
fn default_bg_border_color() -> String {
    "#7dd3fc".into()
}
fn default_bg_border_width() -> f64 {
    1.5
}
fn default_bg_radius() -> f64 {
    14.0
}

/// A layer's background: a rectangle around everything on the layer, with
/// a margin, behind it all.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct LayerBackground {
    /// Space between the layer's items and the rectangle's edge.
    #[serde(default = "default_bg_margin")]
    pub margin: f64,
    #[serde(default = "default_bg_fill")]
    pub fill: String,
    /// Opacity of the fill, in percent (0 is clear, 100 solid).
    #[serde(default = "default_bg_opacity")]
    pub opacity: f64,
    #[serde(default)]
    pub border: BorderStyle,
    #[serde(default = "default_bg_border_color")]
    pub border_color: String,
    #[serde(default = "default_bg_border_width")]
    pub border_width: f64,
    /// Rounded corners (with `radius`), else square ones.
    #[serde(default = "yes")]
    pub rounded: bool,
    #[serde(default = "default_bg_radius")]
    pub radius: f64,
}

impl Default for LayerBackground {
    fn default() -> Self {
        LayerBackground {
            margin: default_bg_margin(),
            fill: default_bg_fill(),
            opacity: default_bg_opacity(),
            border: BorderStyle::Solid,
            border_color: default_bg_border_color(),
            border_width: default_bg_border_width(),
            rounded: true,
            radius: default_bg_radius(),
        }
    }
}

// Frame (border with header and footer around the diagram)

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Default)]
#[serde(rename_all = "lowercase")]
pub enum FrameTemplate {
    /// A plain outline with ruled header and footer.
    #[default]
    Lines,
    /// Technical drawing: double border and a labeled table at the bottom.
    TitleBlock,
    /// A colored header band and a tinted footer band.
    Banner,
    /// No outline: a title above, a rule and small print below.
    Minimal,
    /// An accent bar down the left, company name or logo top right.
    Corporate,
    /// Like a document page: centered title, rules, page number.
    Report,
    /// A rounded card with a soft shadow.
    Card,
    /// Technical drawing with coordinate zones and a full title block.
    Iso,
    /// The title block with a revision table above it.
    Revision,
    /// White lines and text on blueprint blue, with a grid.
    Blueprint,
    /// Change ID, status badge and approver.
    Change,
    /// Colored bars at top and bottom with the classification level.
    Classification,
    /// Site name, address and code, and a legend of the line types used.
    Site,
    /// Document control: ID, version, owner, review date; "uncontrolled
    /// when printed".
    Controlled,
    /// Signature boxes: prepared, reviewed and approved by.
    SignOff,
    /// Project, version, status badge, owner and reviewers.
    Solution,
    /// Customer and own logo, prepared for and by, confidentiality notice.
    Proposal,
    /// A 16:9 slide around the diagram.
    Presentation,
    /// A title band and a summary text beside the diagram.
    Executive,
    /// An AS-BUILT stamp with installer, commissioning and acceptance.
    AsBuilt,
    /// Incident ID, severity badge, start and end, affected services.
    Incident,
    /// A location path (region, site, building, room, rack) and its code.
    Location,
    /// Dark wall display with a status pill.
    Noc,
}
str_enum!(FrameTemplate {
    Lines => "lines", TitleBlock => "titleblock", Banner => "banner", Minimal => "minimal", Corporate => "corporate",
    Report => "report", Card => "card", Iso => "iso", Revision => "revision", Blueprint => "blueprint",
    Change => "change", Classification => "classification", Site => "site", Controlled => "controlled",
    SignOff => "signoff", Solution => "solution", Proposal => "proposal", Presentation => "presentation",
    Executive => "executive", AsBuilt => "asbuilt", Incident => "incident", Location => "location", Noc => "noc"
});

/// One row of a revision table.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Revision {
    #[serde(default)]
    pub rev: String,
    #[serde(default)]
    pub date: String,
    #[serde(default)]
    pub change: String,
    #[serde(default)]
    pub author: String,
}

fn default_frame_margin() -> f64 {
    40.0
}

fn default_frame_color() -> String {
    "#1e293b".into()
}

/// What a frame shows and how. It sizes itself to the diagram.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct Frame {
    #[serde(default)]
    pub template: FrameTemplate,
    #[serde(default)]
    pub title: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub subtitle: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub author: String,
    /// Free text for the footer, e.g. "Confidential" or a version.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub note: String,
    /// Show the date and time of the last change.
    #[serde(default = "yes")]
    pub show_date: bool,
    /// Space between the diagram and the frame.
    #[serde(default = "default_frame_margin")]
    pub margin: f64,
    #[serde(default = "default_frame_color")]
    pub color: String,
    /// Company name (corporate template).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub company: String,
    /// Drawing number, change ID or site code, depending on the template.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub doc_id: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub revision: String,
    /// Checked or approved by.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub approver: String,
    /// Change status: draft, approved or implemented.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub status: String,
    /// Classification: public, internal, confidential or restricted.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub level: String,
    /// Site address.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub address: String,
    /// Page or sheet, e.g. "1/3".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub page: String,
    /// Show the file name in the footer.
    #[serde(default, skip_serializing_if = "is_false")]
    pub show_file: bool,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub revisions: Vec<Revision>,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub version: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub owner: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub review_date: String,
    /// Reviewed by.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub reviewer: String,
    /// Who it is prepared for (customer proposal).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub customer: String,
    /// A few sentences beside the diagram (executive summary).
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub summary: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub installer: String,
    /// Commissioning date.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub commissioned: String,
    /// Incident severity, P1 to P4.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub severity: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub start: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub end: String,
    /// Affected services.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub services: String,
    /// Location path, e.g. "EU > Berlin > HQ > Room 2.14".
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub location: String,
    /// Large faint text across the diagram, e.g. DRAFT.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub watermark: String,
    /// A PNG or JPEG as a data URI, kept in the file.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub logo: String,
    /// The logo's size in pixels (for its proportions).
    #[serde(default, skip_serializing_if = "is_zero")]
    pub logo_w: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub logo_h: f64,
    /// A second logo, e.g. the customer's.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub logo2: String,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub logo2_w: f64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub logo2_h: f64,
}

fn is_zero(v: &f64) -> bool {
    *v == 0.0
}

impl Default for Frame {
    fn default() -> Self {
        Frame {
            template: FrameTemplate::Lines,
            title: "Network diagram".into(),
            subtitle: String::new(),
            author: String::new(),
            note: String::new(),
            show_date: true,
            margin: default_frame_margin(),
            color: default_frame_color(),
            company: String::new(),
            doc_id: String::new(),
            revision: String::new(),
            approver: String::new(),
            status: String::new(),
            level: String::new(),
            address: String::new(),
            page: String::new(),
            show_file: false,
            revisions: vec![],
            version: String::new(),
            owner: String::new(),
            review_date: String::new(),
            reviewer: String::new(),
            customer: String::new(),
            summary: String::new(),
            installer: String::new(),
            commissioned: String::new(),
            severity: String::new(),
            start: String::new(),
            end: String::new(),
            services: String::new(),
            location: String::new(),
            watermark: String::new(),
            logo: String::new(),
            logo_w: 0.0,
            logo_h: 0.0,
            logo2: String::new(),
            logo2_w: 0.0,
            logo2_h: 0.0,
        }
    }
}

fn is_no_layer(id: &Id) -> bool {
    *id == 0
}

fn is_false(b: &bool) -> bool {
    !*b
}

fn one() -> f64 {
    1.0
}

fn is_one(v: &f64) -> bool {
    *v == 1.0
}

/// A device, link or note, by index into its list.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Item {
    Link(usize),
    Device(usize),
    Note(usize),
    Point(usize),
}

// Diagram

#[derive(Clone, Debug, PartialEq)]
pub struct Diagram {
    pub devices: Vec<Device>,
    pub links: Vec<Link>,
    pub notes: Vec<Note>,
    pub points: Vec<Point>,
    /// Never empty.
    pub layers: Vec<Layer>,
    /// Layer that new items go into.
    pub active_layer: Id,
    /// Local date and time of the last change ("YYYY-MM-DD HH:MM").
    pub changed: Option<String>,
    /// The sheet's name, in a document of several ("" for the default,
    /// "Sheet 1", "Sheet 2"...).
    pub name: String,
    /// What the sheet stands for elsewhere, like a device's `source`
    /// ("dcim.site:1", "dcim.location:4" for a NetBox location); "" if
    /// nothing.
    pub source: String,
    /// Cabling that runs on through devices, from end to end (see
    /// [`Trace`]).
    pub traces: Vec<Trace>,
    next_id: Id,
}

/// Cabling that runs on through devices (in at a patch panel's front port
/// and on from its rear port, say): its lines one after another, from one
/// end to the other. With devices along it hidden, it is drawn through
/// them, between the ones still shown (see [`Diagram::visible_only`]).
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Default)]
pub struct Trace {
    pub links: Vec<Id>,
}

impl Default for Diagram {
    fn default() -> Self {
        let layer = Layer::new(1, "Layer 1");
        Diagram {
            devices: vec![],
            links: vec![],
            notes: vec![],
            points: vec![],
            layers: vec![layer],
            active_layer: 1,
            changed: None,
            name: String::new(),
            source: String::new(),
            traces: vec![],
            next_id: 2,
        }
    }
}

pub const FORMAT_NAME: &str = "stratagram";
/// The format's name before the app was renamed; such files still open.
pub const LEGACY_FORMAT_NAME: &str = "netdiag";
pub const FORMAT_VERSION: u32 = 1;

/// A file: its first sheet, with the format, and the further sheets (if
/// any) in `sheets`, so a one-sheet file reads as it always did and older
/// versions open the first sheet of a newer file.
#[derive(Serialize, Deserialize)]
struct DiagramFile {
    format: String,
    version: u32,
    #[serde(flatten)]
    first: SheetFile,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    sheets: Vec<SheetFile>,
}

#[derive(Serialize, Deserialize)]
struct SheetFile {
    #[serde(default, skip_serializing_if = "String::is_empty")]
    name: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    source: String,
    #[serde(default)]
    devices: Vec<Device>,
    #[serde(default)]
    links: Vec<Link>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    notes: Vec<Note>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    points: Vec<Point>,
    #[serde(default)]
    layers: Vec<Layer>,
    #[serde(default)]
    active_layer: Id,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    changed: Option<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    traces: Vec<Trace>,
}

impl Diagram {
    pub fn alloc_id(&mut self) -> Id {
        self.next_id = self.next_id.max(1);
        let id = self.next_id;
        self.next_id += 1;
        id
    }

    pub fn device(&self, id: Id) -> Option<&Device> {
        self.devices.iter().find(|d| d.id == id)
    }
    pub fn device_mut(&mut self, id: Id) -> Option<&mut Device> {
        self.devices.iter_mut().find(|d| d.id == id)
    }
    pub fn link(&self, id: Id) -> Option<&Link> {
        self.links.iter().find(|l| l.id == id)
    }
    pub fn link_mut(&mut self, id: Id) -> Option<&mut Link> {
        self.links.iter_mut().find(|l| l.id == id)
    }

    pub fn note(&self, id: Id) -> Option<&Note> {
        self.notes.iter().find(|n| n.id == id)
    }
    pub fn note_mut(&mut self, id: Id) -> Option<&mut Note> {
        self.notes.iter_mut().find(|n| n.id == id)
    }

    pub fn point(&self, id: Id) -> Option<&Point> {
        self.points.iter().find(|p| p.id == id)
    }
    pub fn point_mut(&mut self, id: Id) -> Option<&mut Point> {
        self.points.iter_mut().find(|p| p.id == id)
    }

    /// Top left corner of a device or note, or a connection point's position.
    pub fn item_pos(&self, id: Id) -> Option<Pt> {
        self.device(id)
            .map(|d| Pt::new(d.x, d.y))
            .or_else(|| self.note(id).map(|n| Pt::new(n.x, n.y)))
            .or_else(|| self.point(id).map(Point::pos))
    }
    pub fn set_item_pos(&mut self, id: Id, p: Pt) {
        if let Some(d) = self.device_mut(id) {
            (d.x, d.y) = (p.x, p.y);
        } else if let Some(n) = self.note_mut(id) {
            (n.x, n.y) = (p.x, p.y);
        } else if let Some(q) = self.point_mut(id) {
            (q.x, q.y) = (p.x, p.y);
        }
    }

    /// Position and side of a link end. A connection point has no side of
    /// its own; this gives Right, and [`crate::geom::link_route`] turns it
    /// to face the other end.
    pub fn endpoint_pos(&self, ep: Endpoint) -> Option<(Pt, Side)> {
        if ep.is_point() {
            return self.point(ep.device).map(|p| (p.pos(), Side::Right));
        }
        let d = self.device(ep.device)?;
        let p = d.port(ep.port)?;
        Some((d.port_exit(p), d.port_side(p)))
    }

    pub fn add_device(&mut self, name: &str, kind: &str, r: Rect, color: &str) -> Id {
        let id = self.alloc_id();
        let (layer, z) = (self.target_layer(), self.top_z());
        self.devices.push(Device {
            id,
            name: name.into(),
            kind: kind.into(),
            x: r.x,
            y: r.y,
            w: r.w,
            h: r.h,
            color: color.into(),
            ports: vec![],
            layer,
            z,
            scale: 1.0,
            source: String::new(),
            panel: None,
        });
        id
    }

    pub fn add_point(&mut self, at: Pt) -> Id {
        let id = self.alloc_id();
        let (layer, z) = (self.target_layer(), self.top_z());
        self.points.push(Point { id, x: at.x, y: at.y, layer, z, scale: 1.0, label: String::new() });
        id
    }

    /// Move every link end at `from` to `to`.
    pub fn reattach(&mut self, from: Endpoint, to: Endpoint) {
        for l in &mut self.links {
            if l.from == from {
                l.from = to;
            }
            if l.to == from {
                l.to = to;
            }
        }
    }

    /// Turn a port into a free connection point at `at`, keeping its links.
    /// The point goes on the device's layer, in front.
    pub fn detach_port(&mut self, ep: Endpoint, at: Pt) -> Option<Id> {
        let d = self.device(ep.device)?;
        d.port(ep.port)?;
        let (layer, scale) = (d.layer, d.scale);
        let (id, z) = (self.alloc_id(), self.top_z());
        self.points.push(Point { id, x: at.x, y: at.y, layer, z, scale, label: String::new() });
        self.reattach(ep, Endpoint::point(id));
        self.remove_port(ep.device, ep.port);
        Some(id)
    }

    /// Turn a connection point into a port on `device` (a new one at
    /// `side`/`t`, or the existing `port`), keeping its links.
    pub fn attach_point(&mut self, point: Id, device: Id, port: Option<Id>, side: Side, t: f64) -> Option<Endpoint> {
        self.point(point)?;
        let ep = match port {
            Some(port) => Endpoint { device, port },
            None => self.add_port(device, side, t)?,
        };
        self.reattach(Endpoint::point(point), ep);
        self.points.retain(|p| p.id != point);
        Some(ep)
    }

    /// Split link `id` in two at a new connection point at `at`, so its line
    /// can be bent there. The first half keeps the link (and its label and
    /// start head), the second gets a copy of its style and the end head.
    pub fn split_link(&mut self, id: Id, at: Pt) -> Option<Id> {
        let l = self.link(id)?.clone();
        let (point, z) = (self.alloc_id(), self.top_z());
        self.points.push(Point { id: point, x: at.x, y: at.y, layer: l.layer, z, scale: l.scale, label: String::new() });
        let second = self.alloc_id();
        let style = LinkStyle { head_start: Head::None, ..l.style.clone() };
        let to = l.to;
        let first = self.link_mut(id)?;
        first.to = Endpoint::point(point);
        first.style.head_end = Head::None;
        self.links.push(Link { id: second, from: Endpoint::point(point), to, style, label: String::new(), label_at: None, ..l });
        Some(point)
    }

    /// Undo a [`split_link`](Self::split_link): if exactly two links meet at
    /// connection point `id` (and go to different places), join them into
    /// one and remove the point. Returns whether it did.
    pub fn heal_point(&mut self, id: Id) -> bool {
        let here = Endpoint::point(id);
        let at: Vec<Id> = self.links.iter().filter(|l| l.from == here || l.to == here).map(|l| l.id).collect();
        let [a, b] = at[..] else { return false };
        if !self.join(a, b, |ep| ep == here) {
            return false;
        }
        self.points.retain(|p| p.id != id);
        true
    }

    /// Put device `dev` into link `id`, in between its ends: the link now
    /// ends at a new port on `dev`, and a copy (same type, label and style)
    /// goes on from a second new port to where it went. The start arrow
    /// stays on the first half, the end arrow moves to the second. Each port
    /// is on the side facing the end it leads to. Returns the new link.
    pub fn insert_into_link(&mut self, id: Id, dev: Id) -> Option<Id> {
        let l = self.link(id)?.clone();
        let (a, _) = self.endpoint_pos(l.from)?;
        let (b, _) = self.endpoint_pos(l.to)?;
        let d = self.device(dev)?;
        let c = Pt::new(d.x + d.w / 2.0, d.y + d.h / 2.0);
        // Compare directions relative to the box's shape, so a wide box
        // takes lines left and right on its short sides.
        let side_toward = |p: Pt| {
            let (dx, dy) = ((p.x - c.x) / d.w, (p.y - c.y) / d.h);
            if dx.abs() >= dy.abs() {
                if dx >= 0.0 { Side::Right } else { Side::Left }
            } else if dy >= 0.0 {
                Side::Bottom
            } else {
                Side::Top
            }
        };
        let (sa, sb) = (side_toward(a), side_toward(b));
        let (mut ta, mut tb) = (0.5, 0.5);
        if sa == sb {
            // Both on one side: in the order the ends lie along it.
            let along = |p: Pt| if sa.is_horizontal() { p.y } else { p.x };
            (ta, tb) = if along(a) <= along(b) { (0.3, 0.7) } else { (0.7, 0.3) };
        }
        let pa = self.add_port(dev, sa, ta)?;
        let pb = self.add_port(dev, sb, tb)?;
        let second = self.alloc_id();
        let first = self.link_mut(id)?;
        first.to = pa;
        first.style.head_end = Head::None;
        let style = LinkStyle { head_start: Head::None, ..l.style.clone() };
        self.links.push(Link { id: second, from: pb, to: l.to, style, label_at: None, ..l });
        Some(second)
    }

    /// Before device `id` goes away: join its links that pass through it
    /// back into one. Links pair up by type and label; a pair that is the
    /// only one with that type and label becomes a single link between
    /// their far ends (keeping the arrows there). Returns how many were joined.
    pub fn join_through_device(&mut self, id: Id) -> usize {
        let at: Vec<Id> = self
            .links
            .iter()
            .filter(|l| !l.from.is_point() && l.from.device == id || !l.to.is_point() && l.to.device == id)
            .map(|l| l.id)
            .collect();
        let mut joined = 0;
        for (i, &a) in at.iter().enumerate() {
            let Some(la) = self.link(a) else { continue };
            let key = (la.style.kind, la.label.clone());
            let same: Vec<Id> = at
                .iter()
                .copied()
                .filter(|x| self.link(*x).is_some_and(|l| (l.style.kind, l.label.clone()) == key))
                .collect();
            if let [x, y] = same[..]
                && x == a
                && at[i + 1..].contains(&y)
                && self.join(x, y, |ep| !ep.is_point() && ep.device == id)
            {
                joined += 1;
            }
        }
        joined
    }

    /// Join links `a` and `b`, which meet at the ends `here` matches, into
    /// one link (`a`) between their other ends. Not done for a link with
    /// both ends there, or when both go to the same place.
    fn join(&mut self, a: Id, b: Id, here: impl Fn(Endpoint) -> bool) -> bool {
        let (Some(la), Some(lb)) = (self.link(a), self.link(b)) else { return false };
        // The far end of a link, and the arrow drawn there.
        let far = |l: &Link| match (here(l.from), here(l.to)) {
            (true, false) => Some((l.to, l.style.head_end)),
            (false, true) => Some((l.from, l.style.head_start)),
            _ => None,
        };
        let (Some((start, head_start)), Some((end, head_end))) = (far(la), far(lb)) else { return false };
        if start == end {
            return false;
        }
        let label = lb.label.clone();
        let first = self.link_mut(a).unwrap();
        (first.from, first.to) = (start, end);
        (first.style.head_start, first.style.head_end) = (head_start, head_end);
        if first.label.is_empty() {
            first.label = label;
        }
        self.links.retain(|l| l.id != b);
        true
    }

    /// Remove a connection point and the links attached to it.
    pub fn remove_point(&mut self, id: Id) {
        self.points.retain(|p| p.id != id);
        self.links.retain(|l| l.from.owner() != id && l.to.owner() != id);
    }

    pub fn add_note(&mut self, text: &str, at: Pt) -> Id {
        let id = self.alloc_id();
        let (layer, z) = (self.target_layer(), self.top_z());
        self.notes.push(Note {
            id,
            text: text.into(),
            x: at.x,
            y: at.y,
            size: default_note_size(),
            color: default_note_color(),
            bold: false,
            layer,
            z,
            scale: 1.0,
            w: 0.0,
            h: 0.0,
        });
        id
    }

    /// The cords to draw on patch panel `d`: its sockets with lines, and
    /// whether each leaves upwards.
    /// The cords to draw on patch panel or switch `d`: its sockets with
    /// lines, whether each leaves upwards, and whether it is the socket's
    /// back (a rear port, drawn as a tie down to the panel's edge).
    pub fn panel_cords(&self, d: &Device) -> Vec<(u32, bool, bool)> {
        d.ports
            .iter()
            .filter(|p| d.socket_pos(p.socket).is_some() && self.links_at_port(p.id) > 0)
            .map(|p| (p.socket, d.port_side(p) == Side::Top, p.rear && d.rear_pos(p.socket, false).is_some()))
            .collect()
    }

    /// A port on a patch panel's bottom edge right under a socket is that
    /// socket's rear port; moved away, it is a port on the border again.
    pub fn settle_rear(&mut self, ep: Endpoint) {
        let Some(d) = self.device(ep.device) else { return };
        let Some(p) = d.port(ep.port) else { return };
        if !p.rear && p.socket != 0 {
            return; // in a socket at the front
        }
        let socket = d.rear_socket_at(p.side, p.t);
        if let Some(p) = self.device_mut(ep.device).and_then(|d| d.ports.iter_mut().find(|p| p.id == ep.port)) {
            (p.rear, p.socket) = (socket.is_some(), socket.unwrap_or(0));
        }
    }

    /// The port in socket `n` of patch panel `device`: the one there, or a
    /// new one whose lines leave towards `side`.
    pub fn socket_port(&mut self, device: Id, n: u32, side: Side) -> Option<Endpoint> {
        let d = self.device(device)?;
        d.socket_pos(n)?;
        if let Some(p) = d.ports.iter().find(|p| p.socket == n && !p.rear) {
            return Some(Endpoint { device, port: p.id });
        }
        let ep = self.add_port(device, side, 0.5)?;
        let p = self.device_mut(device)?.ports.iter_mut().find(|p| p.id == ep.port)?;
        p.socket = n;
        Some(ep)
    }

    /// A new port on `device`. Like other new items it goes into the
    /// active layer, when that is not the device's own.
    pub fn add_port(&mut self, device: Id, side: Side, t: f64) -> Option<Endpoint> {
        let id = self.alloc_id();
        let active = self.active_layer;
        let own_layer = self.layer(active).is_some_and(|l| l.frame.is_none());
        let d = self.device_mut(device)?;
        let layer = if own_layer && active != d.layer { active } else { 0 };
        d.ports.push(Port { id, name: String::new(), side, t: clamp_t(t), layer, source: String::new(), socket: 0, rear: false });
        Some(Endpoint { device, port: id })
    }

    /// Whether a port of `d` is shown: its device is, and its own layer if
    /// it has one.
    pub fn port_shown(&self, d: &Device, p: &Port) -> bool {
        self.layer_visible(d.layer) && (p.layer == 0 || self.layer_visible(p.layer))
    }

    /// Whether a port of `d` can be clicked and changed: shown, and
    /// neither its device's layer nor its own is locked.
    pub fn port_editable(&self, d: &Device, p: &Port) -> bool {
        self.port_shown(d, p) && !self.layer_locked(d.layer) && (p.layer == 0 || !self.layer_locked(p.layer))
    }

    pub fn add_link(&mut self, from: Endpoint, to: Endpoint, style: LinkStyle) -> Id {
        let id = self.alloc_id();
        let (layer, z) = (self.target_layer(), self.top_z());
        self.links.push(Link { id, from, to, style, label: String::new(), label_at: None, label_hidden: false, layer, z, scale: 1.0, source: String::new() });
        id
    }

    pub fn links_at_port(&self, port: Id) -> usize {
        self.links.iter().filter(|l| l.from.port == port || l.to.port == port).count()
    }

    pub fn remove_device(&mut self, id: Id) {
        self.devices.retain(|d| d.id != id);
        self.links.retain(|l| l.from.device != id && l.to.device != id);
    }

    pub fn remove_port(&mut self, device: Id, port: Id) {
        if let Some(d) = self.device_mut(device) {
            d.ports.retain(|p| p.id != port);
        }
        self.links.retain(|l| l.from.port != port && l.to.port != port);
    }

    pub fn remove_link(&mut self, id: Id) {
        self.links.retain(|l| l.id != id);
    }

    pub fn remove_note(&mut self, id: Id) {
        self.notes.retain(|n| n.id != id);
    }

    /// Drop dangling links and make sure new ids never collide with loaded ones.
    fn repair(&mut self) {
        // Written without a color: the type's.
        for d in self.devices.iter_mut().filter(|d| d.color.trim().is_empty()) {
            let kind = crate::editor::known_kind(&d.kind).unwrap_or(&d.kind);
            d.color = crate::editor::kind_color(kind).unwrap_or(DEVICE_COLOR).into();
        }
        // Traces of lines no longer there are gone.
        let links: std::collections::HashSet<Id> = self.links.iter().map(|l| l.id).collect();
        self.traces.retain(|t| t.links.len() >= 2 && t.links.iter().all(|id| links.contains(id)));
        // Patch panels (and switches with ports) written without a size:
        // as wide as their ports.
        for d in self.devices.iter_mut().filter(|d| d.w == default_device_w() && d.h == default_device_h()) {
            d.fit_panel();
        }
        let mut max_id = 0;
        for d in &self.devices {
            max_id = max_id.max(d.id);
            for p in &d.ports {
                max_id = max_id.max(p.id);
            }
        }
        for l in &self.links {
            max_id = max_id.max(l.id);
        }
        for l in &self.layers {
            max_id = max_id.max(l.id);
        }
        for p in &self.points {
            max_id = max_id.max(p.id);
        }
        for n in &mut self.notes {
            max_id = max_id.max(n.id);
            n.size = if n.size.is_finite() { n.size.clamp(NOTE_MIN_SIZE, NOTE_MAX_SIZE) } else { default_note_size() };
        }
        let sane = |s: &mut f64| {
            if !s.is_finite() || *s <= 0.0 {
                *s = 1.0;
            }
        };
        self.links.iter_mut().for_each(|x| sane(&mut x.scale));
        self.notes.iter_mut().for_each(|x| sane(&mut x.scale));
        self.points.iter_mut().for_each(|x| sane(&mut x.scale));
        for d in &mut self.devices {
            sane(&mut d.scale);
            d.w = d.w.max(MIN_W * d.scale);
            d.h = d.h.max(MIN_H * d.scale);
            for p in &mut d.ports {
                p.t = clamp_t(p.t);
            }
        }
        let snapshot = self.clone();
        self.links.retain(|l| snapshot.endpoint_pos(l.from).is_some() && snapshot.endpoint_pos(l.to).is_some());
        self.next_id = max_id + 1;
        // Files from before layers: everything goes into one layer. The
        // frame's layer holds nothing else, so there is always another.
        if self.layers.iter().all(|l| l.frame.is_some()) {
            let id = self.alloc_id();
            self.layers.push(Layer::new(id, "Layer 1"));
        }
        let first = self.layers.iter().find(|l| l.frame.is_none()).unwrap().id;
        // Anything on the frame's layer (or an unknown one) moves to the first other layer.
        let known: Vec<Id> = self.layers.iter().filter(|l| l.frame.is_none()).map(|l| l.id).collect();
        let fix = |layer: &mut Id| {
            if !known.contains(layer) {
                *layer = first;
            }
        };
        self.devices.iter_mut().for_each(|d| fix(&mut d.layer));
        self.links.iter_mut().for_each(|l| fix(&mut l.layer));
        self.notes.iter_mut().for_each(|n| fix(&mut n.layer));
        self.points.iter_mut().for_each(|p| fix(&mut p.layer));
        // A port on a layer that is not there (or the frame's) is on its
        // device's again.
        for p in self.devices.iter_mut().flat_map(|d| d.ports.iter_mut()) {
            if p.layer != 0 && !known.contains(&p.layer) {
                p.layer = 0;
            }
        }
        fix(&mut self.active_layer);
        // Sub-layers of layers that are not there (or of the frame's, or in a
        // circle) are top layers.
        for i in 0..self.layers.len() {
            let (id, parent) = (self.layers[i].id, self.layers[i].parent);
            let bad = parent != 0
                && (parent == id
                    || self.layer(parent).is_none_or(|p| p.frame.is_some())
                    || self.layers[i].frame.is_some()
                    || self.layer_ancestors(parent).contains(&id));
            if bad {
                self.layers[i].parent = 0;
            }
        }
        self.renumber_z();
    }

    // Layers

    pub fn layer(&self, id: Id) -> Option<&Layer> {
        self.layers.iter().find(|l| l.id == id)
    }
    pub fn layer_mut(&mut self, id: Id) -> Option<&mut Layer> {
        self.layers.iter_mut().find(|l| l.id == id)
    }

    pub fn add_layer(&mut self, name: &str) -> Id {
        let id = self.alloc_id();
        self.layers.push(Layer::new(id, name));
        id
    }

    /// A new layer under `parent`, after its other sub-layers.
    pub fn add_sublayer(&mut self, parent: Id, name: &str) -> Id {
        let id = self.alloc_id();
        let mut l = Layer::new(id, name);
        l.parent = parent;
        // In the list after the parent and what is already under it.
        let at = self.layers.iter().rposition(|x| x.id == parent || self.layer_ancestors(x.id).contains(&parent)).map_or(self.layers.len(), |i| i + 1);
        self.layers.insert(at, l);
        id
    }

    /// Move layer `id`, with the layers under it, `before` or `after`
    /// layer `target` (under the same layer as it), or `into` it (as its
    /// last sub-layer). Not into itself or a layer under it, and the
    /// frame's layer neither moves nor takes sub-layers.
    pub fn move_layer(&mut self, id: Id, target: Id, place: &str) -> bool {
        let (Some(l), Some(t)) = (self.layer(id), self.layer(target)) else { return false };
        if id == target || l.frame.is_some() || self.layer_ancestors(target).contains(&id) || place == "into" && t.frame.is_some() {
            return false;
        }
        let parent = match place {
            "into" => target,
            "before" | "after" => t.parent,
            _ => return false,
        };
        // The layer and those under it, in their order, out of the list...
        let under = |d: &Diagram, of: Id| -> Vec<Id> { d.layers.iter().filter(|x| x.id == of || d.layer_ancestors(x.id).contains(&of)).map(|x| x.id).collect() };
        let block_ids = under(self, id);
        let mut block = vec![];
        self.layers.retain(|x| {
            if block_ids.contains(&x.id) {
                block.push(x.clone());
                false
            } else {
                true
            }
        });
        // ...and back in before the target, or after it and those under it.
        let at = match place {
            "before" => self.layers.iter().position(|x| x.id == target).unwrap_or(0),
            _ => {
                let t_block = under(self, target);
                self.layers.iter().rposition(|x| t_block.contains(&x.id)).map_or(self.layers.len(), |i| i + 1)
            }
        };
        block[0].parent = parent;
        for (k, x) in block.into_iter().enumerate() {
            self.layers.insert(at + k, x);
        }
        true
    }

    /// The layers `id` is under, nearest first.
    pub fn layer_ancestors(&self, id: Id) -> Vec<Id> {
        let mut out = vec![];
        let mut at = self.layer(id).map_or(0, |l| l.parent);
        // (As deep as they go; a circle, which a file could hold, ends it.)
        while at != 0 && !out.contains(&at) && at != id {
            out.push(at);
            at = self.layer(at).map_or(0, |l| l.parent);
        }
        out
    }

    /// Its name with the names of the layers it is under ("Servers /
    /// srv-01").
    pub fn layer_path(&self, id: Id) -> String {
        let mut names: Vec<&str> = self.layer_ancestors(id).iter().rev().filter_map(|a| self.layer(*a)).map(|l| l.name.as_str()).collect();
        names.extend(self.layer(id).map(|l| l.name.as_str()));
        names.join(" / ")
    }

    /// The top layer `id` is under (itself if it is one).
    pub fn layer_root(&self, id: Id) -> Id {
        self.layer_ancestors(id).last().copied().unwrap_or(id)
    }

    /// The layers in the order of the layer list: each followed by its
    /// sub-layers (in their order), with how deep it is.
    pub fn layer_tree(&self) -> Vec<(&Layer, usize)> {
        fn add<'a>(d: &'a Diagram, parent: Id, depth: usize, out: &mut Vec<(&'a Layer, usize)>) {
            for l in d.layers.iter().filter(|l| l.parent == parent) {
                if out.iter().any(|(x, _)| x.id == l.id) {
                    continue;
                }
                out.push((l, depth));
                add(d, l.id, depth + 1, out);
            }
        }
        let mut out = vec![];
        add(self, 0, 0, &mut out);
        out
    }

    /// The frame, if its layer is shown (the first layer with one).
    pub fn frame(&self) -> Option<&Frame> {
        self.layers.iter().find(|l| l.frame.is_some() && l.visible).and_then(|l| l.frame.as_ref())
    }

    /// Backgrounds of the shown layers that have one, back to front (in
    /// layer order), each around the devices and texts on its layer plus
    /// its margin (lines may run off to other layers, so they do not
    /// count). Layers without devices or texts get none.
    pub fn layer_backgrounds(&self) -> Vec<(Id, Rect, &LayerBackground)> {
        let mut out = vec![];
        for l in self.layers.iter().filter(|l| self.layer_visible(l.id)) {
            let Some(bg) = &l.background else { continue };
            let boxes = self
                .devices
                .iter()
                .filter(|d| d.layer == l.id)
                .map(|d| d.rect())
                .chain(self.notes.iter().filter(|n| n.layer == l.id).map(|n| n.rect()));
            if let Some(r) = boxes.reduce(|a, b| a.union(&b)) {
                out.push((l.id, r.inflate(bg.margin.max(0.0)), bg));
            }
        }
        out
    }

    /// The layer holding the frame, shown or not.
    pub fn frame_layer(&self) -> Option<Id> {
        self.layers.iter().find(|l| l.frame.is_some()).map(|l| l.id)
    }

    /// Remove a layer and everything on it (and links to devices on it).
    /// The last layer and locked layers cannot be removed.
    pub fn remove_layer(&mut self, id: Id) -> bool {
        let Some(i) = self.layers.iter().position(|l| l.id == id) else { return false };
        // One layer for items always stays (the frame's layer cannot hold them).
        let under: Vec<Id> = self.layers.iter().filter(|l| self.layer_ancestors(l.id).contains(&id)).map(|l| l.id).collect();
        let item_layers = self.layers.iter().filter(|l| l.frame.is_none() && l.id != id && !under.contains(&l.id)).count();
        if self.layers.len() == 1 || self.layer_locked(id) || (self.layers[i].frame.is_none() && item_layers == 0) {
            return false;
        }
        // Its sub-layers go with it, with what is on them (deepest first).
        if under.iter().any(|l| self.layer(*l).is_some_and(|l| l.locked)) {
            return false;
        }
        for sub in under.iter().rev() {
            self.remove_layer(*sub);
        }
        let Some(i) = self.layers.iter().position(|l| l.id == id) else { return false };
        let gone: Vec<Id> = self.devices.iter().filter(|d| d.layer == id).map(|d| d.id).collect();
        for d in gone {
            self.remove_device(d);
        }
        let gone: Vec<Id> = self.points.iter().filter(|p| p.layer == id).map(|p| p.id).collect();
        for p in gone {
            self.remove_point(p);
        }
        self.links.retain(|l| l.layer != id);
        self.notes.retain(|n| n.layer != id);
        // Ports on it go back to their device's layer (removing them would
        // take their lines too).
        for p in self.devices.iter_mut().flat_map(|d| d.ports.iter_mut()).filter(|p| p.layer == id) {
            p.layer = 0;
        }
        self.layers.remove(i);
        if self.active_layer == id {
            self.active_layer = self.layers[i.min(self.layers.len() - 1)].id;
            if self.layer(self.active_layer).is_some_and(|l| l.frame.is_some()) {
                self.active_layer = self.layers.iter().find(|l| l.frame.is_none()).map_or(0, |l| l.id);
            }
        }
        true
    }

    /// Number of devices, links and notes on a layer.
    pub fn layer_count(&self, id: Id) -> usize {
        self.devices.iter().filter(|d| d.layer == id).count()
            + self.links.iter().filter(|l| l.layer == id).count()
            + self.notes.iter().filter(|n| n.layer == id).count()
            + self.points.iter().filter(|p| p.layer == id).count()
    }

    /// The layer new items go into: the active one, shown if it was hidden
    /// so the new item does not vanish.
    pub fn target_layer(&mut self) -> Id {
        // Never the frame's layer: it holds only the frame.
        if self.layer(self.active_layer).is_none_or(|l| l.frame.is_some()) {
            self.active_layer = self.layers.iter().find(|l| l.frame.is_none()).map_or(0, |l| l.id);
        }
        let id = self.active_layer;
        if let Some(l) = self.layer_mut(id) {
            l.visible = true;
        }
        id
    }

    /// Shown: the layer, and every layer it is under.
    pub fn layer_visible(&self, id: Id) -> bool {
        self.layer(id).is_none_or(|l| l.visible) && self.layer_ancestors(id).iter().all(|a| self.layer(*a).is_none_or(|l| l.visible))
    }

    /// The layer name to show on a device, if its layer shows names.
    pub fn layer_badge(&self, d: &Device) -> Option<&str> {
        self.layer(d.layer).filter(|l| l.show_name).map(|l| l.name.as_str())
    }

    /// Locked: the layer, or a layer it is under.
    pub fn layer_locked(&self, id: Id) -> bool {
        self.layer(id).is_some_and(|l| l.locked) || self.layer_ancestors(id).iter().any(|a| self.layer(*a).is_some_and(|l| l.locked))
    }

    /// Shown and on an unlocked layer: can be clicked, selected and changed.
    /// A link only needs its own layer unlocked; its ends follow their
    /// devices or points.
    pub fn is_editable(&self, id: Id) -> bool {
        self.is_visible(id) && self.layer_of(id).is_none_or(|l| !self.layer_locked(l))
    }

    /// Whether the device, note or link `id` is on a shown layer. A link
    /// also needs both of its devices shown.
    pub fn is_visible(&self, id: Id) -> bool {
        if let Some(d) = self.device(id) {
            self.layer_visible(d.layer)
        } else if let Some(n) = self.note(id) {
            self.layer_visible(n.layer)
        } else if let Some(l) = self.link(id) {
            self.link_visible(l)
        } else if let Some(p) = self.point(id) {
            self.layer_visible(p.layer)
        } else {
            false
        }
    }

    pub fn link_visible(&self, l: &Link) -> bool {
        let end_shown = |ep: Endpoint| match ep.is_point() {
            true => self.point(ep.owner()).is_some_and(|p| self.layer_visible(p.layer)),
            false => self.device(ep.owner()).is_some_and(|d| self.layer_visible(d.layer)),
        };
        self.layer_visible(l.layer) && end_shown(l.from) && end_shown(l.to)
    }

    /// A copy with everything on hidden layers left out. Cabling that runs
    /// on through hidden devices (patch panels on a hidden layer, say) is
    /// drawn through them: a line from the last device shown before them
    /// to the next one shown after them.
    pub fn visible_only(&self) -> Diagram {
        let mut d = self.clone();
        d.devices.retain(|x| self.layer_visible(x.layer));
        d.notes.retain(|x| self.layer_visible(x.layer));
        d.points.retain(|x| self.layer_visible(x.layer));
        d.links.retain(|x| self.link_visible(x));
        let through = self.through_links();
        let n_through = through.len();
        for mut l in through {
            // It keeps the id of the first cable it stands for (hidden, so not
            // here), to select that one through it; once only.
            if d.links.iter().any(|x| x.id == l.id) {
                l.id = d.alloc_id();
            }
            d.links.push(l);
        }
        // A line standing in for hidden devices is routed afresh: clear of
        // what is shown (its first cable's routing may now cross a device).
        for i in d.links.len() - n_through..d.links.len() {
            let obstacles = crate::geom::link_obstacles(&d, &d.links[i]);
            crate::geom::route_clear_of(&mut d, i, &obstacles);
        }
        d
    }

    /// Whether `t` is a chain of lines, each going on from the device where
    /// the one before it ends.
    pub fn is_trace(&self, t: &Trace) -> bool {
        self.trace_chain(t).is_some()
    }

    /// The lines of trace `t` one after another, each with the end it
    /// starts from (where the one before ends, on the same device) and the
    /// end it goes to. None if it is not such a chain (any more).
    fn trace_chain(&self, t: &Trace) -> Option<Vec<(Endpoint, Endpoint, &Link)>> {
        let links: Vec<&Link> = t.links.iter().map(|id| self.link(*id)).collect::<Option<_>>()?;
        let (first, second) = (links.first()?, links.get(1)?);
        let meets = |ep: Endpoint, l: &Link| ep.owner() == l.from.owner() || ep.owner() == l.to.owner();
        // It starts at the end of the first line away from the second.
        let start = if meets(first.to, second) { first.from } else { first.to };
        let mut out = vec![];
        let mut prev: Option<Endpoint> = None;
        for l in links {
            let (a, b) = match prev {
                None if l.from == start => (l.from, l.to),
                None => (l.to, l.from),
                Some(p) if l.from.owner() == p.owner() => (l.from, l.to),
                Some(p) if l.to.owner() == p.owner() => (l.to, l.from),
                Some(_) => return None,
            };
            out.push((a, b, l));
            prev = Some(b);
        }
        Some(out)
    }

    /// The lines that stand for cabling through hidden devices: along each
    /// way through (a trace from NetBox, or from socket to socket through
    /// patch panels: in at a socket's front, on from its back, or the other
    /// way), between two devices shown with only hidden ones in between, a
    /// line from where the cabling leaves the first to where it reaches the
    /// second, looking like the first line it stands for. Each once.
    pub fn through_links(&self) -> Vec<Link> {
        let shown = |ep: Endpoint| !ep.is_point() && self.device(ep.device).is_some_and(|d| self.layer_visible(d.layer));
        let mut chains: Vec<Vec<(Endpoint, Endpoint, &Link)>> = self.traces.iter().filter_map(|t| self.trace_chain(t)).collect();
        chains.extend(self.socket_chains());
        let mut out = vec![];
        let mut done: HashSet<(Endpoint, Endpoint)> = HashSet::new();
        for chain in &chains {
            // The devices along it: where it starts, where each line meets
            // the next, where it ends (with the end it arrives at).
            let mut last = None;
            for k in 0..=chain.len() {
                let arrive = if k == 0 { chain[0].0 } else { chain[k - 1].1 };
                if !shown(arrive) {
                    continue;
                }
                if let Some(j) = last
                    && k > j + 1
                {
                    let run = &chain[j..k];
                    let (from, to) = (run[0].0, arrive);
                    let fresh = done.insert((from, to)) && done.insert((to, from));
                    if fresh && from.owner() != to.owner() && run.iter().all(|(_, _, l)| self.layer_visible(l.layer)) {
                        let head = |(a, _, l): &(Endpoint, Endpoint, &Link), start: bool| match (l.from == *a) == start {
                            true => l.style.head_start,
                            false => l.style.head_end,
                        };
                        let mut l = run[0].2.clone();
                        (l.from, l.to) = (from, to);
                        l.style.head_start = head(&run[0], true);
                        l.style.head_end = head(&run[run.len() - 1], false);
                        l.label.clear();
                        l.label_at = None;
                        l.source.clear();
                        out.push(l);
                    }
                }
                last = Some(k);
            }
        }
        out
    }

    /// The other side of the socket port `ep` of a patch panel is: its back
    /// for the front, its front for the back (the port there, if any).
    fn through_socket(&self, ep: Endpoint) -> Option<Endpoint> {
        if ep.is_point() {
            return None;
        }
        let d = self.device(ep.device)?;
        let p = d.port(ep.port)?;
        if !d.is_patch_panel() || p.socket == 0 || d.socket_pos(p.socket).is_none() {
            return None;
        }
        let other = d.ports.iter().find(|q| q.id != p.id && q.socket == p.socket && q.rear != p.rear)?;
        Some(Endpoint { device: d.id, port: other.id })
    }

    /// Cabling from socket to socket through patch panels: lines one after
    /// another where one ends at a socket (front or back) and the next goes
    /// on from the other side of that socket. Each at least two lines long.
    /// The ends of the longest cabling run that line `link` is part of
    /// (along a trace, or through patch panel sockets), and how many lines
    /// it has; None when the line is not part of one.
    pub fn cabling_run(&self, link: Id) -> Option<(Endpoint, Endpoint, usize)> {
        self.traces
            .iter()
            .filter_map(|t| self.trace_chain(t))
            .chain(self.socket_chains())
            .filter(|c| c.iter().any(|(_, _, l)| l.id == link))
            .max_by_key(|c| c.len())
            .map(|c| (c[0].0, c[c.len() - 1].1, c.len()))
    }

    fn socket_chains(&self) -> Vec<Vec<(Endpoint, Endpoint, &Link)>> {
        let at = |ep: Endpoint| -> Vec<&Link> { self.links.iter().filter(|l| l.from == ep || l.to == ep).collect() };
        // The one line on from the other side of the socket at `ep`.
        let next = |ep: Endpoint, came: Id| -> Option<(Endpoint, &Link)> {
            let other = self.through_socket(ep)?;
            match at(other)[..] {
                [l] if l.id != came => Some((other, l)),
                _ => None,
            }
        };
        let mut out = vec![];
        let mut seen: HashSet<Vec<Id>> = HashSet::new();
        for l in &self.links {
            for (start, end) in [(l.from, l.to), (l.to, l.from)] {
                // From an end that does not go on through a socket.
                if next(start, l.id).is_some() {
                    continue;
                }
                let mut chain = vec![(start, end, l)];
                let mut ids = vec![l.id];
                let (mut here, mut came) = (end, l.id);
                while let Some((from, line)) = next(here, came) {
                    if ids.contains(&line.id) {
                        break; // round in a circle
                    }
                    let to = if line.from == from { line.to } else { line.from };
                    chain.push((from, to, line));
                    ids.push(line.id);
                    (here, came) = (to, line.id);
                }
                if chain.len() < 2 {
                    continue;
                }
                let mut key = ids.clone();
                key.sort_unstable();
                if seen.insert(key) {
                    out.push(chain);
                }
            }
        }
        out
    }

    pub fn layer_of(&self, id: Id) -> Option<Id> {
        self.find_item(id).map(|it| match it {
            Item::Link(i) => self.links[i].layer,
            Item::Device(i) => self.devices[i].layer,
            Item::Note(i) => self.notes[i].layer,
            Item::Point(i) => self.points[i].layer,
        })
    }

    pub fn set_layer_of(&mut self, id: Id, layer: Id) {
        match self.find_item(id) {
            Some(Item::Link(i)) => self.links[i].layer = layer,
            Some(Item::Device(i)) => self.devices[i].layer = layer,
            Some(Item::Note(i)) => self.notes[i].layer = layer,
            Some(Item::Point(i)) => self.points[i].layer = layer,
            None => {}
        }
    }

    // Containment

    /// Drawing scale of a device, note, point or link (1 if unknown).
    pub fn scale_of(&self, id: Id) -> f64 {
        match self.find_item(id) {
            Some(Item::Link(i)) => self.links[i].scale,
            Some(Item::Device(i)) => self.devices[i].scale,
            Some(Item::Note(i)) => self.notes[i].scale,
            Some(Item::Point(i)) => self.points[i].scale,
            None => 1.0,
        }
    }

    /// Box of a device, note or connection point (a point has no size).
    pub fn item_box(&self, id: Id) -> Option<Rect> {
        self.device(id)
            .map(|d| d.rect())
            .or_else(|| self.note(id).map(|n| n.rect()))
            .or_else(|| self.point(id).map(|p| Rect { x: p.x, y: p.y, w: 0.0, h: 0.0 }))
    }

    /// For every device, note and point that lies inside a device: that
    /// device. "Inside" means completely within its box and in front of it;
    /// of several such devices the smallest wins, so containers nest.
    pub fn containers(&self) -> std::collections::HashMap<Id, Id> {
        let mut out = std::collections::HashMap::new();
        let boxes = self.devices.iter().enumerate().map(|(i, d)| ((d.z, 1, i), d.id, d.rect()));
        let notes = self.notes.iter().enumerate().map(|(i, n)| ((n.z, 2, i), n.id, n.rect()));
        let points = self.points.iter().enumerate().map(|(i, p)| ((p.z, 3, i), p.id, Rect { x: p.x, y: p.y, w: 0.0, h: 0.0 }));
        for (key, id, b) in boxes.chain(notes).chain(points) {
            if let Some(p) = self.innermost_around(key, id, &b) {
                out.insert(id, p);
            }
        }
        out
    }

    /// Stacking key as in [`Diagram::z_order`], without sorting everything.
    fn stack_key(&self, id: Id) -> Option<(i64, u8, usize)> {
        Some(match self.find_item(id)? {
            Item::Link(i) => (self.links[i].z, 0, i),
            Item::Device(i) => (self.devices[i].z, 1, i),
            Item::Note(i) => (self.notes[i].z, 2, i),
            Item::Point(i) => (self.points[i].z, 3, i),
        })
    }

    /// Devices that hold the box `b` of item `id` (stacking `key`): behind
    /// it and completely around it.
    fn around(&self, key: (i64, u8, usize), id: Id, b: &Rect) -> impl Iterator<Item = (usize, &Device)> {
        self.devices
            .iter()
            .enumerate()
            .filter(move |(i, d)| d.id != id && (d.z, 1, *i) < key && d.rect().contains_rect(b))
    }

    fn innermost_around(&self, key: (i64, u8, usize), id: Id, b: &Rect) -> Option<Id> {
        self.around(key, id, b)
            .min_by(|(i, a), (j, c)| (a.w * a.h).total_cmp(&(c.w * c.h)).then((c.z, *j).cmp(&(a.z, *i))))
            .map(|(_, d)| d.id)
    }

    /// Every device `id` lies in, at any depth (cheaper than building
    /// [`Diagram::containers`] when only one item matters).
    pub fn enclosing(&self, id: Id) -> Vec<Id> {
        match (self.stack_key(id), self.item_box(id)) {
            (Some(key), Some(b)) => self.around(key, id, &b).map(|(_, d)| d.id).collect(),
            _ => vec![],
        }
    }

    /// Everything inside `id`, at any depth.
    pub fn contents(parents: &std::collections::HashMap<Id, Id>, id: Id) -> Vec<Id> {
        let mut out = vec![];
        let mut stack = vec![id];
        while let Some(c) = stack.pop() {
            for (&child, &parent) in parents {
                if parent == c && !out.contains(&child) {
                    out.push(child);
                    stack.push(child);
                }
            }
        }
        out.sort_unstable();
        out
    }

    /// The devices `id` lies in, innermost first.
    pub fn ancestors(parents: &std::collections::HashMap<Id, Id>, id: Id) -> Vec<Id> {
        let mut out = vec![];
        let mut c = id;
        while let Some(&p) = parents.get(&c) {
            if out.contains(&p) {
                break;
            }
            out.push(p);
            c = p;
        }
        out
    }

    // Stacking order

    pub fn find_item(&self, id: Id) -> Option<Item> {
        (self.links.iter().position(|l| l.id == id).map(Item::Link))
            .or_else(|| self.devices.iter().position(|d| d.id == id).map(Item::Device))
            .or_else(|| self.notes.iter().position(|n| n.id == id).map(Item::Note))
            .or_else(|| self.points.iter().position(|p| p.id == id).map(Item::Point))
    }

    pub fn item_id(&self, it: Item) -> Id {
        match it {
            Item::Link(i) => self.links[i].id,
            Item::Device(i) => self.devices[i].id,
            Item::Note(i) => self.notes[i].id,
            Item::Point(i) => self.points[i].id,
        }
    }

    fn z_mut(&mut self, it: Item) -> &mut i64 {
        match it {
            Item::Link(i) => &mut self.links[i].z,
            Item::Device(i) => &mut self.devices[i].z,
            Item::Note(i) => &mut self.notes[i].z,
            Item::Point(i) => &mut self.points[i].z,
        }
    }

    /// Every link, device and note, back to front. Equal `z` (files from
    /// before stacking) keeps the old order: links, then devices, then notes.
    pub fn z_order(&self) -> Vec<Item> {
        let links = self.links.iter().enumerate().map(|(i, l)| ((l.z, 0, i), Item::Link(i)));
        let devices = self.devices.iter().enumerate().map(|(i, d)| ((d.z, 1, i), Item::Device(i)));
        let notes = self.notes.iter().enumerate().map(|(i, n)| ((n.z, 2, i), Item::Note(i)));
        let points = self.points.iter().enumerate().map(|(i, p)| ((p.z, 3, i), Item::Point(i)));
        let mut all: Vec<_> = links.chain(devices).chain(notes).chain(points).collect();
        all.sort_by_key(|(k, _)| *k);
        all.into_iter().map(|(_, it)| it).collect()
    }

    /// Restack so the items `ids` (back to front) take positions 0, 1, 2…;
    /// items not listed keep their order above them.
    pub fn set_z_order(&mut self, ids: &[Id]) {
        let mut order: Vec<Item> = ids.iter().filter_map(|id| self.find_item(*id)).collect();
        let rest: Vec<Item> = self.z_order().into_iter().filter(|it| !order.contains(it)).collect();
        order.extend(rest);
        for (z, it) in order.into_iter().enumerate() {
            *self.z_mut(it) = z as i64;
        }
    }

    fn renumber_z(&mut self) {
        self.set_z_order(&[]);
    }

    /// A `z` above everything.
    pub fn top_z(&self) -> i64 {
        let zs = self.links.iter().map(|l| l.z).chain(self.devices.iter().map(|d| d.z)).chain(self.notes.iter().map(|n| n.z));
        let zs = zs.chain(self.points.iter().map(|p| p.z));
        zs.max().map_or(0, |z| z + 1)
    }

    pub fn bounds(&self) -> Option<Rect> {
        self.devices
            .iter()
            .map(|d| d.rect())
            .chain(self.notes.iter().map(|n| n.rect()))
            .chain(self.points.iter().map(|p| Rect { x: p.x, y: p.y, w: 0.0, h: 0.0 }))
            .reduce(|a, b| a.union(&b))
    }

    /// Forget what the items stand for elsewhere (see `Device::source`).
    pub fn clear_sources(&mut self) {
        for d in &mut self.devices {
            d.source.clear();
            d.ports.iter_mut().for_each(|p| p.source.clear());
        }
        self.links.iter_mut().for_each(|l| l.source.clear());
    }

    fn sheet_file(&self) -> SheetFile {
        SheetFile {
            name: self.name.clone(),
            source: self.source.clone(),
            devices: self.devices.clone(),
            links: self.links.clone(),
            notes: self.notes.clone(),
            points: self.points.clone(),
            layers: self.layers.clone(),
            active_layer: self.active_layer,
            changed: self.changed.clone(),
            traces: self.traces.clone(),
        }
    }

    fn from_sheet_file(f: SheetFile) -> Diagram {
        let mut d = Diagram {
            devices: f.devices,
            links: f.links,
            notes: f.notes,
            points: f.points,
            layers: f.layers,
            active_layer: f.active_layer,
            changed: f.changed,
            name: f.name,
            source: f.source,
            traces: f.traces,
            next_id: 1,
        };
        d.repair();
        d
    }

    /// This diagram alone as a file (a copy for the clipboard, say).
    pub fn to_json(&self) -> String {
        Diagram::document_to_json(std::slice::from_ref(self))
    }

    /// The first sheet of a file.
    pub fn from_json(s: &str) -> Result<Diagram, String> {
        Ok(Diagram::document_from_json(s)?.remove(0))
    }

    /// A document of `sheets` (at least one) as a file.
    pub fn document_to_json(sheets: &[Diagram]) -> String {
        let f = DiagramFile {
            format: FORMAT_NAME.into(),
            version: FORMAT_VERSION,
            first: sheets[0].sheet_file(),
            sheets: sheets[1..].iter().map(Diagram::sheet_file).collect(),
        };
        serde_json::to_string_pretty(&f).expect("diagram serializes")
    }

    /// The sheets of a file (at least one).
    pub fn document_from_json(s: &str) -> Result<Vec<Diagram>, String> {
        let f: DiagramFile = serde_json::from_str(s).map_err(|e| format!("invalid diagram file: {e}"))?;
        if f.format != FORMAT_NAME && f.format != LEGACY_FORMAT_NAME {
            return Err(format!("not a {FORMAT_NAME} file (format \"{}\")", f.format));
        }
        if f.version > FORMAT_VERSION {
            return Err(format!("file version {} is newer than supported ({FORMAT_VERSION})", f.version));
        }
        Ok(std::iter::once(f.first).chain(f.sheets).map(Diagram::from_sheet_file).collect())
    }
}
