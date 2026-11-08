// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Editing logic, independent of Qt. All coordinates are world coordinates;
//! the QML side handles pan and zoom and converts mouse positions first.

use crate::geom::{self, HeadShape};
use crate::model::*;
use serde_json::{Value, json};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const UNDO_LIMIT: usize = 200;
/// Screen pixels the mouse has to travel before a press turns into a drag.
const DRAG_THRESHOLD_PX: f64 = 4.0;

/// Default fill colors per device kind.
pub const KINDS: &[(&str, &str)] = &[
    ("", "#dbeafe"),
    ("router", "#fde68a"),
    ("switch", "#bfdbfe"),
    ("firewall", "#fecaca"),
    ("load balancer", "#ddd6fe"),
    ("server", "#bbf7d0"),
    ("cloud", "#e0f2fe"),
    ("internet", "#e5e7eb"),
    ("client", "#fef3c7"),
    ("access point", "#ccfbf1"),
    ("wlan controller", "#c7d2fe"),
    ("standalone ap", "#ccfbf1"),
    ("managed ap", "#a7f3d0"),
    ("cloud proxy", "#e0e7ff"),
    ("zscaler", "#dbeafe"),
    ("aws", "#ffedd5"),
    ("aws transit gateway", "#ffedd5"),
    ("aws direct connect gateway", "#ffedd5"),
    ("aci l3out", "#d9f2fb"),
    ("patch panel", "#e2e8f0"),
    ("azure", "#dbeafe"),
    ("google cloud", "#e0e7ff"),
    ("alibaba cloud", "#ffe4d6"),
    ("telekom cloud", "#fce7f3"),
    ("stackit", "#d5eeee"),
    ("ospf router", "#fde68a"),
    ("bgp reflector", "#fed7aa"),
    ("virtual switch", "#cfe0fd"),
    ("distributed virtual switch", "#c3d4fa"),
];

pub fn kind_color(kind: &str) -> Option<&'static str> {
    KINDS.iter().find(|(k, _)| *k == kind).map(|(_, c)| *c)
}

/// A device type as the start of a name: "Router", "Access point", "AWS";
/// "Device" for none.
fn kind_title(kind: &str) -> String {
    const UPPER: &[&str] = &["aws", "ospf", "bgp", "wlan", "ap", "stackit"];
    match kind {
        "" => return "Device".into(),
        // As network engineers call them.
        "aws transit gateway" => return "TGW".into(),
        "aws direct connect gateway" => return "DXGW".into(),
        "aci l3out" => return "L3Out".into(),
        "virtual switch" => return "vSwitch".into(),
        "distributed virtual switch" => return "DVS".into(),
        _ => {}
    }
    let words: Vec<String> = kind
        .split(' ')
        .enumerate()
        .map(|(i, w)| match w {
            w if UPPER.contains(&w) => w.to_uppercase(),
            w if i == 0 => w[..1].to_uppercase() + &w[1..],
            w => w.to_string(),
        })
        .collect();
    words.join(" ")
}

/// Short forms accepted by quick add.
const KIND_ALIASES: &[(&str, &str)] = &[
    ("fw", "firewall"),
    ("rtr", "router"),
    ("rt", "router"),
    ("sw", "switch"),
    ("vswitch", "virtual switch"),
    ("vss", "virtual switch"),
    ("standardswitch", "virtual switch"),
    ("dvs", "distributed virtual switch"),
    ("vds", "distributed virtual switch"),
    ("dvswitch", "distributed virtual switch"),
    ("distributedswitch", "distributed virtual switch"),
    ("lb", "load balancer"),
    ("srv", "server"),
    ("ap", "access point"),
    ("wifi", "access point"),
    ("wlc", "wlan controller"),
    ("wirelesscontroller", "wlan controller"),
    ("wirelesslancontroller", "wlan controller"),
    ("sap", "standalone ap"),
    ("autonomousap", "standalone ap"),
    ("apstandalone", "standalone ap"),
    ("lap", "managed ap"),
    ("lightweightap", "managed ap"),
    ("apmanaged", "managed ap"),
    ("proxy", "cloud proxy"),
    ("swg", "cloud proxy"),
    ("cloudsecurityproxy", "cloud proxy"),
    ("zia", "zscaler"),
    ("zscalerinternetaccess", "zscaler"),
    ("amazon", "aws"),
    ("tgw", "aws transit gateway"),
    ("transitgateway", "aws transit gateway"),
    ("awstgw", "aws transit gateway"),
    ("dxgw", "aws direct connect gateway"),
    ("dxgateway", "aws direct connect gateway"),
    ("directconnectgateway", "aws direct connect gateway"),
    ("awsdxgw", "aws direct connect gateway"),
    ("awsdxgateway", "aws direct connect gateway"),
    ("l3out", "aci l3out"),
    ("externalroutednetwork", "aci l3out"),
    ("l3outside", "aci l3out"),
    ("gcp", "google cloud"),
    ("google", "google cloud"),
    ("gcloud", "google cloud"),
    ("alibaba", "alibaba cloud"),
    ("aliyun", "alibaba cloud"),
    ("otc", "telekom cloud"),
    ("telekom", "telekom cloud"),
    ("opentelekomcloud", "telekom cloud"),
    ("tcloud", "telekom cloud"),
    ("ospf", "ospf router"),
    ("rr", "bgp reflector"),
    ("bgprr", "bgp reflector"),
    ("routereflector", "bgp reflector"),
    ("bgproutereflector", "bgp reflector"),
    ("pp", "patch panel"),
    ("patch", "patch panel"),
    ("patchfield", "patch panel"),
    ("patchpanel", "patch panel"),
];

/// The built-in kind `s` names, ignoring case, spaces and punctuation.
pub(crate) fn known_kind(s: &str) -> Option<&'static str> {
    let norm = |s: &str| s.chars().filter(|c| c.is_alphanumeric()).collect::<String>().to_lowercase();
    let s = norm(s);
    if s.is_empty() {
        return None;
    }
    KINDS
        .iter()
        .map(|(k, _)| *k)
        .find(|k| norm(k) == s)
        .or_else(|| KIND_ALIASES.iter().find(|(a, _)| *a == s).map(|(_, k)| *k))
}

/// One device requested by quick add.
#[derive(Clone, Debug, PartialEq)]
pub struct QuickItem {
    pub name: String,
    pub kind: String,
    /// The name was given explicitly (`kind|name`) and is kept as typed.
    pub named: bool,
}

/// Parse a quick add list: entries separated by commas, semicolons or
/// newlines, each either `kind` (also used as the label) or `kind|label`.
pub fn parse_quick_add(spec: &str) -> Vec<QuickItem> {
    spec.split([',', ';', '\n'])
        .map(str::trim)
        .filter(|t| !t.is_empty())
        .filter_map(|t| match t.split_once('|') {
            Some((kind, name)) => {
                let (name, kind) = (name.trim(), kind.trim());
                let kind = known_kind(kind).map_or_else(|| kind.to_lowercase(), String::from);
                match (name.is_empty(), kind.is_empty()) {
                    (true, true) => None,
                    (true, false) => Some(QuickItem { name: kind.clone(), kind, named: false }),
                    _ => Some(QuickItem { name: name.into(), kind, named: true }),
                }
            }
            None => Some(QuickItem { name: t.into(), kind: known_kind(t).unwrap_or("").into(), named: false }),
        })
        .collect()
}

// Qt::CursorShape values.
/// Screen pixels around a device within which dragging a port moves it.
const PORT_MOVE_ZONE_PX: f64 = 14.0;
/// File extension for saved diagrams.
pub const FILE_EXT: &str = "stratagram";

/// `path` with the diagram extension added if it has none.
pub fn with_file_ext(path: &Path) -> PathBuf {
    if path.extension().is_none() { path.with_extension(FILE_EXT) } else { path.to_path_buf() }
}

/// Each deep zoom level is this much smaller than the one around it.
pub const LEVEL_STEP: f64 = 8.0;
/// Items show once they are drawn bigger than this (scale × zoom), fading
/// in up to LEGIBLE_FULL: one level in (1/8) that is past 400% zoom (the
/// limit without deep zoom), fully from 600%.
pub const LEGIBLE_MIN: f64 = 0.5;
pub const LEGIBLE_FULL: f64 = 0.75;

/// Distance from the top of a device with contents to the middle of its
/// name (its type goes one line below).
pub const CONTAINER_NAME_Y: f64 = 14.0;
/// Screen pixels a link dropped on empty space must span to end at a new
/// connection point (shorter drags are taken as slips and dropped).
const MIN_FREE_LINK_PX: f64 = 20.0;
/// Screen pixels within which a Ctrl+drag snaps into line with another item.
const ALIGN_PX: f64 = 8.0;

pub mod cursor {
    pub const ARROW: i32 = 0;
    pub const CROSS: i32 = 2;
    pub const SIZE_FDIAG: i32 = 8;
    pub const SIZE_ALL: i32 = 9;
    pub const POINTING_HAND: i32 = 13;
}

#[derive(Clone, Debug, PartialEq)]
pub enum Selection {
    None,
    /// Devices and notes (ids are unique across both).
    Items(Vec<Id>),
    Port(Endpoint),
    Link(Id),
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Hit {
    Resize(Id),
    Port(Endpoint),
    Edge { device: Id, side: Side, t: f64 },
    /// A socket of a patch panel (from 1) without a port yet.
    Socket { device: Id, socket: u32 },
    Device(Id),
    Note(Id),
    /// A free connection point.
    Point(Id),
    Link(Id),
    /// The label box on a line.
    Label(Id),
    /// A line drawn end to end in place of cables through hidden devices:
    /// the first of those cables.
    Through(Id),
    Empty,
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum LinkSource {
    Port(Endpoint),
    Edge { device: Id, side: Side, t: f64 },
    Socket { device: Id, socket: u32 },
}

/// Text being edited in place on the canvas.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum EditTarget {
    DeviceName(Id),
    PortName(Endpoint),
    LinkLabel(Id),
    NoteText(Id),
    /// The label of a connection point.
    PointLabel(Id),
}

#[derive(Clone, Debug)]
enum Drag {
    None,
    /// Moves devices and notes together. `click_toggle` is the item under a
    /// Ctrl+press: toggled in the selection if the mouse is released unmoved.
    MoveItems { start: Pt, origins: Vec<(Id, Pt)>, click_toggle: Option<Id> },
    Resize { device: Id, start: Pt, w: f64, h: f64 },
    NewLink { src: LinkSource, cursor: Pt },
    /// Dragging a port: moves it while the mouse stays close to its device
    /// (or always, with Shift); turns into a new link once pulled away.
    Port { ep: Endpoint, orig: (Side, f64), force_move: bool },
    Rubber { start: Pt, cur: Pt, additive: bool, base: Vec<Id> },
    /// Pressed on a line: dragging splits it at a new connection point that
    /// follows the mouse, bending the line there.
    BendLink(Id),
    /// Pressed on a line's label: dragging slides it along the line (with
    /// Ctrl, into line with other items). `click_toggle`: a Ctrl+press,
    /// toggling the line in the selection if released unmoved.
    MoveLabel { id: Id, click_toggle: bool },
}

/// Something another item can line up with: a box edge or center, or a
/// port, at `v` on one axis and spanning `lo..hi` on the other.
#[derive(Clone, Copy, Debug)]
struct Feature {
    vertical: bool,
    port: bool,
    v: f64,
    lo: f64,
    hi: f64,
}

/// Start and end of `r` along one axis (x if `along_x`), and its extent on the other.
fn span(r: &Rect, along_x: bool) -> ((f64, f64), (f64, f64)) {
    let (x, y) = ((r.x, r.x + r.w), (r.y, r.y + r.h));
    if along_x { (x, y) } else { (y, x) }
}

fn center(r: &Rect) -> Pt {
    Pt::new(r.x + r.w / 2.0, r.y + r.h / 2.0)
}

/// Nearest box before (or `after`) `g` along one axis that overlaps it on
/// the other, i.e. its neighbor in a row or column. `slack` lets `g`
/// overlap that neighbor a little, as it may while still being dragged.
fn nearest(boxes: &[Rect], g: &Rect, along_x: bool, after: bool, slack: f64) -> Option<Rect> {
    let ((g0, g1), (c0, c1)) = span(g, along_x);
    let candidates = boxes.iter().filter(|r| {
        let ((s0, s1), (o0, o1)) = span(r, along_x);
        o1 > c0 && o0 < c1 && if after { s0 >= g1 - slack && s1 > g1 } else { s1 <= g0 + slack && s0 < g0 }
    });
    if after {
        candidates.min_by(|a, b| span(a, along_x).0.0.total_cmp(&span(b, along_x).0.0)).copied()
    } else {
        candidates.max_by(|a, b| span(a, along_x).0.1.total_cmp(&span(b, along_x).0.1)).copied()
    }
}

/// Gap from box `a` to box `b` that follows it along one axis.
fn gap(a: &Rect, b: &Rect, along_x: bool) -> f64 {
    span(b, along_x).0.0 - span(a, along_x).0.1
}

/// Distance marker across the gap from `a` to `b`, in the middle of where
/// the two overlap on the other axis.
fn gap_marker(a: &Rect, b: &Rect, along_x: bool) -> (Pt, Pt) {
    let ((_, a1), (ac0, ac1)) = span(a, along_x);
    let ((b0, _), (bc0, bc1)) = span(b, along_x);
    let m = (ac0.max(bc0) + ac1.min(bc1)) / 2.0;
    if along_x { (Pt::new(a1, m), Pt::new(b0, m)) } else { (Pt::new(m, a1), Pt::new(m, b0)) }
}

/// Offsets along one axis that give the moving box `g` an equal gap: in the
/// middle between its two neighbors, or continuing the gap between a
/// neighbor and the box beyond it (moving an outer box of a row).
fn equal_gap_offsets(others: &[Rect], g: &Rect, along_x: bool, tol: f64) -> Vec<f64> {
    let ((g0, g1), _) = span(g, along_x);
    let before = nearest(others, g, along_x, false, tol);
    let after = nearest(others, g, along_x, true, tol);
    let mut offs = vec![];
    if let (Some(l), Some(r)) = (before, after) {
        let room = gap(&l, &r, along_x) - (g1 - g0);
        if room > 0.0 {
            offs.push(span(&l, along_x).0.1 + room / 2.0 - g0);
        }
    }
    if let Some(l) = before
        && let Some(ll) = nearest(others, &l, along_x, false, 0.0)
    {
        offs.push(span(&l, along_x).0.1 + gap(&ll, &l, along_x) - g0);
    }
    if let Some(r) = after
        && let Some(rr) = nearest(others, &r, along_x, true, 0.0)
    {
        offs.push(span(&r, along_x).0.0 - gap(&r, &rr, along_x) - g1);
    }
    offs.retain(|o| o.abs() <= tol);
    offs
}

/// Markers for every pair of equal gaps `g` now forms with its row or column.
fn equal_gap_markers(others: &[Rect], g: &Rect, along_x: bool, out: &mut Vec<(Pt, Pt)>) {
    let eq = |a: f64, b: f64| a >= 1.0 && (a - b).abs() < 0.01;
    let before = nearest(others, g, along_x, false, 0.01);
    let after = nearest(others, g, along_x, true, 0.01);
    let mut pairs = vec![];
    if let (Some(l), Some(r)) = (before, after)
        && eq(gap(&l, g, along_x), gap(g, &r, along_x))
    {
        pairs.extend([(l, *g), (*g, r)]);
    }
    if let Some(l) = before
        && let Some(ll) = nearest(others, &l, along_x, false, 0.0)
        && eq(gap(&ll, &l, along_x), gap(&l, g, along_x))
    {
        pairs.extend([(ll, l), (l, *g)]);
    }
    if let Some(r) = after
        && let Some(rr) = nearest(others, &r, along_x, true, 0.0)
        && eq(gap(&r, &rr, along_x), gap(g, &r, along_x))
    {
        pairs.extend([(*g, r), (r, rr)]);
    }
    for (a, b) in pairs {
        let m = gap_marker(&a, &b, along_x);
        if !out.contains(&m) {
            out.push(m);
        }
    }
}

/// Two boxes offset diagonally (apart on both axes) whose centers have
/// their midpoint within `tol` of the center of `g`: the offset that puts
/// `g` exactly halfway, and the pair. The closest match wins, then the
/// closest pair.
fn diagonal_center(others: &[Rect], g: &Rect, tol: f64) -> Option<(Pt, Rect, Rect)> {
    let c = center(g);
    let apart = |a: &Rect, b: &Rect| {
        let sep = |along_x| {
            let ((a0, a1), _) = span(a, along_x);
            let ((b0, b1), _) = span(b, along_x);
            a1 <= b0 || b1 <= a0
        };
        sep(true) && sep(false)
    };
    let mut best: Option<(f64, f64, Pt, Rect, Rect)> = None;
    for (i, a) in others.iter().enumerate() {
        for b in &others[i + 1..] {
            if !apart(a, b) || !apart(a, g) || !apart(g, b) {
                continue;
            }
            let (ca, cb) = (center(a), center(b));
            let off = Pt::new((ca.x + cb.x) / 2.0, (ca.y + cb.y) / 2.0).sub(c);
            if off.x.abs() > tol || off.y.abs() > tol {
                continue;
            }
            let key = (off.len(), ca.dist(cb));
            if best.is_none_or(|(l, d, ..)| key < (l, d)) {
                best = Some((key.0, key.1, off, *a, *b));
            }
        }
    }
    best.map(|(_, _, off, a, b)| (off, a, b))
}

impl Feature {
    fn x(v: f64, lo: f64, hi: f64, port: bool) -> Feature {
        Feature { vertical: true, port, v, lo, hi }
    }
    fn y(v: f64, lo: f64, hi: f64, port: bool) -> Feature {
        Feature { vertical: false, port, v, lo, hi }
    }
}

/// Where a dragged connection point would attach: the device, the port
/// it lands on (if any), else the side and spot for a new one, or the
/// patch panel socket (from 1; 0 for none).
type PointTarget = (Id, Option<Id>, Side, f64, u32);

/// Where a link drag would end if released now.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Target {
    Port(Endpoint),
    NewPort { device: Id, side: Side, t: f64 },
    Socket { device: Id, socket: u32 },
    /// Dropped where there is nothing to attach to: a new connection point.
    NewPoint(Pt),
}

/// Marks a copy of a whole sheet with its layers and frame.
const SHEET_COPY: &str = "stratagram_sheet_copy";

pub struct Editor {
    pub doc: Diagram,
    pub selection: Selection,
    pub path: Option<PathBuf>,
    pub dirty: bool,
    pub zoom: f64,
    pub snap: bool,
    /// The colors the diagram is drawn in, on screen and in exports.
    pub theme: crate::theme::Theme,
    /// Ctrl held: line dragged items up with nearby ones.
    pub align: bool,
    /// Deep zoom: the view can zoom in endlessly, and items added while
    /// zoomed in are made at that zoom level's scale.
    pub deep: bool,
    /// Guide lines showing what the current drag lined up with.
    guides: Vec<(Pt, Pt)>,
    /// Distance markers for the gaps that the current drag made equal.
    spacings: Vec<(Pt, Pt)>,
    /// Style applied to newly drawn links; follows the last edited link.
    pub link_defaults: LinkStyle,
    pub status: String,
    /// Folder of the last opened or saved file, kept across "New".
    pub last_dir: Option<PathBuf>,
    /// All sheets of the document, in order; the active one's place holds
    /// a stand-in while it is `doc`. Shared with the undo steps, so edits
    /// of one sheet do not copy the others.
    sheets: Arc<Vec<Diagram>>,
    active_sheet: usize,
    undo: Vec<Saved>,
    redo: Vec<Saved>,
    drag: Drag,
    press_pos: Pt,
    moved: bool,
    snapshot: Option<Diagram>,
    hover: Hit,
    /// Where the pointer last was over the canvas (for the line's tip).
    hover_at: Pt,
    /// Coalesces repeated edits of one field (typing) into one undo step.
    last_edit: Option<String>,
    /// In-place text editing in progress.
    pub editing: Option<EditTarget>,
    /// Active layer before an undo or redo, kept across it.
    active_before: Option<Id>,
    /// What was pasted last and how many times, so repeated pastes of the
    /// same thing step down and right instead of piling up.
    last_paste: Option<(String, usize)>,
    /// The configuration file with the defaults for frames (see
    /// `crate::config`); none in tests.
    pub config_path: Option<PathBuf>,
    /// How far the items being dragged may move (least and most), so items
    /// made inside a device stay in it; worked out on the first move.
    drag_limits: Option<(Pt, Pt)>,
}

impl Default for Editor {
    fn default() -> Self {
        Editor {
            doc: Diagram::default(),
            selection: Selection::None,
            path: None,
            dirty: false,
            zoom: 1.0,
            snap: true,
            theme: crate::theme::Theme::Light,
            align: false,
            deep: false,
            guides: vec![],
            spacings: vec![],
            link_defaults: LinkStyle::default(),
            status: String::new(),
            last_dir: None,
            sheets: Arc::new(vec![Diagram::default()]),
            active_sheet: 0,
            undo: vec![],
            redo: vec![],
            drag: Drag::None,
            press_pos: Pt::default(),
            moved: false,
            snapshot: None,
            hover: Hit::Empty,
            hover_at: Pt::default(),
            last_edit: None,
            editing: None,
            active_before: None,
            last_paste: None,
            config_path: None,
            drag_limits: None,
        }
    }
}

/// An undo step: the active sheet as it was, with the others and which
/// one was active.
#[derive(Clone)]
struct Saved {
    doc: Diagram,
    sheets: Arc<Vec<Diagram>>,
    active: usize,
}

impl Editor {
    // Undo

    fn checkpoint(&mut self) {
        self.push_undo(self.doc.clone());
        self.touch();
        self.last_edit = None;
    }

    /// Remember `before` (the active sheet before a change) for undo.
    fn push_undo(&mut self, before: Diagram) {
        self.undo.push(Saved { doc: before, sheets: self.sheets.clone(), active: self.active_sheet });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
    }

    /// Go to the history step `doc`, `sheets`, `active`; returns the
    /// document as it was, for the other history stack. Both show the sheet
    /// the step's change is on, so undo and redo land there.
    fn saved_now(&mut self, doc: Diagram, sheets: Arc<Vec<Diagram>>, active: usize) -> Saved {
        if active != self.active_sheet {
            // Another sheet: its layers are not this one's.
            self.active_before = None;
            self.view_reset();
            let all = self.take_sheets();
            self.set_sheets(all, active);
        }
        let now = Saved { doc: std::mem::replace(&mut self.doc, doc), sheets: std::mem::replace(&mut self.sheets, sheets), active: self.active_sheet };
        self.active_sheet = active;
        now
    }

    /// The file's name ("" before it is saved), for frames that show it.
    pub fn file_name(&self) -> String {
        self.path.as_ref().and_then(|p| p.file_name()).map(|s| s.to_string_lossy().into_owned()).unwrap_or_default()
    }

    /// The document changed: unsaved, and stamped with the time (shown in
    /// the frame's footer and saved with the file).
    fn touch(&mut self) {
        self.dirty = true;
        self.doc.changed = Some(chrono::Local::now().format("%Y-%m-%d %H:%M").to_string());
    }

    pub fn can_undo(&self) -> bool {
        !self.undo.is_empty()
    }
    pub fn can_redo(&self) -> bool {
        !self.redo.is_empty()
    }

    pub fn undo(&mut self) {
        self.active_before = Some(self.doc.active_layer);
        if let Some(prev) = self.undo.pop() {
            let now = self.saved_now(prev.doc, prev.sheets, prev.active);
            self.redo.push(now);
            self.after_history_jump();
        }
    }

    pub fn redo(&mut self) {
        self.active_before = Some(self.doc.active_layer);
        if let Some(next) = self.redo.pop() {
            let now = self.saved_now(next.doc, next.sheets, next.active);
            self.undo.push(now);
            self.after_history_jump();
        }
    }

    fn after_history_jump(&mut self) {
        self.keep_active_layer();
        self.status.clear();
        self.drag = Drag::None;
        self.dirty = true;
        self.last_edit = None;
        self.validate_selection();
    }

    /// Undo and redo restore the document, but which layer is active is a
    /// choice of the moment and stays.
    fn keep_active_layer(&mut self) {
        if let Some(id) = self.active_before.take()
            && self.doc.layer(id).is_some()
        {
            self.doc.active_layer = id;
        }
    }

    /// The view zoom changed: items that became too small to see leave the
    /// selection.
    pub fn zoom_changed(&mut self) {
        self.validate_selection();
    }

    /// Drop selected items that were deleted or are on a hidden or locked layer.
    fn validate_selection(&mut self) {
        let doc = &self.doc;
        self.selection = match std::mem::replace(&mut self.selection, Selection::None) {
            Selection::Items(ids) => match ids.into_iter().filter(|id| self.reachable(*id)).collect::<Vec<_>>() {
                ids if ids.is_empty() => Selection::None,
                ids if ids.len() == 1 && doc.link(ids[0]).is_some() => Selection::Link(ids[0]),
                ids => Selection::Items(ids),
            },
            Selection::Port(ep)
                if doc.endpoint_pos(ep).is_some()
                    && self.reachable(ep.device)
                    && doc.device(ep.device).and_then(|d| d.port(ep.port).map(|p| doc.port_editable(d, p))) == Some(true) =>
            {
                Selection::Port(ep)
            }
            Selection::Link(id) if self.reachable(id) => Selection::Link(id),
            _ => Selection::None,
        };
        self.editing = self.editing.filter(|t| {
            let owner = match *t {
                EditTarget::DeviceName(id) | EditTarget::LinkLabel(id) | EditTarget::NoteText(id) | EditTarget::PointLabel(id) => id,
                EditTarget::PortName(ep) => ep.device,
            };
            self.edit_text(*t).is_some() && self.doc.is_editable(owner)
        });
    }

    // Hit testing

    /// Scale for new items: 1, or with deep zoom the deepest level that is
    /// fully shown at this zoom (1/8 from 600%, 1/64 from 4800%...), so new
    /// items are never made too small to see.
    pub fn level(&self) -> f64 {
        if !self.deep || !self.zoom.is_finite() || self.zoom <= 0.0 {
            return 1.0;
        }
        LEVEL_STEP.powf(-((self.zoom / LEGIBLE_FULL).ln() / LEVEL_STEP.ln() + 1e-9).floor())
    }

    /// How visible an item at `scale` is at the current zoom: 0 (too small
    /// to show) to 1.
    fn legibility(&self, scale: f64) -> f64 {
        // Only what is inside objects fades: the diagram itself stays fully
        // shown however far you zoom out.
        if scale >= 1.0 {
            return 1.0;
        }
        // Without deep zoom, what is inside objects stays out of sight.
        if !self.deep {
            return 0.0;
        }
        ((scale * self.zoom - LEGIBLE_MIN) / (LEGIBLE_FULL - LEGIBLE_MIN)).clamp(0.0, 1.0)
    }

    /// Shown at this zoom: big enough to read, and for a link, so are both
    /// of its ends.
    fn legible(&self, id: Id) -> bool {
        if self.legibility(self.doc.scale_of(id)) <= 0.0 {
            return false;
        }
        match self.doc.link(id) {
            Some(l) => [l.from.owner(), l.to.owner()].iter().all(|o| self.legibility(self.doc.scale_of(*o)) > 0.0),
            None => true,
        }
    }

    /// Can be clicked and selected now: editable and big enough to see.
    fn reachable(&self, id: Id) -> bool {
        self.doc.is_editable(id) && self.legible(id)
    }

    /// Grid step for an item drawn at `scale`.
    fn grid(scale: f64) -> f64 {
        GRID * scale
    }

    fn snap_pt(&self, p: Pt, scale: f64) -> Pt {
        if self.snap { Pt::new(snap_to(p.x, Self::grid(scale)), snap_to(p.y, Self::grid(scale))) } else { p }
    }

    fn tol(&self) -> f64 {
        6.0 / self.zoom.max(0.05)
    }

    /// The one selected device or note, which shows a resize handle, and
    /// its box.
    /// The shown line whose label box is under `p`, if any.
    fn label_at(&self, p: Pt) -> Option<Id> {
        let doc = &self.doc;
        let spots = geom::label_spots(doc);
        let mut hits: Vec<&Link> = doc
            .links
            .iter()
            .filter(|l| self.reachable(l.id))
            .filter(|l| {
                let (Some(&(c, _)), Some((w, h))) = (spots.get(&l.id), geom::label_box_in(doc, l)) else { return false };
                Rect { x: c.x - w / 2.0, y: c.y - h / 2.0, w, h }.inflate(1.0).contains(p)
            })
            .collect();
        // The one in front.
        hits.sort_by_key(|l| l.z);
        hits.last().map(|l| l.id)
    }

    fn selected_resizable(&self) -> Option<(Id, Rect, f64)> {
        let Selection::Items(ids) = &self.selection else { return None };
        let [id] = ids[..] else { return None };
        if !self.reachable(id) {
            return None;
        }
        if let Some(d) = self.doc.device(id) {
            return Some((id, d.rect(), d.scale));
        }
        self.doc.note(id).map(|n| (id, n.rect(), n.scale))
    }

    pub fn hit_test(&self, p: Pt) -> Hit {
        let tol = self.tol();
        if let Some((id, r, _)) = self.selected_resizable() {
            let corner = Pt::new(r.x + r.w, r.y + r.h);
            if (p.x - corner.x).abs() <= tol + 2.0 && (p.y - corner.y).abs() <= tol + 2.0 {
                return Hit::Resize(id);
            }
        }
        // Front to back, skipping hidden and locked layers: clicks go
        // through locked items to whatever is underneath.
        let doc = &self.doc;
        let order: Vec<Item> =
            doc.z_order().into_iter().rev().filter(|it| self.reachable(doc.item_id(*it))).collect();
        // Ports and connection points first: they are small and sit on top
        // of borders and line ends.
        let port_r = |scale: f64| geom::PORT_HALF * scale + tol * 0.8;
        for it in &order {
            if let Item::Point(i) = *it {
                let q = &doc.points[i];
                if q.pos().dist(p) <= port_r(q.scale) {
                    return Hit::Point(q.id);
                }
                continue;
            }
            let Item::Device(i) = *it else { continue };
            let d = &doc.devices[i];
            for port in d.ports.iter().rev().filter(|port| doc.port_editable(d, port)) {
                let pp = d.port_pos(port);
                if (p.x - pp.x).abs() <= port_r(d.scale) && (p.y - pp.y).abs() <= port_r(d.scale) {
                    return Hit::Port(Endpoint { device: d.id, port: port.id });
                }
            }
        }
        // Line labels sit on top of their lines.
        if let Some(id) = self.label_at(p) {
            return Hit::Label(id);
        }
        for it in order {
            match it {
                Item::Note(i) => {
                    let n = &doc.notes[i];
                    if n.rect().inflate(tol * 0.5).contains(p) {
                        return Hit::Note(n.id);
                    }
                }
                Item::Device(i) => {
                    let d = &doc.devices[i];
                    let r = d.rect();
                    if !r.inflate(tol).contains(p) {
                        continue;
                    }
                    let edge_band = tol.min(d.w / 4.0).min(d.h / 4.0);
                    if !r.inflate(-edge_band).contains(p) {
                        let (side, t) = d.nearest_side(p);
                        return Hit::Edge { device: d.id, side, t };
                    }
                    // A patch panel's sockets take lines like ports.
                    if let Some(socket) = d.socket_at(p, tol * 0.3)
                        && !doc.layer_locked(d.layer)
                    {
                        return Hit::Socket { device: d.id, socket };
                    }
                    // Zoomed into it (a level deeper than its own), its inside
                    // is the background you work on.
                    if self.deep && d.scale > self.level() * 1.5 {
                        continue;
                    }
                    return Hit::Device(d.id);
                }
                Item::Link(i) => {
                    let l = &doc.links[i];
                    // Tunnels and carriers are drawn as wider bands.
                    let band = l.style.bands().first().map_or(0.0, |b| b.1 * l.scale);
                    if geom::link_route(doc, l)
                        .is_some_and(|pts| dist_to_polyline(p, &pts) <= tol + l.style.width / 2.0 + band)
                    {
                        return Hit::Link(l.id);
                    }
                }
                Item::Point(_) => {}
            }
        }
        // Lines standing in for cables through hidden devices: they are
        // there only as shown.
        if self.doc.layers.iter().any(|l| !l.visible) {
            let shown = self.doc.visible_only();
            for l in shown.links.iter().rev() {
                // (Each has the id of the first cable it stands for.)
                let stands_in = self.doc.link(l.id).is_some_and(|real| !self.doc.link_visible(real));
                // (Its cable's own layer counts, not the hidden devices.)
                let unlocked = shown.layer_visible(l.layer) && !shown.layer_locked(l.layer);
                if stands_in
                    && unlocked
                    && self.legible(l.id)
                    && geom::link_route(&shown, l).is_some_and(|pts| dist_to_polyline(p, &pts) <= tol + l.style.width / 2.0)
                {
                    return Hit::Through(l.id);
                }
            }
        }
        Hit::Empty
    }

    pub fn cursor_for(&self, hit: Hit) -> i32 {
        match hit {
            Hit::Resize(_) => cursor::SIZE_FDIAG,
            Hit::Port(_) | Hit::Edge { .. } | Hit::Socket { .. } => cursor::CROSS,
            Hit::Device(_) | Hit::Note(_) | Hit::Point(_) => cursor::SIZE_ALL,
            Hit::Link(_) | Hit::Through(_) => cursor::POINTING_HAND,
            Hit::Label(_) => cursor::SIZE_ALL,
            Hit::Empty => cursor::ARROW,
        }
    }

    // Mouse interaction

    pub fn press(&mut self, p: Pt, shift: bool, ctrl: bool) {
        self.press_pos = p;
        self.moved = false;
        self.drag_limits = None;
        self.snapshot = Some(self.doc.clone());
        self.last_edit = None;
        self.hover = Hit::Empty;
        self.editing = None;
        self.status.clear();
        self.align = ctrl;
        self.clear_guides();
        let hit = self.hit_test(p);
        self.drag = match hit {
            Hit::Resize(id) => {
                let r = self.doc.item_box(id).unwrap();
                Drag::Resize { device: id, start: p, w: r.w, h: r.h }
            }
            Hit::Port(ep) => {
                self.selection = Selection::Port(ep);
                let port = self.doc.device(ep.device).and_then(|d| d.port(ep.port)).unwrap();
                Drag::Port { ep, orig: (port.side, port.t), force_move: shift }
            }
            Hit::Edge { device, side, t } => {
                if !self.is_item_selected(device) {
                    self.selection = Selection::Items(vec![device]);
                }
                Drag::NewLink { src: LinkSource::Edge { device, side, t }, cursor: p }
            }
            Hit::Socket { device, socket } => {
                if !self.is_item_selected(device) {
                    self.selection = Selection::Items(vec![device]);
                }
                Drag::NewLink { src: LinkSource::Socket { device, socket }, cursor: p }
            }
            // Shift+drag from a connection point draws a link from it.
            Hit::Point(id) if shift => {
                self.selection = Selection::Items(vec![id]);
                Drag::NewLink { src: LinkSource::Port(Endpoint::point(id)), cursor: p }
            }
            Hit::Device(id) | Hit::Note(id) | Hit::Point(id) => {
                if shift {
                    self.toggle_item(id);
                    Drag::None
                } else if ctrl {
                    // Ctrl+click toggles the item on release; Ctrl+drag moves
                    // it (or the selection it belongs to) with alignment.
                    let origins = if self.is_item_selected(id) { self.move_origins() } else { self.with_contents(&[id]) };
                    Drag::MoveItems { start: p, origins, click_toggle: Some(id) }
                } else {
                    if !self.is_item_selected(id) {
                        self.selection = Selection::Items(vec![id]);
                    }
                    self.start_move(p)
                }
            }
            Hit::Label(id) => {
                if shift {
                    self.toggle_item(id);
                    Drag::None
                } else if ctrl {
                    Drag::MoveLabel { id, click_toggle: true }
                } else {
                    self.selection = Selection::Link(id);
                    Drag::MoveLabel { id, click_toggle: false }
                }
            }
            Hit::Link(id) => {
                if ctrl || shift {
                    self.toggle_item(id);
                    Drag::None
                } else {
                    self.selection = Selection::Link(id);
                    Drag::BendLink(id)
                }
            }
            // It stands for several cables: selected, not bent.
            Hit::Through(id) => {
                if ctrl || shift {
                    self.toggle_item(id);
                } else {
                    self.selection = Selection::Link(id);
                }
                self.status = "This line stands for cables through hidden devices: show their layer to change its route.".into();
                Drag::None
            }
            Hit::Empty => {
                let base = match (&self.selection, ctrl || shift) {
                    (Selection::Items(_) | Selection::Link(_), true) => self.selected_ids(),
                    _ => vec![],
                };
                if !(ctrl || shift) {
                    self.selection = Selection::None;
                }
                Drag::Rubber { start: p, cur: p, additive: ctrl || shift, base }
            }
        };
    }

    fn is_item_selected(&self, id: Id) -> bool {
        matches!(&self.selection, Selection::Items(ids) if ids.contains(&id))
    }

    /// Add `id` to the selection or take it out (devices, notes and links).
    fn toggle_item(&mut self, id: Id) {
        let mut ids = match &self.selection {
            Selection::Items(_) | Selection::Link(_) => self.selected_ids(),
            _ => vec![],
        };
        if let Some(i) = ids.iter().position(|x| *x == id) {
            ids.remove(i);
        } else {
            ids.push(id);
        }
        self.select_ids(ids);
    }

    fn move_origins(&self) -> Vec<(Id, Pt)> {
        match &self.selection {
            Selection::Items(ids) => self.with_contents(ids),
            _ => vec![],
        }
    }

    /// `ids` and everything inside them (devices carry their contents), with
    /// their positions. Locked or hidden contents stay where they are.
    fn with_contents(&self, ids: &[Id]) -> Vec<(Id, Pt)> {
        let parents = self.doc.containers();
        let mut all: Vec<Id> = ids.to_vec();
        for id in ids {
            for c in Diagram::contents(&parents, *id) {
                if !all.contains(&c) && self.doc.is_editable(c) {
                    all.push(c);
                }
            }
        }
        all.iter().filter_map(|id| self.doc.item_pos(*id).map(|pos| (*id, pos))).collect()
    }

    fn start_move(&self, p: Pt) -> Drag {
        Drag::MoveItems { start: p, origins: self.move_origins(), click_toggle: None }
    }

    pub fn drag_to(&mut self, p: Pt) {
        if !self.moved && p.dist(self.press_pos) * self.zoom < DRAG_THRESHOLD_PX {
            return;
        }
        self.moved = true;
        let snap = self.snap;
        match &mut self.drag {
            Drag::None => {}
            Drag::MoveItems { start, origins, click_toggle } => {
                let (start, origins) = (*start, origins.clone());
                if let Some(id) = click_toggle.take()
                    && !self.is_item_selected(id)
                {
                    // Ctrl+drag on an unselected item moves just that item.
                    self.selection = Selection::Items(vec![id]);
                }
                let d = self.move_delta(&origins, p.sub(start));
                // Items made inside a device stay inside it.
                let (lo, hi) = *self.drag_limits.get_or_insert_with(|| {
                    let ids: Vec<Id> = origins.iter().map(|(id, _)| *id).collect();
                    belonging_limits(self.snapshot.as_ref().unwrap_or(&self.doc), &ids)
                });
                let d = Pt::new(d.x.clamp(lo.x, hi.x.max(lo.x)), d.y.clamp(lo.y, hi.y.max(lo.y)));
                for (id, o) in origins {
                    self.doc.set_item_pos(id, o.add(d));
                }
            }
            Drag::Resize { device, start, w, h } => {
                let (id, mut nw, mut nh) = (*device, *w + p.x - start.x, *h + p.y - start.y);
                let scale = self.doc.scale_of(id);
                if snap {
                    nw = snap_to(nw, Self::grid(scale));
                    nh = snap_to(nh, Self::grid(scale));
                }
                // Items made inside a device do not grow out of it.
                if let (Some(home), Some(at)) = (home_box(&self.doc, id), self.doc.item_pos(id)) {
                    nw = nw.min(home.x + home.w - at.x);
                    nh = nh.min(home.y + home.h - at.y);
                }
                if let Some(d) = self.doc.device_mut(id) {
                    d.w = nw.max(MIN_W * scale);
                    d.h = nh.max(MIN_H * scale);
                } else if let Some(n) = self.doc.note_mut(id) {
                    // A note gets a fixed box: its text wraps to it.
                    n.w = (nw / scale).max(NOTE_MIN_BOX);
                    n.h = (nh / scale).max(n.size * NOTE_LINE_HEIGHT);
                }
            }
            Drag::NewLink { cursor, .. } => *cursor = p,
            Drag::MoveLabel { id, click_toggle } => {
                // Along the line only: the nearest point of the line to the
                // mouse, kept a little off its ends. A line bent through
                // points counts as one: the label moves on through them onto
                // the next piece, up to a piece with a label of its own.
                let id = *id;
                if std::mem::take(click_toggle) {
                    self.selection = Selection::Link(id);
                }
                let Some(l) = self.doc.link(id).cloned() else { return };
                let bent = geom::bent_line(&self.doc, &l);
                let at = bent.pieces.iter().position(|(pid, _)| *pid == l.id).unwrap_or(0);
                let free = |pid: Id| pid == l.id || self.doc.link(pid).is_some_and(|x| x.label.is_empty());
                let (mut first, mut last) = (at, at);
                while first > 0 && free(bent.pieces[first - 1].0) {
                    first -= 1;
                }
                while last + 1 < bent.pieces.len() && free(bent.pieces[last + 1].0) {
                    last += 1;
                }
                // The pieces' routes along the line, and where each starts.
                let mut route: Vec<Pt> = vec![];
                let mut starts: Vec<(Id, bool, f64, f64)> = vec![];
                let mut along = 0.0;
                for &(pid, rev) in &bent.pieces[first..=last] {
                    let Some(mut r) = self.doc.link(pid).and_then(|x| geom::link_route(&self.doc, x)) else { return };
                    if rev {
                        r.reverse();
                    }
                    let len: f64 = r.windows(2).map(|w| w[0].dist(w[1])).sum();
                    starts.push((pid, rev, along, len));
                    along += len;
                    route.extend(r);
                }
                let total = along.max(1e-9);
                if route.len() < 2 {
                    return;
                }
                // The whole box clear of the line's real ends and their ports
                // (the points in between are no ends).
                let (w, h) = geom::label_box(&l).unwrap_or((0.0, 0.0));
                let room = |a: Pt, b: Pt| {
                    let u = b.sub(a).norm();
                    ((w * u.x.abs() + h * u.y.abs()) / 2.0 + (geom::PORT_HALF + 2.0) * l.scale) / total
                };
                let n = route.len();
                let lo = if first == 0 { room(route[0], route[1]) } else { 0.0 };
                let hi = if last + 1 == bent.pieces.len() { 1.0 - room(route[n - 2], route[n - 1]) } else { 1.0 };
                let t = geom::nearest_along(&route, p);
                let t = if lo < hi { t.clamp(lo, hi) } else { 0.5 };
                self.clear_guides();
                let run: Vec<Id> = bent.pieces[first..=last].iter().map(|(pid, _)| *pid).collect();
                let t = if self.align && lo < hi { self.align_label(&route, t, (lo, hi), &run) } else { t };
                // The piece that is, and the spot along it (from its own start).
                let s = t * total;
                let &(pid, rev, start, len) =
                    starts.iter().find(|(_, _, start, len)| s <= start + len + 1e-9).unwrap_or(starts.last().unwrap());
                let local = if len > 1e-9 { ((s - start) / len).clamp(0.0, 1.0) } else { 0.5 };
                let local = if rev { 1.0 - local } else { local };
                if pid != l.id {
                    // Onto the next piece: the label (and the type's tag
                    // with it) goes there.
                    if let Some(old) = self.doc.link_mut(l.id) {
                        old.label.clear();
                        old.label_at = None;
                    }
                    if let Some(new) = self.doc.link_mut(pid) {
                        new.label = l.label.clone();
                        new.label_hidden = l.label_hidden;
                    }
                    self.drag = Drag::MoveLabel { id: pid, click_toggle: false };
                    self.selection = Selection::Link(pid);
                }
                if let Some(x) = self.doc.link_mut(pid) {
                    x.label_at = Some(local);
                }
            }
            Drag::BendLink(link) => {
                let link = *link;
                let scale = self.doc.scale_of(link);
                let at = self.snap_pt(p, scale);
                if let Some(id) = self.doc.split_link(link, at) {
                    self.selection = Selection::Items(vec![id]);
                    self.drag = Drag::MoveItems { start: p, origins: vec![(id, at)], click_toggle: None };
                    self.status = "Bent. Delete the point to straighten the line again.".into();
                }
            }
            Drag::Port { ep, orig, force_move } => {
                let (ep, orig, force_move) = (*ep, *orig, *force_move);
                let zone = PORT_MOVE_ZONE_PX / self.zoom.max(0.05);
                let Some(d) = self.doc.device(ep.device) else { return };
                let near = d.rect().inflate(zone).contains(p);
                // On a patch panel, inside the border it goes from socket to
                // socket (staying where it is between them).
                let band = self.tol().min(d.w / 4.0).min(d.h / 4.0);
                if d.front().is_some() && d.rect().inflate(-band).contains(p) {
                    let socket = d.socket_at(p, self.tol() * 0.3);
                    self.clear_guides();
                    if let Some(n) = socket
                        && let Some(port) = self.doc.device_mut(ep.device).and_then(|d| d.ports.iter_mut().find(|x| x.id == ep.port))
                    {
                        if port.socket == 0 || port.rear {
                            port.side = Side::Bottom;
                        }
                        port.socket = n;
                        port.rear = false;
                    }
                    return;
                }
                let (side, mut t) = if force_move || near { port_spot(d, p, snap) } else { orig };
                self.clear_guides();
                if self.align && (force_move || near) {
                    t = self.align_port(ep, side, t);
                }
                if let Some(port) = self
                    .doc
                    .device_mut(ep.device)
                    .and_then(|d| d.ports.iter_mut().find(|x| x.id == ep.port))
                {
                    port.side = side;
                    port.t = t;
                    port.socket = 0;
                    port.rear = false;
                }
                // Under a patch panel's socket, it is the socket's back.
                self.doc.settle_rear(ep);
                if !force_move && !near {
                    if self.doc.links_at_port(ep.port) > 0
                        && let Some(id) = self.doc.detach_port(ep, p)
                    {
                        // Pulled off its device, a linked port becomes a free
                        // point that keeps its lines and follows the mouse.
                        self.selection = Selection::Items(vec![id]);
                        self.drag = Drag::MoveItems { start: p, origins: vec![(id, p)], click_toggle: None };
                        self.status = "Detached. Drop it on a border to attach it there.".into();
                    } else {
                        // An unlinked port draws a new link instead.
                        self.drag = Drag::NewLink { src: LinkSource::Port(ep), cursor: p };
                    }
                }
            }
            Drag::Rubber { start, cur, additive, base } => {
                *cur = p;
                let r = Rect::from_points(*start, p);
                let mut ids: Vec<Id> = if *additive { base.clone() } else { vec![] };
                let devices = self.doc.devices.iter().map(|d| (d.id, d.rect()));
                let notes = self.doc.notes.iter().map(|n| (n.id, n.rect()));
                for (id, rect) in devices.chain(notes) {
                    if r.contains_rect(&rect) && !ids.contains(&id) && self.reachable(id) {
                        ids.push(id);
                    }
                }
                for q in &self.doc.points {
                    if r.contains(q.pos()) && !ids.contains(&q.id) && self.reachable(q.id) {
                        ids.push(q.id);
                    }
                }
                // Links whose whole line is inside the rectangle.
                for l in &self.doc.links {
                    if !ids.contains(&l.id)
                        && self.reachable(l.id)
                        && geom::link_route(&self.doc, l).is_some_and(|pts| pts.iter().all(|q| r.contains(*q)))
                    {
                        ids.push(l.id);
                    }
                }
                self.select_ids(ids);
            }
        }
    }

    /// The line a single dragged device would be inserted into if dropped
    /// now: one running through the middle of it, not attached to it (or
    /// to anything moving with it).
    fn insert_target(&self) -> Option<Id> {
        let Drag::MoveItems { origins, .. } = &self.drag else { return None };
        let Selection::Items(ids) = &self.selection else { return None };
        let ([id], true) = (&ids[..], self.moved) else { return None };
        let d = self.doc.device(*id)?;
        let c = Pt::new(d.x + d.w / 2.0, d.y + d.h / 2.0);
        let reach = (d.w.min(d.h) * 0.3).max(self.tol());
        let moving: Vec<Id> = origins.iter().map(|(id, _)| *id).collect();
        self.doc
            .links
            .iter()
            .filter(|l| !moving.contains(&l.from.owner()) && !moving.contains(&l.to.owner()) && self.reachable(l.id))
            .filter_map(|l| Some((l.id, dist_to_polyline(c, &geom::link_route(&self.doc, l)?))))
            .filter(|(_, dist)| *dist <= reach)
            .min_by(|a, b| a.1.total_cmp(&b.1))
            .map(|(id, _)| id)
    }

    pub fn release(&mut self, p: Pt) {
        self.drag_to(p);
        self.clear_guides();
        let insert = self.insert_target();
        let drag = std::mem::replace(&mut self.drag, Drag::None);
        match drag {
            Drag::NewLink { src, .. } if self.moved => self.finish_link(src, p),
            Drag::MoveItems { click_toggle: Some(id), .. } if !self.moved => self.toggle_item(id),
            Drag::MoveLabel { id, click_toggle: true } if !self.moved => self.toggle_item(id),
            Drag::MoveItems { origins, .. } if self.moved => {
                if let [(id, _)] = origins[..]
                    && let Some(target) = self.dragged_point_target(id)
                {
                    self.attach_dragged_point(id, target);
                } else {
                    if let (Some(link), Some(&(dev, _))) = (insert, origins.first())
                        && self.doc.insert_into_link(link, dev).is_some()
                    {
                        let name = self.doc.device(dev).map(|d| d.name.clone()).unwrap_or_default();
                        self.status = format!("Inserted {name} into the line. Delete it to join the line again.");
                    }
                    self.raise_into_container(&origins.iter().map(|(id, _)| *id).collect::<Vec<_>>());
                }
            }
            _ => {}
        }
        if let Some(snap) = self.snapshot.take()
            && snap != self.doc
        {
            self.push_undo(snap);
            self.touch();
        }
        self.hover = self.hit_test(p);
    }

    /// Items dropped inside a device that is drawn in front of them would
    /// vanish behind it; bring them (in their order) to the front instead,
    /// which also makes them its contents.
    fn raise_into_container(&mut self, moved: &[Id]) {
        let doc = &self.doc;
        let order: Vec<Id> = doc.z_order().into_iter().map(|it| doc.item_id(it)).collect();
        let rank = |id: Id| order.iter().position(|x| *x == id).unwrap_or(0);
        let hidden = moved.iter().any(|&id| {
            doc.item_box(id).is_some_and(|b| {
                doc.devices.iter().any(|d| !moved.contains(&d.id) && rank(d.id) > rank(id) && d.rect().contains_rect(&b))
            })
        });
        if hidden {
            let mut order: Vec<Id> = order.iter().copied().filter(|id| !moved.contains(id)).collect();
            order.extend(doc.z_order().into_iter().map(|it| doc.item_id(it)).filter(|id| moved.contains(id)));
            self.doc.set_z_order(&order);
        }
    }

    /// Where the connection point `id` would attach if dropped now: a port
    /// it sits on, or the border of a device (with the snapped spot).
    fn dragged_point_target(&self, id: Id) -> Option<PointTarget> {
        let at = self.doc.point(id)?.pos();
        let tol = self.tol();
        let port_r = |scale: f64| geom::PORT_HALF * scale + tol * 0.8;
        let doc = &self.doc;
        let devices: Vec<&Device> = doc
            .z_order()
            .into_iter()
            .rev()
            .filter_map(|it| match it {
                Item::Device(i) if self.reachable(doc.devices[i].id) => Some(&doc.devices[i]),
                _ => None,
            })
            .collect();
        for d in &devices {
            for port in d.ports.iter().filter(|port| self.doc.port_editable(d, port)) {
                if d.port_pos(port).dist(at) <= port_r(d.scale) {
                    return Some((d.id, Some(port.id), port.side, port.t, 0));
                }
            }
        }
        for d in devices {
            let r = d.rect();
            if !r.inflate(tol).contains(at) {
                continue;
            }
            let band = tol.min(d.w / 4.0).min(d.h / 4.0);
            if r.inflate(-band).contains(at) {
                // Inside the box: on a socket of a patch panel, or nothing.
                let socket = d.socket_at(at, tol * 0.3)?;
                // Its lines leave towards where they go.
                let to = self
                    .doc
                    .links
                    .iter()
                    .find_map(|l| match (l.from == Endpoint::point(id), l.to == Endpoint::point(id)) {
                        (true, _) => Some(l.to),
                        (_, true) => Some(l.from),
                        _ => None,
                    })
                    .and_then(|o| self.doc.endpoint_pos(o))
                    .map_or(at, |(p, _)| p);
                return Some((d.id, None, d.socket_side(socket, to), 0.5, socket));
            }
            let (side, t) = port_spot(d, at, self.snap);
            return Some((d.id, None, side, t, 0));
        }
        None
    }

    fn attach_dragged_point(&mut self, id: Id, (device, port, side, t, socket): PointTarget) {
        let port = match socket {
            0 => port,
            n => self.doc.socket_port(device, n, side).map(|ep| ep.port),
        };
        if let Some(ep) = self.doc.attach_point(id, device, port, side, t) {
            if socket == 0 && port.is_none() {
                self.doc.settle_rear(ep);
            }
            self.selection = Selection::Port(ep);
            let name = self.doc.device(device).map(|d| d.name.clone()).unwrap_or_default();
            self.status = format!("Attached to {name}.");
        }
    }

    pub fn cancel(&mut self) {
        if let Some(snap) = self.snapshot.take()
            && !matches!(self.drag, Drag::None) {
                self.doc = snap;
            }
        if matches!(self.drag, Drag::None) {
            self.selection = Selection::None;
        }
        self.drag = Drag::None;
        self.hover = Hit::Empty;
        self.clear_guides();
        self.validate_selection();
    }

    // Alignment (Ctrl+drag)

    /// Edges and centers of devices and notes, and port positions. With
    /// `moving` set, only the items in `ids` shifted by `delta`; otherwise
    /// everything else.
    fn align_features(&self, ids: &[Id], moving: bool, delta: Pt) -> Vec<Feature> {
        let off = if moving { delta } else { Pt::default() };
        let mut out = vec![];
        let shown = |layer: Id| self.doc.layer_visible(layer);
        for d in self.doc.devices.iter().filter(|d| ids.contains(&d.id) == moving && shown(d.layer)) {
            let (x, y) = (d.x + off.x, d.y + off.y);
            let (x1, y1) = (x + d.w, y + d.h);
            for v in [x, x + d.w / 2.0, x1] {
                out.push(Feature::x(v, y, y1, false));
            }
            for v in [y, y + d.h / 2.0, y1] {
                out.push(Feature::y(v, x, x1, false));
            }
            for port in d.ports.iter().filter(|port| self.doc.port_shown(d, port)) {
                let pp = d.port_pos(port).add(off);
                out.push(Feature::x(pp.x, pp.y, pp.y, true));
                out.push(Feature::y(pp.y, pp.x, pp.x, true));
            }
        }
        // Connection points line up like ports, so lines to them run straight.
        for q in self.doc.points.iter().filter(|q| ids.contains(&q.id) == moving && shown(q.layer)) {
            let pp = q.pos().add(off);
            out.push(Feature::x(pp.x, pp.y, pp.y, true));
            out.push(Feature::y(pp.y, pp.x, pp.x, true));
        }
        for n in self.doc.notes.iter().filter(|n| ids.contains(&n.id) == moving && shown(n.layer)) {
            let r = n.rect();
            let (x, y) = (r.x + off.x, r.y + off.y);
            // Text width is only estimated, so notes align by their left edge.
            out.push(Feature::x(x, y, y + r.h, false));
            for v in [y, y + r.h / 2.0, y + r.h] {
                out.push(Feature::y(v, x, x + r.w, false));
            }
        }
        out
    }

    fn clear_guides(&mut self) {
        self.guides.clear();
        self.spacings.clear();
    }

    fn item_rect(&self, id: Id) -> Option<Rect> {
        self.doc.device(id).map(|d| d.rect()).or_else(|| self.doc.note(id).map(|n| n.rect()))
    }

    /// Boxes for equal spacing: the moving group's bounding box (shifted by
    /// `delta`) and every other box. Along x, notes are left out because
    /// their width is only estimated.
    fn spacing_boxes(&self, ids: &[Id], delta: Pt, along_x: bool) -> (Option<Rect>, Vec<Rect>) {
        let usable = |id: Id| !along_x || self.doc.note(id).is_none();
        let group = if ids.iter().all(|id| usable(*id)) {
            ids.iter()
                .filter_map(|id| self.item_rect(*id))
                .map(|r| Rect { x: r.x + delta.x, y: r.y + delta.y, ..r })
                .reduce(|a, b| a.union(&b))
        } else {
            None
        };
        let devices = self.doc.devices.iter().map(|d| (d.id, d.rect()));
        let notes = self.doc.notes.iter().map(|n| (n.id, n.rect()));
        let others = devices
            .chain(notes)
            .filter(|(id, _)| !ids.contains(id) && usable(*id) && self.doc.is_visible(*id))
            .map(|(_, r)| r)
            .collect();
        (group, others)
    }

    /// Delta for moving the items at `origins` by `raw`: snapped to the grid,
    /// and with Ctrl held, pulled into line with the nearest box edge, box
    /// center or port on each axis. Boxes line up with boxes, and ports with
    /// ports, so links between them become straight. Boxes also snap to
    /// equal spacing: midway between two neighbors in a row or column,
    /// continuing the gap of a neighboring pair, or halfway between two
    /// boxes offset diagonally.
    fn move_delta(&mut self, origins: &[(Id, Pt)], raw: Pt) -> Pt {
        self.clear_guides();
        let mut d = raw;
        if self.snap && let Some(&(id, o)) = origins.first() {
            // Snap the first item (to its level's grid); move the rest by the
            // same delta.
            let g = Self::grid(self.doc.scale_of(id));
            d = Pt::new(snap_to(o.x + raw.x, g) - o.x, snap_to(o.y + raw.y, g) - o.y);
        }
        if !self.align {
            return d;
        }
        // Measure from where the drag started, not from the previous step.
        for &(id, o) in origins {
            self.doc.set_item_pos(id, o);
        }
        let ids: Vec<Id> = origins.iter().map(|(id, _)| *id).collect();
        let targets = self.align_features(&ids, false, Pt::default());
        let sources = self.align_features(&ids, true, raw);
        let tol = ALIGN_PX / self.zoom.max(0.05);
        let closest = |offs: &mut dyn Iterator<Item = f64>| offs.min_by(|a, b| a.abs().total_cmp(&b.abs()));

        // Per axis (`vertical` guides = moving along x): the nearest edge or
        // port alignment, and the nearest equal-gap position.
        let mut feature = [None, None];
        let mut spacing = [None, None];
        for (i, vertical) in [true, false].into_iter().enumerate() {
            feature[i] = sources
                .iter()
                .filter(|s| s.vertical == vertical)
                .flat_map(|s| {
                    targets.iter().filter(move |t| t.vertical == vertical && t.port == s.port).map(move |t| (t.v - s.v, *t))
                })
                .filter(|(off, _)| off.abs() <= tol)
                .min_by(|a, b| a.0.abs().total_cmp(&b.0.abs()));
            let (group, others) = self.spacing_boxes(&ids, raw, vertical);
            spacing[i] = group.and_then(|g| closest(&mut equal_gap_offsets(&others, &g, vertical, tol).into_iter()));
        }
        let best: Vec<Option<f64>> =
            (0..2).map(|i| closest(&mut feature[i].map(|f| f.0).into_iter().chain(spacing[i]))).collect();

        // Diagonal centering moves both axes at once; it wins only if neither
        // axis has a closer match of its own.
        let (group, others) = self.spacing_boxes(&ids, raw, true);
        let diagonal = group
            .and_then(|g| diagonal_center(&others, &g, tol))
            .filter(|(off, ..)| best[0].is_none_or(|b| off.x.abs() <= b.abs()) && best[1].is_none_or(|b| off.y.abs() <= b.abs()));

        let mut matched = vec![];
        if let Some((off, ..)) = diagonal {
            d = raw.add(off);
        } else {
            for (i, b) in best.iter().enumerate() {
                let Some(off) = *b else { continue };
                if i == 0 { d.x = raw.x + off } else { d.y = raw.y + off }
                if let Some((foff, t)) = feature[i]
                    && foff == off
                {
                    matched.push(t);
                }
            }
        }

        // One guide per aligned axis, spanning everything now on that line.
        let sources = self.align_features(&ids, true, d);
        for m in matched {
            let on_line = |f: &&Feature| f.vertical == m.vertical && f.port == m.port && (f.v - m.v).abs() < 0.01;
            let (lo, hi) = sources
                .iter()
                .chain(&targets)
                .filter(on_line)
                .fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), f| (lo.min(f.lo), hi.max(f.hi)));
            let ext = 2.0 * GRID;
            self.guides.push(if m.vertical {
                (Pt::new(m.v, lo - ext), Pt::new(m.v, hi + ext))
            } else {
                (Pt::new(lo - ext, m.v), Pt::new(hi + ext, m.v))
            });
        }

        // Distance markers: wherever gaps in a row or column are now equal
        // (by a spacing snap or because an edge alignment made them so), or
        // center to center for a diagonal pair.
        if let Some((_, a, b)) = diagonal
            && let (Some(g), _) = self.spacing_boxes(&ids, d, true)
        {
            self.spacings.push((center(&a), center(&g)));
            self.spacings.push((center(&g), center(&b)));
        } else {
            for along_x in [true, false] {
                if let (Some(g), others) = self.spacing_boxes(&ids, d, along_x) {
                    equal_gap_markers(&others, &g, along_x, &mut self.spacings);
                }
            }
        }
        d
    }

    /// Slide a line's label (centered `t` of the way along `route`, kept
    /// within `range`) to where its center lines up with the nearest box
    /// edge or center, port, connection point or other line label, with a
    /// guide. `run` are the pieces of the line the label can move along.
    fn align_label(&mut self, route: &[Pt], t: f64, range: (f64, f64), run: &[Id]) -> f64 {
        let c = geom::point_along(route, t);
        let tol = ALIGN_PX / self.zoom.max(0.05);
        // The line's own points are on it already: no use lining up with them.
        let own: Vec<Id> = run
            .iter()
            .filter_map(|id| self.doc.link(*id))
            .flat_map(|l| [l.from, l.to])
            .filter(|e| e.is_point())
            .map(|e| e.device)
            .collect();
        let mut targets = self.align_features(&own, false, Pt::default());
        for (lid, (q, _)) in geom::label_spots(&self.doc) {
            if !run.contains(&lid) {
                targets.push(Feature::x(q.x, q.y, q.y, false));
                targets.push(Feature::y(q.y, q.x, q.x, false));
            }
        }
        // Where the line crosses each target's line, if near the label.
        let total: f64 = route.windows(2).map(|w| w[0].dist(w[1])).sum();
        let mut best: Option<(f64, f64, Pt, Feature)> = None;
        let mut walked = 0.0;
        for w in route.windows(2) {
            let (a, b) = (w[0], w[1]);
            let len = a.dist(b);
            for f in &targets {
                let (av, bv) = if f.vertical { (a.x, b.x) } else { (a.y, b.y) };
                if (bv - av).abs() < 1e-9 {
                    continue;
                }
                let s = (f.v - av) / (bv - av);
                if !(0.0..=1.0).contains(&s) {
                    continue;
                }
                let q = a.add(b.sub(a).scale(s));
                let tq = (walked + s * len) / total.max(1e-9);
                let d = q.dist(c);
                if d <= tol && tq >= range.0 && tq <= range.1 && best.is_none_or(|b| d < b.0) {
                    best = Some((d, tq, q, *f));
                }
            }
            walked += len;
        }
        let Some((_, tq, q, f)) = best else { return t };
        let ext = 2.0 * GRID;
        self.guides.push(if f.vertical {
            (Pt::new(f.v, q.y.min(f.lo) - ext), Pt::new(f.v, q.y.max(f.hi) + ext))
        } else {
            (Pt::new(q.x.min(f.lo) - ext, f.v), Pt::new(q.x.max(f.hi) + ext, f.v))
        });
        tq
    }

    /// Slide a port being dragged along `side` into line with the nearest
    /// port on another device, so the link between them runs straight.
    fn align_port(&mut self, ep: Endpoint, side: Side, t: f64) -> f64 {
        let Some(d) = self.doc.device(ep.device) else { return t };
        let p = d.point_on_side(side, t);
        let tol = ALIGN_PX / self.zoom.max(0.05);
        // Ports on top/bottom slide horizontally and line up vertically, and vice versa.
        let along = |q: Pt| if side.is_horizontal() { q.y } else { q.x };
        let best = self
            .doc
            .devices
            .iter()
            .filter(|o| o.id != ep.device)
            .flat_map(|o| o.ports.iter().map(move |q| o.port_pos(q)))
            .filter(|q| (along(*q) - along(p)).abs() <= tol)
            .min_by(|a, b| (along(*a) - along(p)).abs().total_cmp(&(along(*b) - along(p)).abs()));
        let Some(q) = best else { return t };
        let (start, len) = if side.is_horizontal() { (d.y, d.h) } else { (d.x, d.w) };
        let nt = clamp_t((along(q) - start) / len);
        let np = d.point_on_side(side, nt);
        if (along(np) - along(q)).abs() > 0.01 {
            return t; // out of reach near a corner
        }
        self.guides.push((np, q));
        nt
    }

    pub fn hover(&mut self, p: Pt) -> i32 {
        self.hover = self.hit_test(p);
        self.hover_at = p;
        self.cursor_for(self.hover)
    }

    /// The pointer left the canvas: nothing is pointed at.
    pub fn hover_out(&mut self) {
        self.hover = Hit::Empty;
    }

    /// What line `id` connects, for its tip: its ends (of the whole line
    /// when it is bent through points), the cabling run end to end when
    /// it goes on through patch panels, and its type and label.
    pub fn link_tip(&self, id: Id) -> Vec<String> {
        self.tip_lines(id, false)
    }

    /// [`Editor::link_tip`], or with `through`, for the line drawn end to end
    /// in place of cable `id` and those after it through hidden devices.
    fn tip_lines(&self, id: Id, through: bool) -> Vec<String> {
        let doc = &self.doc;
        let Some(l) = doc.link(id) else { return vec![] };
        let end = |ep: Endpoint| -> String {
            if ep.is_point() {
                return match doc.point(ep.owner()).map(|q| q.label.as_str()) {
                    Some(label) if !label.is_empty() => label.to_string(),
                    _ => "a free point".to_string(),
                };
            }
            let Some(d) = doc.device(ep.device) else { return "?".into() };
            let device = if d.name.is_empty() { crate::shape::kind_label(&d.kind).to_string() } else { d.name.clone() };
            let port = match d.port(ep.port) {
                Some(p) if p.socket > 0 && p.rear => format!("back of socket {}", p.socket),
                Some(p) if p.socket > 0 && p.name.is_empty() => format!("socket {}", p.socket),
                Some(p) => p.name.clone(),
                None => String::new(),
            };
            if port.is_empty() { device } else { format!("{device} {port}") }
        };
        let kind = crate::frame::kind_label(l.style.kind);
        if through {
            let shown = doc.visible_only();
            let Some(s) = shown.link(id) else { return vec![] };
            let cables = doc.cabling_run(id).map_or(String::new(), |(_, _, n)| format!(" ({n} cables)"));
            return vec![
                format!("{}  ⟷  {}", end(s.from), end(s.to)),
                format!("Through hidden devices{cables}"),
                kind.to_string(),
            ];
        }
        let bent = geom::bent_line(doc, l);
        let mut out = vec![format!("{}  ⟷  {}", end(bent.from), end(bent.to))];
        if let Some((a, b, n)) = doc.cabling_run(id)
            && (a.owner(), b.owner()) != (bent.from.owner(), bent.to.owner())
            && (a.owner(), b.owner()) != (bent.to.owner(), bent.from.owner())
        {
            out.push(format!("End to end ({n} cables): {}  ⟷  {}", end(a), end(b)));
        }
        out.push(if l.label.is_empty() { kind.to_string() } else { format!("{kind} · {}", l.label) });
        out
    }

    fn link_target(&self, src: LinkSource, p: Pt) -> Option<Target> {
        let t = match self.hit_test(p) {
            Hit::Port(ep) => Target::Port(ep),
            Hit::Edge { device, side, t } => Target::NewPort { device, side, t },
            Hit::Socket { device, socket } => Target::Socket { device, socket },
            Hit::Device(id) | Hit::Resize(id) => {
                let from = self.source_pos(src)?.0;
                let (side, t) = facing_spot(self.doc.device(id)?, from);
                Target::NewPort { device: id, side, t }
            }
            Hit::Point(id) => Target::Port(Endpoint::point(id)),
            // Nothing to attach to: the line ends at a new connection point,
            // unless it is too short to be meant.
            Hit::Note(_) | Hit::Link(_) | Hit::Label(_) | Hit::Through(_) | Hit::Empty => {
                // Locked devices cannot take a new port, and a loose end on
                // top of one would only look attached.
                if self.locked_device_at(p) {
                    return None;
                }
                let from = self.source_pos(src)?.0;
                if from.dist(p) * self.zoom < MIN_FREE_LINK_PX {
                    return None;
                }
                Target::NewPoint(p)
            }
        };
        match (src, t) {
            (LinkSource::Port(a), Target::Port(b)) if a == b => None,
            (LinkSource::Socket { device, socket }, Target::Socket { device: d2, socket: s2 }) if device == d2 && socket == s2 => None,
            (LinkSource::Edge { device, side, t: ta }, Target::NewPort { device: d2, side: s2, t: tb })
                if device == d2 && side == s2 && (ta - tb).abs() < 0.03 =>
            {
                None
            }
            _ => Some(t),
        }
    }

    fn locked_device_at(&self, p: Pt) -> bool {
        self.doc.devices.iter().any(|d| {
            self.doc.layer_visible(d.layer) && self.doc.layer_locked(d.layer) && d.rect().contains(p)
        })
    }

    /// A spot on a border where a new port would go: under a patch panel's
    /// socket, at the edge right above or below it (it is that socket's back).
    fn edge_pos(&self, device: Id, side: Side, t: f64) -> Option<Pt> {
        let d = self.doc.device(device)?;
        Some(d.rear_socket_at(side, t).and_then(|n| d.rear_pos(n, side == Side::Top)).unwrap_or_else(|| d.point_on_side(side, t)))
    }

    /// The socket back a line being drawn (or a point being dragged) would
    /// take if dropped now, to show its tie before: (device, socket, up).
    fn pending_rear(&self) -> Option<(Id, u32, bool)> {
        let at_edge = |device: Id, side: Side, t: f64| {
            let d = self.doc.device(device)?;
            let n = d.rear_socket_at(side, t)?;
            Some((device, n, side == Side::Top))
        };
        match &self.drag {
            Drag::NewLink { src, cursor } if self.moved => {
                if let Some(Target::NewPort { device, side, t }) = self.link_target(*src, *cursor) {
                    return at_edge(device, side, t);
                }
                match *src {
                    LinkSource::Edge { device, side, t } => at_edge(device, side, t),
                    _ => None,
                }
            }
            Drag::MoveItems { origins, .. } if self.moved && origins.len() == 1 => match self.dragged_point_target(origins[0].0)? {
                (device, None, side, t, 0) => at_edge(device, side, t),
                _ => None,
            },
            // Pointed at, before a line is drawn from there.
            Drag::None => match self.hover {
                Hit::Edge { device, side, t } => at_edge(device, side, t),
                _ => None,
            },
            _ => None,
        }
    }

    fn target_pos(&self, t: Target) -> Option<(Pt, Side)> {
        match t {
            Target::Port(ep) => self.doc.endpoint_pos(ep),
            Target::NewPort { device, side, t } => Some((self.edge_pos(device, side, t)?, side)),
            Target::Socket { device, socket } => {
                let d = self.doc.device(device)?;
                Some((d.socket_pos(socket)?, Side::Bottom))
            }
            Target::NewPoint(p) => Some((p, Side::Left)),
        }
    }

    fn source_pos(&self, src: LinkSource) -> Option<(Pt, Side)> {
        match src {
            LinkSource::Port(ep) => self.doc.endpoint_pos(ep),
            LinkSource::Edge { device, side, t } => Some((self.edge_pos(device, side, t)?, side)),
            LinkSource::Socket { device, socket } => {
                let d = self.doc.device(device)?;
                Some((d.socket_pos(socket)?, Side::Bottom))
        }
    }
    }

    /// Which way a line from socket `socket` of `device` leaves towards `to`.
    fn socket_side(&self, device: Id, socket: u32, to: Pt) -> Side {
        self.doc.device(device).map_or(Side::Bottom, |d| d.socket_side(socket, to))
    }

    fn finish_link(&mut self, src: LinkSource, p: Pt) {
        let Some(target) = self.link_target(src, p) else { return };
        if !self.can_add() {
            return;
        }
        let (Some((a, _)), Some((b, _))) = (self.source_pos(src), self.target_pos(target)) else {
            return;
        };
        let from = match src {
            LinkSource::Port(ep) => ep,
            LinkSource::Edge { device, side, t } => match self.doc.add_port(device, side, t) {
                Some(ep) => {
                    self.doc.settle_rear(ep);
                    ep
                }
                None => return,
            },
            LinkSource::Socket { device, socket } => {
                let side = self.socket_side(device, socket, b);
                match self.doc.socket_port(device, socket, side) {
                    Some(ep) => ep,
                    None => return,
                }
            }
        };
        let to = match target {
            Target::Port(ep) => ep,
            Target::NewPort { device, side, t } => match self.doc.add_port(device, side, t) {
                Some(ep) => {
                    self.doc.settle_rear(ep);
                    ep
                }
                None => return,
            },
            Target::Socket { device, socket } => {
                let side = self.socket_side(device, socket, a);
                match self.doc.socket_port(device, socket, side) {
                    Some(ep) => ep,
                    None => return,
                }
            }
            Target::NewPoint(p) => {
                let p = self.snap_pt(p, self.level());
                let q = self.doc.add_point(p);
                self.doc.point_mut(q).unwrap().scale = self.level();
                Endpoint::point(q)
            }
        };
        // Drawn at the level being worked on; the default width is nominal.
        let level = self.level();
        let mut style = self.link_defaults.clone();
        style.width *= level;
        let id = self.doc.add_link(from, to, style);
        self.doc.link_mut(id).unwrap().scale = level;
        self.selection = Selection::Link(id);
        self.status = "Link created. Set arrows and style in the panel on the right.".into();
    }

    /// Double click: on empty canvas adds a device, on a device border adds a
    /// port; then starts editing the text of whatever is under the mouse.
    pub fn double_click(&mut self, p: Pt) {
        // The second press of the double click already started a (still
        // unmoved) drag; discard it so it does not record its own undo step.
        if !self.moved {
            self.drag = Drag::None;
            self.snapshot = None;
        }
        let target = match self.hit_test(p) {
            Hit::Empty if !self.can_add() => return,
            Hit::Empty => EditTarget::DeviceName(self.add_device_at(p)),
            Hit::Edge { device, side, t } => {
                self.checkpoint();
                let snapped = self.doc.device(device).map(|d| port_spot(d, d.point_on_side(side, t), self.snap));
                let (side, t) = snapped.unwrap_or((side, t));
                match self.doc.add_port(device, side, t) {
                    Some(ep) => {
                        self.doc.settle_rear(ep);
                        EditTarget::PortName(ep)
                    }
                    None => return,
                }
            }
            Hit::Socket { device, socket } => {
                self.checkpoint();
                match self.doc.socket_port(device, socket, Side::Bottom) {
                    Some(ep) => EditTarget::PortName(ep),
                    None => return,
                }
            }
            Hit::Port(ep) => EditTarget::PortName(ep),
            Hit::Device(id) | Hit::Resize(id) => EditTarget::DeviceName(id),
            Hit::Note(id) => EditTarget::NoteText(id),
            Hit::Link(id) | Hit::Label(id) => EditTarget::LinkLabel(id),
            Hit::Point(id) => EditTarget::PointLabel(id),
            // Its label is not shown: nothing to edit there.
            Hit::Through(_) => return,
        };
        self.begin_edit(target);
    }

    // In-place text editing

    fn edit_text(&self, t: EditTarget) -> Option<String> {
        match t {
            EditTarget::DeviceName(id) => self.doc.device(id).map(|d| d.name.clone()),
            EditTarget::PortName(ep) => self.doc.device(ep.device)?.port(ep.port).map(|p| p.name.clone()),
            EditTarget::LinkLabel(id) => self.doc.link(id).map(|l| l.label.clone()),
            EditTarget::NoteText(id) => self.doc.note(id).map(|n| n.text.clone()),
            EditTarget::PointLabel(id) => self.doc.point(id).map(|q| q.label.clone()),
        }
    }

    pub fn begin_edit(&mut self, t: EditTarget) {
        if self.edit_text(t).is_none() {
            return;
        }
        self.selection = match t {
            EditTarget::DeviceName(id) | EditTarget::NoteText(id) | EditTarget::PointLabel(id) => Selection::Items(vec![id]),
            EditTarget::PortName(ep) => Selection::Port(ep),
            EditTarget::LinkLabel(id) => Selection::Link(id),
        };
        self.drag = Drag::None;
        self.editing = Some(t);
    }

    /// Start editing the text of the current selection (F2 / Enter / typing).
    pub fn begin_edit_selection(&mut self) -> bool {
        let t = match &self.selection {
            Selection::Items(ids) if ids.len() == 1 && self.doc.note(ids[0]).is_some() => EditTarget::NoteText(ids[0]),
            Selection::Items(ids) if ids.len() == 1 && self.doc.point(ids[0]).is_some() => EditTarget::PointLabel(ids[0]),
            Selection::Items(ids) if ids.len() == 1 => EditTarget::DeviceName(ids[0]),
            Selection::Port(ep) => EditTarget::PortName(*ep),
            Selection::Link(id) => EditTarget::LinkLabel(*id),
            _ => return false,
        };
        self.begin_edit(t);
        self.editing.is_some()
    }

    /// Apply the edited text as one undo step (nothing if unchanged). Only
    /// notes keep line breaks; a note left empty is removed.
    #[cfg(test)]
    pub fn commit_edit(&mut self, text: &str) {
        self.commit_edit_sized(text, 0.0, 0.0);
    }

    /// [`commit_edit`](Self::commit_edit), and for a note the size its
    /// editor was given (at the note's scale; 0 keeps its size): the note
    /// keeps that box, wrapping and cutting its text to it.
    pub fn commit_edit_sized(&mut self, text: &str, w: f64, h: f64) {
        let Some(t) = self.editing else { return };
        let sized = (w > 0.0 && h > 0.0 && w.is_finite() && h.is_finite()).then_some((w, h));
        if let (EditTarget::NoteText(id), Some((w, h))) = (t, sized)
            && let Some(n) = self.doc.note(id)
            && (n.w - w).abs().max((n.h - h).abs()) > 0.5
            && !text.trim().is_empty()
        {
            self.checkpoint();
            let home = home_box(&self.doc, id);
            let n = self.doc.note_mut(id).unwrap();
            let (mut w, mut h) = (w, h);
            // Not past the device it was made in.
            if let Some(r) = home {
                w = w.min((r.x + r.w - n.x) / n.scale);
                h = h.min((r.y + r.h - n.y) / n.scale);
            }
            (n.w, n.h) = (w.max(NOTE_MIN_BOX), h.max(n.size * NOTE_LINE_HEIGHT));
            // The text change, if any, joins this undo step.
            self.last_edit = Some("sized".into());
        }
        let Some(t) = self.editing.take() else { return };
        let text = if matches!(t, EditTarget::NoteText(_)) {
            text.trim().replace("\r\n", "\n")
        } else {
            text.split_whitespace().collect::<Vec<_>>().join(" ")
        };
        let text = text.as_str();
        if self.edit_text(t).as_deref() == Some(text) {
            self.last_edit = None;
            return;
        }
        if self.last_edit.as_deref() != Some("sized") {
            self.checkpoint();
        }
        self.last_edit = None;
        match t {
            EditTarget::DeviceName(id) => {
                if let Some(d) = self.doc.device_mut(id) {
                    d.name = text.into();
                }
            }
            EditTarget::PortName(ep) => {
                if let Some(p) = self.doc.device_mut(ep.device).and_then(|d| d.ports.iter_mut().find(|p| p.id == ep.port)) {
                    p.name = text.into();
                }
            }
            EditTarget::LinkLabel(id) => {
                if let Some(l) = self.doc.link_mut(id) {
                    // A label typed in is meant to be seen.
                    if !text.is_empty() {
                        l.label_hidden = false;
                    }
                    l.label = text.into();
                }
            }
            EditTarget::NoteText(id) if text.is_empty() => {
                self.doc.remove_note(id);
                self.validate_selection();
            }
            EditTarget::NoteText(id) => {
                if let Some(n) = self.doc.note_mut(id) {
                    n.text = text.into();
                }
            }
            EditTarget::PointLabel(id) => {
                if let Some(q) = self.doc.point_mut(id) {
                    q.label = text.into();
                }
            }
        }
    }

    pub fn cancel_edit(&mut self) {
        self.editing = None;
    }

    /// Where QML should overlay the text field, in world coordinates:
    /// anchor point plus alignment, font and minimum width. Empty if not editing.
    pub fn edit_box_json(&self) -> String {
        let Some(t) = self.editing else { return String::new() };
        let Some(text) = self.edit_text(t) else { return String::new() };
        let doc = &self.doc;
        let v = match t {
            EditTarget::DeviceName(id) => {
                let Some(d) = doc.device(id) else { return String::new() };
                // A device with contents has its name at the top.
                // Sizes are nominal (`s` scales them for deeper levels).
                let s = d.scale;
                let container = doc.containers().values().any(|c| *c == id);
                let local = Rect { x: d.x, y: d.y, w: d.w / s, h: d.h / s };
                let (dx, dy) = crate::shape::device_shape(&d.kind, &d.name, &local, container, d.panel.as_ref()).label;
                let cy = if container {
                    d.y + CONTAINER_NAME_Y * s
                } else {
                    d.y + d.h / 2.0 + (dy - if d.kind.is_empty() { 0.0 } else { 6.0 }) * s
                };
                json!({"key": format!("d{id}"), "text": text, "x": d.x + d.w / 2.0 + dx * s, "y": cy, "s": s,
                       "align": "center", "base": "middle", "size": 13, "bold": true, "minw": d.w / s - 8.0, "multi": false})
            }
            EditTarget::PortName(ep) => {
                let (Some(d), Some(p)) = (doc.device(ep.device), doc.device(ep.device).and_then(|d| d.port(ep.port))) else {
                    return String::new();
                };
                let dirs = geom::port_link_dirs(doc);
                let (lp, align, base) =
                    geom::port_label(d, p, &dirs);
                json!({"key": format!("p{}", ep.port), "text": text, "x": lp.x, "y": lp.y, "s": d.scale,
                       "align": align, "base": base, "size": 10, "bold": false, "minw": 60.0, "multi": false})
            }
            EditTarget::LinkLabel(id) => {
                let Some(l) = doc.link(id) else { return String::new() };
                let Some(route) = geom::link_route(doc, l) else { return String::new() };
                let m = geom::label_spots(doc).get(&id).map_or_else(|| geom::midpoint(&route), |s| s.0);
                json!({"key": format!("l{id}"), "text": text, "x": m.x, "y": m.y, "s": l.scale,
                       "align": "center", "base": "middle", "size": 11, "bold": false, "minw": 70.0, "multi": false})
            }
            EditTarget::PointLabel(id) => {
                let Some(q) = doc.point(id) else { return String::new() };
                let (at, align, base) = geom::point_label_anchor(doc, q);
                json!({"key": format!("q{id}"), "text": text, "x": at.x, "y": at.y, "s": q.scale,
                       "align": align, "base": base, "size": 11, "bold": false, "minw": 60.0, "multi": false})
            }
            EditTarget::NoteText(id) => {
                let Some(n) = doc.note(id) else { return String::new() };
                // At the text's top left corner (the field puts its inner
                // margin outside it); a boxed note opens at its box.
                json!({"key": format!("n{id}"), "text": text, "x": n.x, "y": n.y, "s": n.scale, "inset": true,
                       "align": "left", "base": "top", "size": n.size, "bold": n.bold, "minw": 80.0, "multi": true,
                       "bw": n.w, "bh": n.h})
            }
        };
        v.to_string()
    }

    // Commands

    /// The first free name "<base> 1", "<base> 2"...
    fn next_device_name(&self, base: &str) -> String {
        (1..)
            .map(|n| format!("{base} {n}"))
            .find(|name| !self.doc.devices.iter().any(|d| &d.name == name))
            .unwrap()
    }

    /// Add a plain device centered on `p`.
    pub fn add_device_at(&mut self, p: Pt) -> Id {
        self.add_device_of("", p)
    }

    /// Add a device of type `kind` ("" for a plain one) at `p` or, when
    /// that spot is taken, at the nearest free one beside it (the toolbar's
    /// buttons add at the middle of the view, again and again).
    pub fn add_device_near(&mut self, kind: &str, p: Pt) -> Id {
        let s = self.level();
        let (w, h, gap) = (120.0 * s, 60.0 * s, 40.0 * s);
        let box_at = |c: Pt| Rect { x: c.x - w / 2.0, y: c.y - h / 2.0, w, h };
        // Taken by a device beside it; one it lies inside (being zoomed
        // into) leaves room.
        let free = |c: Pt| {
            let r = box_at(c).inflate(gap / 4.0);
            !self.doc.devices.iter().any(|d| {
                let o = d.rect();
                !o.contains_rect(&r) && r.x < o.x + o.w && o.x < r.x + r.w && r.y < o.y + o.h && o.y < r.y + r.h
            })
        };
        // Rings around the spot, nearest first: right, below, left, above.
        let spot = (0..6i32)
            .flat_map(|ring| {
                let mut cells = vec![];
                for dy in -ring..=ring {
                    for dx in -ring..=ring {
                        if dx.abs().max(dy.abs()) == ring {
                            cells.push((dx, dy));
                        }
                    }
                }
                cells.sort_by_key(|&(dx, dy)| (dx * dx + dy * dy, (dy < 0) as i32, -dx));
                cells
            })
            .map(|(dx, dy)| Pt::new(p.x + dx as f64 * (w + gap), p.y + dy as f64 * (h + gap)))
            .find(|c| free(*c))
            .unwrap_or(p);
        self.add_device_of(kind, spot)
    }

    /// Add a device of type `kind` ("" for a plain one) centered on `p`,
    /// named after its type ("Router 1") and in the type's color.
    pub fn add_device_of(&mut self, kind: &str, p: Pt) -> Id {
        let kind = known_kind(kind).unwrap_or(kind.trim());
        self.checkpoint();
        let s = self.level();
        let (w, h) = (120.0 * s, 60.0 * s);
        let at = self.snap_pt(Pt::new(p.x - w / 2.0, p.y - h / 2.0), s);
        let name = self.next_device_name(&kind_title(kind));
        let color = kind_color(kind).unwrap_or(KINDS[0].1);
        let id = self.doc.add_device(&name, kind, Rect { x: at.x, y: at.y, w, h }, color);
        let d =
        self.doc.device_mut(id).unwrap();
        d.scale = s;
        d.fit_panel();
        self.selection = Selection::Items(vec![id]);
        id
    }

    /// Add the devices listed in `spec` (see [`parse_quick_add`]) as one undo
    /// step, laid out in rows centered on `center` and moved down below the
    /// existing diagram if they would overlap it. Returns the new ids, which become selected.
    pub fn quick_add(&mut self, spec: &str, center: Pt) -> Vec<Id> {
        const COLS: usize = 6;
        let s = self.level();
        let (w, h, gap) = (120.0 * s, 60.0 * s, 40.0 * s);
        let items = parse_quick_add(spec);
        if items.is_empty() || !self.can_add() {
            return vec![];
        }
        self.checkpoint();
        let cols = items.len().min(COLS);
        let rows = items.len().div_ceil(COLS);
        let block_w = cols as f64 * (w + gap) - gap;
        let block_h = rows as f64 * (h + gap) - gap;
        let g = Self::grid(s);
        let x0 = snap_to(center.x - block_w / 2.0, g);
        let mut y0 = snap_to(center.y - block_h / 2.0, g);
        // Below everything already there, so new boxes never sit across
        // links (at the top level; zoomed in, right where you are).
        if s >= 1.0
            && let Some(b) = self.doc.bounds()
        {
            y0 = y0.max(snap_to(b.y + b.h + 60.0, g));
        }
        let ids = items
            .iter()
            .enumerate()
            .map(|(i, it)| {
                let name = if it.named { it.name.clone() } else { self.unique_name(&it.name) };
                let r = Rect {
                    x: x0 + (i % COLS) as f64 * (w + gap),
                    y: y0 + (i / COLS) as f64 * (h + gap),
                    w,
                    h,
                };
                let id = self.doc.add_device(&name, &it.kind, r, kind_color(&it.kind).unwrap_or(KINDS[0].1));
                let d =
                self.doc.device_mut(id).unwrap();
                d.scale = s;
                d.fit_panel();
                id
            })
            .collect::<Vec<_>>();
        self.selection = Selection::Items(ids.clone());
        ids
    }

    /// `base`, or `base 2`, `base 3`… if a device already has that name.
    fn unique_name(&self, base: &str) -> String {
        let taken = |n: &str| self.doc.devices.iter().any(|d| d.name == n);
        if !taken(base) {
            return base.into();
        }
        (2..).map(|n| format!("{base} {n}")).find(|n| !taken(n)).unwrap()
    }

    /// Add a free connection point at `p` and select it.
    pub fn add_point_at(&mut self, p: Pt) -> Id {
        self.checkpoint();
        let at = self.snap_pt(p, self.level());
        let id = self.doc.add_point(at);
        self.doc.point_mut(id).unwrap().scale = self.level();
        self.selection = Selection::Items(vec![id]);
        id
    }

    /// Add a text note with its top left corner at `p` and start editing it.
    pub fn add_note_at(&mut self, p: Pt) -> Id {
        self.checkpoint();
        let at = self.snap_pt(p, self.level());
        let id = self.doc.add_note("Text", at);
        self.doc.note_mut(id).unwrap().scale = self.level();
        self.begin_edit(EditTarget::NoteText(id));
        id
    }

    pub fn add_port(&mut self, side: Side) -> bool {
        let id = match &self.selection {
            Selection::Items(ids) if ids.len() == 1 => ids[0],
            Selection::Port(ep) => ep.device,
            _ => return false,
        };
        let Some(t) = self.doc.device(id).map(|d| d.free_spot(side)) else { return false };
        self.checkpoint();
        if let Some(ep) = self.doc.add_port(id, side, t) {
            self.selection = Selection::Port(ep);
        }
        true
    }

    pub fn delete_selected(&mut self) {
        match self.selection.clone() {
            Selection::None => return,
            Selection::Items(ids) => {
                self.checkpoint();
                for id in ids {
                    // A bend in a line straightens out; other points go
                    // with their links.
                    if self.doc.heal_point(id) {
                        continue;
                    }
                    // Lines passing through a device join up again.
                    self.doc.join_through_device(id);
                    self.doc.remove_device(id);
                    self.doc.remove_note(id);
                    self.doc.remove_point(id);
                    self.doc.remove_link(id);
                }
            }
            Selection::Port(ep) => {
                self.checkpoint();
                self.doc.remove_port(ep.device, ep.port);
            }
            Selection::Link(id) => {
                self.checkpoint();
                self.doc.remove_link(id);
            }
        }
        self.selection = Selection::None;
    }

    pub fn select_all(&mut self) {
        let devices = self.doc.devices.iter().map(|d| d.id);
        let notes = self.doc.notes.iter().map(|n| n.id);
        let links = self.doc.links.iter().map(|l| l.id);
        let points = self.doc.points.iter().map(|p| p.id);
        let ids: Vec<Id> =
            devices.chain(notes).chain(points).chain(links).filter(|id| self.reachable(*id)).collect();
        self.select_ids(ids);
    }

    /// Duplicate selected devices and notes, together with the links between
    /// the devices. The copies go into the active layer, in front of
    /// everything, stacked like the originals.
    pub fn duplicate(&mut self) {
        let Selection::Items(ids) = self.selection.clone() else { return };
        if !self.can_add() {
            return;
        }
        self.checkpoint();
        let layer = self.doc.target_layer();
        let mut port_map = std::collections::HashMap::new();
        let mut dev_map = std::collections::HashMap::new();
        // Original id -> copy id, for restacking.
        let mut copies = std::collections::HashMap::new();
        let mut new_ids = vec![];
        for id in &ids {
            if let Some(mut q) = self.doc.point(*id).cloned() {
                q.id = self.doc.alloc_id();
                q.x += 2.0 * GRID * q.scale;
                q.y += 2.0 * GRID * q.scale;
                q.layer = layer;
                copies.insert(*id, q.id);
                new_ids.push(q.id);
                self.doc.points.push(q);
                continue;
            }
            if let Some(mut note) = self.doc.note(*id).cloned() {
                note.id = self.doc.alloc_id();
                note.layer = layer;
                copies.insert(*id, note.id);
                note.x += 2.0 * GRID * note.scale;
                note.y += 2.0 * GRID * note.scale;
                new_ids.push(note.id);
                self.doc.notes.push(note);
                continue;
            }
            let Some(src) = self.doc.device(*id).cloned() else { continue };
            let mut copy = src.clone();
            copy.id = self.doc.alloc_id();
            copy.x += 2.0 * GRID * copy.scale;
            copy.y += 2.0 * GRID * copy.scale;
            copy.name = self.copy_name(&src.name);
            copy.layer = layer;
            // A copy is not the device it was made from (in NetBox, say).
            copy.source.clear();
            copies.insert(src.id, copy.id);
            for port in &mut copy.ports {
                let new_port = self.doc.alloc_id();
                port_map.insert(port.id, new_port);
                port.id = new_port;
                port.source.clear();
            }
            dev_map.insert(src.id, copy.id);
            new_ids.push(copy.id);
            self.doc.devices.push(copy);
        }
        let internal: Vec<Link> = self
            .doc
            .links
            .iter()
            .filter(|l| ids.contains(&l.from.owner()) && ids.contains(&l.to.owner()))
            .cloned()
            .collect();
        let map_end = |ep: Endpoint| match ep.is_point() {
            true => Endpoint::point(copies[&ep.owner()]),
            false => Endpoint { device: dev_map[&ep.device], port: port_map[&ep.port] },
        };
        let mut link_copies = vec![];
        for mut l in internal {
            let orig = l.id;
            l.id = self.doc.alloc_id();
            l.layer = layer;
            l.source.clear();
            link_copies.push((orig, l.id));
            l.from = map_end(l.from);
            l.to = map_end(l.to);
            self.doc.links.push(l);
        }
        copies.extend(link_copies);
        let old_order: Vec<Id> = self.doc.z_order().into_iter().map(|it| self.doc.item_id(it)).collect();
        let copy_ids: Vec<Id> = copies.values().copied().collect();
        let mut order: Vec<Id> = old_order.iter().copied().filter(|id| !copy_ids.contains(id)).collect();
        order.extend(old_order.iter().filter_map(|id| copies.get(id)));
        self.doc.set_z_order(&order);
        self.selection = Selection::Items(new_ids);
    }

    fn copy_name(&self, name: &str) -> String {
        if name.is_empty() {
            return String::new();
        }
        // "sw-01" -> "sw-02", "Core" -> "Core 2"
        let digits = name.chars().rev().take_while(|c| c.is_ascii_digit()).count();
        let (stem, num, width) = if digits > 0 {
            let (s, n) = name.split_at(name.len() - digits);
            (s.to_string(), n.parse::<u64>().unwrap_or(1), digits)
        } else {
            (format!("{name} "), 1, 1)
        };
        (num + 1..)
            .map(|n| format!("{stem}{n:0width$}"))
            .find(|cand| !self.doc.devices.iter().any(|d| &d.name == cand))
            .unwrap()
    }

    pub fn nudge(&mut self, dx: f64, dy: f64) {
        let Selection::Items(ids) = self.selection.clone() else { return };
        if self.last_edit.as_deref() != Some("nudge") {
            self.checkpoint();
            self.last_edit = Some("nudge".into());
        }
        // Items made inside a device stay inside it.
        let (lo, hi) = belonging_limits(&self.doc, &ids);
        let (dx, dy) = (dx.clamp(lo.x, hi.x.max(lo.x)), dy.clamp(lo.y, hi.y.max(lo.y)));
        for id in ids {
            if let Some(p) = self.doc.item_pos(id) {
                self.doc.set_item_pos(id, Pt::new(p.x + dx, p.y + dy));
            }
        }
    }

    pub fn reverse_link(&mut self) {
        let Selection::Link(id) = self.selection else { return };
        self.checkpoint();
        if let Some(l) = self.doc.link_mut(id) {
            std::mem::swap(&mut l.from, &mut l.to);
            std::mem::swap(&mut l.style.head_start, &mut l.style.head_end);
        }
    }

    /// Edit a property of the current selection. Values arrive as strings
    /// from the UI. Returns false if the key or value is not valid.
    pub fn set_prop(&mut self, key: &str, value: &str) -> bool {
        let tag = format!("{:?}/{key}", self.selection);
        let before = self.doc.clone();
        let ok = self.apply_prop(key, value);
        // Moving items to a hidden layer hides them.
        self.validate_selection();
        if ok && self.doc != before {
            if self.last_edit.as_deref() != Some(tag.as_str()) {
                self.push_undo(before);
            }
            self.touch();
            self.last_edit = Some(tag);
        }
        ok
    }

    /// Set a property of the frame: "template" ("none" removes the frame;
    /// any other adds it on a new bottom layer if there is none), its texts
    /// ("title", "subtitle", "author", "note", "company", "doc_id",
    /// "revision", "approver", "status", "level", "address", "page"),
    /// "show_date", "show_file", "margin", "color", "revisions" (one row
    /// per line: rev | date | change | by) or "logo" (a PNG or JPEG file's
    /// path, kept in the diagram; empty removes it).
    pub fn set_frame(&mut self, key: &str, value: &str) -> bool {
        let tag = format!("frame/{key}");
        let before = self.doc.clone();
        let ok = self.apply_frame(key, value);
        if ok && self.doc != before {
            if self.last_edit.as_deref() != Some(tag.as_str()) {
                self.push_undo(before);
            }
            self.touch();
            self.last_edit = Some(tag);
        }
        ok
    }

    fn apply_frame(&mut self, key: &str, value: &str) -> bool {
        if key == "template" && value == "none" {
            let Some(id) = self.doc.frame_layer() else { return true };
            let empty = self.doc.layer_count(id) == 0
                && !self.doc.notes.iter().any(|n| n.layer == id)
                && !self.doc.points.iter().any(|p| p.layer == id);
            // Its own layer goes with it, unless something else is on it.
            if !(empty && self.doc.layers.len() > 1 && self.doc.remove_layer(id))
                && let Some(l) = self.doc.layer_mut(id)
            {
                l.frame = None;
            }
            return true;
        }
        if key == "template" && FrameTemplate::parse(value).is_some() {
            // A new frame takes all defaults from the configuration file; a
            // different template fills in what is still empty.
            let only_empty = self.doc.frame_layer().is_some();
            if !only_empty {
                // Its own layer, first in the list: drawn under everything.
                self.doc.add_layer("Frame");
                let l = self.doc.layers.pop().unwrap();
                self.doc.layers.insert(0, Layer { frame: Some(Frame::default()), ..l });
            }
            self.fill_frame(value, only_empty);
        }
        if self.doc.frame_layer().is_none() {
            return false;
        }
        let Some(f) = self.doc.layers.iter_mut().find_map(|l| l.frame.as_mut()) else { return false };
        match key {
            "template" => match FrameTemplate::parse(value) {
                Some(t) => f.template = t,
                None => return false,
            },
            "title" => f.title = value.into(),
            "subtitle" => f.subtitle = value.into(),
            "author" => f.author = value.into(),
            "note" => f.note = value.into(),
            "company" => f.company = value.into(),
            "doc_id" => f.doc_id = value.into(),
            "revision" => f.revision = value.into(),
            "approver" => f.approver = value.into(),
            "status" => f.status = value.into(),
            "level" => f.level = value.into(),
            "address" => f.address = value.into(),
            "page" => f.page = value.into(),
            "version" => f.version = value.into(),
            "owner" => f.owner = value.into(),
            "review_date" => f.review_date = value.into(),
            "reviewer" => f.reviewer = value.into(),
            "customer" => f.customer = value.into(),
            "summary" => f.summary = value.into(),
            "installer" => f.installer = value.into(),
            "commissioned" => f.commissioned = value.into(),
            "severity" => f.severity = value.into(),
            "start" => f.start = value.into(),
            "end" => f.end = value.into(),
            "services" => f.services = value.into(),
            "location" => f.location = value.into(),
            "watermark" => f.watermark = value.into(),
            "show_date" => f.show_date = value == "true",
            "show_file" => f.show_file = value == "true",
            "revisions" => {
                f.revisions = value
                    .lines()
                    .filter(|l| !l.trim().is_empty())
                    .map(|l| {
                        let mut c = l.split('|').map(|s| s.trim().to_string());
                        let mut next = || c.next().unwrap_or_default();
                        Revision { rev: next(), date: next(), change: next(), author: next() }
                    })
                    .collect()
            }
            "logo" | "logo2" if value.is_empty() => {
                let slot = if key == "logo" { (&mut f.logo, &mut f.logo_w, &mut f.logo_h) } else { (&mut f.logo2, &mut f.logo2_w, &mut f.logo2_h) };
                (*slot.0, *slot.1, *slot.2) = (String::new(), 0.0, 0.0);
            }
            "logo" | "logo2" => match crate::frame::logo_from(value) {
                Ok((data, w, h)) => {
                    let slot = if key == "logo" { (&mut f.logo, &mut f.logo_w, &mut f.logo_h) } else { (&mut f.logo2, &mut f.logo2_w, &mut f.logo2_h) };
                    (*slot.0, *slot.1, *slot.2) = (data, w, h);
                }
                Err(e) => {
                    self.status = format!("Logo not set: {e}.");
                    return false;
                }
            },
            "margin" => match value.parse::<f64>() {
                Ok(v) if v.is_finite() => f.margin = v.clamp(0.0, 400.0),
                _ => return false,
            },
            "color" => match geom::normalize_color(value) {
                Some(c) => f.color = c,
                None => return false,
            },
            _ => return false,
        }
        true
    }

    /// Ids of the selected devices, notes and links (the device of a port).
    fn selected_ids(&self) -> Vec<Id> {
        match &self.selection {
            Selection::Items(ids) => ids.clone(),
            Selection::Link(id) => vec![*id],
            Selection::Port(ep) => vec![ep.device],
            Selection::None => vec![],
        }
    }

    /// Select `ids`; a lone link becomes a link selection.
    fn select_ids(&mut self, ids: Vec<Id>) {
        self.selection = match ids.as_slice() {
            [] => Selection::None,
            [id] if self.doc.link(*id).is_some() => Selection::Link(*id),
            _ => Selection::Items(ids),
        };
    }

    /// Set `key` on every selected item it applies to, so one call edits a
    /// whole selection. `device:`, `note:` or `link:` in front of the key
    /// limits it to that kind (for mixed selections). `name_pattern`
    /// renames the selected devices with numbers, see [`Editor::rename_pattern`].
    fn apply_prop(&mut self, key: &str, value: &str) -> bool {
        if key == "layer" && !matches!(self.selection, Selection::Port(_)) {
            // Move the selected items (or link) to another layer.
            // Not onto the frame's layer: it holds only the frame.
            let Some(layer) = value.parse::<Id>().ok().filter(|l| self.doc.layer(*l).is_some_and(|l| l.frame.is_none()))
            else {
                return false;
            };
            let ids = match &self.selection {
                Selection::Items(_) | Selection::Link(_) => self.selected_ids(),
                _ => return false,
            };
            for id in ids {
                self.doc.set_layer_of(id, layer);
            }
            return true;
        }
        if let Selection::Port(ep) = self.selection {
            let Some(d) = self.doc.device_mut(ep.device) else { return false };
            let Some(port) = d.ports.iter_mut().find(|p| p.id == ep.port) else { return false };
            match key {
                "name" => port.name = value.into(),
                "side" => match Side::parse(value) {
                    Some(s) => port.side = s,
                    None => return false,
                },
                // Position along the side in percent (0 = left/top).
                "pos" => match value.parse::<f64>() {
                    Ok(v) if v.is_finite() => port.t = clamp_t(v / 100.0),
                    _ => return false,
                },
                // A layer of its own, or "" / 0 for its device's.
                "layer" => {
                    let id: Id = if value.trim().is_empty() { 0 } else { value.parse().ok().unwrap_or(Id::MAX) };
                    let usable = id == 0 || self.doc.layer(id).is_some_and(|l| l.frame.is_none());
                    if !usable {
                        return false;
                    }
                    let d = self.doc.device_mut(ep.device).expect("device of the port");
                    let port = d.ports.iter_mut().find(|p| p.id == ep.port).expect("port");
                    port.layer = if id == d.layer { 0 } else { id };
                    // On a hidden layer it is gone from view, and so from
                    // the selection.
                    self.validate_selection();
                }
                _ => return false,
            }
            return true;
        }
        let ids = self.selected_ids();
        if key == "name_pattern" {
            return self.rename_pattern(&ids, value);
        }
        let (only, key) = match key.split_once(':') {
            Some((kind, k)) => (Some(kind), k),
            None => (None, key),
        };
        let wants = |kind: &str| only.is_none_or(|k| k == kind);
        let mut ok = false;
        let mut restyled = None;
        for id in ids {
            let applied = match self.doc.find_item(id) {
                Some(Item::Device(i)) if wants("device") => set_device_prop(&mut self.doc.devices[i], key, value),
                Some(Item::Note(i)) if wants("note") => set_note_prop(&mut self.doc.notes[i], key, value),
                Some(Item::Point(i)) if wants("point") => set_point_prop(&mut self.doc.points[i], key, value),
                Some(Item::Link(i)) if wants("link") => {
                    let l = &mut self.doc.links[i];
                    let r = set_link_prop(l, key, value);
                    if r == Some(true) && key != "label" && key != "label_at" {
                        // Remembered at nominal width, whatever the level.
                        let mut st = l.style.clone();
                        st.width /= l.scale;
                        restyled = Some(st);
                    }
                    r
                }
                _ => Some(false),
            };
            match applied {
                None => return false,
                Some(a) => ok |= a,
            }
        }
        // New links use the style set last.
        if let Some(style) = restyled {
            self.link_defaults = style;
        }
        ok
    }

    /// Rename the selected devices from `pattern`, numbering them in reading
    /// order (rows top to bottom, then left to right). A run of `#` becomes
    /// the number, zero padded to its length (`sw-##` gives sw-01, sw-02…);
    /// without `#` the number is appended.
    fn rename_pattern(&mut self, ids: &[Id], pattern: &str) -> bool {
        let pattern = pattern.trim();
        if pattern.is_empty() {
            return false;
        }
        let mut devs: Vec<(Id, Pt)> = ids
            .iter()
            .filter_map(|id| self.doc.device(*id))
            .map(|d| (d.id, Pt::new(d.x + d.w / 2.0, d.y + d.h / 2.0)))
            .collect();
        if devs.is_empty() {
            return false;
        }
        devs.sort_by(|a, b| a.1.y.total_cmp(&b.1.y));
        // Boxes whose centers are within half a box of the row's first one
        // share its row.
        let mut rows: Vec<Vec<(Id, Pt)>> = vec![];
        for d in devs {
            match rows.last_mut() {
                Some(row) if d.1.y - row[0].1.y <= 30.0 => row.push(d),
                _ => rows.push(vec![d]),
            }
        }
        let order = rows.into_iter().flat_map(|mut row| {
            row.sort_by(|a, b| a.1.x.total_cmp(&b.1.x));
            row.into_iter().map(|(id, _)| id)
        });
        for (n, id) in order.enumerate() {
            let name = number_name(pattern, n + 1);
            if let Some(d) = self.doc.device_mut(id) {
                d.name = name;
            }
        }
        true
    }

    // Clipboard

    /// What to copy: the selection (a port counts as its device), or the
    /// whole shown diagram when nothing is selected. Lines between copied
    /// items come along; a selected line whose other end is not copied
    /// keeps that end as a loose end at the same spot.
    fn clipboard_doc(&self) -> Diagram {
        let shown = self.doc.visible_only();
        let ids = self.selected_ids();
        if ids.is_empty() {
            return shown;
        }
        let mut d = shown.clone();
        d.devices.retain(|x| ids.contains(&x.id));
        d.notes.retain(|x| ids.contains(&x.id));
        d.points.retain(|x| ids.contains(&x.id));
        let kept = |d: &Diagram, ep: Endpoint| d.endpoint_pos(ep).is_some();
        let keep: Vec<bool> = d.links.iter().map(|l| ids.contains(&l.id) || kept(&d, l.from) && kept(&d, l.to)).collect();
        let mut keep = keep.into_iter();
        d.links.retain(|_| keep.next().unwrap());
        for i in 0..d.links.len() {
            for end in [false, true] {
                let ep = if end { d.links[i].to } else { d.links[i].from };
                if kept(&d, ep) {
                    continue;
                }
                let Some((at, _)) = shown.endpoint_pos(ep) else { continue };
                let q = d.alloc_id();
                d.points.push(Point { id: q, x: at.x, y: at.y, layer: d.links[i].layer, z: 0, scale: d.links[i].scale, label: String::new() });
                if end { d.links[i].to = Endpoint::point(q) } else { d.links[i].from = Endpoint::point(q) }
            }
        }
        d
    }

    /// The selection (or the whole diagram) as draw.io shapes, ready to be
    /// pasted into draw.io. None if there is nothing to copy.
    pub fn copy_drawio(&mut self) -> Option<String> {
        let d = self.clipboard_doc();
        let n = d.devices.len() + d.notes.len() + d.links.len();
        if n == 0 {
            self.status = "Nothing to copy.".into();
            return None;
        }
        let what = if self.selected_ids().is_empty() { "the diagram" } else { "the selection" };
        self.status = format!("Copied {what} ({n} item{}): paste it into draw.io with Ctrl+V.", if n == 1 { "" } else { "s" });
        Some(crate::drawio::clipboard(&d))
    }

    /// The same as [`Editor::copy_drawio`] in stratagram's own format, for
    /// pasting back into stratagram without losing anything.
    pub fn copy_native(&self) -> String {
        self.clipboard_doc().to_json()
    }

    /// The whole sheet in view as text to paste into Stratagram (here, in
    /// another window, in the app or in NetBox): everything on it, its
    /// layers (hidden ones too) and its frame. Pasted, the layers and the
    /// frame come along (see [`Editor::paste`]).
    pub fn copy_sheet(&mut self) -> String {
        let mut v: Value = serde_json::from_str(&self.doc.to_json()).unwrap_or(Value::Null);
        if let Some(o) = v.as_object_mut() {
            o.insert(SHEET_COPY.into(), Value::Bool(true));
        }
        let n = self.doc.devices.len() + self.doc.notes.len() + self.doc.links.len() + self.doc.points.len();
        let frame = if self.doc.frame_layer().is_some() { " and frame" } else { "" };
        self.status = format!("Copied the sheet ({n} item{}) with its layers{frame}: paste it into Stratagram with Ctrl+V.", if n == 1 { "" } else { "s" });
        serde_json::to_string_pretty(&v).unwrap_or_default()
    }

    /// Paste clipboard contents: stratagram's own data (`native`, or as
    /// text) if there is any, else draw.io shapes in `text`, else `text` as
    /// a new note. New items go into the active layer, in front, selected;
    /// one undo step. A sheet copied with its layers and frame
    /// ([`Editor::copy_sheet`]) keeps them. `view` is the visible part of
    /// the canvas, in world coordinates.
    pub fn paste(&mut self, native: &str, text: &str, view: Rect) -> bool {
        if !self.can_add() {
            return false;
        }
        // A copy from another window or app comes as text.
        let ours = |s: &str| s.trim_start().starts_with('{') && Diagram::from_json(s).is_ok();
        let native = if native.trim().is_empty() && ours(text) { text } else { native };
        if !native.trim().is_empty() {
            let whole = serde_json::from_str::<Value>(native).ok().is_some_and(|v| v.get(SHEET_COPY) == Some(&Value::Bool(true)));
            match Diagram::from_json(native) {
                Ok(d) if whole => return self.insert_sheet(d),
                Ok(d) => return self.insert(d, Some(native), view),
                Err(e) => self.status = format!("Cannot paste: {e}"),
            }
            return false;
        }
        if crate::drawio_import::looks_like_drawio(text) {
            return match crate::drawio_import::import(text) {
                Ok(d) => self.insert(d, None, view),
                Err(e) => {
                    self.status = format!("Cannot paste the draw.io shapes: {e}");
                    false
                }
            };
        }
        let text = text.trim();
        if text.is_empty() {
            self.status = "Nothing to paste.".into();
            return false;
        }
        let center = Pt::new(view.x + view.w / 2.0, view.y + view.h / 2.0);
        self.checkpoint();
        let at = self.snap_pt(center, self.level());
        let id = self.doc.add_note(text, at);
        self.doc.note_mut(id).unwrap().scale = self.level();
        self.selection = Selection::Items(vec![id]);
        self.status = "Pasted the text as a note.".into();
        true
    }

    /// Add a whole sheet copied with its layers and frame: each item on the
    /// layer of its layer's name (made, with its settings, when there is
    /// none), and the frame if this sheet has none. Into an empty sheet it
    /// comes as it was (where it was, standing for the same NetBox
    /// objects); else beside what is there. One undo step.
    fn insert_sheet(&mut self, mut d: Diagram) -> bool {
        let empty = self.doc.devices.is_empty() && self.doc.notes.is_empty() && self.doc.points.is_empty() && self.doc.links.is_empty();
        let Some(b) = d.bounds() else {
            self.status = "Nothing to paste.".into();
            return false;
        };
        if !empty {
            d.clear_sources();
        }
        self.checkpoint();
        let off = match self.doc.bounds() {
            Some(t) if !empty => Pt::new(t.x + t.w + 80.0 - b.x, t.y - b.y),
            _ => Pt::default(),
        };
        // Layers: the same name, or a new one like it. An empty sheet takes
        // the copy's layers as they are, in their order.
        let mut layers: HashMap<Id, Id> = HashMap::new();
        if empty {
            let copied_frame = d.frame_layer().is_some();
            self.doc.layers.retain(|l| l.frame.is_some() && !copied_frame);
        }
        let had_frame = self.doc.frame_layer().is_some();
        let mut frame = None;
        // Parents before their sub-layers.
        let copied: Vec<Layer> = d.layer_tree().into_iter().map(|(l, _)| l.clone()).collect();
        for l in &copied {
            if l.frame.is_some() {
                if !had_frame {
                    let mut f = l.clone();
                    f.id = self.doc.alloc_id();
                    frame = Some(f.id);
                    layers.insert(l.id, f.id);
                    self.doc.layers.insert(0, f);
                }
                continue;
            }
            // The same name, under the layer of the same name.
            let parent = layers.get(&l.parent).copied().unwrap_or(0);
            let same = (!empty)
                .then(|| self.doc.layers.iter().find(|x| x.frame.is_none() && x.name == l.name && x.parent == parent).map(|x| x.id))
                .flatten();
            let id = same.unwrap_or_else(|| {
                let mut n = l.clone();
                n.id = self.doc.alloc_id();
                n.parent = parent;
                let id = n.id;
                self.doc.layers.push(n);
                id
            });
            layers.insert(l.id, id);
        }
        let fallback = self.doc.target_layer();
        let layer_of = |old: Id| layers.get(&old).copied().unwrap_or(fallback);
        if empty {
            self.doc.active_layer = layers.get(&d.active_layer).copied().unwrap_or(fallback);
            if self.doc.source.is_empty() {
                self.doc.source = d.source.clone();
            }
        }
        let mut ids: HashMap<Id, Id> = HashMap::new();
        fn new_id(doc: &mut Diagram, ids: &mut HashMap<Id, Id>, old: Id) -> Id {
            let id = doc.alloc_id();
            ids.insert(old, id);
            id
        }
        for mut dv in std::mem::take(&mut d.devices) {
            dv.id = new_id(&mut self.doc, &mut ids, dv.id);
            (dv.x, dv.y, dv.layer) = (dv.x + off.x, dv.y + off.y, layer_of(dv.layer));
            for p in &mut dv.ports {
                p.id = new_id(&mut self.doc, &mut ids, p.id);
                if p.layer != 0 {
                    p.layer = layer_of(p.layer);
                }
            }
            self.doc.devices.push(dv);
        }
        for mut n in std::mem::take(&mut d.notes) {
            n.id = new_id(&mut self.doc, &mut ids, n.id);
            (n.x, n.y, n.layer) = (n.x + off.x, n.y + off.y, layer_of(n.layer));
            self.doc.notes.push(n);
        }
        for mut q in std::mem::take(&mut d.points) {
            q.id = new_id(&mut self.doc, &mut ids, q.id);
            (q.x, q.y, q.layer) = (q.x + off.x, q.y + off.y, layer_of(q.layer));
            self.doc.points.push(q);
        }
        let mut pasted_links = vec![];
        for mut l in std::mem::take(&mut d.links) {
            let end = |ep: Endpoint| -> Option<Endpoint> {
                Some(if ep.is_point() { Endpoint::point(*ids.get(&ep.owner())?) } else { Endpoint { device: *ids.get(&ep.device)?, port: *ids.get(&ep.port)? } })
            };
            let (Some(from), Some(to)) = (end(l.from), end(l.to)) else { continue };
            let old = l.id;
            l.id = new_id(&mut self.doc, &mut ids, l.id);
            (l.from, l.to, l.layer) = (from, to, layer_of(l.layer));
            pasted_links.push((old, l.id));
            self.doc.links.push(l);
        }
        // The cabling from end to end, through the new lines.
        let link_ids: HashMap<Id, Id> = pasted_links.into_iter().collect();
        for t in &d.traces {
            if let Some(links) = t.links.iter().map(|id| link_ids.get(id).copied()).collect::<Option<Vec<Id>>>() {
                self.doc.traces.push(Trace { links });
            }
        }
        let pasted: Vec<Id> = ids.values().copied().filter(|id| self.doc.find_item(*id).is_some()).collect();
        let n = pasted.len();
        if !empty {
            // In front, as one more group of items.
            let mut order: Vec<Id> = self.doc.z_order().into_iter().map(|it| self.doc.item_id(it)).collect();
            order.retain(|id| !pasted.contains(id));
            order.extend(&pasted);
            self.doc.set_z_order(&order);
        }
        self.select_ids(pasted);
        let new_layers = layers.values().filter(|id| Some(**id) != frame).count();
        self.status = format!(
            "Pasted the sheet: {n} item{} on {new_layers} layer{}{}.",
            if n == 1 { "" } else { "s" },
            if new_layers == 1 { "" } else { "s" },
            match (frame.is_some(), had_frame) {
                (true, _) => ", with its frame",
                (false, true) => " (this sheet keeps its own frame)",
                _ => "",
            }
        );
        true
    }

    /// Add everything in `d` with new ids. Our own copies (`native`) land
    /// 20 px down and right of the original, a step further each time the
    /// same copy is pasted again; foreign shapes, or copies that would land
    /// out of view, go to the middle of the view.
    fn insert(&mut self, mut d: Diagram, native: Option<&str>, view: Rect) -> bool {
        // Copies are not what they were made from.
        d.clear_sources();
        // Made to fit the zoom level being worked at: the largest item ends
        // up at that level's scale.
        let top = d.devices.iter().map(|x| x.scale).chain(d.notes.iter().map(|x| x.scale)).chain(d.points.iter().map(|x| x.scale)).fold(0.0, f64::max);
        let level = self.level();
        if top > 0.0 && (top - level).abs() > 1e-9 {
            rescale(&mut d, level / top);
        }
        let Some(b) = d.bounds() else {
            self.status = "Nothing to paste.".into();
            return false;
        };
        let count = match (&self.last_paste, native) {
            (Some((last, n)), Some(now)) if last == now => n + 1,
            _ => 1,
        };
        self.last_paste = native.map(|n| (n.to_string(), count));
        let step = 2.0 * Self::grid(level) * count as f64;
        let mut off = Pt::new(step, step);
        let landed = Rect { x: b.x + off.x, y: b.y + off.y, ..b };
        let in_view = landed.x < view.x + view.w && landed.x + landed.w > view.x && landed.y < view.y + view.h && landed.y + landed.h > view.y;
        if native.is_none() || !in_view {
            off = Pt::new(view.x + view.w / 2.0 - (b.x + b.w / 2.0), view.y + view.h / 2.0 - (b.y + b.h / 2.0));
            if self.snap {
                let g = Self::grid(level);
                off = Pt::new(snap_to(b.x + off.x, g) - b.x, snap_to(b.y + off.y, g) - b.y);
            }
        }

        self.checkpoint();
        let layer = self.doc.target_layer();
        let mut ids: HashMap<Id, Id> = HashMap::new();
        fn new_id(doc: &mut Diagram, ids: &mut HashMap<Id, Id>, old: Id) -> Id {
            let id = doc.alloc_id();
            ids.insert(old, id);
            id
        }
        for mut dv in d.devices.clone() {
            dv.id = new_id(&mut self.doc, &mut ids, dv.id);
            (dv.x, dv.y, dv.layer) = (dv.x + off.x, dv.y + off.y, layer);
            for p in &mut dv.ports {
                p.id = new_id(&mut self.doc, &mut ids, p.id);
            }
            self.doc.devices.push(dv);
        }
        for mut n in d.notes.clone() {
            n.id = new_id(&mut self.doc, &mut ids, n.id);
            (n.x, n.y, n.layer) = (n.x + off.x, n.y + off.y, layer);
            self.doc.notes.push(n);
        }
        for mut q in d.points.clone() {
            q.id = new_id(&mut self.doc, &mut ids, q.id);
            (q.x, q.y, q.layer) = (q.x + off.x, q.y + off.y, layer);
            self.doc.points.push(q);
        }
        let map_end = |ids: &HashMap<Id, Id>, ep: Endpoint| -> Option<Endpoint> {
            Some(if ep.is_point() { Endpoint::point(ids[&ep.owner()]) } else { Endpoint { device: *ids.get(&ep.device)?, port: *ids.get(&ep.port)? } })
        };
        for mut l in d.links.clone() {
            let (Some(from), Some(to)) = (map_end(&ids, l.from), map_end(&ids, l.to)) else { continue };
            l.id = new_id(&mut self.doc, &mut ids, l.id);
            (l.from, l.to, l.layer) = (from, to, layer);
            self.doc.links.push(l);
        }
        // In front of everything, stacked as they were.
        let mut order: Vec<Id> = self.doc.z_order().into_iter().map(|it| self.doc.item_id(it)).collect();
        let pasted: Vec<Id> = d.z_order().into_iter().filter_map(|it| ids.get(&d.item_id(it)).copied()).collect();
        order.retain(|id| !pasted.contains(id));
        order.extend(&pasted);
        self.doc.set_z_order(&order);
        let n = pasted.len();
        self.select_ids(pasted);
        self.status = format!(
            "Pasted {n} item{}{}.",
            if n == 1 { "" } else { "s" },
            if native.is_none() { " from draw.io" } else { "" }
        );
        true
    }

    // Auto layout

    /// Arrange the selected devices (or, with fewer than two selected,
    /// every visible unlocked device) with `kind`, `gap` apart. The result
    /// keeps its top left corner; free points move with the devices they
    /// are linked to; with `tidy_ports`, ports move to the side facing their
    /// links; with `avoid`, lines that would cross a device or text get a
    /// routing that does not (see [`Editor::route_clear`]). One undo step.
    pub fn auto_layout(&mut self, kind: &str, gap: f64, tidy_ports: bool, avoid: bool) -> bool {
        let Some(kind) = crate::layout::Layout::parse(kind) else { return false };
        let selected: Vec<Id> = self.selected_ids().into_iter().filter(|id| self.doc.device(*id).is_some()).collect();
        // Only the outermost devices are arranged; whatever is inside a
        // device moves along with it.
        let parents = self.doc.containers();
        let ids: Vec<Id> = if selected.len() >= 2 {
            selected
        } else {
            self.doc.devices.iter().map(|d| d.id).filter(|id| self.doc.is_editable(*id)).collect()
        };
        let ids: Vec<Id> = ids
            .into_iter()
            .filter(|id| !Diagram::ancestors(&parents, *id).iter().any(|a| self.doc.is_editable(*a)))
            .collect();
        if ids.len() < 2 {
            self.status = "Auto layout needs at least two devices.".into();
            return false;
        }
        let devs: Vec<&Device> = ids.iter().filter_map(|id| self.doc.device(*id)).collect();
        let nodes: Vec<crate::layout::Node> = devs
            .iter()
            .map(|d| crate::layout::Node {
                w: d.w,
                h: d.h,
                pos: Pt::new(d.x, d.y),
                rank: crate::layout::kind_rank(&d.kind),
                kind: d.kind.clone(),
                // Sub-layers lay out with the layer they are under.
                layer: self.doc.layers.iter().position(|l| l.id == self.doc.layer_root(d.layer)).unwrap_or(0),
            })
            .collect();
        let index = |id: Id| ids.iter().position(|x| *x == id);
        let linked: Vec<(&Link, (usize, usize))> = self
            .doc
            .links
            .iter()
            .filter(|l| !l.from.is_point() && !l.to.is_point())
            .filter_map(|l| Some((l, (index(l.from.device)?, index(l.to.device)?))))
            .collect();
        let edges: Vec<(usize, usize)> = linked.iter().map(|(_, e)| *e).collect();
        // Room for each link's label (with its margin) between the devices.
        let rooms: Vec<crate::layout::LabelRoom> = linked.iter().map(|(l, _)| label_room(l)).collect();
        // Spacing at the devices' own level (smaller deeper in).
        let level = devs.iter().map(|d| d.scale).fold(0.0, f64::max).max(1e-9);
        let gap = if gap.is_finite() { gap.clamp(10.0, 1000.0) } else { 60.0 } * level;
        let placed = crate::layout::arrange_with_labels(kind, &nodes, &edges, &rooms, gap);

        // Keep the arrangement where the devices were.
        let old = devs.iter().map(|d| d.rect()).reduce(|a, b| a.union(&b)).unwrap();
        let (mut x0, mut y0) = (f64::MAX, f64::MAX);
        for p in &placed {
            x0 = x0.min(p.x);
            y0 = y0.min(p.y);
        }
        // (Tidy works in place.)
        let shift = if kind == crate::layout::Layout::Tidy { Pt::default() } else { Pt::new(old.x - x0, old.y - y0) };
        let targets: Vec<(Id, Pt)> = ids
            .iter()
            .zip(&placed)
            .zip(&nodes)
            .map(|((id, p), n)| {
                let q = p.add(shift);
                // Tidy snaps centers, so centered devices stay centered.
                if kind == crate::layout::Layout::Tidy {
                    let half = Pt::new(n.w / 2.0, n.h / 2.0);
                    return (*id, self.snap_pt(q.add(half), level).sub(half));
                }
                (*id, self.snap_pt(q, level))
            })
            .collect();

        self.checkpoint();
        // Free points follow the devices they are linked to (on average).
        let mut moves: std::collections::HashMap<Id, Pt> = targets
            .iter()
            .filter_map(|(id, p)| self.doc.item_pos(*id).map(|o| (*id, p.sub(o))))
            .collect();
        // Contents follow their container.
        let mut carried: Vec<(Id, Pt)> = vec![];
        for (id, d) in moves.clone() {
            for c in Diagram::contents(&parents, id) {
                if let Some(o) = self.doc.item_pos(c).filter(|_| self.doc.is_editable(c)) {
                    carried.push((c, o.add(d)));
                    moves.insert(c, d);
                }
            }
        }
        let point_moves: Vec<(Id, Pt)> = self
            .doc
            .points
            .iter()
            .filter(|q| !carried.iter().any(|(id, _)| *id == q.id))
            .filter_map(|q| {
                let ds: Vec<Pt> = self
                    .doc
                    .links
                    .iter()
                    .filter_map(|l| match (l.from.owner() == q.id, l.to.owner() == q.id) {
                        (true, _) => moves.get(&l.to.owner()).copied(),
                        (_, true) => moves.get(&l.from.owner()).copied(),
                        _ => None,
                    })
                    .collect();
                (!ds.is_empty()).then(|| {
                    let d = ds.iter().fold(Pt::default(), |a, b| a.add(*b)).scale(1.0 / ds.len() as f64);
                    (q.id, q.pos().add(d))
                })
            })
            .collect();
        for (id, p) in targets.into_iter().chain(carried).chain(point_moves) {
            self.doc.set_item_pos(id, p);
        }
        if tidy_ports {
            let axis = match kind {
                crate::layout::Layout::Hierarchy => Some(true),
                crate::layout::Layout::HierarchyLr => Some(false),
                _ => None,
            };
            self.tidy_ports(&ids, axis);
        }
        if kind == crate::layout::Layout::Tidy {
            self.even_ports(&ids);
            self.straighten_ports(&ids);
        }
        let rerouted = if avoid { self.route_clear(&ids) } else { 0 };
        let untangled = if avoid { self.untangle(&ids) } else { 0 };
        self.status = format!(
            "Arranged {} devices ({}){}{}. Ctrl+Z undoes it.",
            ids.len(),
            kind.name(),
            match rerouted {
                0 => String::new(),
                n => format!("; {n} line{} rerouted around objects", if n == 1 { "" } else { "s" }),
            },
            match untangled {
                0 => String::new(),
                n => format!("; {n} crossing{} of lines removed", if n == 1 { "" } else { "s" }),
            }
        );
        true
    }

    /// Fewer lines crossing each other among those touching the devices
    /// `ids`: a line may leave its socket the other way (up instead of
    /// down), take another routing, or two devices next to each other in
    /// a row may swap places, whatever leaves the fewest crossings without
    /// running a line through a device. Returns how many crossings fewer.
    fn untangle(&mut self, ids: &[Id]) -> usize {
        // Lines through devices weigh far more than lines crossing lines.
        const THROUGH: usize = 1000;
        let route = |doc: &Diagram, i: usize| -> Option<Vec<Pt>> {
            let l = &doc.links[i];
            if doc.link_visible(l) { geom::link_route(doc, l) } else { None }
        };
        let all_routes = |doc: &Diagram| -> Vec<Option<Vec<Pt>>> { (0..doc.links.len()).map(|i| route(doc, i)).collect() };
        // What the lines `set` cost: their crossings with each other (once)
        // and with all others, and lines through devices.
        let cost = |doc: &Diagram, routes: &[Option<Vec<Pt>>], set: &[usize]| -> usize {
            let mut n = 0;
            for &i in set {
                let Some(ri) = &routes[i] else { continue };
                let li = &doc.links[i];
                if geom::crosses(ri, &geom::link_obstacles(doc, li)) {
                    n += THROUGH;
                }
                for (j, rj) in routes.iter().enumerate() {
                    let Some(rj) = rj else { continue };
                    if j == i || set.contains(&j) && j < i {
                        continue;
                    }
                    let lj = &doc.links[j];
                    // Lines from the same port meet there by design.
                    let shared = [li.from, li.to].iter().any(|e| *e == lj.from || *e == lj.to);
                    if !shared {
                        n += geom::route_crossings(ri, rj);
                    }
                }
            }
            n
        };
        let crossings = |doc: &Diagram, routes: &[Option<Vec<Pt>>]| -> usize {
            let all: Vec<usize> = (0..doc.links.len()).collect();
            cost(doc, routes, &all) % THROUGH
        };
        let mut routes = all_routes(&self.doc);
        let before = crossings(&self.doc, &routes);
        let all: Vec<usize> = (0..self.doc.links.len()).collect();
        if cost(&self.doc, &routes, &all) == 0 {
            return 0; // nothing crosses anything
        }
        let touches = |l: &Link| ids.contains(&l.from.owner()) || ids.contains(&l.to.owner());
        // A port in a socket on a front (not at its back), leaving up or down.
        let flippable = |doc: &Diagram, ep: Endpoint| -> bool {
            !ep.is_point()
                && doc.device(ep.device).is_some_and(|d| {
                    d.port(ep.port).is_some_and(|p| {
                        !p.rear && p.socket > 0 && d.socket_pos(p.socket).is_some() && matches!(p.side, Side::Top | Side::Bottom)
                    })
                })
        };
        let flip = |doc: &mut Diagram, ep: Endpoint| {
            if let Some(p) = doc.device_mut(ep.device).and_then(|d| d.ports.iter_mut().find(|p| p.id == ep.port)) {
                p.side = if p.side == Side::Top { Side::Bottom } else { Side::Top };
            }
        };
        let at_ports = |doc: &Diagram, eps: &[Endpoint]| -> Vec<usize> {
            (0..doc.links.len()).filter(|&j| eps.contains(&doc.links[j].from) || eps.contains(&doc.links[j].to)).collect()
        };
        let parents = self.doc.containers();
        for _pass in 0..4 {
            let mut better = false;
            // Lines: which way they leave their sockets, and their routing.
            for i in 0..self.doc.links.len() {
                let l = self.doc.links[i].clone();
                if !touches(&l) || routes[i].is_none() {
                    continue;
                }
                let ends: Vec<Endpoint> = [l.from, l.to].into_iter().filter(|e| flippable(&self.doc, *e)).collect();
                let own = l.style.routing;
                let mut routings = vec![own];
                let others: &[Routing] = if own == Routing::Orthogonal { &[Routing::Around] } else { &[Routing::Straight, Routing::Orthogonal, Routing::Around] };
                routings.extend(others.iter().filter(|r| **r != own));
                let mut set = at_ports(&self.doc, &ends);
                if !set.contains(&i) {
                    set.push(i);
                }
                let base = cost(&self.doc, &routes, &set);
                let mut best: Option<(usize, usize, Routing)> = None;
                // Its own routing first (flipping its ends), then the others.
                for &r in &routings {
                    for mask in 0..(1usize << ends.len()) {
                        if mask == 0 && r == own {
                            continue;
                        }
                        let flipped: Vec<Endpoint> = ends.iter().enumerate().filter(|(k, _)| mask >> k & 1 == 1).map(|(_, e)| *e).collect();
                        for e in &flipped {
                            flip(&mut self.doc, *e);
                        }
                        self.doc.links[i].style.routing = r;
                        let mut trial = routes.clone();
                        for &j in &set {
                            trial[j] = route(&self.doc, j);
                        }
                        let c = cost(&self.doc, &trial, &set);
                        if c < base && best.is_none_or(|b| c < b.0) {
                            best = Some((c, mask, r));
                        }
                        for e in &flipped {
                            flip(&mut self.doc, *e);
                        }
                        self.doc.links[i].style.routing = own;
                    }
                }
                if let Some((_, mask, r)) = best {
                    for (k, e) in ends.iter().enumerate() {
                        if mask >> k & 1 == 1 {
                            flip(&mut self.doc, *e);
                        }
                    }
                    self.doc.links[i].style.routing = r;
                    routes = all_routes(&self.doc);
                    better = true;
                }
            }
            // Devices next to each other in a row: swap them.
            let mut rows: Vec<(f64, Vec<(f64, Id)>)> = vec![];
            for &id in ids {
                let Some(d) = self.doc.device(id) else { continue };
                if parents.contains_key(&id) || parents.values().any(|p| *p == id) {
                    continue; // inside another or holding others: stays
                }
                let cy = d.y + d.h / 2.0;
                match rows.iter_mut().find(|r| (r.0 - cy).abs() < 1.0) {
                    Some(r) => r.1.push((d.x, id)),
                    None => rows.push((cy, vec![(d.x, id)])),
                }
            }
            for (_, row) in rows.iter_mut() {
                row.sort_by(|a, b| a.0.total_cmp(&b.0));
                for k in 0..row.len().saturating_sub(1) {
                    let (a, b) = (row[k].1, row[k + 1].1);
                    let (ra, rb) = (self.doc.device(a).unwrap().rect(), self.doc.device(b).unwrap().rect());
                    // b takes a's left edge, a ends where b ended.
                    let (nb, na) = (ra.x, rb.x + rb.w - ra.w);
                    if na < nb + rb.w {
                        continue; // they would overlap
                    }
                    let meet = |p: &Rect, q: &Rect| p.x < q.x + q.w && q.x < p.x + p.w && p.y < q.y + q.h && q.y < p.y + p.h;
                    let clear = |r: Rect| self.doc.devices.iter().all(|o| o.id == a || o.id == b || !meet(&o.rect(), &r));
                    if !clear(Rect { x: na, ..ra }) || !clear(Rect { x: nb, ..rb }) {
                        continue;
                    }
                    let set: Vec<usize> = (0..self.doc.links.len()).filter(|&j| {
                        let l = &self.doc.links[j];
                        [a, b].contains(&l.from.owner()) || [a, b].contains(&l.to.owner())
                    }).collect();
                    if set.is_empty() {
                        continue;
                    }
                    let base = cost(&self.doc, &routes, &set);
                    self.doc.device_mut(a).unwrap().x = na;
                    self.doc.device_mut(b).unwrap().x = nb;
                    let mut trial = routes.clone();
                    for &j in &set {
                        trial[j] = route(&self.doc, j);
                    }
                    if cost(&self.doc, &trial, &set) < base {
                        routes = all_routes(&self.doc);
                        row.swap(k, k + 1);
                        row[k].0 = nb;
                        row[k + 1].0 = na;
                        better = true;
                    } else {
                        self.doc.device_mut(a).unwrap().x = ra.x;
                        self.doc.device_mut(b).unwrap().x = rb.x;
                    }
                }
            }
            if !better {
                break;
            }
        }
        before.saturating_sub(crossings(&self.doc, &routes))
    }

    /// Give each line touching `ids` that passes through a device or text
    /// the first routing that keeps it clear: its own, then straight, then
    /// orthogonal, else around objects. Returns how many lines changed.
    fn route_clear(&mut self, ids: &[Id]) -> usize {
        let mut changed = 0;
        for i in 0..self.doc.links.len() {
            let obstacles = geom::link_obstacles(&self.doc, &self.doc.links[i]);
            let l = &self.doc.links[i];
            if !ids.contains(&l.from.owner()) && !ids.contains(&l.to.owner()) || !self.doc.link_visible(l) {
                continue;
            }
            let own = l.style.routing;
            changed += usize::from(geom::route_clear_of(&mut self.doc, i, &obstacles) != own);
        }
        changed
    }

    /// Move each linked port of `ids` to the side of its device that faces
    /// the other ends of its links, spread evenly and in the order of those
    /// ends so lines do not cross. `vertical` prefers top/bottom (Some(true))
    /// or left/right (Some(false)) for links to other rows or columns.
    /// Spread the ports on each side of the devices `ids` evenly, in their
    /// order along the side: one in the middle, two at the thirds, and so
    /// on.
    fn even_ports(&mut self, ids: &[Id]) {
        for &id in ids {
            let Some(d) = self.doc.device_mut(id) else { continue };
            for side in [Side::Top, Side::Right, Side::Bottom, Side::Left] {
                let mut on: Vec<usize> = (0..d.ports.len()).filter(|&i| d.ports[i].side == side).collect();
                on.sort_by(|&a, &b| d.ports[a].t.total_cmp(&d.ports[b].t));
                let n = on.len() as f64;
                for (k, &i) in on.iter().enumerate() {
                    d.ports[i].t = (k as f64 + 1.0) / (n + 1.0);
                }
            }
        }
    }

    /// Make lines between facing devices straight: where a line runs from
    /// one device's bottom to another's top (or right to left) and the two
    /// overlap across it, both ports move to one spot within the overlap,
    /// nearest to where they were. Ports sharing their side with other
    /// linked ports stay put, so lines do not end up on top of each other.
    fn straighten_ports(&mut self, ids: &[Id]) {
        let lone = |doc: &Diagram, ep: Endpoint| {
            let Some(d) = doc.device(ep.device) else { return false };
            let Some(side) = d.port(ep.port).map(|p| p.side) else { return false };
            !d.ports.iter().any(|p| p.id != ep.port && p.side == side && doc.links_at_port(p.id) > 0)
        };
        let pairs: Vec<(Endpoint, Endpoint)> = self
            .doc
            .links
            .iter()
            .filter(|l| !l.from.is_point() && !l.to.is_point())
            .filter(|l| ids.contains(&l.from.device) && ids.contains(&l.to.device))
            .map(|l| (l.from, l.to))
            .collect();
        for (a, b) in pairs {
            let (Some(da), Some(db)) = (self.doc.device(a.device), self.doc.device(b.device)) else { continue };
            let (Some(pa), Some(pb)) = (da.port(a.port), db.port(b.port)) else { continue };
            let vertical = match (pa.side, pb.side) {
                (Side::Bottom, Side::Top) | (Side::Top, Side::Bottom) => true,
                (Side::Right, Side::Left) | (Side::Left, Side::Right) => false,
                _ => continue,
            };
            if !lone(&self.doc, a) || !lone(&self.doc, b) {
                continue;
            }
            // The stretch both sides share, kept off the corners.
            let span = |d: &Device| if vertical { (d.x, d.x + d.w) } else { (d.y, d.y + d.h) };
            let inset = 8.0 * da.scale.max(db.scale);
            let ((a0, a1), (b0, b1)) = (span(da), span(db));
            let (lo, hi) = (a0.max(b0) + inset, a1.min(b1) - inset);
            if lo > hi {
                continue;
            }
            let along = |p: Pt| if vertical { p.x } else { p.y };
            // Already straight (devices lined up, ports in the middle).
            if (along(da.port_pos(pa)) - along(db.port_pos(pb))).abs() < 0.5 {
                continue;
            }
            let at = ((along(da.port_pos(pa)) + along(db.port_pos(pb))) / 2.0).clamp(lo, hi);
            for ep in [a, b] {
                if let Some(d) = self.doc.device_mut(ep.device) {
                    let t = if vertical { (at - d.x) / d.w } else { (at - d.y) / d.h };
                    if let Some(p) = d.ports.iter_mut().find(|p| p.id == ep.port) {
                        p.t = t.clamp(0.0, 1.0);
                    }
                }
            }
        }
    }

    fn tidy_ports(&mut self, ids: &[Id], vertical: Option<bool>) {
        for &id in ids {
            let Some(d) = self.doc.device(id) else { continue };
            let c = Pt::new(d.x + d.w / 2.0, d.y + d.h / 2.0);
            // Per port: its new side and the spot along that side it aims at.
            let mut plan: Vec<(Id, Side, f64)> = vec![];
            for port in &d.ports {
                let ep = Endpoint { device: id, port: port.id };
                let others: Vec<Pt> = self
                    .doc
                    .links
                    .iter()
                    .filter_map(|l| match (l.from == ep, l.to == ep) {
                        (true, _) => Some(l.to),
                        (_, true) => Some(l.from),
                        _ => None,
                    })
                    .filter_map(|o| match self.doc.device(o.owner()) {
                        Some(od) => Some(Pt::new(od.x + od.w / 2.0, od.y + od.h / 2.0)),
                        None => self.doc.endpoint_pos(o).map(|(p, _)| p),
                    })
                    .collect();
                if others.is_empty() {
                    // Unlinked ports stay on their side, at their spot.
                    let along = d.port_pos(port);
                    plan.push((port.id, port.side, if port.side.is_horizontal() { along.y } else { along.x }));
                    continue;
                }
                let aim = others.iter().fold(Pt::default(), |a, b| a.add(*b)).scale(1.0 / others.len() as f64);
                let v = aim.sub(c);
                let side = match vertical {
                    Some(true) if v.y.abs() > d.h / 2.0 => if v.y < 0.0 { Side::Top } else { Side::Bottom },
                    Some(false) if v.x.abs() > d.w / 2.0 => if v.x < 0.0 { Side::Left } else { Side::Right },
                    // Compare against the box's shape, so wide boxes still
                    // use top and bottom for things above and below.
                    _ if v.x.abs() / d.w >= v.y.abs() / d.h => if v.x < 0.0 { Side::Left } else { Side::Right },
                    _ => if v.y < 0.0 { Side::Top } else { Side::Bottom },
                };
                plan.push((port.id, side, if side.is_horizontal() { aim.y } else { aim.x }));
            }
            let Some(d) = self.doc.device_mut(id) else { continue };
            for side in Side::ALL {
                let mut on_side: Vec<&(Id, Side, f64)> = plan.iter().filter(|p| p.1 == side).collect();
                on_side.sort_by(|a, b| a.2.total_cmp(&b.2));
                let n = on_side.len();
                for (k, (pid, _, _)) in on_side.into_iter().enumerate() {
                    if let Some(port) = d.ports.iter_mut().find(|p| p.id == *pid) {
                        port.side = side;
                        port.t = clamp_t((k + 1) as f64 / (n + 1) as f64);
                    }
                }
            }
        }
    }

    // Layers

    /// Add a layer and make it the one new items go into.
    pub fn add_layer(&mut self) -> Id {
        self.checkpoint();
        let name = (self.doc.layers.len() + 1..)
            .map(|n| format!("Layer {n}"))
            .find(|n| !self.doc.layers.iter().any(|l| &l.name == n))
            .unwrap();
        let id = self.doc.add_layer(&name);
        self.doc.active_layer = id;
        id
    }

    /// A sub-layer of the active layer, made the one new items go into.
    pub fn add_sublayer(&mut self) -> Option<Id> {
        let parent = self.doc.active_layer;
        if self.doc.layer(parent).is_none_or(|l| l.frame.is_some()) {
            return None;
        }
        self.checkpoint();
        let name = (self.doc.layers.len() + 1..)
            .map(|n| format!("Layer {n}"))
            .find(|n| !self.doc.layers.iter().any(|l| &l.name == n))
            .unwrap();
        let id = self.doc.add_sublayer(parent, &name);
        if let Some(p) = self.doc.layer_mut(parent) {
            p.expanded = true;
        }
        self.doc.active_layer = id;
        Some(id)
    }

    /// Move a layer (with the layers under it) `before` or `after` another,
    /// or `into` it as its last sub-layer (then shown unfolded). One undo
    /// step.
    pub fn move_layer(&mut self, id: Id, target: Id, place: &str) -> bool {
        let before = self.doc.clone();
        if !self.doc.move_layer(id, target, place) {
            return false;
        }
        if self.doc == before {
            return false;
        }
        self.push_undo(before);
        self.touch();
        self.last_edit = None;
        if place == "into"
            && let Some(t) = self.doc.layer_mut(target)
        {
            t.expanded = true;
        }
        let name = |e: &Editor, id: Id| e.doc.layer(id).map(|l| l.name.clone()).unwrap_or_default();
        self.status = match self.doc.layer(id).map_or(0, |l| l.parent) {
            0 => format!("Moved layer {}.", name(self, id)),
            p => format!("Moved layer {} under {}.", name(self, id), name(self, p)),
        };
        true
    }

    /// Show a layer's sub-layers under it in the list, or fold them away
    /// (how the list looks: no undo step).
    pub fn set_layer_expanded(&mut self, id: Id, expanded: bool) {
        if let Some(l) = self.doc.layer_mut(id) {
            l.expanded = expanded;
        }
    }

    /// Remove a layer with everything on it, its sub-layers too. The last
    /// layer, and locked layers, stay.
    pub fn remove_layer(&mut self, id: Id) -> bool {
        let Some(name) = self.doc.layer(id).map(|l| l.name.clone()) else { return false };
        let under: Vec<Id> = self.doc.layers.iter().filter(|l| self.doc.layer_ancestors(l.id).contains(&id)).map(|l| l.id).collect();
        let item_layers = self.doc.layers.iter().filter(|l| l.frame.is_none() && l.id != id && !under.contains(&l.id)).count();
        if self.doc.layers.len() == 1 || (item_layers == 0 && self.doc.layer(id).is_some_and(|l| l.frame.is_none())) {
            self.status = "The last layer cannot be removed.".into();
            return false;
        }
        if self.doc.layer_locked(id) || under.iter().any(|l| self.doc.layer_locked(*l)) {
            self.status = format!("The layer \"{name}\" (or one under it) is locked. Unlock it to remove it.");
            return false;
        }
        self.checkpoint();
        let n = self.doc.layer_count(id) + under.iter().map(|l| self.doc.layer_count(*l)).sum::<usize>();
        self.doc.remove_layer(id);
        self.validate_selection();
        self.status = match n {
            0 => format!("Removed layer {name}."),
            n => format!("Removed layer {name} and the {n} item{} on it (Ctrl+Z to undo).", if n == 1 { "" } else { "s" }),
        };
        true
    }

    pub fn set_layer_visible(&mut self, id: Id, visible: bool) {
        if self.doc.layer(id).is_none_or(|l| l.visible == visible) {
            return;
        }
        self.checkpoint();
        if let Some(l) = self.doc.layer_mut(id) {
            l.visible = visible;
        }
        self.validate_selection();
    }

    /// Lock or unlock a layer. Locked items stay visible but cannot be
    /// selected, moved, edited or connected to.
    pub fn set_layer_locked(&mut self, id: Id, locked: bool) {
        if self.doc.layer(id).is_none_or(|l| l.locked == locked) {
            return;
        }
        self.checkpoint();
        if let Some(l) = self.doc.layer_mut(id) {
            l.locked = locked;
        }
        self.validate_selection();
    }

    /// Show or hide the layer's name in the corner of its devices.
    pub fn set_layer_show_name(&mut self, id: Id, show: bool) {
        if self.doc.layer(id).is_none_or(|l| l.show_name == show) {
            return;
        }
        self.checkpoint();
        if let Some(l) = self.doc.layer_mut(id) {
            l.show_name = show;
        }
    }

    /// Whether new items may go into the active layer; says why not.
    pub fn can_add(&mut self) -> bool {
        match self.doc.layer(self.doc.active_layer) {
            Some(l) if self.doc.layer_locked(l.id) => {
                self.status =
                    format!("The layer \"{}\" is locked. Unlock it or choose another layer for new items.", l.name);
                false
            }
            _ => true,
        }
    }

    pub fn rename_layer(&mut self, id: Id, name: &str) -> bool {
        let name = name.trim();
        if name.is_empty() || self.doc.layer(id).is_none_or(|l| l.name == name) {
            return false;
        }
        self.checkpoint();
        if let Some(l) = self.doc.layer_mut(id) {
            l.name = name.into();
        }
        true
    }

    /// Choose the layer new items go into. Not an undo step.
    /// Set a property of layer `id`'s background: "enabled" ("true" adds
    /// one with defaults, "false" removes it), "margin", "fill", "opacity"
    /// (percent), "border" (none, solid, dashed, dotted), "border_color",
    /// "border_width", "rounded" ("true"/"false") or "radius".
    pub fn set_layer_background(&mut self, id: Id, key: &str, value: &str) -> bool {
        let tag = format!("background/{id}/{key}");
        let before = self.doc.clone();
        let ok = self.apply_layer_background(id, key, value);
        if ok && self.doc != before {
            if self.last_edit.as_deref() != Some(tag.as_str()) {
                self.push_undo(before);
            }
            self.touch();
            self.last_edit = Some(tag);
        }
        ok
    }

    fn apply_layer_background(&mut self, id: Id, key: &str, value: &str) -> bool {
        // The frame's layer holds nothing to put a background behind.
        let Some(l) = self.doc.layer_mut(id).filter(|l| l.frame.is_none()) else { return false };
        if key == "enabled" {
            l.background = (value == "true").then(|| l.background.clone().unwrap_or_default());
            return true;
        }
        let Some(bg) = l.background.as_mut() else { return false };
        let num = |lo: f64, hi: f64| value.parse::<f64>().ok().filter(|v| v.is_finite()).map(|v| v.clamp(lo, hi));
        match key {
            "margin" => bg.margin = num(0.0, 1000.0).unwrap_or(bg.margin),
            "opacity" => bg.opacity = num(0.0, 100.0).unwrap_or(bg.opacity),
            "border_width" => bg.border_width = num(0.5, 20.0).unwrap_or(bg.border_width),
            "radius" => bg.radius = num(0.0, 200.0).unwrap_or(bg.radius),
            "rounded" => bg.rounded = value == "true",
            "border" => match BorderStyle::parse(value) {
                Some(b) => bg.border = b,
                None => return false,
            },
            "fill" | "border_color" => match geom::normalize_color(value) {
                Some(c) if key == "fill" => bg.fill = c,
                Some(c) => bg.border_color = c,
                None => return false,
            },
            _ => return false,
        }
        true
    }

    pub fn set_active_layer(&mut self, id: Id) {
        if self.doc.layer(id).is_some_and(|l| l.frame.is_none()) {
            self.doc.active_layer = id;
        }
    }

    /// The layers in list order (each followed by its sub-layers, `depth`
    /// deep): `folded` if a layer it is under is not expanded, `shown` and
    /// `held` (locked) as it is with the layers it is under, `path` its
    /// name with theirs.
    pub fn layers_json(&self) -> String {
        let doc = &self.doc;
        let v: Vec<Value> = doc
            .layer_tree()
            .into_iter()
            .map(|(l, depth)| {
                let above = doc.layer_ancestors(l.id);
                json!({"id": l.id, "name": l.name, "visible": l.visible, "locked": l.locked, "show_name": l.show_name,
                       "active": l.id == doc.active_layer, "count": doc.layer_count(l.id),
                       "frame": l.frame.is_some(),
                       "parent": l.parent, "depth": depth, "expanded": l.expanded,
                       "children": doc.layers.iter().filter(|x| x.parent == l.id).count(),
                       "folded": above.iter().any(|a| doc.layer(*a).is_some_and(|p| !p.expanded)),
                       "shown": doc.layer_visible(l.id), "held": doc.layer_locked(l.id),
                       "path": doc.layer_path(l.id),
                       "background": l.background.as_ref().map(|b| json!({
                           "margin": b.margin, "fill": b.fill, "opacity": b.opacity, "border": b.border.as_str(),
                           "border_color": b.border_color, "border_width": b.border_width, "rounded": b.rounded,
                           "radius": b.radius}))})
            })
            .collect();
        Value::Array(v).to_string()
    }

    // Stacking order (global across layers)

    /// Bounding box of a device, note or link, for deciding what overlaps.
    fn item_box(&self, id: Id) -> Option<Rect> {
        if let Some(r) = self.item_rect(id) {
            return Some(r);
        }
        let l = self.doc.link(id)?;
        let pts = geom::link_route(&self.doc, l)?;
        let r = pts.iter().map(|p| Rect { x: p.x, y: p.y, w: 0.0, h: 0.0 }).reduce(|a, b| a.union(&b))?;
        Some(r.inflate(l.style.width / 2.0 + 2.0))
    }

    /// Restack the selection: `front`, `back`, or one step `forward` /
    /// `backward` past the next visible item it overlaps. Stacking is
    /// global, so this works across layers.
    pub fn arrange(&mut self, op: &str) -> bool {
        let sel = self.selected_ids();
        if sel.is_empty() {
            return false;
        }
        let doc = &self.doc;
        let mut order: Vec<Id> = doc.z_order().into_iter().map(|it| doc.item_id(it)).collect();
        let before = order.clone();
        let is_sel = |id: &Id| sel.contains(id);
        match op {
            "front" | "back" => {
                let (mut picked, rest): (Vec<Id>, Vec<Id>) = order.iter().partition(|id| is_sel(id));
                order = if op == "front" {
                    rest.into_iter().chain(picked).collect()
                } else {
                    picked.extend(rest);
                    picked
                };
            }
            "forward" | "backward" => {
                let fwd = op == "forward";
                if !fwd {
                    order.reverse();
                }
                // From the top down, hop each selected item over the next
                // shown, unselected item above it that it overlaps.
                for i in (0..order.len()).rev() {
                    if !is_sel(&order[i]) {
                        continue;
                    }
                    let Some(mine) = self.item_box(order[i]) else { continue };
                    let me = order[i];
                    // A device's own links touch it at the ports; passing them
                    // changes nothing you can see.
                    let attached = |a: Id, b: Id| {
                        doc.link(a).is_some_and(|l| l.from.device == b || l.to.device == b)
                    };
                    let hop = (i + 1..order.len()).find(|&j| {
                        let o = order[j];
                        !is_sel(&o)
                            && doc.is_visible(o)
                            && !attached(o, me)
                            && !attached(me, o)
                            && self.item_box(o).is_some_and(|r| overlaps(&r, &mine))
                    });
                    if let Some(j) = hop {
                        let id = order.remove(i);
                        order.insert(j, id);
                    }
                }
                if !fwd {
                    order.reverse();
                }
            }
            _ => return false,
        }
        if order == before {
            return false;
        }
        self.checkpoint();
        self.doc.set_z_order(&order);
        true
    }

    // Files

    pub fn new_diagram(&mut self) {
        let config_path = self.config_path.take();
        *self = Editor { zoom: self.zoom, snap: self.snap, theme: self.theme, last_dir: self.last_dir.take(), config_path, ..Editor::default() };
        self.auto_frame();
    }

    /// Give a new, empty diagram the frame set up in the configuration file,
    /// if it says `auto=true` under `[frame]`. The diagram stays unchanged
    /// as far as saving and undo are concerned.
    pub fn auto_frame(&mut self) {
        let config = self.config();
        if config.flag("frame", "auto") && self.doc.frame_layer().is_none() {
            let template = config.get("frame", "template").filter(|t| FrameTemplate::parse(t).is_some());
            self.apply_frame("template", template.as_deref().unwrap_or("lines"));
        }
    }

    /// Save the frame's fields that carry over to other diagrams (author,
    /// company, note, color, margin, what to show, level, owner, reviewer,
    /// approver, installer and the logos) as defaults in the configuration
    /// file: under `[frame]` with `all_templates` (with the template, used
    /// by `auto=true`), else under `[frame-<template>]`. Empty fields are
    /// taken out of that section. The logos are copied next to the file.
    pub fn save_frame_defaults(&mut self, all_templates: bool) -> bool {
        let Some(path) = self.config_path.clone() else { return false };
        let Some(f) = self.doc.layers.iter().find_map(|l| l.frame.clone()) else {
            self.status = "There is no frame to take defaults from.".into();
            return false;
        };
        let section = if all_templates { "frame".to_string() } else { format!("frame-{}", f.template.as_str()) };
        let mut values: Vec<(&str, Option<String>)> = vec![];
        for key in ["author", "company", "note", "owner", "reviewer", "approver", "installer", "level"] {
            values.push((key, frame_text(&f, key).filter(|v| !v.is_empty()).map(String::from)));
        }
        values.push(("color", Some(f.color.clone())));
        values.push(("margin", Some(f.margin.to_string())));
        values.push(("show_date", Some(f.show_date.to_string())));
        values.push(("show_file", Some(f.show_file.to_string())));
        if all_templates {
            values.push(("template", Some(f.template.as_str().into())));
        }
        for (key, data) in [("logo", &f.logo), ("logo2", &f.logo2)] {
            if data.is_empty() {
                values.push((key, None));
                continue;
            }
            match save_logo_copy(&path, &format!("{section}-{key}"), data) {
                Ok(file) => values.push((key, Some(file.display().to_string()))),
                Err(e) => {
                    self.status = format!("Defaults not saved: {e}.");
                    return false;
                }
            }
        }
        if let Err(e) = crate::config::set_values(&path, &section, &values) {
            self.status = format!("Defaults not saved: cannot write {}: {e}.", crate::config::shown(&path));
            return false;
        }
        let scope = if all_templates { "all templates".to_string() } else { format!("the {} template", f.template.as_str()) };
        self.status = format!("Saved as defaults for {scope} in {} [{section}].", crate::config::shown(&path));
        true
    }

    fn config(&self) -> crate::config::Config {
        self.config_path.as_deref().map(crate::config::Config::load).unwrap_or_default()
    }

    /// Fill the frame from the defaults in the configuration file: those
    /// under `[frame]`, overridden by those under `[frame-<template>]`.
    /// With `only_empty`, only texts (and the logo) not filled in yet.
    fn fill_frame(&mut self, template: &str, only_empty: bool) {
        let config = self.config();
        let mut values = config.section("frame");
        for (k, v) in config.section(&format!("frame-{template}")) {
            values.retain(|(x, _)| *x != k);
            values.push((k, v));
        }
        for (key, value) in values {
            if matches!(key.as_str(), "template" | "auto") {
                continue;
            }
            let empty = self.doc.layers.iter().find_map(|l| l.frame.as_ref()).and_then(|f| frame_text(f, &key)).map(str::is_empty);
            if only_empty && empty != Some(true) {
                continue;
            }
            let status = std::mem::take(&mut self.status);
            if !self.apply_frame(&key, &value) {
                let why = std::mem::take(&mut self.status);
                self.status = format!("stratagram.conf: cannot use {key}={value}. {why}").trim_end().to_string();
            } else {
                self.status = status;
            }
        }
    }

    pub fn load(&mut self, path: &Path) -> Result<(), String> {
        let text = std::fs::read_to_string(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
        self.load_json(&text, path)?;
        self.last_dir = path.parent().map(Path::to_path_buf);
        Ok(())
    }

    /// Open a diagram given as text, named `path` (just a name where there
    /// are no files, as in the browser).
    pub fn load_json(&mut self, text: &str, path: &Path) -> Result<(), String> {
        let sheets = Diagram::document_from_json(text)?;
        *self = Editor {
            zoom: self.zoom,
            snap: self.snap,
            theme: self.theme,
            config_path: self.config_path.take(),
            last_dir: self.last_dir.take(),
            path: Some(path.to_path_buf()),
            ..Editor::default()
        };
        self.set_sheets(sheets, 0);
        self.status = format!("Opened {}", path.display());
        Ok(())
    }

    /// The diagram was stored somewhere else (in the browser: downloaded or
    /// sent to a server) under `name`: no unsaved changes any more.
    pub fn saved_as(&mut self, name: &str) {
        self.path = Some(PathBuf::from(name));
        self.dirty = false;
        self.status = format!("Saved {name}");
    }

    pub fn save(&mut self, path: &Path) -> Result<(), String> {
        let tmp = path.with_extension("tmp~");
        std::fs::write(&tmp, self.document_json()).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        std::fs::rename(&tmp, path).map_err(|e| format!("cannot write {}: {e}", path.display()))?;
        self.path = Some(path.to_path_buf());
        self.last_dir = path.parent().map(Path::to_path_buf);
        self.dirty = false;
        self.status = format!("Saved {}", path.display());
        Ok(())
    }

    // Sheets

    /// All sheets, the active one as it is now.
    pub fn all_sheets(&self) -> Vec<Diagram> {
        let mut all = self.sheets.as_ref().clone();
        all[self.active_sheet] = self.doc.clone();
        all
    }

    /// The whole document for draw.io: one page per sheet. With one sheet
    /// the page is called `title`.
    pub fn export_drawio(&self, title: &str) -> String {
        let all = self.all_sheets();
        if all.len() == 1 {
            return crate::drawio::export(&all[0], title, &self.file_name());
        }
        let names: Vec<String> = (0..all.len()).map(|i| self.sheet_name(i)).collect();
        let pages: Vec<(&str, &Diagram)> = names.iter().map(String::as_str).zip(all.iter()).collect();
        crate::drawio::export_pages(&pages, &self.file_name())
    }

    /// The whole document as a file.
    pub fn document_json(&self) -> String {
        Diagram::document_to_json(&self.all_sheets())
    }

    /// Take the sheets out (the active one included), for rearranging them.
    fn take_sheets(&mut self) -> Vec<Diagram> {
        let mut all = Arc::unwrap_or_clone(std::mem::take(&mut self.sheets));
        all[self.active_sheet] = std::mem::take(&mut self.doc);
        all
    }

    /// Put `all` back, with sheet `active` (clamped) shown.
    fn set_sheets(&mut self, mut all: Vec<Diagram>, active: usize) {
        if all.is_empty() {
            all.push(Diagram::default());
        }
        let active = active.min(all.len() - 1);
        self.doc = std::mem::take(&mut all[active]);
        self.sheets = Arc::new(all);
        self.active_sheet = active;
    }

    /// Nothing selected, hovered, dragged or being edited (another sheet is
    /// coming up).
    fn view_reset(&mut self) {
        self.selection = Selection::None;
        self.editing = None;
        self.drag = Drag::None;
        self.snapshot = None;
        self.hover = Hit::Empty;
        self.guides.clear();
        self.spacings.clear();
        self.last_edit = None;
        self.last_paste = None;
    }

    pub fn sheet_count(&self) -> usize {
        self.sheets.len()
    }

    pub fn active_sheet(&self) -> usize {
        self.active_sheet
    }

    /// Sheet `i`'s name, "Sheet <n>" if it has none.
    pub fn sheet_name(&self, i: usize) -> String {
        let name = if i == self.active_sheet { &self.doc.name } else { &self.sheets[i].name };
        if name.trim().is_empty() { format!("Sheet {}", i + 1) } else { name.clone() }
    }

    /// The sheets for the tabs: [{"name": ..., "active": ..., "empty": ...}].
    pub fn sheets_json(&self) -> String {
        let tabs: Vec<serde_json::Value> = (0..self.sheet_count())
            .map(|i| {
                let d = if i == self.active_sheet { &self.doc } else { &self.sheets[i] };
                let empty = d.devices.is_empty() && d.notes.is_empty() && d.points.is_empty() && d.links.is_empty();
                serde_json::json!({"name": self.sheet_name(i), "active": i == self.active_sheet, "empty": empty})
            })
            .collect();
        serde_json::Value::Array(tabs).to_string()
    }

    /// Show sheet `i` (not an edit: nothing to undo).
    pub fn switch_sheet(&mut self, i: usize) {
        if i == self.active_sheet || i >= self.sheet_count() {
            return;
        }
        self.view_reset();
        let all = self.take_sheets();
        self.set_sheets(all, i);
        self.status.clear();
    }

    /// Add an empty sheet after the active one and show it. Undoable.
    pub fn add_sheet(&mut self) -> usize {
        self.view_reset();
        self.push_undo(self.doc.clone());
        let mut all = self.take_sheets();
        // Name them all once there are several, so names stay put when
        // sheets are moved or removed.
        for (i, d) in all.iter_mut().enumerate() {
            if d.name.trim().is_empty() {
                d.name = format!("Sheet {}", i + 1);
            }
        }
        let name = (1..).map(|n| format!("Sheet {n}")).find(|n| !all.iter().any(|d| &d.name == n)).unwrap();
        let at = self.active_sheet + 1;
        let mut sheet = Diagram::default();
        sheet.name = name;
        all.insert(at, sheet);
        self.set_sheets(all, at);
        self.auto_frame();
        self.touch();
        self.status = format!("Added {}.", self.doc.name);
        at
    }

    /// Remove sheet `i` and everything on it (not the last one). Undoable.
    pub fn remove_sheet(&mut self, i: usize) -> bool {
        if self.sheet_count() < 2 || i >= self.sheet_count() {
            self.status = "A document keeps at least one sheet.".into();
            return false;
        }
        let name = self.sheet_name(i);
        self.view_reset();
        self.push_undo(self.doc.clone());
        let mut all = self.take_sheets();
        all.remove(i);
        let active = match self.active_sheet {
            a if a > i => a - 1,
            a => a.min(all.len() - 1),
        };
        self.set_sheets(all, active);
        self.touch();
        self.status = format!("Removed {name}. Ctrl+Z brings it back.");
        true
    }

    /// Rename sheet `i`. Undoable.
    pub fn rename_sheet(&mut self, i: usize, name: &str) -> bool {
        let name = name.trim();
        if i >= self.sheet_count() || name.is_empty() || self.sheet_name(i) == name {
            return false;
        }
        self.push_undo(self.doc.clone());
        if i == self.active_sheet {
            self.doc.name = name.into();
        } else {
            Arc::make_mut(&mut self.sheets)[i].name = name.into();
        }
        self.touch();
        self.last_edit = None;
        true
    }

    /// Move sheet `from` to position `to` (in front of what is there now,
    /// or after it when moving right). The sheet in view stays in view.
    /// Undoable.
    pub fn move_sheet(&mut self, from: usize, to: usize) -> bool {
        let n = self.sheet_count();
        if from >= n || to >= n || from == to {
            return false;
        }
        self.view_reset();
        self.push_undo(self.doc.clone());
        let shown = self.active_sheet;
        let mut all = self.take_sheets();
        let sheet = all.remove(from);
        all.insert(to, sheet);
        let active = match shown {
            a if a == from => to,
            a if from < a && a <= to => a - 1,
            a if to <= a && a < from => a + 1,
            a => a,
        };
        self.set_sheets(all, active);
        self.touch();
        true
    }

    // Generated files

    /// Lay out every sheet of a file written by a script (or by Claude):
    /// device types given by any name the app knows ("Firewall", "fw",
    /// "tgw") become its type names, then the auto layout `kind` with
    /// ports facing their lines and lines kept clear of devices. Returns
    /// how many devices were arranged, or what went wrong.
    pub fn layout_document(&mut self, kind: &str, gap: f64) -> Result<usize, String> {
        crate::layout::Layout::parse(kind).ok_or_else(|| format!("unknown layout {kind}"))?;
        let shown = self.active_sheet();
        let mut arranged = 0;
        for i in 0..self.sheet_count() {
            self.switch_sheet(i);
            for d in self.doc.devices.iter_mut() {
                if let Some(k) = known_kind(&d.kind) {
                    d.kind = k.into();
                }
                if d.w == 120.0 && d.h == 60.0 {
                    d.fit_panel();
                }
            }
            // Written without positions, devices lie on top of each other
            // and would count as inside one another: spread those out first.
            let stacked = |d: &Device, all: &[Device]| all.iter().any(|o| o.id != d.id && (o.x - d.x).abs() < 1.0 && (o.y - d.y).abs() < 1.0);
            let spread: Vec<Id> = self.doc.devices.iter().filter(|d| stacked(d, &self.doc.devices)).map(|d| d.id).collect();
            for (k, id) in spread.iter().enumerate() {
                if let Some(d) = self.doc.device_mut(*id) {
                    (d.x, d.y) = ((k % 8) as f64 * 200.0, (k / 8) as f64 * 140.0);
                }
            }
            if self.doc.devices.len() >= 2 && self.auto_layout(kind, gap, true, true) {
                arranged += self.doc.devices.len();
            }
        }
        self.switch_sheet(shown);
        Ok(arranged)
    }

    // NetBox

    /// A new diagram of a NetBox site (see `crate::netbox`): `json` is the
    /// site's devices, interfaces, cables and locations, `options` how to
    /// map them (JSON, "" for the defaults). The site on the first sheet,
    /// a sheet for each location after it; laid out top down; unsaved.
    pub fn import_netbox(&mut self, json: &str, options: &str) -> Result<(), String> {
        let data = crate::netbox::SiteData::parse(json)?;
        let opts = netbox_options(options)?;
        let doc = crate::netbox::site_diagram(&data, &opts);
        if doc.devices.is_empty() {
            return Err("NetBox has no cabled devices there.".into());
        }
        let site = data.site.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        let slug = data.site.get("slug").and_then(|n| n.as_str()).filter(|s| !s.is_empty()).unwrap_or("site").to_string();
        self.new_diagram();
        // Lines drawn by hand run like the cables.
        self.link_defaults.routing = opts.routing;
        // The frame set up as default, around each sheet's devices.
        let frame = self.doc.layers.iter().find(|l| l.frame.is_some()).cloned();
        let mut all = vec![ doc];
        all.extend(crate::netbox::location_diagrams(&data, &opts));
        let sheets = all.len();
            self.set_sheets(all, 0);
        for i in 0..sheets {
            self.switch_sheet(i);
                self.dress_netbox_sheet(frame.clone(), &site);
            }
        self.switch_sheet(0);
        let (devices, links) = (self.doc.devices.len(), self.doc.links.len());
        self.undo.clear();
        self.redo.clear();
        self.path = Some(PathBuf::from(format!("{slug}.stratagram")));
        self.dirty = true;
        let from = if site.is_empty() { String::new() } else { format!(" of {site}") };
        let locations = match sheets - 1 {
            0 => String::new(),
            1 => ", and a sheet for its location".into(),
            n => format!(", and a sheet for each of its {n} locations"),
        };
        self.status = format!("Imported {devices} devices and {links} links{from} from NetBox{locations}.");
        Ok(())
    }

    /// The sheet in view, fresh from NetBox: `frame` (the default one) put
    /// around it, titled with its name (and the `site` below a location's),
    /// laid out top down.
    fn dress_netbox_sheet(&mut self, frame: Option<Layer>, site: &str) {
        if let Some(mut layer) = frame {
            layer.id = self.doc.alloc_id();
            self.doc.layers.insert(0, layer);
            let title = if self.doc.name.is_empty() { site.to_string() } else { self.doc.name.clone() };
            if !title.is_empty() {
                self.apply_frame("title", &title);
            }
            if self.doc.source.starts_with("dcim.location:") && !site.is_empty() {
                self.apply_frame("subtitle", site);
            }
        }
        // All devices are laid out together, by their place in the network
        // (a later layer would go below everything): those from other
        // locations, and those on layers by type.
        let home = self.doc.layers.iter().find(|l| l.frame.is_none()).map_or(0, |l| l.id);
        let layers: Vec<(Id, Id)> = self.doc.devices.iter().map(|d| (d.id, d.layer)).collect();
        for d in self.doc.devices.iter_mut() {
            d.layer = home;
        }
        if !self.doc.devices.is_empty() {
            self.auto_layout("hierarchy", 60.0, true, true);
        }
        for (id, layer) in layers {
            if let Some(d) = self.doc.device_mut(id) {
                d.layer = layer;
            }
        }
    }

    /// Bring the diagram up to date with NetBox (the same input as
    /// `import_netbox`); undoable. Nothing already placed moves. Every
    /// sheet from NetBox is refreshed (the first one counts as the site's
    /// if it stands for nothing else), renamed locations rename their
    /// sheets, and new locations get sheets of their own at the end.
    pub fn refresh_netbox(&mut self, json: &str, options: &str) -> Result<(), String> {
        let data = crate::netbox::SiteData::parse(json)?;
        let opts = netbox_options(options)?;
        let site = crate::netbox::site_diagram(&data, &opts);
        let locations = crate::netbox::location_diagrams(&data, &opts);
        let before = (self.doc.clone(), self.sheets.clone(), self.active_sheet);
        let mut all = self.all_sheets();
        if all[0].source.is_empty() {
            all[0].source = site.source.clone();
        }
        let mut total = crate::netbox::Refreshed::default();
        let mut sheets_renamed = 0;
        for sheet in all.iter_mut() {
            let Some(fresh) = std::iter::once(&site).chain(&locations).find(|f| !f.source.is_empty() && f.source == sheet.source) else {
                continue;
            };
        let r = crate::netbox::refresh(sheet,fresh);
            total.added_devices += r.added_devices;
            total.added_links += r.added_links;
            total.renamed += r.renamed;
            total.gone += r.gone;
            total.panels += r.panels;
            if fresh.source != site.source && !fresh.name.is_empty() && sheet.name != fresh.name {
                sheet.name = fresh.name.clone();
                sheets_renamed += 1;
            }
        }
        let new: Vec<Diagram> = locations.into_iter().filter(|f| !all.iter().any(|s| s.source == f.source)).collect();
        let (added, changed) = (new.len(), total != crate::netbox::Refreshed::default() || sheets_renamed > 0);
        if added == 0 && !changed {
            self.status = total.summary();
            return Ok(());
        }
        let first_new = all.len();
        let frame = all.iter().flat_map(|d| d.layers.iter()).find(|l| l.frame.is_some()).cloned();
        let site_name = data.site.get("name").and_then(|n| n.as_str()).unwrap_or("").to_string();
        all.extend(new);
        let shown = self.active_sheet;
        self.view_reset();
        self.set_sheets(all, shown);
        for i in first_new..self.sheet_count() {
            self.switch_sheet(i);
            self.dress_netbox_sheet(frame.clone(), &site_name);
        }
        self.switch_sheet(shown);
        self.undo.push(Saved { doc: before.0, sheets: before.1, active: before.2 });
        if self.undo.len() > UNDO_LIMIT {
            self.undo.remove(0);
        }
        self.redo.clear();
            self.touch();
            self.selection = Selection::None;
        let mut status = total.summary();
        if sheets_renamed > 0 {
            status = format!("{} {sheets_renamed} location sheet{} renamed.", status, if sheets_renamed == 1 { "" } else { "s" });
        }
        if added > 0 {
            status = format!("{status} {added} new location sheet{} added.", if added == 1 { "" } else { "s" });
        }
        self.status = status;
        Ok(())
    }

    /// Show the sheet standing for `source` (a NetBox location, say);
    /// false if there is none.
    pub fn show_sheet_of(&mut self, source: &str) -> bool {
        let found = (0..self.sheet_count()).find(|&i| {
            let d = if i == self.active_sheet { &self.doc } else { &self.sheets[i] };
            !source.is_empty() && d.source == source
        });
        if let Some(i) = found {
            self.switch_sheet(i);
        }
        found.is_some()
    }

    pub fn title(&self) -> String {
        let name = self
            .path
            .as_ref()
            .and_then(|p| p.file_name())
            .map(|n| n.to_string_lossy().into_owned())
            .unwrap_or_else(|| "Untitled".into());
        format!("{}{} - Stratagram", if self.dirty { "*" } else { "" }, name)
    }

    // Output for the UI

    pub fn scene_json(&self) -> String {
        // What is shown: visible layers, and big enough at this zoom.
        let mut shown = self.doc.visible_only();
        shown.devices.retain(|x| self.legible(x.id));
        shown.notes.retain(|x| self.legible(x.id));
        shown.points.retain(|x| self.legible(x.id));
        shown.links.retain(|x| self.legible(x.id));
        // Fading in as they become big enough.
        let alpha = |scale: f64| self.legibility(scale);
        let doc = &shown;
        let sel_items: &[Id] = match &self.selection {
            Selection::Items(ids) => ids,
            _ => &[],
        };
        let sel_port = match self.selection {
            Selection::Port(ep) => Some(ep),
            _ => None,
        };
        let hover_port = match self.hover {
            Hit::Port(ep) => Some(ep),
            _ => None,
        };
        // The ports at the ends of the selected lines (of the whole line
        // when it is bent through points), to find them at a glance.
        // (As shown: a line standing in for cables through hidden devices
        // lights up the ports it is drawn between.)
        let lit_ports: std::collections::HashSet<Endpoint> = doc
            .links
            .iter()
            .filter(|l| self.selection == Selection::Link(l.id) || sel_items.contains(&l.id))
            .flat_map(|l| {
                let b = geom::bent_line(doc, l);
                [l.from, l.to, b.from, b.to]
            })
            .filter(|ep| !ep.is_point())
            .collect();

        // Index into `links` below, which leaves out links without a route.
        let mut link_slot = std::collections::HashMap::new();
        let insert = self.insert_target();
        // Labels slide along their lines where they would overlap.
        let spots = geom::label_spots(doc);
        // Where each item comes in the stacking order (back to front).
        let z_rank: HashMap<Id, usize> = doc.z_order().into_iter().enumerate().map(|(i, it)| (doc.item_id(it), i)).collect();
        let links: Vec<Value> = doc
            .links
            .iter()
            .filter_map(|l| {
                let route = geom::link_route(doc, l)?;
                link_slot.insert(l.id, link_slot.len());
                // In deep zoom, lines of outer levels do not grow past 1.5x
                // their look (or 3 px): no giant lines and arrows.
                let (mut style, mut ls) = (l.style.clone(), l.scale);
                if self.deep {
                    let cap = (1.5 / (l.scale * self.zoom)).min(1.0);
                    ls *= cap;
                    style.width = (style.width * cap).min(3.0 / self.zoom).max(style.width.min(1.0 / self.zoom));
                }
                // A bent line runs on through its joints: no ends there; its
                // bands go once along the whole of it, with its first piece.
                let open = (!geom::is_joint(doc, l.from), !geom::is_joint(doc, l.to));
                let g = geom::link_geometry_ends(&route, &style, ls, open);
                let bent = geom::bent_line(doc, l);
                let band_route = if bent.pieces.len() > 1 {
                    let rank = |id: Id| z_rank.get(&id).copied().unwrap_or(usize::MAX);
                    if geom::band_piece(&bent, rank) == l.id { geom::bent_route(doc, &bent) } else { None }
                } else {
                    Some(route.clone())
                };
                // Bands, outermost first: the carriers, and the tunnel itself.
                let bands: Vec<Value> = match &band_route {
                    Some(r) => l
                        .style
                        .bands()
                        .into_iter()
                        .map(|(k, extra)| {
                            json!({"pts": flat(&geom::tunnel_band_between(doc, bent.from, bent.to, r, style.width / 2.0 + extra * ls)),
                                   "dash": geom::band_dash(k).map(|(a, b)| [a * ls, b * ls])})
                        })
                        .collect(),
                    None => vec![],
                };
                let link_alpha = [l.scale, doc.scale_of(l.from.owner()), doc.scale_of(l.to.owner())]
                    .into_iter()
                    .map(alpha)
                    .fold(1.0, f64::min);
                Some(json!({
                    "id": l.id,
                    "pts": flat(&g.line),
                    "heads": g.heads.iter().map(head_json).collect::<Vec<_>>(),
                    "color": l.style.color,
                    "width": style.width,
                    "style": l.style.style.as_str(),
                    "sel": self.selection == Selection::Link(l.id) || sel_items.contains(&l.id),
                    "drop": insert == Some(l.id),
                    "editing": self.editing == Some(EditTarget::LinkLabel(l.id)),
                    "label": if l.label_hidden { "" } else { l.label.as_str() },
                    "kind": l.style.kind.as_str(),
                    "tag": if l.label_hidden || !geom::shows_tag(doc, l) { String::new() } else { l.style.full_tag() },
                    "bands": bands,
                    "bandW": l.style.bands().first().map_or(0.0, |b| style.width + 2.0 * b.1 * ls),
                    "beads": flat(&g.beads),
                    "beadSquare": l.style.kind == LinkKind::L2,
                    "beadR": geom::BEAD_RADIUS * ls + style.width / 2.0,
                    "ticks": g.ticks.iter().flat_map(|(a, b)| [a.x, a.y, b.x, b.y]).collect::<Vec<_>>(),
                    "lx": spots.get(&l.id).map_or(g.label_pos.x, |s| s.0.x),
                    "ly": spots.get(&l.id).map_or(g.label_pos.y, |s| s.0.y),
                    "s": ls,
                    "a": link_alpha,
                }))
            })
            .collect();

        let dirs = geom::port_link_dirs(doc);
        let parents = doc.containers();
        let pending_rear = self.pending_rear();
        let devices: Vec<Value> = doc
            .devices
            .iter()
            .map(|d| {
                let ports: Vec<Value> = d
                    .ports
                    .iter()
                    .filter(|p| doc.port_shown(d, p))
                    .map(|p| {
                        let pos = d.port_pos(p);
                        let (lp, align, base) = geom::port_label(d, p, &dirs);
                        let ep = Endpoint { device: d.id, port: p.id };
                        let (sel, hot, lit) = (sel_port == Some(ep), hover_port == Some(ep), lit_ports.contains(&ep));
                        // A socket's port shows its name only when pointed at or selected.
                        let name = if d.name_shown(p) || sel || hot || lit { p.name.as_str() } else { "" };
                        json!({
                            "id": p.id, "x": pos.x, "y": pos.y, "side": p.side.as_str(),
                            "name": name, "lx": lp.x, "ly": lp.y, "align": align, "base": base,
                            "sel": sel, "hot": hot, "lit": lit,
                            "editing": self.editing == Some(EditTarget::PortName(ep)),
                            "linked": doc.links_at_port(p.id) > 0,
                        })
                    })
                    .collect();
                let container = parents.values().any(|c| *c == d.id);
                // In the device's own frame (unscaled size), like the canvas draws it.
                let local = Rect { x: d.x, y: d.y, w: d.w / d.scale, h: d.h / d.scale };
                let mut shape = crate::shape::device_shape(&d.kind, &d.name, &local, container, d.panel.as_ref());
                let mut cords = doc.panel_cords(d);
                // The tie of a socket's back about to be cabled, shown already.
                if let Some((_, n, up)) = pending_rear.filter(|r| r.0 == d.id)
                    && !cords.iter().any(|c| c.0 == n && c.2)
                {
                    cords.push((n, up, true));
                }
                crate::shape::add_cords(&mut shape, d.front().as_ref(), &local, &cords);
                json!({
                    "id": d.id, "x": d.x, "y": d.y, "w": d.w, "h": d.h,
                    "name": d.name, "kind": d.kind, "kindLabel": crate::shape::kind_label(&d.kind),
                    "shape": shape.outline, "detail": shape.detail, "mark": shape.mark, "solid": shape.solid,
                    "ldx": shape.label.0, "ldy": shape.label.1,
                    "tile": shape.tile.map(|t| json!({"x": t.x, "y": t.y, "size": t.size, "bg": t.bg, "fg": t.fg,
                                                      "text": t.text, "font": t.font})),
                    "texts": shape.texts.iter().map(|t| json!({"x": t.x, "y": t.y, "text": t.text, "size": t.size})).collect::<Vec<_>>(),
                    "fill": d.color, "stroke": geom::darken(&d.color, 0.45), "text": geom::text_on(&d.color),
                    "sel": sel_items.contains(&d.id),
                    "editing": self.editing == Some(EditTarget::DeviceName(d.id)),
                    "ports": ports,
                    "badge": doc.layer_badge(d).unwrap_or(""),
                    "container": container,
                    "s": d.scale,
                    "a": alpha(d.scale),
                })
            })
            .collect();

        let notes: Vec<Value> = doc
            .notes
            .iter()
            .map(|n| {
                let r = n.rect();
                json!({
                    "id": n.id, "x": n.x, "y": n.y, "w": r.w, "h": r.h,
                    "lines": n.shown_lines(),
                    "text": n.text, "box": n.boxed(),
                    "size": n.font_size(), "lh": n.font_size() * NOTE_LINE_HEIGHT, "color": n.color, "bold": n.bold,
                    "s": n.scale, "a": alpha(n.scale),
                    "sel": sel_items.contains(&n.id),
                    "editing": self.editing == Some(EditTarget::NoteText(n.id)),
                })
            })
            .collect();

        let points: Vec<Value> = doc
            .points
            .iter()
            .map(|q| {
                // The label's anchor as an offset at the point's own scale.
                let (at, align, base) = geom::point_label_anchor(doc, q);
                json!({"id": q.id, "x": q.x, "y": q.y, "sel": sel_items.contains(&q.id), "s": q.scale, "a": alpha(q.scale),
                       "linked": doc.links.iter().any(|l| l.from.owner() == q.id || l.to.owner() == q.id),
                       "label": q.label, "ldx": (at.x - q.x) / q.scale, "ldy": (at.y - q.y) / q.scale,
                       "align": align, "base": base,
                       "editing": self.editing == Some(EditTarget::PointLabel(q.id))})
            })
            .collect();

        let mut preview = Value::Null;
        let mut ghost = Value::Null;
        let mut target_dev: i64 = -1;
        let mut rubber = Value::Null;
        match &self.drag {
            Drag::NewLink { src, cursor } if self.moved => {
                if let Some((a, mut sa)) = self.source_pos(*src) {
                    let tgt = self.link_target(*src, *cursor);
                    let (b, mut sb) = match tgt.and_then(|t| self.target_pos(t)) {
                        Some((b, sb)) => (b, Some(sb)),
                        None => (*cursor, None),
                    };
                    // Connection points face the other end.
                    if matches!(src, LinkSource::Port(ep) if ep.is_point()) {
                        sa = facing_side(a, b);
                    }
                    if matches!(tgt, Some(Target::NewPoint(_))) || matches!(tgt, Some(Target::Port(ep)) if ep.is_point()) {
                        sb = Some(facing_side(b, a));
                    }
                    if let LinkSource::Socket { device, socket } = *src {
                        sa = self.socket_side(device, socket, b);
                    }
                    if let Some(Target::Socket { device, socket }) = tgt {
                        sb = Some(self.socket_side(device, socket, a));
                    }
                    if let Some(Target::NewPort { device, .. } | Target::Socket { device, .. } | Target::Port(Endpoint { device, .. })) = tgt {
                        target_dev = device as i64;
                    }
                    if let Some(Target::NewPort { .. } | Target::Socket { .. } | Target::NewPoint(_)) = tgt {
                        ghost = json!({"x": b.x, "y": b.y});
                    }
                    let route = match (self.link_defaults.routing, sb) {
                        (Routing::Around, Some(sb)) => geom::route_around_scaled(a, sa, b, sb, &geom::obstacles(&self.doc), self.level()),
                        _ => None,
                    }
                    .unwrap_or_else(|| geom::route_scaled(a, sa, b, sb, self.link_defaults.routing, self.level()));
                    let g = geom::link_geometry(&route, &self.link_defaults, self.level());
                    preview = json!({
                        "pts": flat(&g.line),
                        "heads": g.heads.iter().map(head_json).collect::<Vec<_>>(),
                        "width": self.link_defaults.width * self.level(),
                        "src": {"x": a.x, "y": a.y},
                    });
                }
            }
            // A connection point dragged onto a border shows where it attaches.
            Drag::MoveItems { origins, .. } if self.moved && origins.len() == 1 => {
                if let Some((device, port, side, t, socket)) = self.dragged_point_target(origins[0].0)
                    && let Some(d) = doc.device(device)
                {
                    target_dev = device as i64;
                    let at = match port.and_then(|p| d.port(p)) {
                        Some(p) => d.port_pos(p),
                        None => d.socket_pos(socket).or_else(|| self.edge_pos(device, side, t)).unwrap_or_else(|| d.point_on_side(side, t)),
                    };
                    ghost = json!({"x": at.x, "y": at.y});
                }
            }
            Drag::Rubber { start, cur, .. } if self.moved => {
                let r = Rect::from_points(*start, *cur);
                rubber = json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h});
            }
            Drag::None => { let at = match self.hover { Hit::Edge { device, side, t } => self.edge_pos(device, side, t),
                    Hit::Socket { device, socket } => doc.device(device).and_then(|d| d.socket_pos(socket)),
                    _ => None,
                };
                if let Some(p) = at {
                    ghost = json!({"x": p.x, "y": p.y});
                }
            }
            _ => {}
        }

        // What the line pointed at connects, next to the pointer.
        let tip = match (self.hover, &self.drag, self.editing) {
            (Hit::Link(id) | Hit::Label(id), Drag::None, None) => {
                json!({"x": self.hover_at.x, "y": self.hover_at.y, "lines": self.link_tip(id)})
            }
            (Hit::Through(id), Drag::None, None) => {
                json!({"x": self.hover_at.x, "y": self.hover_at.y, "lines": self.tip_lines(id, true)})
            }
            _ => Value::Null,
        };

        // The socket whose back a line is about to be cabled to, lit up.
        let socket_lit = pending_rear
            .and_then(|(device, n, _)| {
                let d = doc.device(device)?;
                let (sockets, k) = d.sockets();
                let sk = sockets.into_iter().find(|s| s.n == n)?;
                let c = d.socket_pos(n)?;
                let (hw, hh) = crate::shape::socket_half(sk.connector);
                Some(json!({"x": c.x, "y": c.y, "hw": hw * sk.k * k, "hh": hh * sk.k * k}))
            })
            .unwrap_or(Value::Null);

        let handle = self
            .selected_resizable()
            .map(|(_, r, s)| json!({"x": r.x + r.w, "y": r.y + r.h, "s": s}))
            .unwrap_or(Value::Null);

        // The frame, back to front; fitting the view includes it.
        let frame = crate::frame::of(doc, &self.file_name());
        let frame_prims: Vec<Value> = frame.iter().flat_map(|f| &f.prims).map(prim_json).collect();
        let frame_over: Vec<Value> = frame.iter().flat_map(|f| &f.overlay).map(prim_json).collect();
        // Layer backgrounds, back to front.
        let backgrounds: Vec<Value> = doc
            .layer_backgrounds()
            .iter()
            .map(|(_, r, b)| {
                json!({"x": r.x, "y": r.y, "w": r.w, "h": r.h, "fill": b.fill, "opacity": b.opacity / 100.0,
                       "border": b.border.as_str(), "stroke": b.border_color, "width": b.border_width,
                       "radius": if b.rounded { b.radius } else { 0.0 }})
            })
            .collect();
        let bounds = doc
            .bounds()
            .into_iter()
            .chain(doc.layer_backgrounds().into_iter().map(|(_, r, _)| r))
            .chain(geom::label_rects(&doc.visible_only()))
            .chain(frame.as_ref().map(|f| f.outer))
            .reduce(|a, b| a.union(&b))
            .map(|b| json!({"x": b.x, "y": b.y, "w": b.w, "h": b.h}))
            .unwrap_or(Value::Null);

        // Back to front: ["l" | "d" | "n", index into links / devices / notes].
        let order: Vec<Value> = doc
            .z_order()
            .into_iter()
            .filter_map(|it| match it {
                Item::Link(i) => link_slot.get(&doc.links[i].id).map(|k| json!(["l", k])),
                Item::Device(i) => Some(json!(["d", i])),
                Item::Note(i) => Some(json!(["n", i])),
                Item::Point(i) => Some(json!(["p", i])),
            })
            .collect();

        let mut scene = json!({
            "devices": devices,
            "links": links,
            "notes": notes,
            "points": points,
            "order": order,
            "level": self.level(),
            "preview": preview,
            "ghost": ghost,
            "socketLit": socket_lit,
            "tip": tip,
            "target": target_dev,
            "rubber": rubber,
            "handle": handle,
            "bounds": bounds,
            "frame": frame_prims,
            "backgrounds": backgrounds,
            "frameOver": frame_over,
            "guides": self.guides.iter().map(|(a, b)| [a.x, a.y, b.x, b.y]).collect::<Vec<_>>(),
            "spacings": self.spacings.iter().map(|(a, b)| [a.x, a.y, b.x, b.y]).collect::<Vec<_>>(),
        });
        // In the theme's colors, with those around the diagram.
        self.theme.recolor_scene(&mut scene);
        scene["ui"] = self.theme.palette_json();
        scene.to_string()
    }

    pub fn selection_json(&self) -> String {
        let doc = &self.doc;
        let mut v = match &self.selection {
            Selection::None => json!({"type": "none"}),
            Selection::Items(ids) if ids.len() == 1 && doc.point(ids[0]).is_some() => {
                let q = doc.point(ids[0]).unwrap();
                let links = doc.links.iter().filter(|l| l.from.owner() == q.id || l.to.owner() == q.id).count();
                json!({"type": "point", "id": q.id, "x": q.x, "y": q.y, "links": links, "label": q.label})
            }
            Selection::Items(ids) if ids.len() == 1 => match (doc.device(ids[0]), doc.note(ids[0])) {
                (Some(d), _) => json!({
                    "type": "device", "id": d.id, "name": d.name, "kind": d.kind,
                    "color": d.color, "w": d.w / d.scale, "h": d.h / d.scale, "ports": d.ports.len(),
                    // A front's sockets (a switch without: 0 ports, a box).
                    "panel_ports": d.front().map_or(0, |f| f.panel.ports),
                    "connector": d.front().map_or(Connector::Rj45, |f| f.panel.connector).as_str(),
                    "uplinks": d.front().map_or(0, |f| f.panel.uplinks),
                    // Whether it can show its ports (switch, router, firewall, server).
                    "takes_ports": crate::shape::takes_ports(&d.kind),
                }),
                (None, Some(n)) => json!({
                    "type": "note", "id": n.id, "text": n.text, "size": n.size, "color": n.color, "bold": n.bold,
                    "boxed": n.boxed(),
                }),
                _ => json!({"type": "none"}),
            },
            Selection::Items(ids) => {
                // Several items: per kind, each value shared by all of them,
                // or null where they differ.
                let devs: Vec<&Device> = ids.iter().filter_map(|id| doc.device(*id)).collect();
                let notes: Vec<&Note> = ids.iter().filter_map(|id| doc.note(*id)).collect();
                let links: Vec<&Link> = ids.iter().filter_map(|id| doc.link(*id)).collect();
                let points = ids.iter().filter(|id| doc.point(**id).is_some()).count();
                let summary = |n: usize, f: &dyn Fn(usize) -> Value, keys: &[&str]| -> Value {
                    if n == 0 {
                        return Value::Null;
                    }
                    let all: Vec<Value> = (0..n).map(f).collect();
                    let mut out = serde_json::Map::new();
                    for k in keys {
                        let first = &all[0][*k];
                        let same = all.iter().all(|v| &v[*k] == first);
                        out.insert(k.to_string(), if same { first.clone() } else { Value::Null });
                    }
                    Value::Object(out)
                };
                json!({
                    "type": "multi", "count": ids.len(),
                    "devices": devs.len(), "notes": notes.len(), "links": links.len(), "points": points,
                    "dev": summary(devs.len(), &|i| { let d = devs[i];
                        json!({"kind": d.kind, "color": d.color, "w": d.w / d.scale, "h": d.h / d.scale}) },
                        &["kind", "color", "w", "h"]),
                    "note": summary(notes.len(), &|i| { let n = notes[i];
                        json!({"size": n.size, "color": n.color, "bold": n.bold}) },
                        &["size", "color", "bold"]),
                    "link": summary(links.len(), &|i| { let l = links[i];
                        json!({"label": l.label, "color": l.style.color, "width": l.style.width / l.scale,
                               "style": l.style.style.as_str(), "head_start": l.style.head_start.as_str(),
                               "head_end": l.style.head_end.as_str(), "routing": l.style.routing.as_str(),
                               "kind": l.style.kind.as_str(), "label_margin": l.style.label_margin,
                               "label_hidden": l.label_hidden}) },
                        &["label", "color", "width", "style", "head_start", "head_end", "routing", "kind", "label_margin"]),
                })
            }
            Selection::Port(ep) => match (doc.device(ep.device), doc.device(ep.device).and_then(|d| d.port(ep.port))) {
                (Some(d), Some(p)) => json!({
                    "type": "port", "device": d.name, "name": p.name, "side": p.side.as_str(),
                    "pos": (p.t * 100.0).round(),
                    "links": doc.links_at_port(p.id),
                    "port_layer": p.layer, "device_layer": d.layer,
                }),
                _ => json!({"type": "none"}),
            },
            Selection::Link(id) => match doc.link(*id) {
                Some(l) => {
                    let end = |ep: Endpoint| {
                        if let Some(q) = ep.is_point().then(|| doc.point(ep.owner())).flatten() {
                            return format!("point ({:.0}, {:.0})", q.x, q.y);
                        }
                        let d = doc.device(ep.device);
                        let dn = d.map(|d| d.name.clone()).unwrap_or_default();
                        let pn = d.and_then(|d| d.port(ep.port)).map(|p| p.name.clone()).unwrap_or_default();
                        if pn.is_empty() { dn } else { format!("{dn} [{pn}]") }
                    };
                    json!({
                        "type": "link", "id": l.id, "label": l.label,
                        "color": l.style.color, "width": l.style.width / l.scale,
                        "style": l.style.style.as_str(),
                        "head_start": l.style.head_start.as_str(),
                        "head_end": l.style.head_end.as_str(),
                        "routing": l.style.routing.as_str(),
                        "kind": l.style.kind.as_str(),
                        "over": l.style.over.iter().map(|k| k.as_str()).collect::<Vec<_>>(),
                        "tag": l.style.full_tag(),
                        "label_margin": l.style.label_margin,
                        "label_placed": l.label_at.is_some(),
                        "label_hidden": l.label_hidden,
                        "from": end(l.from), "to": end(l.to),
                    })
                }
                None => json!({"type": "none"}),
            },
        };
        // The layer of the selection, or -1 when it spans several.
        let ids = self.selected_ids();
        let mut layers: Vec<Id> = ids.iter().filter_map(|id| doc.layer_of(*id)).collect();
        layers.sort_unstable();
        layers.dedup();
        if let Some(obj) = v.as_object_mut() {
            obj.insert("layer".into(), json!(if layers.len() == 1 { layers[0] as i64 } else { -1 }));
            let frame = doc.layers.iter().find_map(|l| l.frame.as_ref());
            obj.insert(
                "frame".into(),
                frame.map_or(Value::Null, |f| {
                    let revisions: Vec<String> = f
                        .revisions
                        .iter()
                        .map(|r| format!("{} | {} | {} | {}", r.rev, r.date, r.change, r.author))
                        .collect();
                    json!({"template": f.template.as_str(), "title": f.title, "subtitle": f.subtitle,
                           "author": f.author, "note": f.note, "show_date": f.show_date, "margin": f.margin,
                           "color": f.color, "company": f.company, "doc_id": f.doc_id, "revision": f.revision,
                           "approver": f.approver, "status": f.status, "level": f.level, "address": f.address,
                           "page": f.page, "show_file": f.show_file, "revisions": revisions.join("\n"),
                           "version": f.version, "owner": f.owner, "review_date": f.review_date,
                           "reviewer": f.reviewer, "customer": f.customer, "summary": f.summary,
                           "installer": f.installer, "commissioned": f.commissioned, "severity": f.severity,
                           "start": f.start, "end": f.end, "services": f.services, "location": f.location,
                           "watermark": f.watermark, "logo": !f.logo.is_empty(), "logo2": !f.logo2.is_empty()})
                }),
            );
        }
        v.to_string()
    }
}

/// The room a link's label needs along the line, across x and y: its box
/// (the type's tag and the label, as drawn) plus the label margin on both
/// sides. Nothing for a line without label or tag.
fn netbox_options(json: &str) -> Result<crate::netbox::Options, String> {
    if json.trim().is_empty() {
        return Ok(Default::default());
    }
    serde_json::from_str(json).map_err(|e| format!("bad NetBox options: {e}"))
}

fn label_room(l: &Link) -> crate::layout::LabelRoom {
    let Some((w, h)) = geom::label_box(l) else { return Default::default() };
    crate::layout::LabelRoom { w, h, margin: l.style.label_margin * l.scale }
}

/// The box of the device that item `id` belongs to: the one it lies in and
/// was made inside of, a level deeper (deep zoom). Such items stay inside
/// it. Items of the same level that merely lie in a device are free.
fn home_box(doc: &Diagram, id: Id) -> Option<Rect> {
    let c = *doc.containers().get(&id)?;
    let dev = doc.device(c)?;
    (doc.scale_of(id) < dev.scale * 0.99).then(|| dev.rect())
}

/// How far `ids` may move together (least and most, in x and y) so that
/// those belonging to a device (see [`home_box`]) stay inside it. Items
/// whose device moves with them are free.
fn belonging_limits(doc: &Diagram, ids: &[Id]) -> (Pt, Pt) {
    let parents = doc.containers();
    let (mut lo, mut hi) = (Pt::new(f64::NEG_INFINITY, f64::NEG_INFINITY), Pt::new(f64::INFINITY, f64::INFINITY));
    for &id in ids {
        let Some(&c) = parents.get(&id) else { continue };
        if ids.contains(&c) {
            continue;
        }
        let (Some(b), Some(dev)) = (doc.item_box(id), doc.device(c)) else { continue };
        if doc.scale_of(id) >= dev.scale * 0.99 {
            continue;
        }
        let r = dev.rect();
        lo = Pt::new(lo.x.max(r.x - b.x), lo.y.max(r.y - b.y));
        hi = Pt::new(hi.x.min(r.x + r.w - (b.x + b.w)), hi.y.min(r.y + r.h - (b.y + b.h)));
    }
    (lo, hi)
}

/// Write a logo (a data URI) as a file in a `logos` folder next to the
/// configuration file `config`, named `name` with the image's extension.
fn save_logo_copy(config: &Path, name: &str, data: &str) -> Result<PathBuf, String> {
    let (head, b64) = data.split_once(";base64,").ok_or("the logo is not embedded as expected")?;
    let ext = if head.ends_with("jpeg") { "jpg" } else { "png" };
    let bytes = crate::drawio_import::base64_decode(b64).ok_or("the logo is not valid base64")?;
    let dir = config.parent().unwrap_or(Path::new(".")).join("stratagram").join("logos");
    std::fs::create_dir_all(&dir).map_err(|e| format!("cannot create {}: {e}", dir.display()))?;
    let file = dir.join(format!("{name}.{ext}"));
    std::fs::write(&file, bytes).map_err(|e| format!("cannot write {}: {e}", file.display()))?;
    Ok(file)
}

/// A frame's text field (or its logo) by key, for "is it filled in yet".
fn frame_text<'a>(f: &'a Frame, key: &str) -> Option<&'a str> {
    Some(match key {
        "title" => &f.title,
        "subtitle" => &f.subtitle,
        "author" => &f.author,
        "note" => &f.note,
        "company" => &f.company,
        "doc_id" => &f.doc_id,
        "revision" => &f.revision,
        "approver" => &f.approver,
        "status" => &f.status,
        "level" => &f.level,
        "address" => &f.address,
        "page" => &f.page,
        "version" => &f.version,
        "owner" => &f.owner,
        "review_date" => &f.review_date,
        "reviewer" => &f.reviewer,
        "customer" => &f.customer,
        "summary" => &f.summary,
        "installer" => &f.installer,
        "commissioned" => &f.commissioned,
        "severity" => &f.severity,
        "start" => &f.start,
        "end" => &f.end,
        "services" => &f.services,
        "location" => &f.location,
        "watermark" => &f.watermark,
        "logo" => &f.logo,
        "logo2" => &f.logo2,
        _ => return None,
    })
}

fn prim_json(p: &crate::frame::Prim) -> Value {
    use crate::frame::Prim;
    match p {
        Prim::Rect { r, fill, stroke, width, radius, dash } => {
            json!({"t": "rect", "x": r.x, "y": r.y, "w": r.w, "h": r.h, "fill": fill, "stroke": stroke, "width": width,
                   "radius": radius, "dash": dash})
        }
        Prim::Line { a, b, color, width, dash } => {
            json!({"t": "line", "pts": [a.x, a.y, b.x, b.y], "color": color, "width": width, "dash": dash})
        }
        Prim::Image { r, data } => json!({"t": "image", "x": r.x, "y": r.y, "w": r.w, "h": r.h, "data": data}),
        Prim::Watermark { at, text, size, color, angle, opacity } => {
            json!({"t": "watermark", "x": at.x, "y": at.y, "text": text, "size": size, "color": color, "angle": angle,
                   "opacity": opacity})
        }
        Prim::Text { at, text, size, bold, color, align } => {
            json!({"t": "text", "x": at.x, "y": at.y, "text": text, "size": size, "bold": bold, "color": color,
                   "align": align.as_str()})
        }
    }
}

/// Scale everything in `d` by `f` around its top left corner: positions,
/// sizes, line widths, and each item's drawing scale.
fn rescale(d: &mut Diagram, f: f64) {
    let Some(b) = d.bounds() else { return };
    let at = |x: f64, o: f64| o + (x - o) * f;
    for x in &mut d.devices {
        (x.x, x.y, x.w, x.h, x.scale) = (at(x.x, b.x), at(x.y, b.y), x.w * f, x.h * f, x.scale * f);
    }
    for x in &mut d.notes {
        (x.x, x.y, x.scale) = (at(x.x, b.x), at(x.y, b.y), x.scale * f);
    }
    for x in &mut d.points {
        (x.x, x.y, x.scale) = (at(x.x, b.x), at(x.y, b.y), x.scale * f);
    }
    for x in &mut d.links {
        (x.style.width, x.scale) = (x.style.width * f, x.scale * f);
    }
}

/// `pattern` with its first run of `#` replaced by `n`, zero padded to
/// the run's length; without `#`, `n` is appended.
fn number_name(pattern: &str, n: usize) -> String {
    match pattern.find('#') {
        Some(start) => {
            let len = pattern[start..].chars().take_while(|c| *c == '#').count();
            format!("{}{n:0len$}{}", &pattern[..start], &pattern[start + len..])
        }
        None => format!("{pattern}{n}"),
    }
}

/// Property setters for one item: `Some(true)` if set, `Some(false)` if
/// the key does not apply to this kind of item, `None` if the value is bad.
fn set_device_prop(d: &mut Device, key: &str, value: &str) -> Option<bool> {
    match key {
        "name" => d.name = value.into(),
        "kind" => {
            // Recolor only if the device still has its kind's default color.
            let old_default = kind_color(&d.kind);
            if let Some(c) = kind_color(value)
                && (old_default == Some(d.color.as_str()) || old_default.is_none())
            {
                d.color = c.into();
            }
            let was_front = d.front().is_some();
            d.kind = value.into();
            // A patch panel (or a switch with ports) is as wide as its
            // ports; back to the usual size when it stops being one.
            if d.front().is_some() {
                d.fit_panel();
            } else if was_front {
                (d.w, d.h) = (120.0 * d.scale, 60.0 * d.scale);
            }
        }
        // Only where they show: patch panels, switches, routers, firewalls
        // and servers.
        "panel_ports" | "uplinks" | "connector" if !d.is_patch_panel() && !crate::shape::takes_ports(&d.kind) => return None,
        // The sockets of a patch panel or a switch: how many, of what
        // type, and a switch's uplinks. A switch set to 0 ports is a box.
        "panel_ports" => {
            let n = value.trim().parse::<u32>().ok().filter(|n| *n <= PANEL_MAX_PORTS)?;
            if n == 0 {
                if d.is_patch_panel() || d.panel.is_none() {
                    return None;
                }
                d.panel = None;
                (d.w, d.h) = (120.0 * d.scale, 60.0 * d.scale);
            } else {
                d.panel.get_or_insert_default().ports = n;
                d.fit_panel();
            }
        }
        "uplinks" => {
            let n = value.trim().parse::<u32>().ok().filter(|n| *n <= MAX_UPLINKS)?;
            d.panel.get_or_insert_default().uplinks = n;
            d.fit_panel();
        }
        "connector" => {
            d.panel.get_or_insert_default().connector = Connector::parse(value)?;
            d.fit_panel();
        }
        "color" => d.color = geom::normalize_color(value)?,
        // Sizes in the panel are nominal: as they look at the device's level.
        "w" | "h" => {
            let v = value.parse::<f64>().ok().filter(|v| v.is_finite())?;
            if key == "w" { d.w = v.max(MIN_W) * d.scale } else { d.h = v.max(MIN_H) * d.scale }
        }
        _ => return Some(false),
    }
    Some(true)
}

fn set_note_prop(n: &mut Note, key: &str, value: &str) -> Option<bool> {
    match key {
        "text" => n.text = value.into(),
        "color" => n.color = geom::normalize_color(value)?,
        "size" => n.size = value.parse::<f64>().ok().filter(|v| v.is_finite())?.clamp(NOTE_MIN_SIZE, NOTE_MAX_SIZE),
        "bold" => n.bold = value == "true",
        // "fit": no fixed box, the note fits its text again.
        "box" if value == "fit" => (n.w, n.h) = (0.0, 0.0),
        _ => return Some(false),
    }
    Some(true)
}

fn set_point_prop(q: &mut Point, key: &str, value: &str) -> Option<bool> {
    let v = || value.parse::<f64>().ok().filter(|v| v.is_finite());
    match key {
        "x" => q.x = v()?,
        "y" => q.y = v()?,
        "label" => q.label = value.replace('\n', " "),
        _ => return Some(false),
    }
    Some(true)
}

fn set_link_prop(l: &mut Link, key: &str, value: &str) -> Option<bool> {
    let s = &mut l.style;
    match key {
        "label" => l.label = value.into(),
        "label_hidden" => l.label_hidden = value == "true",
        // "auto": placed automatically again; or a spot along the line.
        "label_at" if value == "auto" => l.label_at = None,
        "label_at" => l.label_at = Some(value.parse::<f64>().ok().filter(|v| v.is_finite())?.clamp(0.0, 1.0)),
        "color" => s.color = geom::normalize_color(value)?,
        // Nominal, as for device sizes.
        "width" => s.width = value.parse::<f64>().ok().filter(|w| *w > 0.0 && *w <= 20.0)? * l.scale,
        "style" => s.style = LineStyle::parse(value)?,
        "head_start" => s.head_start = Head::parse(value)?,
        "head_end" => s.head_end = Head::parse(value)?,
        "routing" => s.routing = Routing::parse(value)?,
        "label_margin" => s.label_margin = value.parse::<f64>().ok().filter(|v| v.is_finite())?.clamp(0.0, 400.0),
        // What the line is carried over, innermost first: "ipsec,mpls";
        // empty for nothing.
        "over" => {
            let over: Option<Vec<LinkKind>> =
                value.split(',').map(str::trim).filter(|v| !v.is_empty()).map(LinkKind::parse).collect();
            s.over = over?.into_iter().filter(|k| *k != LinkKind::Plain).collect();
        }
        "kind" => {
            // SD-WAN overlays are dashed by default; leaving SD-WAN makes
            // such a line solid again. Other dash choices are kept.
            let kind = LinkKind::parse(value)?;
            match (s.kind, kind, s.style) {
                (old, LinkKind::Sdwan, LineStyle::Solid) if old != LinkKind::Sdwan => s.style = LineStyle::Dashed,
                (LinkKind::Sdwan, new, LineStyle::Dashed) if new != LinkKind::Sdwan => s.style = LineStyle::Solid,
                _ => {}
            }
            s.kind = kind;
        }
        _ => return Some(false),
    }
    Some(true)
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// Port position on the side of `d` nearest to `p`, snapped to the grid
/// along that side when `snap` is set.
fn port_spot(d: &Device, p: Pt, snap: bool) -> (Side, f64) {
    let (side, t) = d.nearest_side(p);
    if !snap {
        return (side, t);
    }
    let (start, len) = if side.is_horizontal() { (d.y, d.h) } else { (d.x, d.w) };
    (side, clamp_t((snap_to(start + t * len, GRID * d.scale) - start) / len))
}

/// Side of `d` that faces `from`, and a spot on it aligned with `from` so the
/// resulting link is straight where possible. Avoids stacking on an existing port.
fn facing_spot(d: &Device, from: Pt) -> (Side, f64) {
    let c = Pt::new(d.x + d.w / 2.0, d.y + d.h / 2.0);
    let v = Pt::new((from.x - c.x) / d.w, (from.y - c.y) / d.h);
    let side = if v.x.abs() >= v.y.abs() {
        if v.x >= 0.0 { Side::Right } else { Side::Left }
    } else if v.y >= 0.0 {
        Side::Bottom
    } else {
        Side::Top
    };
    let t = clamp_t(if side.is_horizontal() { (from.y - d.y) / d.h } else { (from.x - d.x) / d.w });
    let len = if side.is_horizontal() { d.h } else { d.w };
    let crowded = d.ports.iter().any(|p| p.side == side && ((p.t - t) * len).abs() < 12.0);
    (side, if crowded { d.free_spot(side) } else { t })
}

fn flat(pts: &[Pt]) -> Vec<f64> {
    pts.iter().flat_map(|p| [p.x, p.y]).collect()
}

fn head_json(h: &HeadShape) -> Value {
    match h {
        HeadShape::Filled(pts) => json!({"t": "fill", "pts": flat(pts)}),
        HeadShape::Chevron(pts) => json!({"t": "chev", "pts": flat(pts)}),
        HeadShape::Circle { c, r } => json!({"t": "circle", "x": c.x, "y": c.y, "r": r}),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn quick_add_parses_kinds_and_labels() {
        let items = parse_quick_add(" Firewall, router;; firewall|my-box, Router | test1,\nsrv|web, custom kind|x, Gateway, switch|, |a");
        let got: Vec<(&str, &str, bool)> = items.iter().map(|i| (i.name.as_str(), i.kind.as_str(), i.named)).collect();
        assert_eq!(
            got,
            [
                ("Firewall", "firewall", false),
                ("router", "router", false),
                ("my-box", "firewall", true),
                ("test1", "router", true),
                ("web", "server", true),
                ("x", "custom kind", true),
                ("Gateway", "", false),
                ("switch", "switch", false),
                ("a", "", true),
            ]
        );
        assert_eq!(known_kind("Load-Balancer"), Some("load balancer"));
        assert_eq!(known_kind("GCP"), Some("google cloud"));
        assert_eq!(known_kind("rt"), Some("router"));
        assert_eq!(known_kind("WLC"), Some("wlan controller"));
        assert_eq!(known_kind("Wireless LAN Controller"), Some("wlan controller"));
        assert_eq!(known_kind("sap"), Some("standalone ap"));
        assert_eq!(known_kind("Managed AP"), Some("managed ap"));
        assert_eq!(known_kind("lap"), Some("managed ap"));
        assert_eq!(known_kind("SWG"), Some("cloud proxy"));
        assert_eq!(known_kind("Zscaler"), Some("zscaler"));
        assert_eq!(known_kind("zia"), Some("zscaler"));
        assert_eq!(known_kind("Telekom Cloud"), Some("telekom cloud"));
        assert_eq!(known_kind("STACKIT"), Some("stackit"));
        assert!(parse_quick_add(" , |, ").is_empty());
    }

    #[test]
    fn quick_add_creates_selected_devices_without_overlap() {
        let (mut e, a, b) = two_devices();
        let ids = e.quick_add("Firewall, Router, Switch, firewall|my-box, Router", Pt::new(250.0, 100.0));
        assert_eq!(ids.len(), 5);
        assert_eq!(e.selection, Selection::Items(ids.clone()));
        let devs: Vec<&Device> = ids.iter().map(|id| e.doc.device(*id).unwrap()).collect();
        let names: Vec<&str> = devs.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Firewall", "Router", "Switch", "my-box", "Router 2"]);
        assert_eq!(devs[3].kind, "firewall");
        assert_eq!(devs[3].color, kind_color("firewall").unwrap());
        // One row, below the existing devices, which it would otherwise cover.
        assert!(devs.iter().all(|d| d.y == devs[0].y && d.y >= 130.0 + 60.0));
        for old in [a, b] {
            let r = e.doc.device(old).unwrap().rect();
            assert!(devs.iter().all(|d| d.y >= r.y + r.h));
        }
        // A single undo removes the whole batch.
        e.undo();
        assert_eq!(e.doc.devices.len(), 2);
        assert!(e.quick_add("  ", Pt::new(0.0, 0.0)).is_empty());
        // Seven entries wrap onto a second row; empty space below stays put.
        let ids = e.quick_add("a,b,c,d,e,f,g", Pt::new(300.0, 1000.0));
        let row2 = e.doc.device(ids[6]).unwrap();
        assert_eq!((row2.x, row2.y), (e.doc.device(ids[0]).unwrap().x, e.doc.device(ids[0]).unwrap().y + 100.0));
        assert_eq!(e.doc.device(ids[0]).unwrap().y, snap(1000.0 - 160.0 / 2.0));
    }

    #[test]
    fn tidy_makes_lines_straight() {
        let mut e = Editor::default();
        let a = e.doc.add_device("a", "router", Rect { x: 100.0, y: 0.0, w: 120.0, h: 60.0 }, "#fde68a");
        let b = e.doc.add_device("b", "switch", Rect { x: 112.0, y: 200.0, w: 120.0, h: 60.0 }, "#bfdbfe");
        let (pa, pb) = (e.doc.add_port(a, Side::Bottom, 0.2).unwrap(), e.doc.add_port(b, Side::Top, 0.9).unwrap());
        e.doc.add_link(pa, pb, LinkStyle::default());
        let before = (e.doc.device(a).unwrap().rect(), e.doc.device(b).unwrap().rect());
        assert!(e.auto_layout("tidy", 60.0, false, false));
        let (da, db) = (e.doc.device(a).unwrap(), e.doc.device(b).unwrap());
        // Barely moved, now in one column, and the line runs straight down.
        // (They meet in the middle, snapped to the grid.)
        assert!((da.x - before.0.x).abs() <= 10.0 && (db.x - before.1.x).abs() <= 10.0 && da.y == before.0.y);
        assert_eq!(da.x, db.x);
        let (qa, qb) = (da.port_pos(da.port(pa.port).unwrap()), db.port_pos(db.port(pb.port).unwrap()));
        assert!((qa.x - qb.x).abs() < 1e-9, "{qa:?} {qb:?}");
        // Lined up, the ports sit in the middle of their sides.
        assert_eq!((da.port(pa.port).unwrap().t, db.port(pb.port).unwrap().t), (0.5, 0.5));
        // Several on one side are spread evenly, in their order.
        let extra = e.doc.add_port(b, Side::Top, 0.1).unwrap();
        let third = e.doc.add_port(b, Side::Top, 0.95).unwrap();
        assert!(e.auto_layout("tidy", 60.0, false, false));
        let db = e.doc.device(b).unwrap();
        let ts: Vec<f64> = [extra, pb, third].iter().map(|ep| db.port(ep.port).unwrap().t).collect();
        assert_eq!(ts, vec![0.25, 0.5, 0.75]);
    }

    #[test]
    fn auto_layout_keeps_the_label_margin_free() {
        let gap_with = |margin: f64| {
            let (mut e, a, b) = two_devices();
            e.doc.devices[1].x = 1000.0;
            let (pa, pb) = (e.doc.add_port(a, Side::Right, 0.5).unwrap(), e.doc.add_port(b, Side::Left, 0.5).unwrap());
            let style = LinkStyle { kind: LinkKind::Ipsec, label_margin: margin, ..Default::default() };
            let id = e.doc.add_link(pa, pb, style);
            e.doc.link_mut(id).unwrap().label = "Site A to data center".into();
            // A wide hierarchy (left to right) puts the two side by side.
            assert!(e.auto_layout("hierarchy-lr", 40.0, false, false));
            let (da, db) = (e.doc.device(a).unwrap(), e.doc.device(b).unwrap());
            (db.x - (da.x + da.w)).abs().max((da.x - (db.x + db.w)).abs()).min((db.x - da.x).abs() - da.w)
        };
        let label_w = 5.0 * 6.4 + 6.0 + 21.0 * 6.6 + 10.0;
        let tight = gap_with(0.0);
        let roomy = gap_with(60.0);
        assert!(tight >= label_w - 0.5, "the label fits even with no margin: {tight}");
        assert!((roomy - tight - 120.0).abs() < 0.5, "60 on each side: {tight} {roomy}");
        // Kept with the file; the default is not written.
        let mut l = LinkStyle::default();
        assert!(!serde_json::to_string(&l).unwrap().contains("label_margin"));
        l.label_margin = 60.0;
        assert!(serde_json::to_string(&l).unwrap().contains("\"label_margin\":60"));
    }

    #[test]
    fn hierarchy_puts_each_layer_on_its_own_rows() {
        let mut e = Editor::default();
        let r = |x: f64| Rect { x, y: 100.0, w: 120.0, h: 60.0 };
        let core: Vec<Id> = (0..2).map(|i| e.doc.add_device(&format!("core{i}"), "switch", r(200.0 * i as f64), "#bfdbfe")).collect();
        let l2 = e.add_layer();
        let access: Vec<Id> =
            (0..2).map(|i| e.doc.add_device(&format!("acc{i}"), "switch", r(400.0 + 200.0 * i as f64), "#bfdbfe")).collect();
        assert_eq!(e.doc.layer_of(access[0]), Some(l2));
        for &c in &core {
            for &a in &access {
                let (pc, pa) = (e.doc.add_port(c, Side::Bottom, 0.5).unwrap(), e.doc.add_port(a, Side::Top, 0.5).unwrap());
                e.doc.add_link(pc, pa, LinkStyle::default());
            }
        }
        assert!(e.auto_layout("hierarchy", 60.0, false, false));
        let y = |id: Id| e.doc.device(id).unwrap().y;
        assert_eq!(y(core[0]), y(core[1]));
        assert_eq!(y(access[0]), y(access[1]));
        assert!(y(core[0]) < y(access[0]), "Layer 2 below Layer 1");
    }

    #[test]
    fn new_items_go_into_the_active_layer() {
        let mut e = Editor::default();
        let first = e.doc.active_layer;
        let a = e.add_device_at(Pt::new(100.0, 100.0));
        let l2 = e.add_layer();
        assert_eq!(e.doc.active_layer, l2);
        assert_eq!(e.doc.layer(l2).unwrap().name, "Layer 2");
        let b = e.add_device_at(Pt::new(400.0, 100.0));
        let n = e.add_note_at(Pt::new(0.0, 300.0));
        e.cancel_edit();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!("no link") };
        assert_eq!(e.doc.layer_of(a), Some(first));
        for id in [b, n, link] {
            assert_eq!(e.doc.layer_of(id), Some(l2));
        }
        // Quick add and duplicates follow the active layer too.
        e.set_active_layer(first);
        let q = e.quick_add("router", Pt::new(0.0, 0.0));
        assert_eq!(e.doc.layer_of(q[0]), Some(first));
        e.selection = Selection::Items(vec![b]);
        e.duplicate();
        let Selection::Items(copy) = e.selection.clone() else { panic!() };
        assert_eq!(e.doc.layer_of(copy[0]), Some(first));
        // Undo keeps the layer the user chose.
        e.undo();
        assert_eq!(e.doc.active_layer, first);
        let layers = e.layers_json();
        assert!(layers.contains(r#""name":"Layer 2""#), "{layers}");
    }

    #[test]
    fn hidden_layers_cannot_be_seen_or_touched() {
        let (mut e, a, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let l2 = e.add_layer();
        let c = e.add_device_at(Pt::new(100.0, 300.0));
        e.selection = Selection::Items(vec![a, c]);
        e.set_layer_visible(l2, false);
        assert_eq!(e.selection, Selection::Items(vec![a]));
        assert_eq!(e.hit_test(Pt::new(100.0, 300.0)), Hit::Empty);
        e.select_all();
        let Selection::Items(all) = e.selection.clone() else { panic!() };
        assert!(!all.contains(&c) && all.contains(&a));
        let scene = e.scene_json();
        assert!(!scene.contains(&format!(r#""id":{c},"#)));
        // Hiding a device's layer hides its links (even on a shown layer).
        let first = e.doc.layer_of(a).unwrap();
        e.selection = Selection::Items(vec![a]);
        e.set_prop("layer", &l2.to_string());
        assert_eq!(e.selection, Selection::None, "moved into a hidden layer");
        assert_eq!(e.doc.layer_of(a), Some(l2));
        let link = e.doc.links[0].id;
        assert!(!e.doc.is_visible(link));
        assert!(!e.scene_json().contains(r#""label":"#));
        // Adding to a hidden active layer shows it again.
        e.set_active_layer(l2);
        e.add_device_at(Pt::new(600.0, 600.0));
        assert!(e.doc.layer(l2).unwrap().visible);
        assert!(e.doc.is_visible(link));
        assert_eq!(e.doc.layer_of(link), Some(first));
    }

    #[test]
    fn removing_a_layer_removes_its_items_and_undoes() {
        let (mut e, a, b) = two_devices();
        let first = e.doc.active_layer;
        assert!(!e.remove_layer(first), "the last layer stays");
        let l2 = e.add_layer();
        let c = e.add_device_at(Pt::new(100.0, 300.0));
        // A link on the first layer to a device on the removed one goes too.
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(100.0, 300.0));
        assert_eq!(e.doc.links.len(), 1);
        assert!(e.remove_layer(l2));
        assert!(e.doc.device(c).is_none() && e.doc.links.is_empty());
        assert!(e.doc.device(a).is_some() && e.doc.device(b).is_some());
        assert_eq!(e.doc.active_layer, first);
        assert!(e.status.contains("2 items"), "{}", e.status);
        e.undo();
        assert!(e.doc.device(c).is_some() && e.doc.layer(l2).is_some());
        assert!(e.rename_layer(l2, " Overlay "));
        assert_eq!(e.doc.layer(l2).unwrap().name, "Overlay");
        assert!(!e.rename_layer(l2, "  "));
    }

    #[test]
    fn stacking_is_global_across_layers() {
        let mut e = Editor::default();
        let a = e.add_device_at(Pt::new(100.0, 100.0)); // 40..160 x 70..130
        e.add_layer();
        let b = e.add_device_at(Pt::new(140.0, 120.0)); // 80..200 x 90..150, on top of a
        let far = e.add_device_at(Pt::new(800.0, 800.0));
        let overlap = Pt::new(120.0, 110.0);
        assert_eq!(e.hit_test(overlap), Hit::Device(b));
        // Layer 1 item brought in front of the newer layer's item.
        e.selection = Selection::Items(vec![a]);
        assert!(e.arrange("front"));
        assert_eq!(e.hit_test(overlap), Hit::Device(a));
        // One step back hops over b (overlapping) but ignores `far`.
        assert!(e.arrange("backward"));
        assert_eq!(e.hit_test(overlap), Hit::Device(b));
        let order: Vec<Id> = e.doc.z_order().into_iter().map(|it| e.doc.item_id(it)).collect();
        assert_eq!(order, [a, b, far]);
        assert!(!e.arrange("backward"), "nothing below to pass");
        assert!(e.arrange("forward"));
        assert_eq!(e.hit_test(overlap), Hit::Device(a));
        assert!(e.arrange("back"));
        assert_eq!(e.doc.z_order().into_iter().map(|it| e.doc.item_id(it)).next(), Some(a));
        // The scene lists items back to front.
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let first = &scene["order"][0];
        assert_eq!(first[0], "d");
        assert_eq!(scene["devices"][first[1].as_u64().unwrap() as usize]["id"], a);
        // One undo step per arrange.
        e.undo();
        assert_eq!(e.hit_test(overlap), Hit::Device(a));
    }

    #[test]
    fn one_step_forward_skips_the_items_own_links() {
        let (mut e, a, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let c = e.add_device_at(Pt::new(60.0, 140.0)); // overlaps a's lower left
        let overlap = Pt::new(50.0, 120.0);
        assert_eq!(e.hit_test(overlap), Hit::Device(c));
        e.selection = Selection::Items(vec![a]);
        e.arrange("back"); // now below its own link as well
        assert!(e.arrange("forward"));
        assert_eq!(e.hit_test(overlap), Hit::Device(a), "one press passes c, not the link");
    }

    #[test]
    fn links_can_be_raised_above_devices() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        // A device placed across the link hides it until the link is raised.
        let c = e.add_device_at(Pt::new(280.0, 100.0));
        let on_line = Pt::new(260.0, 100.0);
        assert_eq!(e.hit_test(on_line), Hit::Device(c));
        e.selection = Selection::Link(link);
        assert!(e.arrange("forward"));
        assert_eq!(e.hit_test(on_line), Hit::Link(link));
    }

    #[test]
    fn layers_and_stacking_survive_save_and_load() {
        let mut e = Editor::default();
        let a = e.add_device_at(Pt::new(100.0, 100.0));
        let l2 = e.add_layer();
        e.rename_layer(l2, "WAN");
        let b = e.add_device_at(Pt::new(140.0, 120.0));
        e.set_layer_visible(l2, false);
        e.selection = Selection::Items(vec![a]);
        e.arrange("front");
        let mut f = Editor { doc: Diagram::from_json(&e.doc.to_json()).unwrap(), ..Editor::default() };
        assert_eq!(f.doc.layers, e.doc.layers);
        assert_eq!(f.doc.active_layer, l2);
        assert_eq!(f.doc.layer_of(b), Some(l2));
        assert!(f.doc.device(a).unwrap().z > f.doc.device(b).unwrap().z);
        // New ids skip the layer ids.
        assert!(f.add_layer() > l2);
    }

    #[test]
    fn files_without_layers_get_one_and_keep_their_look() {
        // An old file, from before layers and before the rename.
        let json = r#"{"format":"netdiag","version":1,
            "devices":[{"id":1,"x":0,"y":0,"w":120,"h":60},{"id":2,"x":300,"y":0,"w":120,"h":60,
                        "ports":[{"id":3,"side":"left","t":0.5}]},{"id":4,"x":0,"y":0,"w":120,"h":60,"ports":[{"id":5,"side":"right","t":0.5}]}],
            "links":[{"id":6,"from":{"device":4,"port":5},"to":{"device":2,"port":3}}],
            "notes":[{"id":7,"text":"hi","x":0,"y":0}]}"#;
        let d = Diagram::from_json(json).unwrap();
        assert_eq!(d.layers.len(), 1);
        let layer = d.layers[0].id;
        assert!(layer > 7);
        assert_eq!(d.active_layer, layer);
        assert!([1, 2, 6, 7].iter().all(|id| d.layer_of(*id) == Some(layer)));
        // Links under devices, devices in file order, notes on top.
        let order: Vec<Id> = d.z_order().into_iter().map(|it| d.item_id(it)).collect();
        assert_eq!(order, [6, 1, 2, 4, 7]);
    }

    #[test]
    fn bulk_edit_applies_per_kind_and_reports_mixed_values() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        let n = e.add_note_at(Pt::new(0.0, 300.0));
        e.cancel_edit();
        e.set_prop("kind", "router");
        e.selection = Selection::Items(vec![b]);
        e.set_prop("w", "200");
        e.select_all();
        let Selection::Items(all) = e.selection.clone() else { panic!() };
        assert_eq!(all.len(), 4, "devices, the note and the link");
        let v: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(v["type"], "multi");
        assert_eq!((v["devices"].as_u64(), v["notes"].as_u64(), v["links"].as_u64()), (Some(2), Some(1), Some(1)));
        assert!(v["dev"]["w"].is_null(), "widths differ");
        assert_eq!(v["dev"]["h"], 60.0);
        assert_eq!(v["link"]["routing"], "straight");
        // Prefixed keys only touch their kind; one undo step each.
        let undo_before = e.undo.len();
        assert!(e.set_prop("device:color", "#ff0000"));
        assert!(e.set_prop("link:color", "#00ff00"));
        assert!(e.set_prop("note:size", "20"));
        assert!(e.set_prop("link:head_end", "arrow"));
        assert!(e.set_prop("device:w", "160"));
        assert_eq!(e.undo.len(), undo_before + 5);
        for id in [a, b] {
            let d = e.doc.device(id).unwrap();
            assert_eq!((d.color.as_str(), d.w), ("#ff0000", 160.0));
        }
        assert_eq!(e.doc.note(n).unwrap().color, "#334155");
        assert_eq!(e.doc.note(n).unwrap().size, 20.0);
        let l = e.doc.link(link).unwrap();
        assert_eq!((l.style.color.as_str(), l.style.head_end), ("#00ff00", Head::Arrow));
        assert_eq!(e.link_defaults.head_end, Head::Arrow, "new links follow");
        // A bad value changes nothing.
        assert!(!e.set_prop("device:w", "wide"));
        assert!(!e.set_prop("link:style", "wavy"));
        // Unprefixed keys go to every kind they fit.
        e.set_prop("color", "#0000ff");
        assert_eq!(e.doc.note(n).unwrap().color, "#0000ff");
        assert_eq!(e.doc.device(a).unwrap().color, "#0000ff");
        // Delete takes the link too.
        e.delete_selected();
        assert!(e.doc.devices.is_empty() && e.doc.links.is_empty() && e.doc.notes.is_empty());
    }

    #[test]
    fn links_join_the_selection_with_ctrl_click_and_rubber_band() {
        let (mut e, _, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        e.selection = Selection::Items(vec![b]);
        e.press(Pt::new(250.0, 100.0), false, true);
        e.release(Pt::new(250.0, 100.0));
        assert_eq!(e.selection, Selection::Items(vec![b, link]));
        // Taking the device out leaves a plain link selection.
        e.press(Pt::new(400.0, 110.0), true, false);
        e.release(Pt::new(400.0, 110.0));
        assert_eq!(e.selection, Selection::Link(link));
        drag(&mut e, Pt::new(0.0, 0.0), Pt::new(500.0, 200.0));
        let Selection::Items(ids) = e.selection.clone() else { panic!() };
        assert!(ids.contains(&link) && ids.len() == 3);
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert_eq!(scene["links"][0]["sel"], true);
    }

    #[test]
    fn rename_pattern_numbers_devices_in_reading_order() {
        let mut e = Editor { snap: false, ..Editor::default() };
        // Second row first, and slightly uneven tops in the first row.
        let c = e.add_device_at(Pt::new(100.0, 300.0));
        let b = e.add_device_at(Pt::new(400.0, 95.0));
        let a = e.add_device_at(Pt::new(100.0, 105.0));
        let n = e.add_note_at(Pt::new(0.0, 0.0));
        e.cancel_edit();
        e.selection = Selection::Items(vec![c, b, a, n]);
        assert!(e.set_prop("name_pattern", "sw-##"));
        let names: Vec<&str> = [a, b, c].iter().map(|id| e.doc.device(*id).unwrap().name.as_str()).collect();
        assert_eq!(names, ["sw-01", "sw-02", "sw-03"]);
        assert_eq!(e.doc.note(n).unwrap().text, "Text");
        e.set_prop("name_pattern", "core");
        assert_eq!(e.doc.device(c).unwrap().name, "core3");
        assert_eq!(number_name("r#-x", 12), "r12-x");
        assert!(!e.set_prop("name_pattern", "  "));
    }

    #[test]
    fn locked_layers_are_shown_but_read_only() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let first = e.doc.active_layer;
        let l2 = e.add_layer();
        let c = e.add_device_at(Pt::new(100.0, 300.0));
        e.selection = Selection::Items(vec![a, c]);
        e.set_layer_locked(first, true);
        // Locking drops locked items from the selection; they stay in the scene.
        assert_eq!(e.selection, Selection::Items(vec![c]));
        assert!(e.scene_json().contains(&format!(r#""id":{a},"#)));
        // Clicks, rubber band and select all pass them by.
        assert_eq!(e.hit_test(Pt::new(100.0, 100.0)), Hit::Empty);
        assert_eq!(e.hit_test(Pt::new(250.0, 100.0)), Hit::Empty, "the link too");
        drag(&mut e, Pt::new(0.0, 0.0), Pt::new(500.0, 400.0));
        assert_eq!(e.selection, Selection::Items(vec![c]));
        e.select_all();
        assert_eq!(e.selection, Selection::Items(vec![c]));
        // Dragging on a locked device draws a rubber band instead of moving it.
        drag(&mut e, Pt::new(100.0, 100.0), Pt::new(150.0, 150.0));
        assert_eq!(e.doc.device(a).unwrap().x, 40.0);
        // Lines cannot be dropped onto locked devices.
        let ports = e.doc.device(b).unwrap().ports.len();
        drag(&mut e, Pt::new(160.0, 300.0), Pt::new(400.0, 110.0));
        assert_eq!(e.doc.device(b).unwrap().ports.len(), ports);
        assert!(e.doc.points.is_empty());
        // Still alignment targets: Ctrl+drag c lines up with a locked box.
        e.press(Pt::new(100.0, 300.0), false, true);
        e.drag_to(Pt::new(140.0, 250.0));
        e.drag_to(Pt::new(403.0, 300.0));
        e.release(Pt::new(403.0, 300.0));
        assert_eq!(e.doc.device(c).unwrap().x, e.doc.device(b).unwrap().x);
        // Nothing can be added to a locked active layer, and it cannot be removed.
        e.set_active_layer(first);
        let n = e.doc.devices.len();
        e.double_click(Pt::new(700.0, 700.0));
        assert!(e.quick_add("router", Pt::new(0.0, 0.0)).is_empty());
        e.selection = Selection::Items(vec![c]);
        e.duplicate();
        assert_eq!(e.doc.devices.len(), n);
        assert!(e.status.contains("locked"), "{}", e.status);
        assert!(!e.remove_layer(first));
        // The other layer still works; unlocking makes items selectable again.
        e.set_active_layer(l2);
        assert_eq!(e.quick_add("router", Pt::new(0.0, 0.0)).len(), 1);
        e.set_layer_locked(first, false);
        assert_eq!(e.hit_test(Pt::new(100.0, 100.0)), Hit::Device(a));
        // Locking is saved, and only written when set.
        e.set_layer_locked(l2, true);
        let json = e.doc.to_json();
        assert_eq!(json.matches(r#""locked": true"#).count(), 1);
        assert!(!json.contains(r#""locked": false"#));
        assert!(Diagram::from_json(&json).unwrap().layer_locked(l2));
        e.undo();
        assert!(!e.doc.layer_locked(l2));
        assert!(e.layers_json().contains(r#""locked":false"#));
    }

    #[test]
    fn point_dropped_on_a_border_becomes_a_port() {
        let (mut e, a, b) = two_devices(); // b: 340..460 x 70..130
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(250.0, 300.0));
        let Selection::Link(link) = e.selection else { panic!() };
        let q = e.doc.link(link).unwrap().to.owner();
        let undo_len = e.undo.len();
        // While dragging, b is highlighted with the spot it would attach to.
        e.press(Pt::new(250.0, 300.0), false, false);
        e.drag_to(Pt::new(300.0, 200.0));
        e.drag_to(Pt::new(341.0, 112.0));
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert_eq!(scene["target"], b);
        assert_eq!((scene["ghost"]["x"].as_f64(), scene["ghost"]["y"].as_f64()), (Some(340.0), Some(110.0)));
        e.release(Pt::new(341.0, 112.0));
        let l = e.doc.link(link).unwrap();
        assert!(!l.to.is_point() && l.to.device == b);
        let port = e.doc.device(b).unwrap().port(l.to.port).unwrap();
        assert_eq!((port.side, port.t), (Side::Left, 2.0 / 3.0));
        assert!(e.doc.points.is_empty());
        assert_eq!(e.selection, Selection::Port(l.to));
        assert_eq!(e.undo.len(), undo_len + 1, "one undo step");
        e.undo();
        assert!(e.doc.link(link).unwrap().to.is_point());
        assert_eq!(e.doc.point(q).unwrap().pos(), Pt::new(250.0, 300.0));
        // Dropped on an existing port it joins that port; inside a box it stays free.
        e.selection = Selection::Items(vec![b]);
        e.add_port(Side::Top);
        let top = Pt::new(400.0, 70.0);
        drag(&mut e, Pt::new(250.0, 300.0), Pt::new(400.0, 100.0));
        assert!(e.doc.link(link).unwrap().to.is_point(), "inside the box: no attach");
        drag(&mut e, Pt::new(400.0, 100.0), Pt::new(401.0, 71.0));
        let l = e.doc.link(link).unwrap();
        assert_eq!(e.doc.endpoint_pos(l.to).unwrap().0, top);
        assert_eq!(e.doc.device(b).unwrap().ports.len(), 1, "no extra port");
        let _ = a;
    }

    #[test]
    fn dragging_a_line_bends_it_at_a_new_point() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        let orig = e.doc.link(link).unwrap().clone();
        e.set_prop("head_end", "arrow");
        let before = e.doc.clone();
        drag(&mut e, Pt::new(250.0, 100.0), Pt::new(250.0, 301.0));
        assert_eq!(e.doc.links.len(), 2);
        assert_eq!(e.doc.points.len(), 1);
        let q = e.doc.points[0].clone();
        assert_eq!(q.pos(), Pt::new(250.0, 300.0), "follows the mouse, on the grid");
        assert_eq!(e.selection, Selection::Items(vec![q.id]));
        let first = e.doc.link(link).unwrap();
        assert_eq!((first.from, first.to), (orig.from, Endpoint::point(q.id)));
        assert_eq!(first.style.head_end, Head::None);
        let second = e.doc.links.iter().find(|l| l.id != link).unwrap();
        assert_eq!((second.from, second.to), (Endpoint::point(q.id), orig.to));
        assert_eq!(second.style.head_end, Head::Arrow, "the end head moves to the second half");
        assert_eq!((first.from.owner(), second.to.owner()), (a, b));
        // One undo step brings the straight line back.
        e.undo();
        assert_eq!(e.doc, before);
        e.redo();
        e.selection = Selection::Items(vec![q.id]);
        // Deleting the bend joins the halves into the original link again.
        e.delete_selected();
        let l = e.doc.link(link).unwrap();
        assert_eq!((e.doc.links.len(), e.doc.points.len()), (1, 0));
        assert_eq!((l.from, l.to, l.style.head_end), (orig.from, orig.to, Head::Arrow));
    }

    #[test]
    fn sdwan_lines_are_dashed_by_default() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        let style = |e: &Editor| e.doc.link(link).unwrap().style.style;
        e.set_prop("kind", "sdwan");
        assert_eq!(style(&e), LineStyle::Dashed);
        e.set_prop("kind", "mpls");
        assert_eq!(style(&e), LineStyle::Solid, "leaving SD-WAN undoes its default");
        // A dash chosen by hand is kept either way.
        e.set_prop("style", "dotted");
        e.set_prop("kind", "sdwan");
        e.set_prop("kind", "plain");
        assert_eq!(style(&e), LineStyle::Dotted);
        // MPLS lines get ticks across, evenly spaced and clear of the ends.
        e.set_prop("kind", "mpls");
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let ticks = scene["links"][0]["ticks"].as_array().unwrap().len() / 4;
        assert!((10..=14).contains(&ticks), "{ticks}");
    }

    #[test]
    fn device_dropped_on_a_line_goes_in_between_and_deleting_it_joins_the_line() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        e.set_prop("kind", "ipsec");
        e.set_prop("label", "to dc");
        e.set_prop("head_end", "arrow");
        let orig = e.doc.link(link).unwrap().clone();
        let c = e.add_device_at(Pt::new(250.0, 300.0));
        let before = e.doc.clone();
        drag(&mut e, Pt::new(250.0, 300.0), Pt::new(250.0, 102.0));
        assert_eq!(e.doc.links.len(), 2);
        let into = e.doc.link(link).unwrap();
        let out = e.doc.links.iter().find(|l| l.id != link).unwrap();
        assert_eq!((into.from, into.to.device, out.from.device, out.to), (orig.from, c, c, orig.to));
        let side = |ep: Endpoint| e.doc.device(c).unwrap().port(ep.port).unwrap().side;
        assert_eq!((side(into.to), side(out.from)), (Side::Left, Side::Right), "ports face the ends");
        for l in [into, out] {
            assert_eq!((l.style.kind, l.label.as_str()), (LinkKind::Ipsec, "to dc"), "both halves keep type and label");
        }
        assert_eq!((into.style.head_end, out.style.head_end), (Head::None, Head::Arrow));
        e.undo();
        assert_eq!(e.doc, before, "one undo step");
        e.redo();
        // Deleting it joins the halves into the original line again.
        e.selection = Selection::Items(vec![c]);
        e.delete_selected();
        assert_eq!(e.doc.links.len(), 1);
        let l = &e.doc.links[0];
        assert_eq!((l.from, l.to, l.style.head_end, l.label.as_str()), (orig.from, orig.to, Head::Arrow, "to dc"));
        assert_eq!((l.from.device, l.to.device), (a, b));
    }

    #[test]
    fn deleting_a_device_joins_only_lines_of_the_same_type_and_label() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        let c = e.add_device_at(Pt::new(250.0, 300.0));
        drag(&mut e, Pt::new(250.0, 300.0), Pt::new(250.0, 102.0));
        e.doc.link_mut(link).unwrap().label = "uplink".into();
        e.selection = Selection::Items(vec![c]);
        e.delete_selected();
        assert!(e.doc.links.is_empty(), "different labels: the lines go with the device");
    }

    #[test]
    fn frame_lives_on_its_own_bottom_layer_and_follows_the_content() {
        let (mut e, _, _) = two_devices();
        let layers = e.doc.layers.len();
        assert!(e.set_frame("template", "banner"));
        assert_eq!(e.doc.layers.len(), layers + 1);
        assert!(e.doc.layers[0].frame.is_some(), "first in the list, drawn under everything");
        assert_ne!(e.doc.active_layer, e.doc.layers[0].id, "new items still go where they went");
        // Typing the title is one undo step.
        for t in ["L", "La", "Lab"] {
            e.set_frame("title", t);
        }
        e.undo();
        assert_eq!(e.doc.frame().unwrap().title, "Network diagram");
        e.redo();
        e.set_frame("author", "Shin");
        assert_eq!(e.doc.frame().unwrap().title, "Lab");
        assert!(e.doc.changed.is_some(), "edits stamp the time of the last change");
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let has = |scene: &Value, t: &str| scene["frame"].as_array().unwrap().iter().any(|p| p["text"] == t);
        assert!(has(&scene, "Lab") && has(&scene, "Shin"));
        // The view fits the frame, which surrounds both devices with the margin.
        let b = &scene["bounds"];
        assert!(b["x"].as_f64().unwrap() <= 0.0 && b["x"].as_f64().unwrap() + b["w"].as_f64().unwrap() >= 500.0);
        // Moving a device far away grows the frame.
        let w0 = b["w"].as_f64().unwrap();
        e.doc.devices[1].x += 300.0;
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert!((scene["bounds"]["w"].as_f64().unwrap() - w0 - 300.0).abs() < 1e-6);
        // Exports draw it; draw.io files read back without it.
        assert!(crate::svg::export(&e.doc, "").contains(">Lab</text>"));
        let back = crate::drawio_import::import(&crate::drawio::export(&e.doc, "t", "")).unwrap();
        assert_eq!((back.devices.len(), back.notes.len()), (2, 0));
        // Hiding its layer hides it.
        let id = e.doc.layers[0].id;
        e.set_layer_visible(id, false);
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert!(scene["frame"].as_array().unwrap().is_empty());
        e.set_layer_visible(id, true);
        // "none" removes it with its (empty) layer.
        e.set_frame("template", "none");
        assert_eq!(e.doc.layers.len(), layers);
        e.undo();
        assert_eq!(e.doc.frame().unwrap().title, "Lab");
    }

    #[test]
    fn frame_logo_and_revisions_are_set_from_the_panel() {
        let (mut e, _, _) = two_devices();
        e.set_frame("template", "revision");
        // A 3x2 PNG header is enough to read its size.
        let dir = std::env::temp_dir().join(format!("stratagram-logo-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("logo.png");
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend([0, 0, 0, 3, 0, 0, 0, 2, 8, 6, 0, 0, 0]);
        std::fs::write(&png, &bytes).unwrap();
        assert!(e.set_frame("logo", png.to_str().unwrap()));
        let f = e.doc.frame().unwrap();
        assert!(f.logo.starts_with("data:image/png;base64,") && (f.logo_w, f.logo_h) == (3.0, 2.0));
        let text = dir.join("notes.txt");
        std::fs::write(&text, "hello").unwrap();
        assert!(!e.set_frame("logo", text.to_str().unwrap()));
        assert!(e.status.contains("PNG or JPEG"), "{}", e.status);
        assert!(e.set_frame("logo", ""));
        assert!(e.doc.frame().unwrap().logo.is_empty());
        std::fs::remove_dir_all(&dir).ok();
        e.set_frame("revisions", "A | 2026-09-01 | First issue | Shin\n\nB | 2026-10-01 | Added DMZ");
        let revs = &e.doc.frame().unwrap().revisions;
        assert_eq!(revs.len(), 2);
        assert_eq!((revs[1].rev.as_str(), revs[1].change.as_str(), revs[1].author.as_str()), ("B", "Added DMZ", ""));
        let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(sel["frame"]["revisions"], "A | 2026-09-01 | First issue | Shin\nB | 2026-10-01 | Added DMZ | ");
    }

    #[test]
    fn frame_defaults_come_from_the_configuration_file() {
        let dir = std::env::temp_dir().join(format!("stratagram-conf-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("logo.png");
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend([0, 0, 0, 4, 0, 0, 0, 2, 8, 6, 0, 0, 0]);
        std::fs::write(&png, &bytes).unwrap();
        let conf = dir.join("stratagram.conf");
        std::fs::write(
            &conf,
            format!(
                "[ui]\npanelVisible=true\n\n[frame]\nauto=true\ntemplate=site\nauthor=Shin\ncolor=#0f766e\nlogo={}\n\n\
                 [frame-site]\ndoc_id=BER-01\n\n[frame-change]\nstatus=draft\nauthor=Change Board\n",
                png.display()
            ),
        )
        .unwrap();
        let mut e = Editor { config_path: Some(conf.clone()), ..Editor::default() };
        e.new_diagram();
        // A new diagram starts with the configured frame, unchanged as far
        // as saving and undo go.
        let f = e.doc.frame().unwrap().clone();
        assert_eq!((f.template, f.author.as_str(), f.doc_id.as_str(), f.color.as_str()), (FrameTemplate::Site, "Shin", "BER-01", "#0f766e"));
        assert!(f.logo.starts_with("data:image/png") && f.logo_w == 4.0);
        assert!(!e.dirty && !e.can_undo());
        // Switching templates fills only what is still empty.
        e.set_frame("author", "Kim");
        e.set_frame("template", "change");
        let f = e.doc.frame().unwrap();
        assert_eq!((f.author.as_str(), f.status.as_str(), f.doc_id.as_str()), ("Kim", "draft", "BER-01"));
        // A value that does not fit is reported, the rest still applies.
        std::fs::write(&conf, "[frame]\nmargin=wide\nnote=Internal\n").unwrap();
        e.set_frame("template", "none");
        e.set_frame("template", "lines");
        assert_eq!(e.doc.frame().unwrap().note, "Internal");
        assert!(e.status.contains("stratagram.conf") && e.status.contains("margin=wide"), "{}", e.status);
        // Without auto=true, new diagrams start without a frame.
        e.new_diagram();
        assert!(e.doc.frame().is_none());
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn frame_fields_save_as_defaults_and_come_back() {
        let dir = std::env::temp_dir().join(format!("stratagram-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let png = dir.join("logo.png");
        let mut bytes = b"\x89PNG\r\n\x1a\n\0\0\0\rIHDR".to_vec();
        bytes.extend([0, 0, 0, 5, 0, 0, 0, 2, 8, 6, 0, 0, 0]);
        std::fs::write(&png, &bytes).unwrap();
        let conf = dir.join("stratagram.conf");
        std::fs::write(&conf, "[ui]\nshowGrid=true\n\n[frame]\nnote=old note\n").unwrap();
        let mut e = Editor { config_path: Some(conf.clone()), ..Editor::default() };
        assert!(!e.save_frame_defaults(true), "nothing to save without a frame");
        e.set_frame("template", "site");
        assert_eq!(e.doc.frame().unwrap().note, "old note", "the existing default applies");
        for (k, v) in [("author", "Shin"), ("company", "ACME, Inc."), ("title", "Berlin"), ("doc_id", "BER-01"), ("color", "#0f766e"), ("note", "")] {
            e.set_frame(k, v);
        }
        e.set_frame("logo", png.to_str().unwrap());
        assert!(e.save_frame_defaults(true), "{}", e.status);
        let c = crate::config::Config::load(&conf);
        assert_eq!(c.get("frame", "author").as_deref(), Some("Shin"));
        assert_eq!(c.get("frame", "company").as_deref(), Some("ACME, Inc."));
        assert_eq!(c.get("frame", "template").as_deref(), Some("site"));
        assert_eq!(c.get("frame", "note"), None, "an empty field is taken out");
        assert_eq!(c.get("frame", "title"), None, "the title belongs to one diagram");
        assert_eq!(c.get("frame", "doc_id"), None);
        assert_eq!(c.get("ui", "showGrid").as_deref(), Some("true"), "the rest stays");
        let logo = std::path::PathBuf::from(c.get("frame", "logo").unwrap());
        assert_eq!(std::fs::read(&logo).unwrap(), bytes, "a copy of the logo");
        // Only for this template.
        e.set_frame("author", "Kim");
        assert!(e.save_frame_defaults(false));
        assert_eq!(crate::config::Config::load(&conf).get("frame-site", "author").as_deref(), Some("Kim"));
        // A new frame in a new diagram takes them, logo included.
        e.new_diagram();
        e.set_frame("template", "lines");
        let f = e.doc.frame().unwrap();
        assert_eq!((f.author.as_str(), f.company.as_str(), f.color.as_str()), ("Shin", "ACME, Inc.", "#0f766e"));
        assert!(f.logo.starts_with("data:image/png") && f.logo_w == 5.0);
        e.set_frame("template", "none");
        e.set_frame("template", "site");
        assert_eq!(e.doc.frame().unwrap().author, "Kim");
        std::fs::remove_dir_all(&dir).ok();
    }

    #[test]
    fn notes_keep_the_box_they_are_given() {
        let mut e = Editor::default();
        let n = e.add_note_at(Pt::new(100.0, 100.0));
        let before_undo = e.undo.len();
        // Typed in an editor resized to 120 x 40 (at the note's scale).
        e.commit_edit_sized("one two three four five six seven eight nine ten eleven twelve", 120.0, 40.0);
        let note = e.doc.note(n).unwrap().clone();
        assert!(note.boxed() && (note.w, note.h) == (120.0, 40.0));
        assert_eq!(note.rect(), Rect { x: 100.0, y: 100.0, w: 120.0, h: 40.0 });
        assert_eq!(e.undo.len(), before_undo + 1, "size and text are one undo step");
        // Wrapped to the width, cut at the height with "…".
        let lines = note.shown_lines();
        assert_eq!(lines.len(), 2, "{lines:?}");
        assert!(lines[1].ends_with('…'));
        let glyph = note.font_size() * 0.58;
        assert!(lines.iter().all(|l| l.chars().count() as f64 * glyph <= 120.0 + 0.01), "{lines:?}");
        // The editor opens at the box; the scene and SVG cut the text to it.
        e.begin_edit(EditTarget::NoteText(n));
        let b: Value = serde_json::from_str(&e.edit_box_json()).unwrap();
        assert_eq!((b["bw"].as_f64(), b["bh"].as_f64()), (Some(120.0), Some(40.0)));
        e.cancel_edit();
        assert!(crate::svg::export(&e.doc, "").contains(&format!("clip-path=\"url(#clip-n{n})\"")));
        // The corner handle resizes it on the canvas.
        e.selection = Selection::Items(vec![n]);
        drag(&mut e, Pt::new(220.0, 140.0), Pt::new(300.0, 180.0));
        let note = e.doc.note(n).unwrap();
        assert_eq!((note.w, note.h), (200.0, 80.0));
        // "Fit to text" lets it size itself again.
        e.set_prop("box", "fit");
        assert!(!e.doc.note(n).unwrap().boxed());
    }

    #[test]
    fn items_made_inside_a_device_stay_inside_it() {
        let mut e = Editor::default();
        let site = e.doc.add_device("site", "", Rect { x: 0.0, y: 0.0, w: 400.0, h: 300.0 }, "#dbeafe");
        // A note one level in (deep zoom), and a same-level device dropped in.
        let n = e.doc.add_note("inside", Pt::new(100.0, 100.0));
        e.doc.note_mut(n).unwrap().scale = 0.125;
        let free = e.doc.add_device("free", "", Rect { x: 200.0, y: 150.0, w: 120.0, h: 60.0 }, "#dbeafe");
        assert_eq!(e.doc.containers().get(&n), Some(&site));
        let inside = |e: &Editor, id: Id| {
            let (b, r) = (e.doc.item_box(id).unwrap(), e.doc.device(site).unwrap().rect());
            b.x >= r.x - 1e-9 && b.y >= r.y - 1e-9 && b.x + b.w <= r.x + r.w + 1e-9 && b.y + b.h <= r.y + r.h + 1e-9
        };
        // Zoomed in far enough to see and grab the note.
        (e.deep, e.zoom) = (true, 8.0);
        // Dragged far out: stops at the border.
        e.selection = Selection::Items(vec![n]);
        let at = e.doc.item_box(n).unwrap();
        drag(&mut e, Pt::new(at.x + 1.0, at.y + 1.0), Pt::new(900.0, 101.0));
        assert!(inside(&e, n), "{:?}", e.doc.item_box(n));
        let b = e.doc.item_box(n).unwrap();
        assert!((b.x + b.w - 400.0).abs() < 1.0, "right at the border: {b:?}");
        // Nudged up past the top: stops there too.
        for _ in 0..30 {
            e.nudge(0.0, -10.0);
        }
        assert!(inside(&e, n) && e.doc.item_box(n).unwrap().y.abs() < 1e-9);
        // Its box cannot grow past the border either.
        let r = e.doc.item_box(n).unwrap();
        drag(&mut e, Pt::new(r.x + r.w, r.y + r.h), Pt::new(r.x + r.w, r.y + 900.0));
        assert!(inside(&e, n), "{:?}", e.doc.item_box(n));
        // A same-level device can still be taken out.
        (e.deep, e.zoom) = (false, 1.0);
        e.selection = Selection::Items(vec![free]);
        drag(&mut e, Pt::new(260.0, 180.0), Pt::new(760.0, 180.0));
        assert!(e.doc.device(free).unwrap().x > 400.0);
        // Moving the device itself carries what is inside along.
        e.selection = Selection::Items(vec![site]);
        let before = e.doc.item_box(n).unwrap();
        drag(&mut e, Pt::new(20.0, 280.0), Pt::new(520.0, 280.0));
        assert_eq!(e.doc.item_box(n).unwrap().x, before.x + 500.0);
    }

    #[test]
    fn layer_backgrounds_group_a_layers_items() {
        let (mut e, a, _) = two_devices(); // 40..160 and 340..460 x 70..130
        let layer = e.doc.active_layer;
        let empty = e.add_layer();
        assert!(e.set_layer_background(layer, "enabled", "true"));
        assert!(e.set_layer_background(empty, "enabled", "true"));
        for v in ["4", "40"] {
            e.set_layer_background(layer, "margin", v);
        }
        e.set_layer_background(layer, "opacity", "25");
        e.set_layer_background(layer, "border", "dashed");
        e.set_layer_background(layer, "rounded", "false");
        let bgs = e.doc.layer_backgrounds();
        assert_eq!(bgs.len(), 1, "an empty layer gets none");
        assert_eq!(bgs[0].1, Rect { x: 0.0, y: 30.0, w: 500.0, h: 140.0 });
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let bg = &scene["backgrounds"][0];
        assert_eq!((bg["opacity"].as_f64(), bg["border"].as_str(), bg["radius"].as_f64()), (Some(0.25), Some("dashed"), Some(0.0)));
        // It follows the devices.
        e.doc.device_mut(a).unwrap().y -= 100.0;
        assert_eq!(e.doc.layer_backgrounds()[0].1.y, -70.0);
        // Exports draw it; draw.io files read back without it.
        assert!(crate::svg::export(&e.doc, "").contains(r#"fill-opacity="0.25""#));
        let back = crate::drawio_import::import(&crate::drawio::export(&e.doc, "t", "")).unwrap();
        assert_eq!(back.devices.len(), 2);
        // Hidden layer, no background; the frame's layer takes none.
        e.set_layer_visible(layer, false);
        assert!(e.doc.layer_backgrounds().is_empty());
        e.set_layer_visible(layer, true);
        e.set_frame("template", "lines");
        assert!(!e.set_layer_background(e.doc.frame_layer().unwrap(), "enabled", "true"));
        // Typing the margin was one undo step; "false" removes it.
        e.set_layer_background(layer, "enabled", "false");
        assert!(e.doc.layer_backgrounds().is_empty());
        let saved = Diagram::from_json(&e.doc.to_json()).unwrap();
        assert!(saved.layer(empty).unwrap().background.is_some(), "kept with the file");
    }

    #[test]
    fn line_labels_slide_along_their_line() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        e.set_prop("label", "uplink");
        let spot = |e: &Editor| geom::label_spots(&e.doc)[&link].0;
        let mid = spot(&e);
        assert_eq!(e.hit_test(mid), Hit::Label(link));
        // Dragged right and far off the line: it stays on the line.
        e.selection = Selection::None;
        let undo = e.undo.len();
        drag(&mut e, mid, Pt::new(mid.x + 60.0, mid.y + 200.0));
        let now = spot(&e);
        assert_eq!(e.selection, Selection::Link(link));
        assert!((now.y - mid.y).abs() < 1e-9 && (now.x - (mid.x + 60.0)).abs() < 1.0, "{mid:?} -> {now:?}");
        assert_eq!(e.undo.len(), undo + 1, "one undo step");
        // A double-click edits it.
        e.double_click(now);
        assert_eq!(e.editing, Some(EditTarget::LinkLabel(link)));
        e.cancel_edit();
        // Not past the line's ends: the box stays clear of the port there.
        drag(&mut e, now, Pt::new(2000.0, now.y));
        let end = e.doc.link(link).unwrap().label_at.unwrap();
        let (w, _) = geom::label_box(e.doc.link(link).unwrap()).unwrap();
        let right = spot(&e).x + w / 2.0;
        assert!(right <= 340.0 - geom::PORT_HALF + 0.01 && right > 300.0, "{right}");
        // Kept with the file.
        let saved = Diagram::from_json(&e.doc.to_json()).unwrap();
        assert_eq!(saved.link(link).unwrap().label_at, Some(end));
        // "Auto position" lets it be placed automatically again.
        e.selection = Selection::Link(link);
        e.set_prop("label_at", "auto");
        assert_eq!(e.doc.link(link).unwrap().label_at, None);
        assert_eq!(spot(&e), mid);
    }

    #[test]
    fn connection_points_have_labels_beyond_their_line() {
        let (mut e, a, _) = two_devices();
        // A line from the device out to the right, ending at a point.
        let port = e.doc.add_port(a, Side::Right, 0.5).unwrap();
        let q = e.doc.add_point(Pt::new(300.0, 100.0));
        e.doc.add_link(port, Endpoint::point(q), LinkStyle { head_end: Head::Arrow, ..Default::default() });
        e.selection = Selection::Items(vec![q]);
        assert!(e.set_prop("label", "Internet"));
        let (at, align, base) = geom::point_label_anchor(&e.doc, e.doc.point(q).unwrap());
        assert!(at.x > 300.0 && (at.y - 100.0).abs() < 1e-9 && align == "left" && base == "middle", "past the arrow: {at:?}");
        // Double-click edits it in place; exports show it.
        e.double_click(Pt::new(300.0, 100.0));
        assert_eq!(e.editing, Some(EditTarget::PointLabel(q)));
        e.commit_edit("Internet uplink");
        assert_eq!(e.doc.point(q).unwrap().label, "Internet uplink");
        assert!(crate::svg::export(&e.doc, "").contains(">Internet uplink</text>"));
        assert!(crate::drawio::export(&e.doc, "t", "").contains("value=\"Internet uplink\""));
        assert_eq!(Diagram::from_json(&e.doc.to_json()).unwrap().point(q).unwrap().label, "Internet uplink");
        // The frame keeps its margin around the label too, as do the page
        // and Fit.
        e.set_frame("template", "lines");
        let label = geom::label_rects(&e.doc).into_iter().map(|r| r.x + r.w).fold(f64::MIN, f64::max);
        assert!(label > 300.0 + 9.0 + 50.0, "the label sticks out past the point: {label}");
        let outer = crate::frame::of(&e.doc, "").unwrap().outer;
        assert!(outer.x + outer.w >= label + 40.0 - 1e-6, "{outer:?} vs {label}");
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let b = &scene["bounds"];
        assert!(b["x"].as_f64().unwrap() + b["w"].as_f64().unwrap() >= label);
        e.set_frame("template", "none");
        // A point without lines has its label to the right.
        let lone = e.doc.add_point(Pt::new(0.0, 400.0));
        let (at, align, _) = geom::point_label_anchor(&e.doc, e.doc.point(lone).unwrap());
        assert!(at.x > 0.0 && align == "left");
    }

    #[test]
    fn devices_go_into_encapsulated_lines_too() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        e.set_prop("kind", "vxlan");
        e.set_prop("over", "ipsec");
        e.set_prop("label", "DCI");
        let c = e.add_device_at(Pt::new(250.0, 300.0));
        drag(&mut e, Pt::new(250.0, 300.0), Pt::new(250.0, 102.0));
        assert_eq!(e.doc.links.len(), 2, "split in two: {:?}", e.status);
        for l in &e.doc.links {
            assert_eq!((l.style.kind, l.style.over.clone(), l.label.as_str()), (LinkKind::Vxlan, vec![LinkKind::Ipsec], "DCI"));
        }
        // Deleting it joins them again.
        e.selection = Selection::Items(vec![c]);
        e.delete_selected();
        assert_eq!(e.doc.links.len(), 1);
        let l = &e.doc.links[0];
        assert_eq!((l.id, l.from.device, l.to.device), (link, a, b));
    }

    #[test]
    fn lines_can_be_carried_over_others() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        e.set_prop("kind", "vxlan");
        assert!(e.set_prop("over", "ipsec"));
        let st = e.doc.link(link).unwrap().style.clone();
        assert_eq!(st.full_tag(), "VXLAN over IPsec");
        // Two bands, the carrier outside, the tunnel inside.
        assert_eq!(st.bands(), vec![(LinkKind::Ipsec, 13.0), (LinkKind::Vxlan, 6.0)]);
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let bands = scene["links"][0]["bands"].as_array().unwrap();
        assert_eq!(bands.len(), 2);
        assert!(bands[0]["dash"].is_null() && bands[1]["dash"].is_array(), "IPsec solid outside, VXLAN dashed inside");
        assert_eq!(scene["links"][0]["tag"], "VXLAN over IPsec");
        // A third layer, then taken off again; bad values are refused.
        assert!(e.set_prop("over", "ipsec, mpls"));
        assert_eq!(e.doc.link(link).unwrap().style.full_tag(), "VXLAN over IPsec over MPLS");
        assert!(!e.set_prop("over", "ipsec,nonsense"));
        e.set_prop("over", "ipsec");
        // Kept in the file, in SVG (two bands), and through draw.io.
        e.set_prop("label", "DC interconnect");
        let saved = Diagram::from_json(&e.doc.to_json()).unwrap();
        assert_eq!(saved.link(link).unwrap().style.over, vec![LinkKind::Ipsec]);
        let svg = crate::svg::export(&e.doc, "");
        assert_eq!(svg.matches(r#"fill-opacity="0.14""#).count(), 2);
        assert!(svg.contains(">VXLAN over IPsec</text>"));
        let back = crate::drawio_import::import(&crate::drawio::export(&e.doc, "t", "")).unwrap();
        let l = &back.links[0];
        assert_eq!((l.style.kind, l.style.over.clone(), l.label.as_str()), (LinkKind::Vxlan, vec![LinkKind::Ipsec], "DC interconnect"));
        // The site sheet legend lists the carrier too.
        e.set_frame("template", "site");
        let legend = crate::frame::of(&e.doc, "").unwrap().prims.iter().any(|p| matches!(p, crate::frame::Prim::Text { text, .. } if text == "IPsec tunnel"));
        assert!(legend);
    }

    #[test]
    fn the_frame_layer_holds_no_items() {
        let (mut e, a, _) = two_devices();
        let items = e.doc.active_layer;
        e.set_frame("template", "lines");
        let frame = e.doc.frame_layer().unwrap();
        // It cannot become the active layer, nor take items moved to it.
        e.set_active_layer(frame);
        assert_eq!(e.doc.active_layer, items);
        e.selection = Selection::Items(vec![a]);
        assert!(!e.set_prop("layer", &frame.to_string()));
        assert_eq!(e.doc.layer_of(a), Some(items));
        // The last layer for items stays.
        assert!(!e.remove_layer(items));
        // Files with items on it get them moved to a layer for items.
        e.doc.devices[0].layer = frame;
        e.doc.active_layer = frame;
        let back = Diagram::from_json(&e.doc.to_json()).unwrap();
        assert_eq!((back.devices[0].layer, back.active_layer), (items, items));
        // A file with only the frame's layer gets a layer for items.
        let mut only = e.doc.clone();
        only.layers.retain(|l| l.id == frame);
        let back = Diagram::from_json(&only.to_json()).unwrap();
        assert_eq!(back.layers.len(), 2);
        assert!(back.devices.iter().all(|d| d.layer != frame));
    }

    #[test]
    fn clicking_a_line_only_selects_it() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let before = e.doc.clone();
        e.press(Pt::new(250.0, 100.0), false, false);
        e.release(Pt::new(250.0, 100.0));
        assert!(matches!(e.selection, Selection::Link(_)));
        assert_eq!(e.doc, before);
    }

    #[test]
    fn linked_port_pulled_off_its_device_becomes_a_free_point() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let Selection::Link(link) = e.selection else { panic!() };
        let to = e.doc.link(link).unwrap().to;
        // Grab b's port and pull it away (the port sits at 340,100).
        e.press(Pt::new(340.0, 100.0), false, false);
        e.drag_to(Pt::new(340.0, 104.0));
        e.drag_to(Pt::new(300.0, 250.0));
        e.drag_to(Pt::new(301.0, 302.0));
        e.release(Pt::new(301.0, 302.0));
        let l = e.doc.link(link).unwrap();
        assert!(l.to.is_point());
        let q = e.doc.point(l.to.owner()).unwrap();
        assert_eq!(q.pos(), Pt::new(300.0, 300.0), "follows the mouse, on the grid");
        assert_eq!(q.layer, e.doc.device(b).unwrap().layer);
        assert!(e.doc.device(b).unwrap().ports.is_empty(), "the port is gone");
        assert_eq!(e.doc.link(link).unwrap().from.device, a, "the other end stays");
        e.undo();
        assert_eq!(e.doc.link(link).unwrap().to, to);
        assert!(e.doc.points.is_empty());
        // In one drag: off b and onto a's bottom border.
        e.press(Pt::new(340.0, 100.0), false, false);
        e.drag_to(Pt::new(340.0, 104.0));
        e.drag_to(Pt::new(250.0, 250.0));
        e.drag_to(Pt::new(100.0, 131.0));
        e.release(Pt::new(100.0, 131.0));
        let l = e.doc.link(link).unwrap();
        assert_eq!(l.to.device, a);
        assert_eq!(e.doc.device(a).unwrap().port(l.to.port).unwrap().side, Side::Bottom);
    }

    fn demo() -> Editor {
        let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/demo.stratagram")).unwrap();
        Editor { doc: Diagram::from_json(&json).unwrap(), ..Editor::default() }
    }

    #[test]
    fn hierarchy_layout_stacks_the_demo_network_and_tidies_ports() {
        let mut e = demo();
        let before = e.doc.clone();
        let top_left = e.doc.bounds().unwrap();
        assert!(e.auto_layout("hierarchy", 60.0, true, true));
        let d = |id: Id| e.doc.device(id).unwrap().clone();
        // internet -> fw -> core -> servers, top to bottom.
        assert!(d(5).y < d(1).y && d(1).y < d(2).y && d(2).y < d(3).y);
        assert_eq!(d(3).y, d(4).y);
        // Laid out where the diagram was, on the grid, nothing overlapping.
        let all: Vec<Rect> = e.doc.devices.iter().map(|d| d.rect()).collect();
        let b = all.iter().copied().reduce(|a, b| a.union(&b)).unwrap();
        assert_eq!((b.x, b.y), (top_left.x, top_left.y));
        assert!(e.doc.devices.iter().all(|d| d.x % GRID == 0.0 && d.y % GRID == 0.0));
        for i in 0..all.len() {
            for j in i + 1..all.len() {
                let (a, c) = (all[i], all[j]);
                assert!(a.x + a.w <= c.x || c.x + c.w <= a.x || a.y + a.h <= c.y || c.y + c.h <= a.y, "{a:?} {c:?}");
            }
        }
        // Ports now face their links: fw's port to internet moved to the top,
        // core's ports to the servers stay on the bottom, left one first.
        let port = |dev: Id, p: Id| d(dev).port(p).unwrap().clone();
        assert_eq!(port(1, 16).side, Side::Top);
        assert_eq!(port(5, 15).side, Side::Bottom);
        assert_eq!((port(2, 9).side, port(2, 12).side), (Side::Bottom, Side::Bottom));
        assert!(port(2, 9).t < port(2, 12).t, "srv-01 is on the left");
        // One undo step brings everything back.
        e.undo();
        assert_eq!(e.doc, before);
        assert!(!e.auto_layout("spiral", 60.0, true, true));
    }

    #[test]
    fn layout_works_on_the_selection_and_skips_locked_devices() {
        let mut e = demo();
        let fixed: Vec<(Id, Pt)> = [1, 5, 18].iter().map(|id| (*id, e.doc.item_pos(*id).unwrap())).collect();
        e.selection = Selection::Items(vec![2, 3, 4]);
        assert!(e.auto_layout("grid", 40.0, false, true));
        assert!(fixed.iter().all(|(id, p)| e.doc.item_pos(*id) == Some(*p)), "only the selection moves");
        assert!(e.status.contains("Arranged 3 devices"), "{}", e.status);
        // Without a selection: everything shown and unlocked, so not edge-rtr
        // once it sits on a locked layer.
        let l2 = e.add_layer();
        e.selection = Selection::Items(vec![18]);
        e.set_prop("layer", &l2.to_string());
        e.set_layer_locked(l2, true);
        let rtr = e.doc.item_pos(18);
        e.selection = Selection::None;
        for kind in ["hierarchy-lr", "radial", "circle", "force"] {
            assert!(e.auto_layout(kind, 60.0, true, true), "{kind}");
            assert_eq!(e.doc.item_pos(18), rtr);
        }
        // Too few devices to arrange.
        let mut f = Editor::default();
        f.add_device_at(Pt::new(0.0, 0.0));
        assert!(!f.auto_layout("grid", 60.0, true, true));
        assert!(!f.can_undo() || f.undo.len() == 1);
    }

    #[test]
    fn layouts_keep_lines_off_other_objects() {
        for kind in ["hierarchy", "hierarchy-lr", "radial", "circle", "grid", "force"] {
            let mut e = demo();
            // Extra links that tangle things: they cut across other boxes
            // unless something is done about them.
            for (a, b) in [(3, 5), (1, 4), (18, 3)] {
                let pa = e.doc.add_port(a, Side::Top, 0.3).unwrap();
                let pb = e.doc.add_port(b, Side::Bottom, 0.7).unwrap();
                e.doc.add_link(pa, pb, LinkStyle::default());
            }
            assert!(e.auto_layout(kind, 60.0, true, true), "{kind}");
            let obstacles = geom::obstacles(&e.doc);
            for l in &e.doc.links {
                let route = geom::link_route(&e.doc, l).unwrap();
                assert!(!geom::crosses(&route, &obstacles), "{kind}: link {} crosses: {route:?} ({:?})", l.id, l.style.routing);
            }
            // Lines that were already clear keep their own style (fw-01 to
            // core-sw-01, straight down in a hierarchy).
            if kind == "hierarchy" {
                assert_eq!(e.doc.link(8).unwrap().style.routing, Routing::Straight);
            }
        }
        // Without the option, routing is left alone.
        let mut e = demo();
        let before: Vec<Routing> = e.doc.links.iter().map(|l| l.style.routing).collect();
        e.auto_layout("grid", 60.0, true, false);
        assert_eq!(e.doc.links.iter().map(|l| l.style.routing).collect::<Vec<_>>(), before);
    }

    #[test]
    #[ignore = "timing only: cargo test -- --ignored --nocapture"]
    fn time_scene_with_many_routed_lines() {
        let mut e = Editor { snap: false, ..Editor::default() };
        let ids: Vec<Id> = (0..40).map(|i| e.add_device_at(Pt::new((i % 8) as f64 * 200.0, (i / 8) as f64 * 150.0))).collect();
        for k in 0..60 {
            let (a, b) = (ids[(k * 7) % 40], ids[(k * 13 + 5) % 40]);
            if a == b {
                continue;
            }
            let pa = e.doc.add_port(a, Side::Right, 0.5).unwrap();
            let pb = e.doc.add_port(b, Side::Left, 0.5).unwrap();
            e.doc.add_link(pa, pb, LinkStyle { routing: Routing::Around, ..Default::default() });
        }
        let t = std::time::Instant::now();
        let n = 5;
        for _ in 0..n {
            std::hint::black_box(e.scene_json());
        }
        eprintln!("scene_json with {} routed links: {:?} each", e.doc.links.len(), t.elapsed() / n);
    }

    #[test]
    fn free_points_follow_their_device_during_layout() {
        let mut e = demo();
        // A loose arrow from fw-01 (port 6 is at its bottom middle).
        let ep = Endpoint { device: 1, port: 16 };
        let at = e.doc.endpoint_pos(ep).unwrap().0;
        let q = e.doc.add_point(at.add(Pt::new(100.0, 0.0)));
        e.doc.add_link(ep, Endpoint::point(q), LinkStyle::default());
        let offset = e.doc.point(q).unwrap().pos().sub(e.doc.item_pos(1).unwrap());
        assert!(e.auto_layout("circle", 60.0, false, true));
        let now = e.doc.point(q).unwrap().pos().sub(e.doc.item_pos(1).unwrap());
        assert!(now.dist(offset) < 0.01, "{offset:?} {now:?}");
    }

    #[test]
    fn layer_names_show_on_devices_when_turned_on() {
        let (mut e, a, _) = two_devices();
        let first = e.doc.active_layer;
        let l2 = e.add_layer();
        e.rename_layer(l2, "DMZ <edge>");
        let c = e.add_device_at(Pt::new(100.0, 300.0));
        let badge = |e: &Editor, id: Id| -> String {
            let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
            let d = scene["devices"].as_array().unwrap().iter().find(|d| d["id"] == id).unwrap().clone();
            d["badge"].as_str().unwrap().to_string()
        };
        assert_eq!(badge(&e, c), "", "off by default");
        e.set_layer_show_name(l2, true);
        assert_eq!(badge(&e, c), "DMZ <edge>");
        assert_eq!(badge(&e, a), "", "only that layer's devices");
        assert!(e.layers_json().contains(r#""show_name":true"#));
        // In the exports too, escaped.
        let svg = crate::svg::export(&e.doc, "");
        assert!(svg.contains(r#"fill-opacity="0.55">DMZ &lt;edge&gt;</text>"#), "{svg}");
        let x = crate::drawio::export(&e.doc, "t", "");
        assert!(x.contains(&format!(r#"<mxCell id="d{c}layer" value="DMZ &amp;lt;edge&amp;gt;""#)), "{x}");
        assert!(!x.contains(&format!(r#"id="d{a}layer""#)));
        // Saved, one undo step, and only written when on.
        let json = e.doc.to_json();
        assert_eq!(json.matches(r#""show_name": true"#).count(), 1);
        assert!(!json.contains(r#""show_name": false"#));
        assert!(Diagram::from_json(&json).unwrap().layer(l2).unwrap().show_name);
        e.undo();
        assert_eq!(badge(&e, c), "");
        let _ = first;
    }

    #[test]
    fn copy_for_drawio_takes_the_selection_or_everything_shown() {
        let mut e = demo();
        // Nothing selected: the whole diagram, as one flat draw.io model.
        let all = e.copy_drawio().unwrap();
        assert!(all.starts_with("<mxGraphModel><root>"));
        assert!(all.contains(r#"<mxCell id="1" parent="0"/>"#));
        assert!(!all.contains("layer"), "no layer cells, everything in draw.io's layer 1");
        assert_eq!(all.matches("stratagram_id=").count(), e.doc.devices.len());
        assert!(e.status.contains("the diagram"), "{}", e.status);
        // fw-01 and core-sw-01: their link comes along with its port names;
        // the line from internet to fw-01 is not selected, so it stays home.
        e.selection = Selection::Items(vec![1, 2]);
        let sel = e.copy_drawio().unwrap();
        assert_eq!(sel.matches("stratagram_id=").count(), 2);
        assert!(sel.contains(r#"id="l8""#) && !sel.contains(r#"id="l17""#));
        assert!(sel.contains(r#"value="Te1/1/1""#));
        // A selected line whose devices are not selected keeps loose ends.
        e.selection = Selection::Items(vec![1, 17]);
        let sel = e.copy_drawio().unwrap();
        let cell = &sel[sel.find(r#"id="l17""#).unwrap()..];
        let cell = &cell[..cell.find("</mxCell>").unwrap()];
        assert!(cell.contains(r#"as="sourcePoint""#) && cell.contains(r#"target="p16""#), "{cell}");
        // Hidden layers are not copied.
        let l2 = e.add_layer();
        e.selection = Selection::Items(vec![18]);
        e.set_prop("layer", &l2.to_string());
        e.set_layer_visible(l2, false);
        e.selection = Selection::None;
        assert!(!e.copy_drawio().unwrap().contains("edge-rtr"));
        assert!(Editor::default().copy_drawio().is_none());
    }

    #[test]
    fn paste_copies_from_stratagram_draw_io_and_plain_text() {
        let view = Rect { x: 0.0, y: 0.0, w: 1000.0, h: 800.0 };
        let mut e = demo();
        e.selection = Selection::Items(vec![1, 2]); // fw-01 and core-sw-01
        let native = e.copy_native();
        let xml = e.copy_drawio().unwrap();
        let (devs, links, undo) = (e.doc.devices.len(), e.doc.links.len(), e.undo.len());
        // Ours: exact copies, 20 px down and right, with fresh ids.
        assert!(e.paste(&native, &xml, view));
        assert_eq!(e.doc.devices.len(), devs + 2);
        assert_eq!(e.doc.links.len(), links + 1, "the link between them");
        assert_eq!(e.undo.len(), undo + 1, "one undo step");
        let Selection::Items(sel) = e.selection.clone() else { panic!() };
        assert_eq!(sel.len(), 3, "both devices and the link are selected");
        let fw = e.doc.devices.iter().filter(|d| d.name == "fw-01").cloned().collect::<Vec<_>>();
        assert_eq!(fw.len(), 2);
        assert_eq!((fw[1].x - fw[0].x, fw[1].y - fw[0].y), (20.0, 20.0));
        assert_eq!(fw[1].ports.len(), fw[0].ports.len());
        assert!(fw[1].ports.iter().all(|p| fw[0].ports.iter().all(|q| q.id != p.id)));
        let copy = e.doc.links.last().unwrap();
        assert!(copy.from.device == fw[1].id && e.doc.device(copy.to.device).unwrap().name == "core-sw-01");
        // Pasting the same again steps further.
        e.paste(&native, &xml, view);
        let fw3 = e.doc.devices.iter().rfind(|d| d.name == "fw-01").unwrap().clone();
        assert_eq!((fw3.x - fw[0].x, fw3.y - fw[0].y), (40.0, 40.0));
        // Out of view: into the middle of the view instead.
        let far = Rect { x: 5000.0, y: 5000.0, w: 1000.0, h: 800.0 };
        e.paste(&native, &xml, far);
        let Selection::Items(sel) = e.selection.clone() else { panic!() };
        let b = sel.iter().filter_map(|id| e.doc.device(*id)).map(|d| d.rect()).reduce(|a, b| a.union(&b)).unwrap();
        assert!((b.x + b.w / 2.0 - 5500.0).abs() <= GRID && (b.y + b.h / 2.0 - 5400.0).abs() <= GRID, "{b:?}");
        // From draw.io (only the XML): shapes in the middle of the view.
        let mut f = Editor::default();
        assert!(f.paste("", &xml, view));
        assert_eq!(f.doc.devices.len(), 2);
        assert_eq!(f.doc.links.len(), 1);
        assert!(f.status.contains("from draw.io"), "{}", f.status);
        // Plain text becomes a note.
        let mut g = Editor::default();
        assert!(g.paste("", "  Site B\nVLAN 30 ", view));
        assert_eq!(g.doc.notes[0].text, "Site B\nVLAN 30");
        assert!(!g.paste("", "   ", view));
        // Nothing lands in a locked layer.
        let first = g.doc.active_layer;
        g.set_layer_locked(first, true);
        assert!(!g.paste(&native, &xml, view));
        assert_eq!(g.doc.devices.len(), 0);
    }

    /// A "site" box holding two devices, one of which holds a note.
    fn site() -> (Editor, Id, Id, Id, Id) {
        let mut e = Editor { snap: false, ..Editor::default() };
        let site = e.add_device_at(Pt::new(300.0, 200.0));
        e.selection = Selection::Items(vec![site]);
        e.set_prop("w", "400");
        e.set_prop("h", "300");
        // Now 240..640 x 170..470 (resizing keeps the corner).
        let a = e.add_device_at(Pt::new(330.0, 300.0)); // 270..390 x 270..330
        let b = e.add_device_at(Pt::new(530.0, 400.0)); // 470..590 x 370..430
        let n = e.add_note_at(Pt::new(480.0, 380.0));
        e.cancel_edit();
        (e, site, a, b, n)
    }

    #[test]
    fn devices_contain_what_lies_inside_and_in_front_of_them() {
        let (e, site, a, b, n) = site();
        let parents = e.doc.containers();
        assert_eq!((parents.get(&a), parents.get(&b), parents.get(&n)), (Some(&site), Some(&site), Some(&b)));
        assert_eq!(parents.get(&site), None);
        assert_eq!(Diagram::contents(&parents, site), { let mut v = vec![a, b, n]; v.sort(); v });
        assert_eq!(Diagram::ancestors(&parents, n), vec![b, site]);
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let flag = |id: Id| scene["devices"].as_array().unwrap().iter().find(|d| d["id"] == id).unwrap()["container"].clone();
        assert_eq!((flag(site), flag(a), flag(b)), (json!(true), json!(false), json!(true)));
    }

    #[test]
    fn moving_a_container_carries_its_contents() {
        let (mut e, site, a, b, n) = site();
        let before: Vec<Pt> = [a, b, n].iter().map(|id| e.doc.item_pos(*id).unwrap()).collect();
        drag(&mut e, Pt::new(600.0, 200.0), Pt::new(700.0, 250.0));
        assert_eq!(e.doc.item_pos(site), Some(Pt::new(340.0, 220.0)));
        for (id, p) in [a, b, n].iter().zip(before) {
            assert_eq!(e.doc.item_pos(*id), Some(p.add(Pt::new(100.0, 50.0))));
        }
        // Moving a child alone leaves the container where it is.
        drag(&mut e, Pt::new(430.0, 350.0), Pt::new(440.0, 360.0));
        assert_eq!(e.doc.item_pos(site), Some(Pt::new(340.0, 220.0)));
    }

    #[test]
    fn dropping_into_a_box_in_front_brings_the_item_forward() {
        let mut e = Editor { snap: false, ..Editor::default() };
        let small = e.add_device_at(Pt::new(800.0, 100.0));
        let big = e.add_device_at(Pt::new(300.0, 200.0));
        e.selection = Selection::Items(vec![big]);
        e.set_prop("w", "400");
        e.set_prop("h", "300");
        drag(&mut e, Pt::new(800.0, 100.0), Pt::new(400.0, 300.0));
        assert_eq!(e.hit_test(Pt::new(400.0, 300.0)), Hit::Device(small), "visible in front of the box");
        assert_eq!(e.doc.containers().get(&small), Some(&big));
    }

    #[test]
    fn layout_moves_containers_with_contents_and_lines_may_leave_them() {
        let (mut e, site, a, _, _) = site();
        let outside = e.add_device_at(Pt::new(1000.0, 200.0));
        let pa = e.doc.add_port(a, Side::Right, 0.5).unwrap();
        let po = e.doc.add_port(outside, Side::Left, 0.5).unwrap();
        let l = e.doc.add_link(pa, po, LinkStyle { routing: Routing::Around, ..Default::default() });
        // The line from inside the site gets out around b, not through it.
        let route = geom::link_route(&e.doc, e.doc.link(l).unwrap()).unwrap();
        assert!(route.len() > 2);
        let offset = e.doc.item_pos(a).unwrap().sub(e.doc.item_pos(site).unwrap());
        e.selection = Selection::None;
        assert!(e.auto_layout("grid", 60.0, false, true));
        assert!(e.status.contains("Arranged 2 devices"), "only the outer ones: {}", e.status);
        assert_eq!(e.doc.item_pos(a).unwrap().sub(e.doc.item_pos(site).unwrap()), offset);
    }

    #[test]
    fn deep_zoom_levels_follow_the_view_zoom() {
        let mut e = Editor { zoom: 8.0, ..Editor::default() };
        assert_eq!(e.level(), 1.0, "off: always the top level");
        e.deep = true;
        for (zoom, level) in [(1.0, 1.0), (4.0, 1.0), (5.0, 1.0), (6.0, 1.0 / 8.0), (8.0, 1.0 / 8.0), (48.0, 1.0 / 64.0), (0.1, 8.0)] {
            e.zoom = zoom;
            assert!((e.level() - level).abs() < 1e-12, "zoom {zoom}: {}", e.level());
        }
    }

    /// A rack, zoomed into 8x, with a small server added inside it.
    fn rack() -> (Editor, Id, Id) {
        let mut e = Editor::default();
        let rack = e.add_device_at(Pt::new(100.0, 100.0)); // 40..160 x 70..130
        e.deep = true;
        e.zoom = 8.0;
        let srv = e.add_device_at(Pt::new(70.0, 90.0));
        (e, rack, srv)
    }

    #[test]
    fn items_added_while_zoomed_in_live_at_that_scale_inside_the_object() {
        let (mut e, rack, srv) = rack();
        let d = e.doc.device(srv).unwrap().clone();
        assert_eq!((d.scale, d.w, d.h), (1.0 / 8.0, 15.0, 7.5));
        // Snapped to the fine grid of that level, not the top one.
        assert_eq!((d.x % 1.25, d.y % 1.25), (0.0, 0.0));
        assert_eq!(e.doc.containers().get(&srv), Some(&rack));
        // The rack keeps its size: nothing outside changes.
        assert_eq!(e.doc.device(rack).unwrap().rect(), Rect { x: 40.0, y: 70.0, w: 120.0, h: 60.0 });
        // The panel shows its nominal size.
        e.selection = Selection::Items(vec![srv]);
        let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!((sel["w"].as_f64(), sel["h"].as_f64()), (Some(120.0), Some(60.0)));
        e.set_prop("w", "160");
        assert_eq!(e.doc.device(srv).unwrap().w, 20.0);
        // A note and a point made in there are at that scale too.
        let n = e.add_note_at(Pt::new(50.0, 110.0));
        e.cancel_edit();
        assert_eq!(e.doc.note(n).unwrap().font_size(), 14.0 / 8.0);
        let q = e.add_point_at(Pt::new(150.0, 110.0));
        assert_eq!(e.doc.point(q).unwrap().scale, 1.0 / 8.0);
    }

    #[test]
    fn zooming_out_hides_what_is_inside_and_zooming_in_shows_it_again() {
        let (mut e, rack, srv) = rack();
        let inside = e.doc.device(srv).unwrap().rect();
        let probe = Pt::new(inside.x + inside.w / 2.0, inside.y + inside.h / 2.0);
        let shown = |e: &Editor, id: Id| {
            let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
            scene["devices"].as_array().unwrap().iter().find(|d| d["id"] == id).map(|d| d["a"].as_f64().unwrap())
        };
        assert_eq!(shown(&e, srv), Some(1.0));
        assert_eq!(e.hit_test(probe), Hit::Device(srv));
        // Zoomed back out: gone from view and out of reach; the rack is hit.
        e.zoom = 1.0;
        e.zoom_changed();
        assert_eq!(e.selection, Selection::None, "the hidden server left the selection");
        assert_eq!(shown(&e, srv), None);
        assert_eq!(e.hit_test(probe), Hit::Device(rack));
        e.select_all();
        assert_eq!(e.selection, Selection::Items(vec![rack]));
        // The rack shows its name in the middle again while its contents
        // are too small to see.
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert_eq!(scene["devices"][0]["container"], false);
        // Not yet at 400% (the limit without deep zoom); fading in above it.
        e.zoom = 4.0;
        assert_eq!(shown(&e, srv), None);
        e.zoom = 5.0;
        let a = shown(&e, srv).unwrap();
        assert!(a > 0.0 && a < 1.0, "{a}");
        // Without deep zoom it stays hidden whatever the zoom.
        e.deep = false;
        e.zoom = 8.0;
        assert_eq!(shown(&e, srv), None);
        e.deep = true;
        // Moving the rack takes its inner world along.
        e.zoom = 1.0;
        drag(&mut e, Pt::new(100.0, 100.0), Pt::new(300.0, 100.0));
        assert_eq!(e.doc.device(srv).unwrap().x, inside.x + 200.0);
        e.zoom = 8.0;
        assert_eq!(shown(&e, srv), Some(1.0));
    }

    #[test]
    fn zoomed_into_a_box_its_inside_is_the_background() {
        let (mut e, rack, srv) = rack();
        e.selection = Selection::None;
        // Double-click inside the rack, away from the server: a new device
        // at this level, in the rack.
        e.double_click(Pt::new(140.0, 115.0));
        let Some(EditTarget::DeviceName(new)) = e.editing else { panic!("nothing added") };
        e.cancel_edit();
        assert_ne!(new, rack);
        assert_eq!(e.doc.device(new).unwrap().scale, 1.0 / 8.0);
        assert_eq!(e.doc.containers().get(&new), Some(&rack));
        // Dragging inside the rack draws a selection rectangle, not a move.
        drag(&mut e, Pt::new(42.0, 72.0), Pt::new(158.0, 128.0));
        let Selection::Items(ids) = e.selection.clone() else { panic!() };
        assert!(ids.contains(&srv) && ids.contains(&new) && !ids.contains(&rack));
        assert_eq!(e.doc.device(rack).unwrap().x, 40.0);
        // Its border still takes links.
        assert!(matches!(e.hit_test(Pt::new(40.2, 100.0)), Hit::Edge { device, .. } if device == rack));
    }

    #[test]
    fn zooming_out_never_fades_the_diagram_itself() {
        let (mut e, rack, srv) = rack();
        for deep in [false, true] {
            e.deep = deep;
            e.zoom = 0.2;
            let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
            let devices = scene["devices"].as_array().unwrap();
            assert_eq!(devices.len(), 1, "only the rack; the server inside is hidden");
            assert_eq!((devices[0]["id"].clone(), devices[0]["a"].as_f64()), (json!(rack), Some(1.0)));
            assert_eq!(e.hit_test(Pt::new(100.0, 100.0)), Hit::Device(rack));
        }
        let _ = srv;
    }

    #[test]
    fn deep_links_pastes_and_files_keep_their_scale() {
        let (mut e, rack, srv) = rack();
        let other = e.add_device_at(Pt::new(130.0, 90.0));
        // A line between two inner devices: thin, with the default width as
        // the panel shows it.
        let (a, b) = (e.doc.device(srv).unwrap().rect(), e.doc.device(other).unwrap().rect());
        drag(&mut e, Pt::new(a.x + a.w, a.y + a.h / 2.0), Pt::new(b.x + b.w / 2.0, b.y + b.h / 2.0));
        let Selection::Link(l) = e.selection else { panic!("no link") };
        assert_eq!((e.doc.link(l).unwrap().style.width, e.doc.link(l).unwrap().scale), (2.0 / 8.0, 1.0 / 8.0));
        let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(sel["width"], 2.0);
        // Saved and loaded with their scale; top-level items stay unmarked.
        let json = e.doc.to_json();
        assert_eq!(json.matches(r#""scale": 0.125"#).count(), 3, "two devices and the link: {json}");
        let back = Diagram::from_json(&json).unwrap();
        assert_eq!(back.device(srv).unwrap().scale, 0.125);
        assert_eq!(back.device(rack).unwrap().scale, 1.0);
        // Copied at the top level, pasted while zoomed in: made small.
        let mut f = Editor::default();
        let top = f.add_device_at(Pt::new(0.0, 0.0));
        f.selection = Selection::Items(vec![top]);
        let native = f.copy_native();
        let view = Rect { x: 40.0, y: 70.0, w: 120.0, h: 60.0 };
        assert!(e.paste(&native, "", view));
        let Selection::Items(p) = e.selection.clone() else { panic!() };
        let pasted = e.doc.device(p[0]).unwrap();
        assert_eq!((pasted.scale, pasted.w), (1.0 / 8.0, 15.0));
        assert_eq!(e.doc.containers().get(&p[0]), Some(&rack));
        // SVG draws them in a scaled group.
        let svg = crate::svg::export(&e.doc, "");
        assert!(svg.contains("scale(0.125)"), "{svg}");
    }

    fn two_devices() -> (Editor, Id, Id) {
        let mut e = Editor::default();
        let a = e.add_device_at(Pt::new(100.0, 100.0)); // rect 40..160 x 70..130
        let b = e.add_device_at(Pt::new(400.0, 100.0)); // rect 340..460 x 70..130
        (e, a, b)
    }

    #[test]
    fn adding_a_device_of_a_type() {
        let mut e = Editor::default();
        let plain = e.add_device_at(Pt::new(0.0, 0.0));
        let r1 = e.add_device_of("router", Pt::new(200.0, 0.0));
        let r2 = e.add_device_of("Router", Pt::new(400.0, 0.0));
        let ap = e.add_device_of("ap", Pt::new(600.0, 0.0));
        let aws = e.add_device_of("aws", Pt::new(800.0, 0.0));
        let d = |id| e.doc.device(id).unwrap();
        assert_eq!((d(plain).name.as_str(), d(plain).kind.as_str()), ("Device 1", ""));
        assert_eq!((d(r1).name.as_str(), d(r1).kind.as_str()), ("Router 1", "router"));
        assert_eq!(d(r2).name, "Router 2");
        assert_eq!(d(r1).color, kind_color("router").unwrap());
        assert_eq!((d(ap).name.as_str(), d(ap).kind.as_str()), ("Access point 1", "access point"));
        assert_eq!(d(aws).name, "AWS 1");
        assert_eq!(e.selection, Selection::Items(vec![aws]));
        // One undo step each.
        e.undo();
        assert!(e.doc.device(aws).is_none() && e.doc.device(ap).is_some());
        // The AWS gateways, by their short forms; named as engineers do.
        let tgw = e.add_device_of("tgw", Pt::new(0.0, 300.0));
        let dx = e.add_device_of("DX gateway", Pt::new(200.0, 300.0));
        let d = |id| e.doc.device(id).unwrap();
        assert_eq!((d(tgw).name.as_str(), d(tgw).kind.as_str()), ("TGW 1", "aws transit gateway"));
        assert_eq!((d(dx).name.as_str(), d(dx).kind.as_str()), ("DXGW 1", "aws direct connect gateway"));
        assert_eq!(crate::shape::kind_label(&d(dx).kind), "aws dx gateway");
        let l3 = e.add_device_of("L3Out", Pt::new(400.0, 300.0));
        assert_eq!((e.doc.device(l3).unwrap().name.as_str(), e.doc.device(l3).unwrap().kind.as_str()), ("L3Out 1", "aci l3out"));
        let o = crate::netbox::Options::default();
        assert_eq!(crate::netbox::kind_for_role("transit-gateway", "Transit Gateway", &o), "aws transit gateway");
        assert_eq!(crate::netbox::kind_for_role("dxgw", "Direct Connect Gateway", &o), "aws direct connect gateway");
        assert_eq!(crate::netbox::kind_for_role("aci-l3out", "ACI L3Out", &o), "aci l3out");
    }

    #[test]
    fn toolbar_devices_do_not_land_on_each_other() {
        let mut e = Editor::default();
        let at = Pt::new(500.0, 400.0);
        let ids: Vec<Id> = (0..6).map(|_| e.add_device_near("router", at)).collect();
        let rects: Vec<Rect> = ids.iter().map(|id| e.doc.device(*id).unwrap().rect()).collect();
        for (i, a) in rects.iter().enumerate() {
            for b in &rects[i + 1..] {
                assert!(a.x + a.w <= b.x || b.x + b.w <= a.x || a.y + a.h <= b.y || b.y + b.h <= a.y, "{a:?} {b:?}");
            }
        }
        // The first at the spot, the next right beside it.
        let c = |r: &Rect| Pt::new(r.x + r.w / 2.0, r.y + r.h / 2.0);
        assert!(c(&rects[0]).dist(at) < 10.0);
        assert!((c(&rects[1]).y - at.y).abs() < 10.0 && c(&rects[1]).x > at.x, "{:?}", rects[1]);
        // Inside a big device (zoomed into it): still at the spot.
        let mut e = Editor::default();
        let big = e.add_device_at(Pt::new(500.0, 400.0));
        let d = e.doc.device_mut(big).unwrap();
        (d.x, d.y, d.w, d.h) = (0.0, 0.0, 1000.0, 800.0);
        let inner = e.add_device_near("", at);
        assert!(c(&e.doc.device(inner).unwrap().rect()).dist(at) < 10.0);
    }

    #[test]
    fn sheets_add_switch_rename_remove_and_undo() {
        let mut e = Editor::default();
        e.add_device_of("router", Pt::new(0.0, 0.0));
        assert_eq!(e.sheets_json(), r#"[{"active":true,"empty":false,"name":"Sheet 1"}]"#);
        // A second sheet: shown, empty; the first keeps its router.
        assert_eq!(e.add_sheet(), 1);
        assert!(e.doc.devices.is_empty() && e.sheet_count() == 2);
        assert_eq!((e.sheet_name(0).as_str(), e.sheet_name(1).as_str()), ("Sheet 1", "Sheet 2"));
        e.add_device_of("firewall", Pt::new(0.0, 0.0));
        e.switch_sheet(0);
        assert_eq!(e.doc.devices[0].kind, "router");
        e.switch_sheet(1);
        assert_eq!(e.doc.devices[0].kind, "firewall");
        assert!(e.rename_sheet(1, "  DMZ "));
        assert_eq!(e.sheet_name(1), "DMZ");
        assert!(!e.rename_sheet(1, "   "), "no empty names");

        // Undo jumps back to the sheet of the edit undone.
        e.switch_sheet(0);
        e.undo(); // the rename
        assert_eq!((e.active_sheet(), e.sheet_name(1).as_str()), (1, "Sheet 2"));
        e.undo(); // the firewall
        assert!(e.doc.devices.is_empty());
        e.redo();
        e.redo();
        assert_eq!((e.doc.devices.len(), e.sheet_name(1).as_str()), (1, "DMZ"));

        // Removing: the neighbor shows; the last one stays; undo brings it back.
        assert!(e.remove_sheet(1));
        assert_eq!((e.sheet_count(), e.active_sheet(), e.doc.devices[0].kind.as_str()), (1, 0, "router"));
        assert!(!e.remove_sheet(0));
        e.undo();
        assert_eq!((e.sheet_count(), e.sheet_name(e.active_sheet()).as_str()), (2, "DMZ"));

        // Reordering: the sheet in view stays in view; one undo step.
        e.add_sheet(); // "Sheet 2" (free again), after DMZ
        e.switch_sheet(0);
        let names = |e: &Editor| (0..e.sheet_count()).map(|i| e.sheet_name(i)).collect::<Vec<_>>();
        assert_eq!(names(&e), ["Sheet 1", "DMZ", "Sheet 2"]);
        assert!(e.move_sheet(0, 2));
        assert_eq!(names(&e), ["DMZ", "Sheet 2", "Sheet 1"]);
        assert_eq!((e.active_sheet(), e.doc.devices[0].kind.as_str()), (2, "router"));
        assert!(e.move_sheet(1, 0));
        assert_eq!(names(&e), ["Sheet 2", "DMZ", "Sheet 1"]);
        assert_eq!(e.active_sheet(), 2, "still showing Sheet 1");
        assert!(!e.move_sheet(1, 1) && !e.move_sheet(0, 5));
        e.undo();
        assert_eq!(names(&e), ["DMZ", "Sheet 2", "Sheet 1"]);
    }

    #[test]
    fn sheets_are_saved_with_the_file() {
        // One sheet: the file is as it always was.
        let mut e = Editor::default();
        e.add_device_of("router", Pt::new(0.0, 0.0));
        let one = e.document_json();
        assert!(!one.contains("\"sheets\"") && !one.contains("\"name\": \"Sheet"), "{one}");
        assert_eq!(one, e.doc.to_json());
        // Several: the first sheet as before, the others in "sheets".
        e.add_sheet();
        e.add_device_of("switch", Pt::new(0.0, 0.0));
        e.rename_sheet(1, "Core");
        let json = e.document_json();
        let mut f = Editor::default();
        f.load_json(&json, Path::new("x.stratagram")).unwrap();
        assert_eq!((f.sheet_count(), f.active_sheet()), (2, 0));
        assert_eq!((f.sheet_name(0).as_str(), f.sheet_name(1).as_str()), ("Sheet 1", "Core"));
        assert_eq!(f.doc.devices[0].kind, "router");
        f.switch_sheet(1);
        assert_eq!(f.doc.devices[0].kind, "switch");
        // An older reader (or a paste) takes the first sheet.
        assert_eq!(Diagram::from_json(&json).unwrap().devices[0].kind, "router");
        assert_eq!(f.document_json(), json);
    }

    #[test]
    fn ports_can_be_on_a_layer_of_their_own() {
        let (mut e, a, b) = two_devices();
        let pa = e.doc.add_port(a, Side::Right, 0.5).unwrap();
        let pb = e.doc.add_port(b, Side::Left, 0.5).unwrap();
        e.doc.add_link(pa, pb, LinkStyle::default());
        // A layer for the interfaces; the port of `a` goes on it.
        let ifaces = e.add_layer();
        let first = e.doc.layers[0].id;
        e.set_active_layer(first);
        e.selection = Selection::Port(pa);
        assert!(e.set_prop("layer", &ifaces.to_string()));
        assert_eq!(e.doc.device(a).unwrap().port(pa.port).unwrap().layer, ifaces);
        assert!(e.selection_json().contains(&format!("\"port_layer\":{ifaces}")));
        // Hidden with its layer: not drawn, not clickable; the line stays.
        e.set_layer_visible(ifaces, false);
        let scene = e.scene_json();
        let ports_shown = |scene: &str| scene.matches("\"side\":").count();
        assert_eq!(ports_shown(&scene), 1, "only b's port: {scene}");
        let at = e.doc.endpoint_pos(pa).unwrap().0;
        e.selection = Selection::None;
        e.press(at, false, false);
        e.release(at);
        assert!(!matches!(e.selection, Selection::Port(p) if p == pa));
        assert_eq!(e.doc.links.len(), 1);
        // SVG leaves it out too.
        let squares = |svg: &str| svg.matches(&format!("width=\"{:.1}\"", 2.0 * geom::PORT_HALF)).count();
        assert_eq!(squares(&crate::svg::export(&e.doc, "")), 1);
        // Shown again: back.
        e.set_layer_visible(ifaces, true);
        assert_eq!(ports_shown(&e.scene_json()), 2);
        // Back on the device's layer ("" or the device's layer itself).
        e.selection = Selection::Port(pa);
        assert!(e.set_prop("layer", ""));
        assert_eq!(e.doc.device(a).unwrap().port(pa.port).unwrap().layer, 0);
        // New ports follow the active layer, unless it is the device's own.
        e.set_active_layer(ifaces);
        let p2 = e.doc.add_port(a, Side::Top, 0.5).unwrap();
        assert_eq!(e.doc.device(a).unwrap().port(p2.port).unwrap().layer, ifaces);
        e.set_active_layer(first);
        let p3 = e.doc.add_port(a, Side::Bottom, 0.5).unwrap();
        assert_eq!(e.doc.device(a).unwrap().port(p3.port).unwrap().layer, 0);
        // Kept in the file; removing the layer puts them back on the device's.
        let json = e.doc.to_json();
        assert_eq!(Diagram::from_json(&json).unwrap().device(a).unwrap().port(p2.port).unwrap().layer, ifaces);
        assert!(e.remove_layer(ifaces));
        assert_eq!(e.doc.device(a).unwrap().port(p2.port).unwrap().layer, 0);
        assert_eq!(e.doc.links.len(), 1, "no line lost");
    }

    #[test]
    fn line_labels_can_be_hidden() {
        let (mut e, a, b) = two_devices();
        let (pa, pb) = (e.doc.add_port(a, Side::Right, 0.5).unwrap(), e.doc.add_port(b, Side::Left, 0.5).unwrap());
        let id = e.doc.add_link(pa, pb, LinkStyle { kind: LinkKind::Trunk, ..Default::default() });
        e.doc.link_mut(id).unwrap().label = "Uplink 10G".into();
        assert!(e.scene_json().contains("Uplink 10G") && crate::svg::export(&e.doc, "").contains("Uplink 10G"));
        e.selection = Selection::Link(id);
        assert!(e.set_prop("link:label_hidden", "true"));
        let l = e.doc.link(id).unwrap();
        assert!(l.label_hidden && l.label == "Uplink 10G", "the text is kept");
        assert!(geom::label_box(l).is_none() && geom::label_spots(&e.doc).is_empty());
        let scene = e.scene_json();
        assert!(!scene.contains("Uplink 10G") && !scene.contains("802.1Q"), "no label, no tag");
        let svg = crate::svg::export(&e.doc, "");
        assert!(!svg.contains("Uplink 10G") && !svg.contains("802.1Q"));
        // New lines do not take it over.
        assert!(!e.link_defaults.color.is_empty());
        let (pc, pd) = (e.doc.add_port(a, Side::Top, 0.5).unwrap(), e.doc.add_port(b, Side::Top, 0.5).unwrap());
        let other = e.doc.add_link(pc, pd, e.link_defaults.clone());
        assert!(!e.doc.link(other).unwrap().label_hidden);
        // Saved, and hidden in draw.io too (read back hidden).
        assert!(Diagram::from_json(&e.doc.to_json()).unwrap().link(id).unwrap().label_hidden);
        let xml = crate::drawio::export(&e.doc, "t", "");
        assert!(xml.contains("noLabel=1;"));
        let back = crate::drawio_import::import(&xml).unwrap();
        assert!(back.links.iter().any(|l| l.label_hidden && l.label == "Uplink 10G"));
        // Typing a label in place shows it again.
        e.selection = Selection::Link(id);
        assert!(e.begin_edit_selection());
        e.commit_edit_sized("Uplink 40G", 0.0, 0.0);
        assert!(!e.doc.link(id).unwrap().label_hidden);
        // Undoable.
        e.undo();
        assert!(e.doc.link(id).unwrap().label_hidden);
    }

    #[test]
    fn a_bent_line_runs_on_through_its_points() {
        let (mut e, a, b) = two_devices();
        let (pa, pb) = (e.doc.add_port(a, Side::Right, 0.5).unwrap(), e.doc.add_port(b, Side::Left, 0.5).unwrap());
        let id = e.doc.add_link(pa, pb, LinkStyle { kind: LinkKind::L3Sub, head_end: Head::Arrow, ..Default::default() });
        e.doc.link_mut(id).unwrap().label = "VLAN 20".into();
        // Bent twice: three pieces through two points.
        let p1 = e.doc.split_link(id, Pt::new(250.0, 40.0)).unwrap();
        let second = e.doc.links.iter().find(|l| l.from == Endpoint::point(p1)).unwrap().id;
        e.doc.split_link(second, Pt::new(300.0, 160.0)).unwrap();
        assert_eq!(e.doc.links.len(), 3);
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let links = scene["links"].as_array().unwrap();
        // End marks at the two devices only, the arrow at the end only.
        let beads: usize = links.iter().map(|l| l["beads"].as_array().unwrap().len() / 2).sum();
        assert_eq!(beads, 2, "{links:?}");
        let heads: usize = links.iter().map(|l| l["heads"].as_array().unwrap().len()).sum();
        assert_eq!(heads, 1);
        // The tag once, with the label.
        let tags: Vec<&str> = links.iter().map(|l| l["tag"].as_str().unwrap()).filter(|t| !t.is_empty()).collect();
        assert_eq!(tags, ["L3 subif"]);
        let svg = crate::svg::export(&e.doc, "");
        assert_eq!(svg.matches(">L3 subif<").count(), 1);
        assert_eq!(svg.matches(">VLAN 20<").count(), 1);
        let xml = crate::drawio::export(&e.doc, "t", "");
        assert_eq!(xml.matches("&lt;b&gt;L3 subif&lt;/b&gt;").count(), 1, "{xml}");
        assert_eq!(xml.matches("endArrow=block").count(), 1);

        // A tunnel: its band once, along all of it.
        for l in e.doc.links.iter_mut() {
            l.style.kind = LinkKind::Vxlan;
        }
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let banded: Vec<&Value> = scene["links"].as_array().unwrap().iter().filter(|l| !l["bands"].as_array().unwrap().is_empty()).collect();
        assert_eq!(banded.len(), 1);
        let band = banded[0]["bands"][0]["pts"].as_array().unwrap().len() / 2;
        assert!(band >= 2 * 4, "goes round the bends: {band} points");

        // Lines of different types meeting at a point keep their ends.
        let first = e.doc.links.iter().find(|l| l.from == pa).unwrap().id;
        e.doc.link_mut(first).unwrap().style.kind = LinkKind::L2;
        assert!(!geom::is_joint(&e.doc, Endpoint::point(p1)));
    }

    #[test]
    fn a_file_without_positions_is_laid_out() {
        // As a script writes it: no positions, sizes, port sides or colors;
        // types by any name the app knows.
        let json = r#"{"format": "stratagram", "version": 1,
            "devices": [{"id": 1, "name": "fw", "kind": "Firewall", "ports": [{"id": 11, "name": "port2"}]},
                        {"id": 2, "name": "sw", "kind": "sw", "ports": [{"id": 21}, {"id": 22}]},
                        {"id": 3, "name": "srv", "kind": "srv", "ports": [{"id": 31}]}],
            "links": [{"id": 4, "from": {"device": 1, "port": 11}, "to": {"device": 2, "port": 21}, "kind": "trunk"},
                      {"id": 5, "from": {"device": 2, "port": 22}, "to": {"device": 3, "port": 31}}]}"#;
        let mut e = Editor::default();
        e.load_json(json, Path::new("gen.stratagram")).unwrap();
        assert_eq!(e.layout_document("hierarchy", 60.0), Ok(3));
        let d = |id: Id| e.doc.device(id).unwrap().clone();
        assert_eq!((d(1).kind.as_str(), d(2).kind.as_str(), d(3).kind.as_str()), ("firewall", "switch", "server"));
        assert_eq!(d(1).color, kind_color("firewall").unwrap());
        assert!(d(1).y < d(2).y && d(2).y < d(3).y, "top down");
        for (a, b) in [(1, 2), (2, 3), (1, 3)] {
            assert!(!d(a).rect().inflate(-1.0).contains_rect(&d(b).rect()) && !overlaps(&d(a).rect(), &d(b).rect()));
        }
        // Ports face their lines.
        assert_eq!(d(1).port(11).unwrap().side, Side::Bottom);
        assert!(e.layout_document("nonsense", 60.0).is_err());
    }

    #[test]
    fn a_label_moves_along_a_bent_line_through_its_points() {
        let (mut e, a, b) = two_devices(); // a: 40..160 x 70..130, b: 340..460 x 70..130
        let (pa, pb) = (e.doc.add_port(a, Side::Bottom, 0.5).unwrap(), e.doc.add_port(b, Side::Bottom, 0.5).unwrap());
        let id = e.doc.add_link(pa, pb, LinkStyle { kind: LinkKind::L3, ..Default::default() });
        e.doc.link_mut(id).unwrap().label = "10G".into();
        // Down, across, up: three pieces through two points.
        let p1 = e.doc.split_link(id, Pt::new(100.0, 400.0)).unwrap();
        let rest = e.doc.links.iter().find(|l| l.from == Endpoint::point(p1)).unwrap().id;
        let p2 = e.doc.split_link(rest, Pt::new(400.0, 400.0)).unwrap();
        let piece = |e: &Editor, from: Endpoint| e.doc.links.iter().find(|l| l.from == from).unwrap().id;
        let (first, middle, last) = (piece(&e, pa), piece(&e, Endpoint::point(p1)), piece(&e, Endpoint::point(p2)));
        let label_on = |e: &Editor| e.doc.links.iter().filter(|l| !l.label.is_empty()).map(|l| l.id).collect::<Vec<_>>();
        assert_eq!(label_on(&e), [first]);
        // Grab the label, drag it along the bottom to the last piece.
        let at = geom::label_spots(&e.doc)[&first].0;
        drag(&mut e, at, Pt::new(250.0, 400.0));
        assert_eq!(label_on(&e), [middle], "across the first point");
        let at = geom::label_spots(&e.doc)[&middle].0;
        drag(&mut e, at, Pt::new(400.0, 250.0));
        assert_eq!(label_on(&e), [last], "and the second");
        let spot = geom::label_spots(&e.doc)[&last].0;
        assert!((spot.x - 400.0).abs() < 1.0 && (spot.y - 250.0).abs() < 1.0, "{spot:?}");
        assert_eq!(e.selection, Selection::Link(last));
        // Not past the line's real end: it stays clear of the port.
        let at = geom::label_spots(&e.doc)[&last].0;
        drag(&mut e, at, Pt::new(400.0, 100.0));
        assert!(geom::label_spots(&e.doc)[&last].0.y > 140.0);
        // Undo puts it back where it was.
        e.undo();
        e.undo();
        assert_eq!(label_on(&e), [middle]);
        // A piece with a label of its own stops it.
        e.doc.link_mut(last).unwrap().label = "uplink".into();
        let at = geom::label_spots(&e.doc)[&middle].0;
        drag(&mut e, at, Pt::new(400.0, 250.0));
        assert_eq!(e.doc.link(middle).unwrap().label, "10G");
        assert_eq!(e.doc.link(last).unwrap().label, "uplink");
    }

    #[test]
    fn a_patch_panel_is_as_wide_as_its_ports() {
        let mut e = Editor::default();
        let id = e.add_device_of("pp", Pt::new(0.0, 0.0));
        let d = e.doc.device(id).unwrap();
        assert_eq!((d.kind.as_str(), d.name.as_str()), ("patch panel", "Patch panel 1"));
        let w24 = d.w;
        assert!(w24 > 300.0);
        e.selection = Selection::Items(vec![id]);
        assert!(e.set_prop("panel_ports", "12"));
        let w12 = e.doc.device(id).unwrap().w;
        assert!(w12 < w24);
        assert!(e.set_prop("connector", "lc"));
        assert!(e.doc.device(id).unwrap().w > w12);
        assert!(!e.set_prop("panel_ports", "0") && !e.set_prop("connector", "toslink"));
        let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!((sel["panel_ports"].as_u64(), sel["connector"].as_str()), (Some(12), Some("lc")));
        // Another type: back to the usual size; and a patch panel again.
        assert!(e.set_prop("kind", "router"));
        assert_eq!((e.doc.device(id).unwrap().w, e.doc.device(id).unwrap().h), (120.0, 60.0));
        assert!(e.set_prop("kind", "patch panel"));
        assert!(e.doc.device(id).unwrap().w > w12, "keeps its 12 LC ports");
        // Saved and read back.
        let back = Diagram::from_json(&e.doc.to_json()).unwrap();
        assert_eq!(back.device(id).unwrap().panel, Some(PatchPanel { ports: 12, connector: Connector::Lc, uplinks: 0 }));
        assert!(e.doc.to_json().contains("\"connector\": \"lc\""));
    }

    #[test]
    fn sub_layers_fold_and_follow_the_layer_they_are_under() {
        let (mut e, a, b) = two_devices();
        let servers = e.doc.active_layer;
        let sub = e.add_sublayer().unwrap();
        assert_eq!((e.doc.layer(sub).unwrap().parent, e.doc.active_layer), (servers, sub));
        e.doc.device_mut(b).unwrap().layer = sub;
        let other = e.add_layer();
        e.doc.device_mut(a).unwrap().layer = other;
        // Listed under it, folded away when it is not expanded.
        let rows = |e: &Editor| -> Vec<Value> { serde_json::from_str::<Value>(&e.layers_json()).unwrap().as_array().unwrap().clone() };
        let row = |e: &Editor, id: Id| rows(e).into_iter().find(|r| r["id"] == json!(id)).unwrap();
        let order: Vec<u64> = rows(&e).iter().map(|r| r["id"].as_u64().unwrap()).collect();
        assert_eq!(order, [servers as u64, sub as u64, other as u64]);
        assert_eq!((row(&e, sub)["depth"].as_u64(), row(&e, sub)["folded"].as_bool()), (Some(1), Some(false)));
        e.set_layer_expanded(servers, false);
        assert_eq!(row(&e, sub)["folded"].as_bool(), Some(true));
        assert_eq!(row(&e, servers)["children"].as_u64(), Some(1));
        // Hidden or locked with the layer above it.
        e.set_layer_visible(servers, false);
        assert!(!e.doc.layer_visible(sub) && !e.doc.is_visible(b) && e.doc.is_visible(a));
        assert_eq!((row(&e, sub)["visible"].as_bool(), row(&e, sub)["shown"].as_bool()), (Some(true), Some(false)));
        e.set_layer_visible(servers, true);
        e.set_layer_locked(servers, true);
        assert!(e.doc.layer_locked(sub) && !e.doc.is_editable(b));
        assert!(!e.remove_layer(servers), "locked");
        e.set_layer_locked(servers, false);
        // Removed with it, what is on it too; one step back.
        assert!(e.remove_layer(servers));
        assert!(e.doc.layer(sub).is_none() && e.doc.device(b).is_none() && e.doc.device(a).is_some());
        e.undo();
        assert!(e.doc.layer(sub).is_some_and(|l| l.parent == servers));
        // Saved and read back; a parent that is not there makes a top layer.
        let back = Diagram::from_json(&e.doc.to_json()).unwrap();
        assert_eq!(back.layer(sub).unwrap().parent, servers);
        let broken = e.doc.to_json().replace(&format!("\"parent\": {servers}"), "\"parent\": 999");
        assert_eq!(Diagram::from_json(&broken).unwrap().layer(sub).unwrap().parent, 0);
        // In draw.io, with the name of the layer above.
        let names = format!("value=\"{} / {}\"", e.doc.layer(servers).unwrap().name, e.doc.layer(sub).unwrap().name);
        assert!(e.export_drawio("x").contains(&names));
    }

    #[test]
    fn layers_move_before_after_and_into_others_as_deep_as_wanted() {
        let mut e = Editor::default();
        let a = e.doc.active_layer;
        let (b, c, d) = (e.add_layer(), e.add_layer(), e.add_layer());
        let names = |e: &Editor| -> Vec<String> {
            e.doc.layer_tree().iter().map(|(l, depth)| format!("{}{}", ".".repeat(*depth), l.name)).collect()
        };
        let (na, nb, nc, nd) = (e.doc.layer(a).unwrap().name.clone(), e.doc.layer(b).unwrap().name.clone(),
                                e.doc.layer(c).unwrap().name.clone(), e.doc.layer(d).unwrap().name.clone());
        // Before another: the order changes, in the list and for stacking.
        assert!(e.move_layer(d, a, "before"));
        assert_eq!(names(&e), [nd.clone(), na.clone(), nb.clone(), nc.clone()]);
        assert_eq!(e.doc.layers.iter().position(|l| l.id == d), Some(0));
        // Into: a sub-layer, the new parent unfolded; deeper and deeper.
        assert!(e.move_layer(c, b, "into") && e.doc.layer(b).unwrap().expanded);
        assert!(e.move_layer(a, c, "into"));
        assert_eq!(names(&e), [nd.clone(), nb.clone(), format!(".{nc}"), format!("..{na}")]);
        let mut deepest = a;
        for n in 0..100 {
            let id = e.doc.add_layer(&format!("deep {n}"));
            assert!(e.move_layer(id, deepest, "into"));
            deepest = id;
        }
        // 99 more, then the one it went into, and the two above that.
        assert_eq!(e.doc.layer_ancestors(deepest).len(), 102);
        assert_eq!(e.doc.layer_tree().last().map(|(l, depth)| (l.id, *depth)), Some((deepest, 102)));
        // Not into itself or a layer under it; the frame stays.
        assert!(!e.move_layer(b, a, "into") && !e.move_layer(b, b, "after"));
        // After a layer: with what is under it, back to the top.
        assert!(e.move_layer(c, d, "after"));
        assert_eq!(e.doc.layer(c).unwrap().parent, 0);
        assert_eq!(names(&e)[..3], [nd.clone(), nc.clone(), format!(".{na}")]);
        assert_eq!(e.doc.layer_ancestors(deepest).len(), 101, "the layers under it came along");
        // One undo step each.
        e.undo();
        assert_eq!(e.doc.layer(c).unwrap().parent, b);
        assert!(e.set_frame("template", "banner"));
        let frame = e.doc.frame_layer().unwrap();
        assert!(!e.move_layer(frame, d, "after") && !e.move_layer(d, frame, "into"));
    }

    #[test]
    fn a_sheet_copied_with_its_layers_and_frame_pastes_with_them() {
        let view = Rect { x: 0.0, y: 0.0, w: 800.0, h: 600.0 };
        // Two layers (one hidden), a frame, devices on both, a line.
        let (mut e, a, b) = two_devices();
        e.doc.device_mut(a).unwrap().source = "dcim.device:1".into();
        assert!(e.set_frame("template", "banner") && e.set_frame("title", "Berlin"));
        let hidden = e.doc.add_layer("Patch panels");
        e.doc.device_mut(b).unwrap().layer = hidden;
        e.doc.layer_mut(hidden).unwrap().visible = false;
        let (pa, pb) = (e.doc.add_port(a, Side::Right, 0.5).unwrap(), e.doc.add_port(b, Side::Left, 0.5).unwrap());
        e.doc.add_link(pa, pb, LinkStyle::default());
        let at = |e: &Editor, name: &str| e.doc.devices.iter().find(|d| d.name == name).map(|d| (d.x, d.y)).unwrap();
        let (name_a, name_b) = (e.doc.device(a).unwrap().name.clone(), e.doc.device(b).unwrap().name.clone());
        let text = e.copy_sheet();
        assert!(e.status.contains("with its layers and frame"), "{}", e.status);

        // Into an empty sheet, as text (from another window): as it was.
        let mut f = Editor::default();
        assert!(f.paste("", &text, view));
        let names: Vec<(&str, bool)> = f.doc.layers.iter().map(|l| (l.name.as_str(), l.visible)).collect();
        let source: Vec<(&str, bool)> = e.doc.layers.iter().map(|l| (l.name.as_str(), l.visible)).collect();
        assert_eq!(names, source, "the same layers, the hidden one hidden");
        let frame = f.doc.frame_layer().and_then(|id| f.doc.layer(id)).and_then(|l| l.frame.clone()).expect("the frame");
        assert_eq!(frame.title, "Berlin");
        assert_eq!((at(&f, &name_a), at(&f, &name_b)), (at(&e, &name_a), at(&e, &name_b)), "where it was");
        assert_eq!(f.doc.layer(f.doc.devices.iter().find(|d| d.name == name_b).unwrap().layer).unwrap().name, "Patch panels");
        assert_eq!((f.doc.links.len(), f.doc.devices[0].source.as_str()), (1, "dcim.device:1"));
        // Undone in one step.
        f.undo();
        assert!(f.doc.devices.is_empty() && f.doc.frame_layer().is_none());

        // Into a sheet with things and a frame of its own: beside them, on
        // the layers of the same names, keeping its frame.
        let mut g = Editor::default();
        g.add_device_at(Pt::new(0.0, 0.0));
        assert!(g.set_frame("template", "lines") && g.set_frame("title", "Mine"));
        let layers = g.doc.layers.len();
        assert!(g.paste("", &text, view));
        assert!(g.status.contains("keeps its own frame"), "{}", g.status);
        let title = g.doc.frame_layer().and_then(|id| g.doc.layer(id)).and_then(|l| l.frame.clone()).unwrap().title;
        assert_eq!(title, "Mine");
        assert_eq!(g.doc.layers.len(), layers + 1, "Patch panels added, Layer 1 shared");
        assert_eq!(g.doc.devices.len(), 3);
        assert!(g.doc.devices.iter().all(|d| d.source.is_empty()), "copies, not the NetBox devices");
        let first = g.doc.devices[0].rect();
        assert!(g.doc.devices[1..].iter().all(|d| d.x > first.x + first.w), "to the right of what is there");

        // An ordinary copy, as text: into the active layer, without the frame.
        let mut h = Editor::default();
        e.selection = Selection::None;
        let plain = e.copy_native();
        assert!(h.paste("", &plain, view));
        assert!(h.doc.frame_layer().is_none() && h.doc.layers.len() == 1);
    }

    #[test]
    fn a_switch_shows_its_ports_once_it_has_some() {
        let mut e = Editor::default();
        let sw = e.add_device_of("sw", Pt::new(0.0, 200.0));
        let d = e.doc.device(sw).unwrap();
        assert_eq!((d.kind.as_str(), d.w, d.h), ("switch", 120.0, 60.0), "a box at first");
        e.selection = Selection::Items(vec![sw]);
        let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(sel["panel_ports"].as_u64(), Some(0));
        assert!(!e.set_prop("panel_ports", "0"), "already a box");
        assert!(e.set_prop("panel_ports", "24") && e.set_prop("uplinks", "4"));
        let d = e.doc.device(sw).unwrap().clone();
        assert!(d.w > 250.0 && d.front().is_some_and(|f| f.switch));
        assert_eq!((d.socket_pos(28).is_some(), d.socket_pos(29)), (true, None), "24 ports and 4 uplinks");
        assert!(!e.set_prop("uplinks", "49"));
        let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!((sel["panel_ports"].as_u64(), sel["uplinks"].as_u64()), (Some(24), Some(4)));

        // Lines leave towards where they go: both of the first column down,
        // port 1's cord a little aside past port 2, so they stay apart.
        let other = e.add_device_at(Pt::new(200.0, 500.0));
        let o = e.doc.device(other).unwrap().rect();
        let target = Pt::new(o.x + o.w / 2.0, o.y + o.h / 2.0);
        let mut exits = vec![];
        for n in [1, 2] {
            let at = e.doc.device(sw).unwrap().socket_pos(n).unwrap();
            assert_eq!(e.hit_test(at), Hit::Socket { device: sw, socket: n });
            e.press(at, false, false);
            e.drag_to(Pt::new(at.x, at.y + 80.0));
            e.drag_to(target);
            e.release(target);
            let l = e.doc.links.last().unwrap().clone();
            let (exit, side) = e.doc.endpoint_pos(l.from).unwrap();
            let d = e.doc.device(sw).unwrap();
            assert_eq!((exit.y, side), (d.y + d.h, Side::Bottom), "port {n}");
            exits.push(exit.x - at.x);
        }
        assert_eq!(exits, vec![-3.5, 0.0]);
        // SFP ports all round; back to a box with 0 ports.
        e.selection = Selection::Items(vec![sw]);
        assert!(e.set_prop("connector", "sfp"));
        assert_eq!(e.doc.device(sw).unwrap().panel.unwrap().connector, Connector::Sfp);
        assert!(e.set_prop("panel_ports", "0"));
        let d = e.doc.device(sw).unwrap();
        assert_eq!((d.panel, d.w, d.h), (None, 120.0, 60.0));
        // Its ports keep their sockets, for when it shows its ports again.
        assert!(d.ports.iter().all(|p| p.socket > 0) && d.socket_pos(1).is_none());
    }

    #[test]
    fn routers_firewalls_and_servers_can_show_their_ports() {
        let mut e = Editor::default();
        for kind in ["router", "firewall", "server"] {
            let id = e.add_device_of(kind, Pt::new(0.0, 0.0));
            let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
            assert_eq!((sel["takes_ports"].as_bool(), sel["panel_ports"].as_u64()), (Some(true), Some(0)), "{kind}: a box at first");
            assert!(e.set_prop("panel_ports", "8") && e.set_prop("uplinks", "2"), "{kind}");
            let d = e.doc.device(id).unwrap();
            assert!(d.front().is_some_and(|f| f.switch && f.panel.ports == 8 && f.panel.uplinks == 2), "{kind}");
            assert!(d.w > 120.0 && d.socket_pos(10).is_some() && d.socket_pos(11).is_none(), "{kind}: sockets 1-10");
            // Back to a box.
            assert!(e.set_prop("panel_ports", "0"));
            assert!(e.doc.device(id).unwrap().front().is_none());
            assert_eq!(e.doc.device(id).unwrap().w, 120.0);
        }
        // Not for types without a front.
        e.add_device_of("load balancer", Pt::new(0.0, 0.0));
        let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(sel["takes_ports"].as_bool(), Some(false));
        assert!(!e.set_prop("panel_ports", "8"));
        // A switch with ports turned into a firewall keeps them.
        let sw = e.add_device_of("switch", Pt::new(0.0, 0.0));
        assert!(e.set_prop("panel_ports", "24"));
        assert!(e.set_prop("kind", "firewall"));
        assert!(e.doc.device(sw).unwrap().front().is_some_and(|f| f.panel.ports == 24));
    }

    #[test]
    fn virtual_switches_are_device_types() {
        let mut e = Editor::default();
        assert_eq!(e.quick_add("vswitch, dvs, vds|dvs-prod", Pt::new(0.0, 0.0)).len(), 3);
        let kinds: Vec<(&str, &str)> = e.doc.devices.iter().map(|d| (d.name.as_str(), d.kind.as_str())).collect();
        assert_eq!(kinds, [("vswitch", "virtual switch"), ("dvs", "distributed virtual switch"), ("dvs-prod", "distributed virtual switch")]);
        // Added from the type list: named after the type.
        let v = e.add_device_of("virtual switch", Pt::new(0.0, 300.0));
        assert_eq!(e.doc.device(v).unwrap().name, "vSwitch 1");
        assert_eq!(kind_color("virtual switch"), Some("#cfe0fd"));
        assert_eq!(crate::layout::kind_rank("distributed virtual switch"), crate::layout::kind_rank("switch"));
        let o = crate::netbox::Options::default();
        assert_eq!(crate::netbox::kind_for_role("vswitch", "vSwitch", &o), "virtual switch");
        assert_eq!(crate::netbox::kind_for_role("dvs", "Distributed vSwitch", &o), "distributed virtual switch");
    }

    /// All crossings of the shown lines of `e` with each other.
    fn line_crossings(e: &Editor) -> usize {
        let routes: Vec<Vec<Pt>> = e.doc.links.iter().filter_map(|l| geom::link_route(&e.doc, l)).collect();
        (0..routes.len()).flat_map(|i| (i + 1..routes.len()).map(move |j| (i, j))).map(|(i, j)| geom::route_crossings(&routes[i], &routes[j])).sum()
    }

    #[test]
    fn crossing_lines_are_untangled_by_swapping_devices() {
        // Two above, two below, cabled across: A to D, B to C.
        let mut e = Editor::default();
        let dev = |e: &mut Editor, x: f64, y: f64| e.doc.add_device("d", "server", Rect { x, y, w: 100.0, h: 40.0 }, "#eee");
        let (a, b, c, d) = (dev(&mut e, 0.0, 0.0), dev(&mut e, 200.0, 0.0), dev(&mut e, 0.0, 200.0), dev(&mut e, 200.0, 200.0));
        for (from, to) in [(a, d), (b, c)] {
            let (p, q) = (e.doc.add_port(from, Side::Bottom, 0.5).unwrap(), e.doc.add_port(to, Side::Top, 0.5).unwrap());
            e.doc.add_link(p, q, LinkStyle::default());
        }
        assert_eq!(line_crossings(&e), 1);
        assert_eq!(e.untangle(&[a, b, c, d]), 1);
        assert_eq!(line_crossings(&e), 0);
        let xs: Vec<f64> = [a, b, c, d].iter().map(|id| e.doc.device(*id).unwrap().x).collect();
        assert!(xs[0] != 0.0 || xs[2] != 0.0, "two of them swapped: {xs:?}");
        // Nothing to untangle: nothing moves.
        let before = e.doc.clone();
        assert_eq!(e.untangle(&[a, b, c, d]), 0);
        assert_eq!(e.doc, before);
    }

    #[test]
    fn a_line_leaves_its_socket_the_way_that_crosses_nothing() {
        // A 24-port switch; a server above it cabled to socket 1, its line
        // leaving the socket downward (through the switch to get up), and
        // a server below cabled to socket 2.
        let mut e = Editor::default();
        let sw = e.doc.add_device("sw", "switch", Rect { x: 0.0, y: 200.0, w: 100.0, h: 40.0 }, "#eee");
        {
            let d = e.doc.device_mut(sw).unwrap();
            d.panel = Some(PatchPanel { ports: 24, ..Default::default() });
            d.fit_panel();
        }
        let up = e.doc.add_device("up", "server", Rect { x: 0.0, y: 0.0, w: 100.0, h: 40.0 }, "#eee");
        let down = e.doc.add_device("down", "server", Rect { x: 0.0, y: 400.0, w: 100.0, h: 40.0 }, "#eee");
        let socket = |e: &mut Editor, n: u32, side: Side| {
            let ep = e.doc.add_port(sw, side, 0.5).unwrap();
            let p = e.doc.device_mut(sw).unwrap().ports.iter_mut().find(|p| p.id == ep.port).unwrap();
            (p.socket, p.side) = (n, side);
            ep
        };
        let ortho = LinkStyle { routing: Routing::Orthogonal, ..Default::default() };
        let s1 = socket(&mut e, 1, Side::Bottom);
        let u = e.doc.add_port(up, Side::Bottom, 0.5).unwrap();
        let l = e.doc.add_link(u, s1, ortho.clone());
        let s2 = socket(&mut e, 2, Side::Bottom);
        let v = e.doc.add_port(down, Side::Top, 0.5).unwrap();
        e.doc.add_link(s2, v, ortho);
        let through = |e: &Editor| geom::crosses(&geom::link_route(&e.doc, e.doc.link(l).unwrap()).unwrap(), &[e.doc.device(sw).unwrap().rect()]);
        assert!(through(&e), "at first through the switch");
        e.untangle(&[sw, up, down]);
        assert_eq!(e.doc.device(sw).unwrap().port(s1.port).unwrap().side, Side::Top, "leaves upward");
        assert!(!through(&e));
        assert_eq!(e.doc.link(l).unwrap().style.routing, Routing::Orthogonal, "still orthogonal");
        assert_eq!(line_crossings(&e), 0);
    }

    /// A switch cabled through socket 1 of a patch panel to a server: the
    /// switch's Gi1/0/1 to the socket's front, its back to the server's eth0.
    fn patched_line() -> (Editor, Id, Id, [Endpoint; 4]) {
        let mut e = Editor::default();
        let sw = e.add_device_at(Pt::new(0.0, 0.0));
        let pp = e.add_device_of("pp", Pt::new(0.0, 300.0));
        let srv = e.add_device_at(Pt::new(0.0, 600.0));
        e.doc.device_mut(sw).unwrap().name = "sw-01".into();
        e.doc.device_mut(srv).unwrap().name = "srv-01".into();
        let mut port = |dev: Id, side: Side, name: &str, socket: u32, rear: bool| {
            let ep = e.doc.add_port(dev, side, 0.5).unwrap();
            let p = e.doc.device_mut(dev).unwrap().ports.iter_mut().find(|p| p.id == ep.port).unwrap();
            (p.name, p.socket, p.rear) = (name.into(), socket, rear);
            ep
        };
        let ends = [
            port(sw, Side::Bottom, "Gi1/0/1", 0, false),
            port(pp, Side::Top, "", 1, false),
            port(pp, Side::Bottom, "", 1, true),
            port(srv, Side::Top, "eth0", 0, false),
        ];
        let l2 = LinkStyle { kind: LinkKind::L2, ..Default::default() };
        let front = e.doc.add_link(ends[0], ends[1], l2.clone());
        e.doc.link_mut(front).unwrap().label = "patch".into();
        let back = e.doc.add_link(ends[2], ends[3], l2);
        (e, front, back, ends)
    }

    #[test]
    fn a_selected_line_lights_up_the_ports_at_its_ends() {
        let (mut e, front, _, ends) = patched_line();
        e.selection = Selection::Link(front);
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let port = |ep: Endpoint| {
            let d = scene["devices"].as_array().unwrap().iter().find(|d| d["id"] == ep.device).unwrap();
            d["ports"].as_array().unwrap().iter().find(|p| p["id"] == ep.port).unwrap().clone()
        };
        let lit: Vec<bool> = ends.iter().map(|ep| port(*ep)["lit"] == true).collect();
        assert_eq!(lit, [true, true, false, false], "the switch port and the socket's front, not the other line's");
        assert_eq!(port(ends[0])["sel"], false, "lit, not selected");
        e.selection = Selection::None;
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert!(scene["devices"].as_array().unwrap().iter().flat_map(|d| d["ports"].as_array().unwrap()).all(|p| p["lit"] == false));
    }

    #[test]
    fn a_line_through_hidden_devices_can_be_selected() {
        let (mut e, front, _, ends) = patched_line();
        let pp = ends[1].device;
        let panels = e.doc.add_layer("Patch panels");
        e.doc.device_mut(pp).unwrap().layer = panels;
        e.set_layer_visible(panels, false);
        // The line drawn straight from the switch to the server, where the
        // panel would be.
        let (a, b) = (e.doc.endpoint_pos(ends[0]).unwrap().0, e.doc.endpoint_pos(ends[3]).unwrap().0);
        let mid = Pt::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0);
        assert_eq!(e.hit_test(mid), Hit::Through(front), "the first cable it stands for");
        e.hover(mid);
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert_eq!(
            scene["tip"]["lines"],
            json!(["sw-01 Gi1/0/1  ⟷  srv-01 eth0", "Through hidden devices (2 cables)", "L2 switched"])
        );
        // Clicked: selected (not bent), drawn selected with both ends lit.
        e.press(mid, false, false);
        e.drag_to(Pt::new(mid.x + 40.0, mid.y));
        e.release(Pt::new(mid.x + 40.0, mid.y));
        assert_eq!(e.selection, Selection::Link(front));
        assert!(e.doc.points.is_empty(), "not bent");
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert!(scene["links"].as_array().unwrap().iter().any(|l| l["id"] == front && l["sel"] == true));
        let lit = |ep: Endpoint| {
            let d = scene["devices"].as_array().unwrap().iter().find(|d| d["id"] == ep.device).unwrap();
            d["ports"].as_array().unwrap().iter().find(|p| p["id"] == ep.port).unwrap()["lit"] == true
        };
        assert!(lit(ends[0]) && lit(ends[3]), "the switch's and the server's ports");
        // Its type set in the panel: the line shown changes with it.
        assert!(e.set_prop("kind", "trunk"));
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert!(scene["links"].as_array().unwrap().iter().any(|l| l["id"] == front && l["kind"] == "trunk"));
    }

    #[test]
    fn hovering_a_line_tells_what_it_connects() {
        let (mut e, front, back, _) = patched_line();
        assert_eq!(
            e.link_tip(front),
            ["sw-01 Gi1/0/1  ⟷  Patch panel 1 socket 1", "End to end (2 cables): sw-01 Gi1/0/1  ⟷  srv-01 eth0", "L2 switched · patch"]
        );
        assert_eq!(e.link_tip(back)[0], "Patch panel 1 back of socket 1  ⟷  srv-01 eth0");
        // In the scene while the pointer is on the line, next to it.
        let (a, b) = (e.doc.endpoint_pos(e.doc.link(front).unwrap().from).unwrap().0, e.doc.endpoint_pos(e.doc.link(front).unwrap().to).unwrap().0);
        let mid = Pt::new((a.x + b.x) / 2.0, (a.y + b.y) / 2.0 - 30.0);
        e.hover(mid);
        assert_eq!(e.hover, Hit::Link(front));
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert_eq!((&scene["tip"]["x"], &scene["tip"]["lines"][0]), (&json!(mid.x), &json!(e.link_tip(front)[0])));
        // Not once the pointer has left the canvas.
        e.hover_out();
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert!(scene["tip"].is_null());
        // A line without anything beyond: its ends and its type.
        let (mut e, a, b) = two_devices();
        let (pa, pb) = (e.doc.add_port(a, Side::Right, 0.5).unwrap(), e.doc.add_port(b, Side::Left, 0.5).unwrap());
        let l = e.doc.add_link(pa, pb, LinkStyle::default());
        assert_eq!(e.link_tip(l), ["Device 1  ⟷  Device 2", "Link"]);
    }

    #[test]
    fn a_line_under_a_socket_is_its_rear_port() {
        let mut e = Editor::default();
        let pp = e.add_device_of("pp", Pt::new(0.0, 0.0));
        let sw = e.add_device_at(Pt::new(100.0, -300.0));
        let srv = e.add_device_at(Pt::new(100.0, 300.0));
        let d = e.doc.device(pp).unwrap().clone();
        let s3 = d.socket_pos(3).unwrap();
        let under = Pt::new(s3.x + 1.0, d.y + d.h);
        // From the server to the panel's bottom edge under socket 3: its back.
        let sr = e.doc.device(srv).unwrap().rect();
        let from = Pt::new(sr.x + sr.w / 2.0, sr.y);
        e.press(from, false, false);
        e.drag_to(Pt::new(from.x, from.y - 60.0));
        e.drag_to(under);
        // Before it is dropped: the socket's tie shows, and the line ends under it.
        assert_eq!(e.pending_rear(), Some((pp, 3, false)));
        let scene: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let shown = scene["devices"].as_array().unwrap().iter().find(|x| x["id"] == pp).unwrap().clone();
        let local = Rect { x: d.x, y: d.y, w: d.w / d.scale, h: d.h / d.scale };
        let mut bare = crate::shape::device_shape(&d.kind, &d.name, &local, false, d.panel.as_ref());
        assert_ne!(shown["solid"], json!(bare.solid), "the tie is drawn");
        crate::shape::add_cords(&mut bare, d.front().as_ref(), &local, &[(3, false, true)]);
        assert_eq!(shown["solid"], json!(bare.solid));
        assert_eq!(scene["ghost"]["x"], json!(s3.x), "snapped under the socket");
        assert_eq!((&scene["socketLit"]["x"], &scene["socketLit"]["y"]), (&json!(s3.x), &json!(s3.y)), "the socket lit up");
        let pts = scene["preview"]["pts"].as_array().unwrap();
        assert_eq!(pts[pts.len() - 2], json!(s3.x));
        e.release(under);
        assert_eq!(e.pending_rear(), None);
        let back = e.doc.links.last().unwrap().to;
        let p = e.doc.device(pp).unwrap().port(back.port).unwrap().clone();
        assert_eq!((p.rear, p.socket), (true, 3));
        let d = e.doc.device(pp).unwrap();
        assert_eq!(e.doc.endpoint_pos(back), Some((Pt::new(s3.x, d.y + d.h), Side::Bottom)), "right under the socket");
        assert_eq!(e.doc.panel_cords(d), vec![(3, false, true)], "a tie from the socket down");
        assert!(!d.name_shown(&Port { name: "R3".into(), ..p.clone() }));
        // The switch's cable in socket 3 at the front: out at the top, clear of it.
        let w = e.doc.device(sw).unwrap().rect();
        let top = Pt::new(w.x + w.w / 2.0, w.y + w.h);
        e.press(top, false, false);
        e.drag_to(Pt::new(top.x, top.y + 60.0));
        e.drag_to(s3);
        e.release(s3);
        let front = e.doc.links.last().unwrap().to;
        let d = e.doc.device(pp).unwrap();
        let fp = d.port(front.port).unwrap();
        assert_eq!((fp.socket, fp.rear, d.port_side(fp)), (3, false, Side::Top));
        assert_eq!(d.socket_side(3, Pt::new(0.0, 1000.0)), Side::Top, "even towards something below");
        // A rear port on the top edge (its cable from above): the front cable
        // of that socket then leaves at the bottom.
        let s7 = d.socket_pos(7).unwrap();
        let t7 = (s7.x - d.x) / d.w;
        let ep = e.doc.add_port(pp, Side::Top, t7).unwrap();
        e.doc.settle_rear(ep);
        let d = e.doc.device(pp).unwrap();
        let r7 = d.port(ep.port).unwrap();
        assert_eq!((r7.rear, r7.socket, d.port_pos(r7)), (true, 7, Pt::new(s7.x, d.y)));
        assert_eq!(d.socket_side(7, Pt::new(0.0, -1000.0)), Side::Bottom);
        // Dragged along the edge away from the sockets: an ordinary port again.
        // (Between two groups of sockets, under none of them.)
        let right = Pt::new((d.socket_pos(6).unwrap().x + d.socket_pos(7).unwrap().x) / 2.0, d.y + d.h);
        let at = e.doc.endpoint_pos(back).unwrap().0;
        e.press(at, false, false);
        e.drag_to(Pt::new(at.x + 5.0, at.y));
        e.drag_to(right);
        e.release(right);
        let p = e.doc.device(pp).unwrap().port(back.port).unwrap();
        assert_eq!((p.rear, p.socket), (false, 0));
    }

    #[test]
    fn cabling_by_hand_through_sockets_stays_end_to_end() {
        // A switch, into socket 5 of pp-a, from its back to the back of
        // socket 1 of pp-b, from its front to a PC; drawn by hand, no traces.
        let mut d = Diagram::default();
        let r = |x: f64, y: f64| Rect { x, y, w: 120.0, h: 60.0 };
        let sw = d.add_device("sw", "switch", r(0.0, 0.0), "#dbeafe");
        let pa = d.add_device("pp-a", "patch panel", r(0.0, 200.0), "#e2e8f0");
        let pb = d.add_device("pp-b", "patch panel", r(0.0, 400.0), "#e2e8f0");
        let pc = d.add_device("pc", "client", r(0.0, 600.0), "#fef3c7");
        for id in [pa, pb] {
            d.device_mut(id).unwrap().fit_panel();
        }
        let port = |d: &mut Diagram, dev: Id, socket: u32, rear: bool, side: Side| {
            let ep = d.add_port(dev, side, 0.5).unwrap();
            let p = d.device_mut(dev).unwrap().ports.iter_mut().find(|p| p.id == ep.port).unwrap();
            (p.socket, p.rear) = (socket, rear);
            ep
        };
        let (s1, a5, a5r) = (d.add_port(sw, Side::Bottom, 0.5).unwrap(), port(&mut d, pa, 5, false, Side::Top), port(&mut d, pa, 5, true, Side::Bottom));
        let (b1r, b1, p1) = (port(&mut d, pb, 1, true, Side::Top), port(&mut d, pb, 1, false, Side::Bottom), d.add_port(pc, Side::Top, 0.5).unwrap());
        d.add_link(s1, a5, LinkStyle { kind: LinkKind::L2, ..Default::default() });
        d.add_link(a5r, b1r, LinkStyle::default());
        d.add_link(b1, p1, LinkStyle::default());
        assert!(d.traces.is_empty() && d.through_links().is_empty(), "all shown: just the cables");
        let ends = |d: &Diagram| -> Vec<(Endpoint, Endpoint)> { d.through_links().iter().map(|l| (l.from, l.to)).collect() };
        // Both panels hidden: switch to PC, looking like the switch's line.
        let panels = d.add_layer("Patch panels");
        for id in [pa, pb] {
            d.device_mut(id).unwrap().layer = panels;
        }
        d.layer_mut(panels).unwrap().visible = false;
        assert_eq!(ends(&d), [(s1, p1)]);
        assert_eq!(d.through_links()[0].style.kind, LinkKind::L2);
        assert_eq!(d.visible_only().links.len(), 1);
        // Only pp-b hidden: from pp-a's back on to the PC.
        d.layer_mut(panels).unwrap().visible = true;
        let own = d.add_layer("pp-b");
        d.device_mut(pb).unwrap().layer = own;
        d.layer_mut(own).unwrap().visible = false;
        assert_eq!(ends(&d), [(a5r, p1)]);
        // The cables hidden: nothing.
        let first = d.layers.iter().find(|l| l.frame.is_none()).unwrap().id;
        d.layer_mut(first).unwrap().visible = false;
        assert!(ends(&d).is_empty());
    }

    #[test]
    fn lines_plug_into_patch_panel_sockets() {
        let mut e = Editor::default();
        let pp = e.add_device_of("pp", Pt::new(0.0, 0.0));
        e.selection = Selection::Items(vec![pp]);
        assert!(e.set_prop("connector", "lc"));
        let sw = e.add_device_at(Pt::new(200.0, 300.0));
        let d = e.doc.device(pp).unwrap().clone();
        // Socket 3: the middle of the duplex adapter, between its halves.
        let s3 = d.socket_pos(3).unwrap();
        let sk = &crate::shape::panel_sockets(&d.front().unwrap(), &d.rect())[2];
        assert_eq!((s3.x, s3.y), crate::shape::socket_center(sk));
        assert!(s3.y > d.y + 34.0 && s3.y < d.y + d.h);
        assert_eq!(d.socket_at(s3, 0.0), Some(3));
        assert_eq!(e.hit_test(s3), Hit::Socket { device: pp, socket: 3 });

        // A line drawn from the socket to the switch below.
        let swc = e.doc.device(sw).unwrap().rect();
        e.press(s3, false, false);
        e.drag_to(Pt::new(s3.x, s3.y + 100.0));
        e.drag_to(Pt::new(swc.x + swc.w / 2.0, swc.y + swc.h / 2.0));
        e.release(Pt::new(swc.x + swc.w / 2.0, swc.y + swc.h / 2.0));
        let l = e.doc.links.last().expect("a line").clone();
        let port = e.doc.device(pp).unwrap().port(l.from.port).unwrap().clone();
        assert_eq!(port.socket, 3);
        // Its line leaves from the panel's edge right below, towards the
        // switch; the cord on the panel runs from the socket there.
        let bottom = d.y + d.h;
        assert_eq!(e.doc.endpoint_pos(l.from), Some((Pt::new(s3.x, bottom), Side::Bottom)));
        assert_eq!(e.doc.panel_cords(e.doc.device(pp).unwrap()), vec![(3, false, false)]);
        // Now a port: clicked, not a new line.
        assert!(matches!(e.hit_test(s3), Hit::Port(ep) if ep == l.from));

        // Dragged to socket 7, it moves there with its line.
        let s7 = e.doc.device(pp).unwrap().socket_pos(7).unwrap();
        e.press(s3, false, false);
        e.drag_to(Pt::new((s3.x + s7.x) / 2.0, s7.y));
        e.drag_to(s7);
        e.release(s7);
        assert_eq!(e.doc.device(pp).unwrap().port(l.from.port).unwrap().socket, 7);
        assert_eq!(e.doc.endpoint_pos(l.from).unwrap().0, Pt::new(s7.x, bottom));

        // A line drawn onto a socket from the switch.
        let s1 = e.doc.device(pp).unwrap().socket_pos(1).unwrap();
        let from = Pt::new(swc.x + swc.w, swc.y + swc.h / 2.0);
        e.press(from, false, false);
        e.drag_to(Pt::new(from.x, from.y - 50.0));
        e.drag_to(s1);
        e.release(s1);
        let l2 = e.doc.links.last().unwrap().clone();
        assert_eq!(e.doc.device(pp).unwrap().port(l2.to.port).unwrap().socket, 1);
        assert_eq!(e.doc.endpoint_pos(l2.to).unwrap().0, Pt::new(s1.x, bottom));

        // Saved and read back; fewer ports: back on the border.
        let back = Diagram::from_json(&e.doc.to_json()).unwrap();
        assert_eq!(back.device(pp).unwrap().port(l.from.port).unwrap().socket, 7);
        e.selection = Selection::Items(vec![pp]);
        assert!(e.set_prop("panel_ports", "4"));
        let d = e.doc.device(pp).unwrap();
        let p7 = d.port(l.from.port).unwrap();
        assert_eq!(d.port_pos(p7), d.point_on_side(p7.side, p7.t));
    }

    #[test]
    fn ctrl_lines_a_label_up_with_other_items() {
        let (mut e, a, b) = two_devices();
        let (pa, pb) = (e.doc.add_port(a, Side::Bottom, 0.5).unwrap(), e.doc.add_port(b, Side::Bottom, 0.5).unwrap());
        let id = e.doc.add_link(pa, pb, LinkStyle::default()); // along y = 130
        e.doc.link_mut(id).unwrap().label = "10G".into();
        let c = e.add_device_at(Pt::new(260.0, 300.0));
        let cx = e.doc.device(c).unwrap().rect().x + e.doc.device(c).unwrap().w / 2.0;
        let ctrl_drag = |e: &mut Editor, to: Pt| {
            let at = geom::label_spots(&e.doc)[&id].0;
            e.press(at, false, true);
            e.drag_to(Pt::new((at.x + to.x) / 2.0, to.y));
            e.drag_to(to);
            let guides = e.guides.len();
            e.release(to);
            guides
        };
        // Near the device's center: pulled onto it, with a guide.
        assert_eq!(ctrl_drag(&mut e, Pt::new(cx + 5.0, 140.0)), 1);
        assert!((geom::label_spots(&e.doc)[&id].0.x - cx).abs() < 0.01);
        assert_eq!(e.selection, Selection::Link(id));
        // Nothing near: it stays where it was dropped.
        assert_eq!(ctrl_drag(&mut e, Pt::new(cx + 30.0, 140.0)), 0);
        assert!((geom::label_spots(&e.doc)[&id].0.x - cx - 30.0).abs() < 0.01);
        // Without Ctrl, no pull.
        let at = geom::label_spots(&e.doc)[&id].0;
        drag(&mut e, at, Pt::new(cx + 5.0, 140.0));
        assert!((geom::label_spots(&e.doc)[&id].0.x - cx - 5.0).abs() < 0.01);
        // A Ctrl+click still toggles the line in the selection.
        e.selection = Selection::Items(vec![c]);
        let at = geom::label_spots(&e.doc)[&id].0;
        e.press(at, false, true);
        e.release(at);
        assert_eq!(e.selected_ids().len(), 2);
    }

    fn drag(e: &mut Editor, from: Pt, to: Pt) {
        e.press(from, false, false);
        e.drag_to(Pt::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0));
        e.drag_to(to);
        e.release(to);
    }

    #[test]
    fn drag_from_edge_to_device_creates_ports_and_link() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        assert_eq!(e.doc.links.len(), 1);
        let l = &e.doc.links[0];
        assert_eq!(l.from.device, a);
        assert_eq!(l.to.device, b);
        assert_eq!(e.doc.device(a).unwrap().ports[0].side, Side::Right);
        assert_eq!(e.doc.device(b).unwrap().ports[0].side, Side::Left);
        assert!(matches!(e.selection, Selection::Link(_)));
        // One undo step removes link and both new ports.
        e.undo();
        assert!(e.doc.links.is_empty());
        assert!(e.doc.devices.iter().all(|d| d.ports.is_empty()));
        e.redo();
        assert_eq!(e.doc.links.len(), 1);
    }

    #[test]
    fn drag_from_port_to_port() {
        let (mut e, a, b) = two_devices();
        e.selection = Selection::Items(vec![a]);
        e.add_port(Side::Right);
        e.selection = Selection::Items(vec![b]);
        e.add_port(Side::Left);
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(340.0, 100.0));
        assert_eq!(e.doc.links.len(), 1);
        assert_eq!(e.doc.device(a).unwrap().ports.len(), 1);
        assert_eq!(e.doc.device(b).unwrap().ports.len(), 1);
    }

    #[test]
    fn short_slip_onto_empty_space_does_nothing() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(170.0, 105.0));
        assert!(e.doc.links.is_empty() && e.doc.points.is_empty());
        assert!(e.doc.devices.iter().all(|d| d.ports.is_empty()));
        assert_eq!(e.undo.len(), 2); // only the two add_device steps
    }

    #[test]
    fn link_dropped_on_empty_space_ends_at_a_connection_point() {
        let (mut e, a, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(252.0, 297.0));
        let Selection::Link(link) = e.selection else { panic!("no link") };
        let l = e.doc.link(link).unwrap().clone();
        assert_eq!(l.from.device, a);
        assert!(l.to.is_point());
        let q = e.doc.point(l.to.owner()).unwrap().clone();
        assert_eq!((q.x, q.y), (250.0, 300.0), "snapped to the grid");
        // The line ends exactly at the point, leaving the device toward it.
        let route = geom::link_route(&e.doc, &l).unwrap();
        assert_eq!(*route.last().unwrap(), q.pos());
        // Dragging the point moves the line end; one undo step each.
        drag(&mut e, q.pos(), Pt::new(300.0, 300.0));
        assert_eq!(e.doc.point(q.id).unwrap().x, 300.0);
        assert_eq!(*geom::link_route(&e.doc, &l).unwrap().last().unwrap(), Pt::new(300.0, 300.0));
        // Shift+drag from the point draws a second line to a new point.
        e.press(Pt::new(300.0, 300.0), true, false);
        e.drag_to(Pt::new(350.0, 350.0));
        e.release(Pt::new(400.0, 400.0));
        assert_eq!(e.doc.links.len(), 2);
        assert_eq!(e.doc.points.len(), 2);
        // Lines can also be dropped onto an existing point.
        drag(&mut e, Pt::new(340.0, 100.0), Pt::new(301.0, 301.0));
        assert_eq!(e.doc.links.len(), 3);
        assert_eq!(e.doc.points.len(), 2);
        // Deleting a point takes its lines.
        e.selection = Selection::Items(vec![q.id]);
        e.delete_selected();
        assert_eq!(e.doc.links.len(), 0);
        // Saved as {"point": id}, and loads back.
        e.undo();
        let json = e.doc.to_json();
        assert!(json.contains(&format!(r#""point": {}"#, q.id)), "{json}");
        let d = Diagram::from_json(&json).unwrap();
        assert_eq!(d.links.len(), 3);
        assert_eq!(d.points, e.doc.points);
    }

    #[test]
    fn connection_points_duplicate_hide_and_align() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(250.0, 300.0));
        let Selection::Link(link) = e.selection else { panic!() };
        let q = e.doc.link(link).unwrap().to.owner();
        // Duplicating a device with its point copies the line too.
        e.selection = Selection::Items(vec![a, q]);
        e.duplicate();
        assert_eq!(e.doc.points.len(), 2);
        assert_eq!(e.doc.links.len(), 2);
        let copy = e.doc.links.iter().find(|l| l.id != link).unwrap();
        assert!(copy.to.is_point() && copy.to.owner() != q);
        // On a hidden layer the point, and its line, disappear.
        let l2 = e.add_layer();
        e.selection = Selection::Items(vec![q]);
        e.set_prop("layer", &l2.to_string());
        e.set_layer_visible(l2, false);
        assert!(!e.doc.is_visible(link));
        assert!(!crate::svg::export(&e.doc, "").contains("polyline points=\"160.0,100.0"));
        e.set_layer_visible(l2, true);
        // Ctrl+drag lines a point up with a port, so the line runs straight.
        let port = e.doc.link(link).unwrap().from;
        let port_y = e.doc.endpoint_pos(port).unwrap().0.y;
        e.snap = false;
        e.align = true;
        e.press(Pt::new(250.0, 300.0), false, true);
        e.drag_to(Pt::new(260.0, 200.0));
        e.drag_to(Pt::new(270.0, port_y + 3.0));
        e.release(Pt::new(270.0, port_y + 3.0));
        assert_eq!(e.doc.point(q).unwrap().y, port_y);
        // A point can be placed exactly from the panel.
        e.selection = Selection::Items(vec![q]);
        assert!(e.set_prop("x", "500"));
        assert!(!e.set_prop("x", "far"));
        assert_eq!(e.doc.point(q).unwrap().x, 500.0);
        let sel: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!((sel["type"].as_str(), sel["links"].as_u64()), (Some("point"), Some(1)));
        let _ = b;
    }

    #[test]
    fn move_device_snaps_and_is_one_undo_step() {
        let (mut e, a, _) = two_devices();
        let undo_before = e.undo.len();
        drag(&mut e, Pt::new(100.0, 100.0), Pt::new(133.0, 147.0));
        let d = e.doc.device(a).unwrap();
        assert_eq!((d.x, d.y), (70.0, 120.0));
        assert_eq!(e.undo.len(), undo_before + 1);
    }

    #[test]
    fn delete_device_removes_links() {
        let (mut e, a, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        e.selection = Selection::Items(vec![a]);
        e.delete_selected();
        assert!(e.doc.links.is_empty());
        assert_eq!(e.doc.devices.len(), 1);
    }

    #[test]
    fn link_props_and_defaults() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        assert!(e.set_prop("head_end", "arrow"));
        assert!(e.set_prop("style", "dashed"));
        assert!(!e.set_prop("style", "wavy"));
        assert!(e.set_prop("label", "10G"));
        let l = &e.doc.links[0];
        assert_eq!(l.style.head_end, Head::Arrow);
        assert_eq!(l.label, "10G");
        // New links inherit the last style.
        assert_eq!(e.link_defaults.head_end, Head::Arrow);
    }

    #[test]
    fn typing_coalesces_into_one_undo_step() {
        let (mut e, _, _) = two_devices();
        let n = e.undo.len();
        for s in ["c", "co", "cor", "core"] {
            e.set_prop("name", s);
        }
        assert_eq!(e.undo.len(), n + 1);
    }

    #[test]
    fn duplicate_copies_internal_links() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        e.selection = Selection::Items(vec![a, b]);
        e.duplicate();
        assert_eq!(e.doc.devices.len(), 4);
        assert_eq!(e.doc.links.len(), 2);
        assert_eq!(e.doc.devices[2].name, "Device 3");
    }

    #[test]
    fn saved_files_get_the_stratagram_extension_if_none_is_given() {
        assert_eq!(with_file_ext(Path::new("/tmp/site")), PathBuf::from("/tmp/site.stratagram"));
        assert_eq!(with_file_ext(Path::new("/tmp/site.stratagram")), PathBuf::from("/tmp/site.stratagram"));
        assert_eq!(with_file_ext(Path::new("/tmp/old.json")), PathBuf::from("/tmp/old.json"), "kept as chosen");
    }

    #[test]
    fn save_load_roundtrip() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        e.set_prop("head_start", "diamond");
        let dir = std::env::temp_dir().join(format!("stratagram-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("t.stratagram");
        e.save(&path).unwrap();
        let mut f = Editor::default();
        f.load(&path).unwrap();
        assert_eq!(f.doc.devices, e.doc.devices);
        assert_eq!(f.doc.links, e.doc.links);
        // New ids do not collide with loaded ones.
        let id = f.add_device_at(Pt::new(0.0, 0.0));
        assert!(e.doc.devices.iter().all(|d| d.id != id));
        std::fs::remove_dir_all(dir).ok();
    }

    #[test]
    fn rubber_band_selects_contained_devices() {
        let (mut e, a, b) = two_devices();
        drag(&mut e, Pt::new(0.0, 0.0), Pt::new(200.0, 200.0));
        assert_eq!(e.selection, Selection::Items(vec![a]));
        drag(&mut e, Pt::new(0.0, 0.0), Pt::new(500.0, 200.0));
        assert_eq!(e.selection, Selection::Items(vec![a, b]));
    }

    #[test]
    fn scene_json_is_valid() {
        let (mut e, _, _) = two_devices();
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let v: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert_eq!(v["devices"].as_array().unwrap().len(), 2);
        assert_eq!(v["links"].as_array().unwrap().len(), 1);
        let s: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(s["type"], "link");
    }

    #[test]
    fn dragging_port_along_border_moves_it() {
        let (mut e, a, _) = two_devices(); // a: 40..160 x 70..130
        e.selection = Selection::Items(vec![a]);
        e.add_port(Side::Right); // at (160, 100)
        let undo_before = e.undo.len();
        // Slide down the right border, then around the corner onto the bottom.
        e.press(Pt::new(160.0, 100.0), false, false);
        e.drag_to(Pt::new(161.0, 112.0));
        e.drag_to(Pt::new(162.0, 121.0));
        let p = &e.doc.device(a).unwrap().ports[0];
        assert_eq!(p.side, Side::Right);
        assert!((p.t - 50.0 / 60.0).abs() < 1e-9, "snapped to y=120, got t={}", p.t);
        e.drag_to(Pt::new(70.0, 133.0));
        e.release(Pt::new(70.0, 133.0));
        let p = &e.doc.device(a).unwrap().ports[0];
        assert_eq!(p.side, Side::Bottom);
        assert!((p.t - 30.0 / 120.0).abs() < 1e-9);
        assert!(e.doc.links.is_empty());
        assert_eq!(e.undo.len(), undo_before + 1);
    }

    #[test]
    fn dragging_port_away_draws_link_and_restores_port() {
        let (mut e, a, b) = two_devices();
        e.selection = Selection::Items(vec![a]);
        e.add_port(Side::Right);
        e.press(Pt::new(160.0, 100.0), false, false);
        e.drag_to(Pt::new(161.0, 110.0)); // small slide: moves the port
        e.drag_to(Pt::new(300.0, 100.0)); // pulled away: becomes a link
        e.drag_to(Pt::new(400.0, 100.0));
        e.release(Pt::new(400.0, 100.0));
        let p = &e.doc.device(a).unwrap().ports[0];
        assert_eq!((p.side, p.t), (Side::Right, 0.5), "port restored to where the drag began");
        assert_eq!(e.doc.links.len(), 1);
        assert_eq!(e.doc.links[0].to.device, b);
    }

    #[test]
    fn shift_drag_moves_port_even_far_away() {
        let (mut e, a, _) = two_devices();
        e.selection = Selection::Items(vec![a]);
        e.add_port(Side::Right);
        e.press(Pt::new(160.0, 100.0), true, false);
        e.drag_to(Pt::new(100.0, 0.0));
        e.release(Pt::new(100.0, 0.0));
        assert_eq!(e.doc.device(a).unwrap().ports[0].side, Side::Top);
        assert!(e.doc.links.is_empty());
    }

    #[test]
    fn port_position_property() {
        let (mut e, a, _) = two_devices();
        e.selection = Selection::Items(vec![a]);
        e.add_port(Side::Top);
        assert!(e.set_prop("pos", "25"));
        assert_eq!(e.doc.device(a).unwrap().ports[0].t, 0.25);
        let s: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(s["pos"], 25.0);
    }

    #[test]
    fn inline_edit_device_name_is_one_undo_step() {
        let mut e = Editor::default();
        e.double_click(Pt::new(100.0, 100.0)); // creates a device and starts editing it
        let id = e.doc.devices[0].id;
        assert_eq!(e.editing, Some(EditTarget::DeviceName(id)));
        let v: Value = serde_json::from_str(&e.edit_box_json()).unwrap();
        assert_eq!(v["text"], "Device 1");
        let n = e.undo.len();
        e.commit_edit("  core-sw-01 ");
        assert_eq!(e.doc.devices[0].name, "core-sw-01");
        assert_eq!(e.undo.len(), n + 1);
        assert_eq!(e.edit_box_json(), "");
        e.undo();
        assert_eq!(e.doc.devices[0].name, "Device 1");
    }

    #[test]
    fn inline_edit_cancel_and_unchanged() {
        let (mut e, a, _) = two_devices();
        e.double_click(Pt::new(100.0, 100.0));
        assert_eq!(e.editing, Some(EditTarget::DeviceName(a)));
        e.cancel_edit();
        assert_eq!(e.editing, None);
        assert!(e.begin_edit_selection());
        let n = e.undo.len();
        e.commit_edit("Device 1");
        assert_eq!(e.undo.len(), n, "unchanged text adds no undo step");
    }

    #[test]
    fn double_click_border_adds_port_and_edits_it_and_link_label() {
        let (mut e, a, _) = two_devices();
        e.double_click(Pt::new(160.0, 103.0));
        let port = e.doc.device(a).unwrap().ports[0].clone();
        assert_eq!(e.editing, Some(EditTarget::PortName(Endpoint { device: a, port: port.id })));
        e.commit_edit("Gi1/0/1");
        assert_eq!(e.doc.device(a).unwrap().ports[0].name, "Gi1/0/1");
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(400.0, 100.0));
        let link = e.doc.links[0].id;
        e.double_click(Pt::new(250.0, 100.0));
        assert_eq!(e.editing, Some(EditTarget::LinkLabel(link)));
        e.commit_edit("10G");
        assert_eq!(e.doc.links[0].label, "10G");
    }

    #[test]
    fn add_note_edits_it_and_keeps_line_breaks() {
        let mut e = Editor::default();
        let id = e.add_note_at(Pt::new(53.0, 47.0));
        assert_eq!(e.editing, Some(EditTarget::NoteText(id)));
        let v: Value = serde_json::from_str(&e.edit_box_json()).unwrap();
        assert_eq!(v["multi"], true);
        e.commit_edit("  DMZ\nVLAN 20 \n");
        let n = e.doc.note(id).unwrap();
        assert_eq!(n.text, "DMZ\nVLAN 20");
        assert_eq!((n.x, n.y), (50.0, 50.0));
        // Other labels are kept on one line.
        let (mut e, a, _) = two_devices();
        e.begin_edit(EditTarget::DeviceName(a));
        e.commit_edit("core\nsw");
        assert_eq!(e.doc.device(a).unwrap().name, "core sw");
    }

    #[test]
    fn emptied_note_is_removed_in_one_undo_step() {
        let mut e = Editor::default();
        let id = e.add_note_at(Pt::new(0.0, 0.0));
        e.commit_edit("hello");
        let n = e.undo.len();
        e.begin_edit(EditTarget::NoteText(id));
        e.commit_edit("   ");
        assert!(e.doc.notes.is_empty());
        assert_eq!(e.selection, Selection::None);
        assert_eq!(e.undo.len(), n + 1);
        e.undo();
        assert_eq!(e.doc.note(id).unwrap().text, "hello");
    }

    #[test]
    fn notes_select_move_and_delete_with_devices() {
        let (mut e, a, _) = two_devices(); // a: 40..160 x 70..130
        let id = e.add_note_at(Pt::new(40.0, 20.0));
        e.commit_edit("Rack 1");
        // Clicking the note selects it; double click edits it.
        e.press(Pt::new(45.0, 28.0), false, false);
        e.release(Pt::new(45.0, 28.0));
        assert_eq!(e.selection, Selection::Items(vec![id]));
        let s: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(s["type"], "note");
        assert!(e.set_prop("size", "200"));
        assert_eq!(e.doc.note(id).unwrap().size, NOTE_MAX_SIZE);
        assert!(e.set_prop("size", "14"));
        assert!(e.set_prop("bold", "true"));
        assert!(e.set_prop("color", "#dc2626"));
        assert!(!e.set_prop("kind", "router"));
        // Rubber band picks up the note and the device; both move together.
        drag(&mut e, Pt::new(0.0, 0.0), Pt::new(200.0, 200.0));
        assert_eq!(e.selection, Selection::Items(vec![a, id]));
        let s: Value = serde_json::from_str(&e.selection_json()).unwrap();
        assert_eq!(s["type"], "multi");
        e.nudge(10.0, 0.0);
        assert_eq!(e.doc.note(id).unwrap().x, 50.0);
        assert_eq!(e.doc.device(a).unwrap().x, 50.0);
        e.duplicate();
        assert_eq!(e.doc.notes.len(), 2);
        assert_eq!(e.doc.devices.len(), 3);
        e.delete_selected();
        assert_eq!(e.doc.notes.len(), 1);
        assert_eq!(e.doc.devices.len(), 2);
    }

    #[test]
    fn notes_survive_save_and_load() {
        let mut e = Editor::default();
        e.add_note_at(Pt::new(0.0, 0.0));
        e.commit_edit("line 1\nline 2");
        e.set_prop("bold", "true");
        let f = Diagram::from_json(&e.doc.to_json()).unwrap();
        assert_eq!(f.notes, e.doc.notes);
        // Files without notes still load, and saving them adds no "notes" key.
        let plain = Diagram::from_json(&Diagram::default().to_json()).unwrap();
        assert!(plain.notes.is_empty());
        assert!(!plain.to_json().contains("notes"));
    }

    fn ctrl_drag(e: &mut Editor, from: Pt, to: Pt) {
        e.press(from, false, true);
        e.drag_to(Pt::new((from.x + to.x) / 2.0, (from.y + to.y) / 2.0));
        e.drag_to(to);
    }

    #[test]
    fn ctrl_drag_aligns_box_with_neighbor_and_shows_guide() {
        let (mut e, a, b) = two_devices(); // a: 40..160 x 70..130, b: 340..460 x 70..130
        e.snap = false;
        ctrl_drag(&mut e, Pt::new(400.0, 100.0), Pt::new(430.0, 105.0));
        let d = e.doc.device(b).unwrap();
        assert_eq!((d.x, d.y), (370.0, 70.0), "x follows the mouse, y lines up with a");
        assert_eq!(e.selection, Selection::Items(vec![b]));
        let v: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let g = &v["guides"][0];
        assert_eq!((g[1].as_f64(), g[3].as_f64()), (Some(70.0), Some(70.0)), "horizontal guide at y=70");
        assert!(g[0].as_f64().unwrap() < 40.0 && g[2].as_f64().unwrap() > 490.0, "spans both boxes");
        let n = e.undo.len();
        e.release(Pt::new(430.0, 105.0));
        assert_eq!(e.undo.len(), n + 1);
        let v: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert!(v["guides"].as_array().unwrap().is_empty());
        // Without Ctrl the same drag does not align.
        drag(&mut e, Pt::new(430.0, 100.0), Pt::new(430.0, 105.0));
        assert_eq!(e.doc.device(b).unwrap().y, 75.0);
        assert_eq!(e.doc.device(a).unwrap().y, 70.0);
    }

    #[test]
    fn ctrl_drag_in_many_steps_aligns_where_the_mouse_is() {
        let (mut e, _, b) = two_devices(); // b: 340..460 x 70..130
        e.snap = false;
        e.press(Pt::new(400.0, 100.0), false, true);
        for y in [160.0, 220.0, 280.0, 250.0, 200.0, 140.0, 104.0] {
            e.drag_to(Pt::new(400.0, y));
            let d = e.doc.device(b).unwrap();
            let expect = if y == 104.0 { 70.0 } else { y - 30.0 };
            assert_eq!(d.y, expect, "mouse at y={y}");
            if let Some(g) = e.guides.first() {
                assert!(g.0.y == g.1.y && [d.y, d.y + 30.0, d.y + 60.0].contains(&g.0.y), "guide touches the box");
            }
        }
    }

    #[test]
    fn ctrl_drag_between_two_boxes_snaps_to_equal_gaps() {
        let (mut e, _, b) = two_devices(); // a: 40..160, b: 340..460 (both y 70..130)
        e.add_device_at(Pt::new(800.0, 100.0)); // c: 740..860
        e.snap = false;
        // Middle between a and c: x = (160 + 740 - 120) / 2 = 390. Mouse lands 4 short.
        ctrl_drag(&mut e, Pt::new(400.0, 100.0), Pt::new(446.0, 100.0));
        assert_eq!(e.doc.device(b).unwrap().x, 390.0);
        let v: Value = serde_json::from_str(&e.scene_json()).unwrap();
        let sp: Vec<Vec<f64>> = serde_json::from_value(v["spacings"].clone()).unwrap();
        assert_eq!(sp, vec![vec![160.0, 100.0, 390.0, 100.0], vec![510.0, 100.0, 740.0, 100.0]]);
        // Too far from the middle: no snap, no markers.
        e.drag_to(Pt::new(420.0, 100.0));
        assert_eq!(e.doc.device(b).unwrap().x, 360.0);
        assert!(e.spacings.is_empty());
        e.release(Pt::new(420.0, 100.0));
    }

    #[test]
    fn ctrl_drag_between_boxes_in_a_column() {
        let mut e = Editor { snap: false, ..Editor::default() };
        e.add_device_at(Pt::new(100.0, 100.0)); // y 70..130
        e.add_device_at(Pt::new(100.0, 400.0)); // y 370..430
        let m = e.add_device_at(Pt::new(300.0, 230.0)); // x 240..360, y 200..260
        // Off to the side the column does not count as "between".
        ctrl_drag(&mut e, Pt::new(300.0, 230.0), Pt::new(300.0, 247.0));
        assert_eq!(e.doc.device(m).unwrap().y, 217.0);
        // Over the column: middle y = (130 + 370 - 60) / 2 = 220.
        e.drag_to(Pt::new(103.0, 247.0));
        let d = e.doc.device(m).unwrap();
        assert_eq!((d.x, d.y), (40.0, 220.0), "x lines up with the column, y centered");
        assert_eq!(e.spacings, vec![(Pt::new(100.0, 130.0), Pt::new(100.0, 220.0)), (Pt::new(100.0, 280.0), Pt::new(100.0, 370.0))]);
        e.release(Pt::new(103.0, 247.0));
    }

    fn three_in_a_row() -> (Editor, Id, Id, Id) {
        let (mut e, a, b) = two_devices(); // a: 40..160, b: 340..460 (both y 70..130)
        let c = e.add_device_at(Pt::new(800.0, 100.0)); // c: 740..860
        e.snap = false;
        (e, a, b, c)
    }

    #[test]
    fn ctrl_drag_outer_box_continues_the_gap() {
        let (mut e, _, _, c) = three_in_a_row();
        // a..b gap is 180, so c snaps to 460 + 180 = 640. Mouse lands 3 off.
        ctrl_drag(&mut e, Pt::new(800.0, 100.0), Pt::new(703.0, 100.0));
        assert_eq!(e.doc.device(c).unwrap().x, 640.0);
        assert_eq!(e.spacings, vec![(Pt::new(160.0, 100.0), Pt::new(340.0, 100.0)), (Pt::new(460.0, 100.0), Pt::new(640.0, 100.0))]);
        e.release(Pt::new(703.0, 100.0));
    }

    #[test]
    fn ctrl_drag_outer_box_centers_the_middle_one() {
        let (mut e, a, _, _) = three_in_a_row();
        // b..c gap is 280: a snaps to 340 - 280 - 120 = -60, leaving b centered.
        ctrl_drag(&mut e, Pt::new(100.0, 100.0), Pt::new(-3.0, 100.0));
        assert_eq!(e.doc.device(a).unwrap().x, -60.0);
        assert_eq!(e.spacings, vec![(Pt::new(60.0, 100.0), Pt::new(340.0, 100.0)), (Pt::new(460.0, 100.0), Pt::new(740.0, 100.0))]);
        e.release(Pt::new(-3.0, 100.0));
    }

    #[test]
    fn ctrl_drag_centers_between_diagonal_boxes() {
        let mut e = Editor { snap: false, ..Editor::default() };
        e.add_device_at(Pt::new(100.0, 100.0));
        e.add_device_at(Pt::new(500.0, 400.0));
        let m = e.add_device_at(Pt::new(300.0, 600.0));
        // Midpoint of the two centers is (300, 250); the mouse lands 3 off on both axes.
        ctrl_drag(&mut e, Pt::new(300.0, 600.0), Pt::new(303.0, 247.0));
        let d = e.doc.device(m).unwrap();
        assert_eq!((d.x + d.w / 2.0, d.y + d.h / 2.0), (300.0, 250.0));
        assert_eq!(
            e.spacings,
            vec![(Pt::new(100.0, 100.0), Pt::new(300.0, 250.0)), (Pt::new(300.0, 250.0), Pt::new(500.0, 400.0))]
        );
        e.drag_to(Pt::new(320.0, 247.0));
        assert!(e.spacings.is_empty(), "too far from the midpoint");
        e.release(Pt::new(320.0, 247.0));
    }

    #[test]
    fn ctrl_drag_straightens_links_between_ports() {
        let (mut e, a, b) = two_devices();
        e.snap = false;
        e.selection = Selection::Items(vec![a]);
        e.add_port(Side::Right); // (160, 100)
        e.selection = Selection::Items(vec![b]);
        e.add_port(Side::Left);
        e.set_prop("pos", "25"); // (340, 85)
        drag(&mut e, Pt::new(160.0, 100.0), Pt::new(340.0, 85.0));
        assert_eq!(e.doc.links.len(), 1);
        // Box edges are 20 apart (too far), but the ports end up 5 apart.
        ctrl_drag(&mut e, Pt::new(400.0, 100.0), Pt::new(400.0, 120.0));
        e.release(Pt::new(400.0, 120.0));
        assert_eq!(e.doc.device(b).unwrap().y, 85.0);
        let (pa, _) = e.doc.endpoint_pos(e.doc.links[0].from).unwrap();
        let (pb, _) = e.doc.endpoint_pos(e.doc.links[0].to).unwrap();
        assert_eq!(pa.y, pb.y);
    }

    #[test]
    fn ctrl_drag_port_lines_up_with_other_port() {
        let (mut e, a, b) = two_devices();
        e.snap = false;
        e.selection = Selection::Items(vec![b]);
        e.add_port(Side::Left); // (340, 100)
        e.selection = Selection::Items(vec![a]);
        e.add_port(Side::Right); // (160, 100)
        e.set_prop("pos", "25"); // (160, 85)
        ctrl_drag(&mut e, Pt::new(160.0, 85.0), Pt::new(161.0, 96.0));
        let (pa, _) = e.doc.endpoint_pos(Endpoint { device: a, port: e.doc.device(a).unwrap().ports[0].id }).unwrap();
        assert_eq!(pa.y, 100.0);
        let v: Value = serde_json::from_str(&e.scene_json()).unwrap();
        assert_eq!(v["guides"].as_array().unwrap().len(), 1);
        e.release(Pt::new(161.0, 96.0));
        assert!(e.doc.links.is_empty());
    }

    #[test]
    fn ctrl_click_without_moving_still_toggles_selection() {
        let (mut e, a, b) = two_devices();
        e.press(Pt::new(100.0, 100.0), false, false);
        e.release(Pt::new(100.0, 100.0));
        e.press(Pt::new(400.0, 100.0), false, true);
        e.release(Pt::new(400.0, 100.0));
        assert_eq!(e.selection, Selection::Items(vec![a, b]));
        e.press(Pt::new(100.0, 100.0), false, true);
        e.release(Pt::new(100.0, 100.0));
        assert_eq!(e.selection, Selection::Items(vec![b]));
        // Ctrl+drag of a selected item moves the whole selection.
        e.press(Pt::new(100.0, 100.0), false, true);
        e.release(Pt::new(100.0, 100.0));
        ctrl_drag(&mut e, Pt::new(100.0, 100.0), Pt::new(100.0, 400.0));
        e.release(Pt::new(100.0, 400.0));
        assert_eq!(e.doc.device(a).unwrap().y, 370.0);
        assert_eq!(e.doc.device(b).unwrap().y, 370.0);
    }
}
