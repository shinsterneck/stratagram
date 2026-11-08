// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! The frame around a diagram: a border with a header and footer, sized to
//! whatever is shown plus a margin. It is laid out here as plain rectangles,
//! lines, texts and an image (the logo), which the canvas, the SVG export
//! and the draw.io export each draw the same way.

use crate::geom;
use crate::model::*;

pub const MUTED: &str = "#64748b";
const WHITE: &str = "#ffffff";

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Align {
    Left,
    Center,
    Right,
}

impl Align {
    pub fn as_str(self) -> &'static str {
        match self {
            Align::Left => "left",
            Align::Center => "center",
            Align::Right => "right",
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub enum Prim {
    Rect { r: Rect, fill: Option<String>, stroke: Option<String>, width: f64, radius: f64, dash: bool },
    Line { a: Pt, b: Pt, color: String, width: f64, dash: bool },
    /// Text centered vertically on `at.y`, aligned on `at.x`.
    Text { at: Pt, text: String, size: f64, bold: bool, color: String, align: Align },
    /// A picture (data URI) stretched over `r`.
    Image { r: Rect, data: String },
    /// Large faint text centered on `at`, turned by `angle` degrees.
    Watermark { at: Pt, text: String, size: f64, color: String, angle: f64, opacity: f64 },
}

#[derive(Debug, Clone, PartialEq)]
pub struct FrameLayout {
    /// The frame's outside edge.
    pub outer: Rect,
    /// Back to front, under the diagram.
    pub prims: Vec<Prim>,
    /// Over the diagram (the watermark).
    pub overlay: Vec<Prim>,
}

/// What the frame shows besides its own settings.
#[derive(Debug, Default, Clone)]
pub struct Info {
    /// Date and time of the last change.
    pub changed: String,
    pub file: String,
    /// The line types in the diagram, each with the color of its first line.
    pub legend: Vec<(LinkKind, String)>,
}

/// Everything shown: boxes, texts, points and lines. Hidden layers do not
/// count.
pub fn content_bounds(doc: &Diagram) -> Option<Rect> {
    let doc = doc.visible_only();
    let lines = doc.links.iter().filter_map(|l| geom::link_route(&doc, l)).flatten().map(|p| Rect { x: p.x, y: p.y, w: 0.0, h: 0.0 });
    // Layer backgrounds and every label count too: the frame goes around
    // them.
    let backgrounds = doc.layer_backgrounds().into_iter().map(|(_, r, _)| r);
    let labels = geom::label_rects(&doc);
    doc.bounds().into_iter().chain(lines).chain(backgrounds).chain(labels).reduce(|a, b| a.union(&b))
}

/// The frame of `doc` (if it has one on a shown layer), around what is
/// shown. `file` is the file's name, for frames that show it.
pub fn of(doc: &Diagram, file: &str) -> Option<FrameLayout> {
    let f = doc.frame()?;
    let content = content_bounds(doc).unwrap_or(Rect { x: 0.0, y: 0.0, w: 320.0, h: 200.0 });
    let mut legend: Vec<(LinkKind, String)> = vec![];
    for l in doc.links.iter().filter(|l| doc.link_visible(l)) {
        // Carriers are in the legend too.
        for k in std::iter::once(l.style.kind).chain(l.style.over.iter().copied()) {
            if !legend.iter().any(|(x, _)| *x == k) {
                legend.push((k, l.style.color.clone()));
            }
        }
    }
    legend.sort_by_key(|(k, _)| LinkKind::ALL.iter().position(|x| x == k));
    let info = Info { changed: doc.changed.clone().unwrap_or_default(), file: file.into(), legend };
    Some(layout(f, content, &info))
}

/// Rough width of `text` at `size` (for sizing the frame to fit its texts).
fn text_w(text: &str, size: f64, bold: bool) -> f64 {
    text.chars().count() as f64 * size * if bold { 0.62 } else { 0.56 }
}

/// `color` mixed with white, `k` of the way (0 = the color, 1 = white).
fn tint(color: &str, k: f64) -> String {
    let c = geom::normalize_color(color).unwrap_or_else(|| "#1e293b".into());
    let ch = |i: usize| u8::from_str_radix(&c[i..i + 2], 16).unwrap_or(0) as f64;
    let mix = |v: f64| (v + (255.0 - v) * k).round() as u8;
    format!("#{:02x}{:02x}{:02x}", mix(ch(1)), mix(ch(3)), mix(ch(5)))
}

/// Non-empty `parts` joined with a middle dot.
fn join(parts: &[&str]) -> String {
    parts.iter().filter(|p| !p.is_empty()).copied().collect::<Vec<_>>().join("  ·  ")
}

/// The classification levels, with their label and color.
fn level(level: &str) -> (&'static str, &'static str) {
    match level.trim().to_lowercase().as_str() {
        "public" => ("PUBLIC", "#16a34a"),
        "confidential" => ("CONFIDENTIAL", "#d97706"),
        "restricted" => ("RESTRICTED", "#dc2626"),
        _ => ("INTERNAL", "#2563eb"),
    }
}

/// Change status badge: label and color.
fn status(status: &str) -> Option<(String, &'static str)> {
    let s = status.trim();
    let color = match s.to_lowercase().as_str() {
        "" => return None,
        "draft" => "#64748b",
        "approved" | "final" | "operational" | "ok" | "up" => "#16a34a",
        "implemented" | "maintenance" => "#2563eb",
        "rejected" | "outage" | "down" => "#dc2626",
        _ => "#d97706",
    };
    Some((s.to_uppercase(), color))
}

/// Incident severity badge: label and color.
fn severity(s: &str) -> (String, &'static str) {
    match s.trim().to_uppercase().as_str() {
        "P1" => ("P1 · CRITICAL".into(), "#dc2626"),
        "P2" => ("P2 · HIGH".into(), "#ea580c"),
        "P3" => ("P3 · MEDIUM".into(), "#d97706"),
        "P4" => ("P4 · LOW".into(), "#64748b"),
        "" => ("INCIDENT".into(), "#64748b"),
        other => (other.to_string(), "#d97706"),
    }
}

/// `text` broken into lines of at most `max_w` at `size` (new lines kept).
fn wrap(text: &str, size: f64, max_w: f64) -> Vec<String> {
    let mut lines = vec![];
    for para in text.lines() {
        let mut line = String::new();
        for word in para.split_whitespace() {
            let candidate = if line.is_empty() { word.to_string() } else { format!("{line} {word}") };
            if !line.is_empty() && text_w(&candidate, size, false) > max_w {
                lines.push(std::mem::replace(&mut line, word.to_string()));
            } else {
                line = candidate;
            }
        }
        lines.push(line);
    }
    while lines.last().is_some_and(|l| l.is_empty()) {
        lines.pop();
    }
    lines
}

/// A location path with its parts joined by arrows: "EU > Berlin" or
/// "EU / Berlin" become "EU › Berlin".
fn breadcrumb(path: &str) -> String {
    path.split(['>', '/', '›', '»']).map(str::trim).filter(|p| !p.is_empty()).collect::<Vec<_>>().join("  ›  ")
}

pub fn kind_label(k: LinkKind) -> &'static str {
    match k {
        LinkKind::Plain => "Link",
        LinkKind::Trunk => "802.1Q trunk",
        LinkKind::Ipsec => "IPsec tunnel",
        LinkKind::Vxlan => "VXLAN tunnel",
        LinkKind::Capwap => "CAPWAP tunnel",
        LinkKind::L2 => "L2 switched",
        LinkKind::Sdwan => "SD-WAN",
        LinkKind::Mpls => "MPLS",
        LinkKind::L3 => "L3 routed",
        LinkKind::L3Sub => "L3 subinterface",
    }
}

struct Out(Vec<Prim>);

impl Out {
    fn rect(&mut self, r: Rect, fill: Option<&str>, stroke: Option<&str>, width: f64) {
        self.rrect(r, fill, stroke, width, 0.0);
    }
    fn rrect(&mut self, r: Rect, fill: Option<&str>, stroke: Option<&str>, width: f64, radius: f64) {
        let (fill, stroke) = (fill.map(String::from), stroke.map(String::from));
        self.0.push(Prim::Rect { r, fill, stroke, width, radius, dash: false });
    }
    fn line(&mut self, a: Pt, b: Pt, color: &str, width: f64) {
        self.0.push(Prim::Line { a, b, color: color.into(), width, dash: false });
    }
    fn hline(&mut self, x0: f64, x1: f64, y: f64, color: &str, width: f64) {
        self.line(Pt::new(x0, y), Pt::new(x1, y), color, width);
    }
    fn vline(&mut self, x: f64, y0: f64, y1: f64, color: &str, width: f64) {
        self.line(Pt::new(x, y0), Pt::new(x, y1), color, width);
    }
    fn text(&mut self, at: Pt, text: &str, size: f64, bold: bool, color: &str, align: Align) {
        if !text.is_empty() {
            self.0.push(Prim::Text { at, text: text.into(), size, bold, color: color.into(), align });
        }
    }
    /// A labeled cell: small caption on top, value below.
    fn cell(&mut self, at: Pt, caption: &str, value: &str, size: f64, bold: bool, color: &str) {
        self.text(Pt::new(at.x + 6.0, at.y + 8.0), caption, 7.0, true, MUTED, Align::Left);
        self.text(Pt::new(at.x + 6.0, at.y + 8.0 + size), value, size, bold, color, Align::Left);
    }
    /// A short sample of a line of type `k`, from `x` across `w` at height `y`.
    fn sample(&mut self, k: LinkKind, x: f64, y: f64, w: f64, c: &str) {
        match k {
            LinkKind::Trunk => {
                self.hline(x, x + w, y - 1.75, c, 1.5);
                self.hline(x, x + w, y + 1.75, c, 1.5);
            }
            LinkKind::Ipsec | LinkKind::Vxlan | LinkKind::Capwap => {
                let r = Rect { x, y: y - 5.0, w, h: 10.0 };
                let (fill, stroke) = (Some(tint(c, 0.86)), Some(c.to_string()));
                self.0.push(Prim::Rect { r, fill, stroke, width: 0.75, radius: 0.0, dash: k != LinkKind::Ipsec });
                self.hline(x, x + w, y, c, 1.5);
            }
            LinkKind::Sdwan => self.0.push(Prim::Line { a: Pt::new(x, y), b: Pt::new(x + w, y), color: c.into(), width: 2.0, dash: true }),
            LinkKind::Mpls => {
                self.hline(x, x + w, y, c, 2.0);
                let mut t = x + 5.0;
                while t < x + w - 3.0 {
                    self.vline(t, y - 3.5, y + 3.5, c, 1.2);
                    t += 5.5;
                }
            }
            LinkKind::Plain => self.hline(x, x + w, y, c, 2.0),
            LinkKind::L2 => {
                self.hline(x, x + w, y, c, 2.0);
                for cx in [x + 5.0, x + w - 5.0] {
                    self.rect(Rect { x: cx - 3.0, y: y - 3.0, w: 6.0, h: 6.0 }, Some(c), None, 0.0);
                }
            }
            LinkKind::L3 | LinkKind::L3Sub => {
                if k == LinkKind::L3Sub {
                    self.hline(x, x + w, y - 1.75, c, 1.5);
                    self.hline(x, x + w, y + 1.75, c, 1.5);
                } else {
                    self.hline(x, x + w, y, c, 2.0);
                }
                for cx in [x + 5.0, x + w - 5.0] {
                    let r = Rect { x: cx - 3.5, y: y - 3.5, w: 7.0, h: 7.0 };
                    self.rrect(r, Some(WHITE), Some(c), 1.2, 3.5);
                }
            }
        }
    }
}

/// The frame's settings laid out around `content`.
pub fn layout(f: &Frame, content: Rect, info: &Info) -> FrameLayout {
    use FrameTemplate as T;
    let color = geom::normalize_color(&f.color).unwrap_or_else(|| "#1e293b".into());
    let changed = if f.show_date { info.changed.as_str() } else { "" };
    let date = if changed.is_empty() { String::new() } else { format!("Last change: {changed}") };
    let page = if f.page.is_empty() { String::new() } else { format!("Page {}", f.page) };
    let file = if f.show_file { info.file.as_str() } else { "" };
    // The texts along the footer: left, center, right.
    let footer = [join(&[&f.author, file]), f.note.clone(), join(&[&date, &page])];
    let footer_w: f64 = footer.iter().map(|t| text_w(t, 11.0, false)).sum::<f64>() + 80.0;
    let has_sub = !f.subtitle.is_empty();
    let m = f.margin.max(0.0);
    let logo = (!f.logo.is_empty() && f.logo_w > 0.0 && f.logo_h > 0.0).then_some((f.logo.as_str(), f.logo_w / f.logo_h));
    let logo2 = (!f.logo2.is_empty() && f.logo2_w > 0.0 && f.logo2_h > 0.0).then_some((f.logo2.as_str(), f.logo2_w / f.logo2_h));
    // A logo's width at height `h`, kept to a sensible size.
    let one_w = |l: Option<(&str, f64)>, h: f64| l.map_or(0.0, |(_, ratio)| (h * ratio).min(180.0));
    // Both logos side by side.
    let logo_w = |h: f64| one_w(logo, h) + one_w(logo2, h) + if logo.is_some() && logo2.is_some() { 10.0 } else { 0.0 };
    let any_logo = logo.is_some() || logo2.is_some();
    let badge = status(&f.status);
    let badge_w = badge.as_ref().map_or(0.0, |(s, _)| text_w(s, 10.0, true) + 20.0);
    let code_w = if f.doc_id.is_empty() { 0.0 } else { text_w(&f.doc_id, 14.0, true).max(30.0) + 20.0 };
    let legend_w: f64 = info.legend.iter().map(|(k, _)| 26.0 + 8.0 + text_w(kind_label(*k), 10.0, false) + 18.0).sum();
    let site_line = join(&[&f.address, &f.subtitle]);
    let revisions = &f.revisions;
    // Revision table columns: rev, date, change, by.
    let rev_cols = {
        let widest = |get: &dyn Fn(&Revision) -> &str, caption: &str, min: f64| {
            revisions.iter().map(|r| text_w(get(r), 9.5, false)).fold(text_w(caption, 7.0, true), f64::max).max(min) + 14.0
        };
        [widest(&|r| &r.rev, "REV", 22.0), widest(&|r| &r.date, "DATE", 60.0), widest(&|r| &r.change, "CHANGE", 140.0), widest(&|r| &r.author, "BY", 50.0)]
    };
    let rev_h = 15.0 * (revisions.len() as f64 + 1.0);
    const ZONE: f64 = 14.0;
    const ISO_BLOCK: (f64, f64) = (330.0, 84.0);
    const PANEL: f64 = 260.0;
    let level_label = if f.level.is_empty() { "" } else { level(&f.level).0 };
    // Document control table (controlled document): two rows of cells.
    let control = [
        [("DOCUMENT", f.doc_id.as_str()), ("VERSION", f.version.as_str()), ("CLASSIFICATION", level_label)],
        [("OWNER", f.owner.as_str()), ("NEXT REVIEW", f.review_date.as_str()), ("LAST CHANGE", changed)],
    ];
    let control_cols: Vec<f64> = (0..3)
        .map(|j| control.iter().map(|row| text_w(row[j].1, 10.0, false).max(text_w(row[j].0, 7.0, true))).fold(0.0, f64::max) + 16.0)
        .collect();
    let control_w: f64 = control_cols.iter().sum();
    let summary = wrap(&f.summary, 11.0, PANEL - 28.0);
    let (sev_label, sev_color) = severity(&f.severity);
    let stamp_w = text_w("AS-BUILT", 15.0, true) + 26.0;
    let customer_line = if f.customer.is_empty() { String::new() } else { format!("Prepared for: {}", f.customer) };
    let supplier_line = {
        let by = join(&[&f.company, &f.author]);
        if by.is_empty() { by } else { format!("Prepared by: {by}") }
    };
    let (head, mut foot, side) = match f.template {
        T::Lines | T::Blueprint => (if has_sub { 54.0 } else { 40.0 }, 26.0, 0.0),
        T::TitleBlock => (if any_logo { 44.0 } else { 0.0 }, 48.0, 0.0),
        T::Banner => (if has_sub { 58.0 } else { 44.0 }, 28.0, 0.0),
        T::Minimal => (if has_sub { 56.0 } else { 40.0 }, 24.0, 0.0),
        T::Corporate => (if has_sub { 60.0 } else { 46.0 }, 28.0, 0.0),
        T::Report => (if has_sub { 50.0 } else { 36.0 }, 28.0, 0.0),
        T::Card => (if has_sub { 56.0 } else { 42.0 }, 26.0, 0.0),
        T::Iso => (ZONE + if any_logo { 44.0 } else { 6.0 }, ZONE + ISO_BLOCK.1 + 6.0, ZONE),
        T::Revision => (if any_logo { 44.0 } else { 0.0 }, 48.0 + rev_h + 6.0, 0.0),
        T::Change => (if has_sub || !f.doc_id.is_empty() { 58.0 } else { 40.0 }, 26.0, 0.0),
        T::Classification => (22.0 + if has_sub { 50.0 } else { 36.0 }, 22.0 + 24.0, 0.0),
        T::Site => (if site_line.is_empty() { 44.0 } else { 58.0 }, 30.0, 0.0),
        T::Controlled => (58.0, 26.0, 0.0),
        T::SignOff => (if has_sub { 54.0 } else { 40.0 }, 72.0, 0.0),
        T::Solution => (if has_sub { 66.0 } else { 52.0 }, 28.0, 0.0),
        T::Proposal => (4.0 + 56.0 + 24.0, 28.0, 0.0),
        T::Presentation => (if has_sub { 80.0 } else { 62.0 }, 36.0, 0.0),
        T::Executive => (if has_sub { 62.0 } else { 48.0 }, 26.0, 0.0),
        T::AsBuilt => (if has_sub || !f.note.is_empty() || !f.author.is_empty() { 54.0 } else { 44.0 }, 48.0, 0.0),
        T::Incident => (5.0 + if f.services.is_empty() { 58.0 } else { 74.0 }, 26.0, 0.0),
        T::Location => (if has_sub { 74.0 } else { 58.0 }, 26.0, 0.0),
        T::Noc => (if has_sub { 66.0 } else { 52.0 }, 30.0, 0.0),
    };
    // The summary beside the diagram needs room: what the diagram does not
    // give goes below it.
    if f.template == T::Executive {
        let need_h = 36.0 + 16.0 * summary.len() as f64 + 16.0;
        foot += (need_h - (content.h + 2.0 * m)).max(0.0);
    }
    // Wide enough for the diagram and for the texts, centered on the diagram.
    let title_w = text_w(&f.title, 18.0, true).max(text_w(&f.subtitle, 12.0, false)) + 32.0;
    let header_extra = logo_w(30.0) + 24.0;
    let need = match f.template {
        T::TitleBlock => text_w(&f.title, 14.0, true) * 2.2 + footer_w,
        T::Revision => (text_w(&f.title, 14.0, true) * 2.2 + footer_w).max(rev_cols.iter().sum::<f64>() + 20.0),
        T::Iso => ISO_BLOCK.0 + text_w(&f.note, 10.0, false) + 40.0,
        T::Corporate => (title_w + header_extra + text_w(&f.company, 13.0, true) + 20.0).max(footer_w),
        T::Change => (title_w + header_extra + badge_w).max(footer_w),
        T::Site => (text_w(&f.title, 18.0, true).max(text_w(&site_line, 12.0, false)) + 32.0 + header_extra + code_w)
            .max(legend_w + text_w(&join(&[&f.author, &f.note, &date, &page, file]), 10.0, false) + 40.0),
        T::Controlled => (title_w + header_extra + control_w).max(footer_w + 120.0),
        T::SignOff => (title_w + header_extra + text_w(&join(&[&f.note, &page, file]), 10.0, false)).max(480.0),
        T::Solution | T::Noc | T::Incident => (title_w + header_extra + badge_w.max(text_w(&sev_label, 10.0, true) + 20.0) + 40.0).max(footer_w),
        T::Proposal => (text_w(&f.title, 18.0, true) + 2.0 * (header_extra + 20.0))
            .max(text_w(&customer_line, 11.0, false) + text_w(&supplier_line, 11.0, false) + 60.0),
        T::AsBuilt => (title_w + header_extra + stamp_w).max(5.0 * 110.0),
        T::Location => (text_w(&breadcrumb(&f.location), 11.0, false) + 32.0).max(title_w) + header_extra + code_w,
        _ => (title_w + header_extra).max(footer_w),
    };
    let mut outer = match f.template {
        // The summary panel takes the right side; the diagram stays left.
        T::Executive => {
            let w = (content.w + 2.0 * m + PANEL).max(need);
            Rect { x: content.x - m, y: content.y - m - head, w, h: content.h + 2.0 * m + head + foot }
        }
        _ => {
            let w = (content.w + 2.0 * m + 2.0 * side).max(need);
            Rect { x: content.x + content.w / 2.0 - w / 2.0, y: content.y - m - head, w, h: content.h + 2.0 * m + head + foot }
        }
    };
    if f.template == T::Presentation {
        // A 16:9 slide: widen it, or add room above and below the diagram.
        let ratio = 16.0 / 9.0;
        if outer.w / outer.h < ratio {
            let w = outer.h * ratio;
            outer.x -= (w - outer.w) / 2.0;
            outer.w = w;
        } else {
            let h = outer.w / ratio;
            outer.y -= (h - outer.h) / 2.0;
            outer.h = h;
        }
    }
    let (x, w) = (outer.x, outer.w);
    let (top, bottom, right) = (outer.y, outer.y + outer.h, outer.x + outer.w);
    let mut o = Out(vec![]);
    // One logo `h` high with its right edge at `rx`, centered on `cy`;
    // returns where the next thing to its left may end.
    let put_one = |o: &mut Out, l: Option<(&str, f64)>, rx: f64, cy: f64, h: f64| -> f64 {
        let Some((data, _)) = l else { return rx };
        let lw = one_w(l, h);
        o.0.push(Prim::Image { r: Rect { x: rx - lw, y: cy - h / 2.0, w: lw, h }, data: data.into() });
        rx - lw - 10.0
    };
    // Both logos, the second to the left of the first.
    let put_logo = |o: &mut Out, rx: f64, cy: f64, h: f64| -> f64 {
        let rx = put_one(o, logo, rx, cy, h);
        put_one(o, logo2, rx, cy, h)
    };
    let footer_texts = |o: &mut Out, y: f64, size: f64, c: &str, pad: f64| {
        o.text(Pt::new(x + pad, y), &footer[0], size, false, c, Align::Left);
        o.text(Pt::new(x + w / 2.0, y), &footer[1], size, false, c, Align::Center);
        o.text(Pt::new(right - pad, y), &footer[2], size, false, c, Align::Right);
    };
    // Title (and subtitle under it) at the left of a header band from
    // `y0`, `h` high.
    let titles = |o: &mut Out, x0: f64, y0: f64, h: f64, size: f64, c: &str, sub: &str, sub_c: &str| {
        let ty = if sub.is_empty() { y0 + h / 2.0 } else { y0 + h / 2.0 - 8.0 };
        o.text(Pt::new(x0, ty), &f.title, size, true, c, Align::Left);
        o.text(Pt::new(x0, ty + size / 2.0 + 9.0), sub, 12.0, false, sub_c, Align::Left);
    };
    // The title block table along the bottom of `inner`, `bh` high: the
    // title takes what author, note and last change leave.
    let title_table = |o: &mut Out, inner: Rect, bh: f64| {
        let by = inner.y + inner.h - bh;
        o.hline(inner.x, inner.x + inner.w, by, &color, 0.75);
        let mut cells: Vec<(&str, &str, f64)> = vec![];
        for (caption, value) in [("AUTHOR", f.author.as_str()), ("NOTE", f.note.as_str()), ("PAGE", f.page.as_str()), ("LAST CHANGE", changed)] {
            if !value.is_empty() {
                cells.push((caption, value, text_w(value, 12.0, false).max(text_w(caption, 7.0, true)) + 20.0));
            }
        }
        let others: f64 = cells.iter().map(|c| c.2).sum();
        let mut cx = inner.x;
        o.cell(Pt::new(cx, by + 4.0), "TITLE", &f.title, 14.0, true, &color);
        o.text(Pt::new(cx + 6.0, by + 40.0), &join(&[&f.subtitle, file]), 9.0, false, MUTED, Align::Left);
        cx += (inner.w - others).max(0.0);
        for (caption, value, cw) in cells {
            o.vline(cx, by, by + bh, &color, 0.75);
            o.cell(Pt::new(cx, by + 6.0), caption, value, 12.0, false, &color);
            cx += cw;
        }
        by
    };
    match f.template {
        T::Lines => {
            o.rect(outer, Some(WHITE), Some(&color), 1.5);
            o.hline(x, right, top + head, &color, 1.0);
            o.hline(x, right, bottom - foot, &color, 1.0);
            titles(&mut o, x + 12.0, top, head, 18.0, &color, &f.subtitle, MUTED);
            put_logo(&mut o, right - 12.0, top + head / 2.0, (head - 14.0).min(34.0));
            footer_texts(&mut o, bottom - foot / 2.0, 11.0, &color, 12.0);
        }
        T::TitleBlock | T::Revision => {
            o.rect(outer, Some(WHITE), Some(&color), 2.0);
            let inner = Rect { x: x + 5.0, y: top + 5.0, w: w - 10.0, h: outer.h - 10.0 };
            o.rect(inner, None, Some(&color), 0.75);
            put_logo(&mut o, inner.x + 8.0 + logo_w(28.0), inner.y + 20.0, 28.0);
            let by = title_table(&mut o, inner, 48.0);
            if f.template == T::Revision {
                // The revision table sits on the title block, at the right.
                let tw: f64 = rev_cols.iter().sum();
                let (tx, ty) = (inner.x + inner.w - tw, by - rev_h);
                o.rect(Rect { x: tx, y: ty, w: tw, h: rev_h }, Some(WHITE), Some(&color), 0.75);
                let rows = std::iter::once(["REV", "DATE", "CHANGE", "BY"].map(String::from))
                    .chain(revisions.iter().map(|r| [r.rev.clone(), r.date.clone(), r.change.clone(), r.author.clone()]));
                for (i, row) in rows.enumerate() {
                    let ry = ty + 15.0 * i as f64;
                    if i > 0 {
                        o.hline(tx, tx + tw, ry, &color, 0.5);
                    }
                    let mut cx = tx;
                    for (j, value) in row.iter().enumerate() {
                        if j > 0 {
                            o.vline(cx, ry, ry + 15.0, &color, 0.5);
                        }
                        let (size, bold, c) = if i == 0 { (7.0, true, MUTED) } else { (9.5, false, color.as_str()) };
                        o.text(Pt::new(cx + 6.0, ry + 7.5), value, size, bold, c, Align::Left);
                        cx += rev_cols[j];
                    }
                }
            }
        }
        T::Banner => {
            o.rect(outer, Some(WHITE), Some(&color), 1.0);
            o.rect(Rect { x, y: top, w, h: head }, Some(&color), None, 0.0);
            o.rect(Rect { x, y: bottom - foot, w, h: foot }, Some(&tint(&color, 0.86)), None, 0.0);
            titles(&mut o, x + 16.0, top, head, 18.0, WHITE, &f.subtitle, &tint(&color, 0.7));
            put_logo(&mut o, right - 16.0, top + head / 2.0, (head - 16.0).min(34.0));
            footer_texts(&mut o, bottom - foot / 2.0, 11.0, &color, 12.0);
        }
        T::Minimal => {
            titles(&mut o, x, top, head, 22.0, &color, &f.subtitle, MUTED);
            put_logo(&mut o, right, top + head / 2.0, (head - 10.0).min(34.0));
            o.hline(x, right, bottom - foot, MUTED, 0.75);
            footer_texts(&mut o, bottom - foot / 2.0 + 2.0, 10.0, MUTED, 0.0);
        }
        T::Corporate => {
            o.rect(outer, Some(WHITE), Some(&tint(&color, 0.75)), 1.0);
            o.rect(Rect { x, y: top, w: 6.0, h: outer.h }, Some(&color), None, 0.0);
            titles(&mut o, x + 24.0, top, head, 20.0, &color, &f.subtitle, MUTED);
            let rx = put_logo(&mut o, right - 16.0, top + head / 2.0, (head - 16.0).min(34.0));
            o.text(Pt::new(rx, top + head / 2.0), &f.company, 13.0, true, &color, Align::Right);
            o.hline(x + 24.0, right - 16.0, bottom - foot, &tint(&color, 0.7), 0.75);
            let all = join(&[&f.author, &f.note, &date, &page, file]);
            o.text(Pt::new(x + 24.0, bottom - foot / 2.0), &all, 10.5, false, MUTED, Align::Left);
        }
        T::Report => {
            o.rect(outer, Some(WHITE), None, 0.0);
            let cx = x + w / 2.0;
            let ty = if has_sub { top + 16.0 } else { top + head / 2.0 };
            o.text(Pt::new(cx, ty), &f.title, 16.0, true, &color, Align::Center);
            o.text(Pt::new(cx, top + 34.0), &f.subtitle, 11.0, false, MUTED, Align::Center);
            put_logo(&mut o, x + 4.0 + logo_w((head - 12.0).min(30.0)), top + head / 2.0, (head - 12.0).min(30.0));
            o.hline(x, right, top + head, &color, 1.0);
            o.hline(x, right, bottom - foot, MUTED, 0.75);
            let fy = bottom - foot / 2.0 + 2.0;
            o.text(Pt::new(x + 4.0, fy), &join(&[&f.author, &date, file]), 10.0, false, MUTED, Align::Left);
            o.text(Pt::new(x + w / 2.0, fy), &page, 10.0, false, MUTED, Align::Center);
            o.text(Pt::new(right - 4.0, fy), &f.note.to_uppercase(), 10.0, true, &color, Align::Right);
        }
        T::Card => {
            let shadow = |dy: f64| Rect { y: top + dy, ..outer };
            o.rrect(shadow(6.0), Some("#eaeff5"), None, 0.0, 14.0);
            o.rrect(shadow(3.0), Some("#dde4ec"), None, 0.0, 13.0);
            o.rrect(outer, Some(WHITE), Some("#cbd5e1"), 1.0, 12.0);
            titles(&mut o, x + 20.0, top + 4.0, head - 4.0, 16.0, &color, &f.subtitle, MUTED);
            put_logo(&mut o, right - 20.0, top + head / 2.0 + 2.0, (head - 18.0).min(30.0));
            footer_texts(&mut o, bottom - foot / 2.0 - 2.0, 10.0, MUTED, 20.0);
        }
        T::Iso => {
            o.rect(outer, Some(WHITE), Some(&color), 2.0);
            let inner = Rect { x: x + ZONE, y: top + ZONE, w: w - 2.0 * ZONE, h: outer.h - 2.0 * ZONE };
            o.rect(inner, None, Some(&color), 0.75);
            // Coordinate zones: numbers along the top and bottom, letters
            // down the sides.
            let cols = ((inner.w / 160.0).round() as usize).max(4);
            let rows = ((inner.h / 160.0).round() as usize).clamp(3, 26);
            for i in 0..cols {
                let (x0, cw) = (inner.x + inner.w * i as f64 / cols as f64, inner.w / cols as f64);
                if i > 0 {
                    o.vline(x0, top, inner.y, &color, 0.75);
                    o.vline(x0, inner.y + inner.h, bottom, &color, 0.75);
                }
                let n = (i + 1).to_string();
                o.text(Pt::new(x0 + cw / 2.0, top + ZONE / 2.0), &n, 8.0, false, &color, Align::Center);
                o.text(Pt::new(x0 + cw / 2.0, bottom - ZONE / 2.0), &n, 8.0, false, &color, Align::Center);
            }
            for j in 0..rows {
                let (y0, rh) = (inner.y + inner.h * j as f64 / rows as f64, inner.h / rows as f64);
                if j > 0 {
                    o.hline(x, inner.x, y0, &color, 0.75);
                    o.hline(inner.x + inner.w, right, y0, &color, 0.75);
                }
                let l = ((b'A' + j as u8) as char).to_string();
                o.text(Pt::new(x + ZONE / 2.0, y0 + rh / 2.0), &l, 8.0, false, &color, Align::Center);
                o.text(Pt::new(right - ZONE / 2.0, y0 + rh / 2.0), &l, 8.0, false, &color, Align::Center);
            }
            put_logo(&mut o, inner.x + 8.0 + logo_w(28.0), inner.y + 20.0, 28.0);
            // The title block in the bottom right corner.
            let (bw, bh) = (ISO_BLOCK.0.min(inner.w), ISO_BLOCK.1);
            let (bx, by) = (inner.x + inner.w - bw, inner.y + inner.h - bh);
            o.rect(Rect { x: bx, y: by, w: bw, h: bh }, Some(WHITE), Some(&color), 1.0);
            o.hline(bx, bx + bw, by + 32.0, &color, 0.75);
            o.hline(bx, bx + bw, by + 58.0, &color, 0.75);
            o.cell(Pt::new(bx, by + 2.0), "TITLE", &f.title, 13.0, true, &color);
            o.text(Pt::new(bx + bw - 6.0, by + 22.0), &f.subtitle, 8.5, false, MUTED, Align::Right);
            let date_only = changed.split(' ').next().unwrap_or("");
            for (row, y) in [
                ([("DRAWING NO.", f.doc_id.as_str()), ("REV", f.revision.as_str()), ("SHEET", f.page.as_str())], by + 32.0),
                ([("DRAWN", f.author.as_str()), ("CHECKED", f.approver.as_str()), ("DATE", date_only)], by + 58.0),
            ] {
                for (i, (caption, value)) in row.iter().enumerate() {
                    let cx = bx + bw * [0.0, 0.45, 0.7][i];
                    if i > 0 {
                        o.vline(cx, y, y + 26.0, &color, 0.75);
                    }
                    o.cell(Pt::new(cx, y - 1.0), caption, value, 10.0, false, &color);
                }
            }
            o.text(Pt::new(inner.x + 8.0, by + bh / 2.0), &join(&[&f.note, file]), 10.0, false, MUTED, Align::Left);
        }
        T::Blueprint => {
            let (bg, minor, major, soft) = ("#163f73", "#22508a", "#33639e", "#c7d7ee");
            o.rect(outer, Some(bg), None, 0.0);
            // A grid over the whole sheet, finer lines first.
            for pass in [false, true] {
                let mut i = 0;
                let mut gx = x;
                while gx <= right {
                    if (i % 5 == 0) == pass {
                        o.vline(gx, top, bottom, if pass { major } else { minor }, 0.75);
                    }
                    gx += 20.0;
                    i += 1;
                }
                let (mut j, mut gy) = (0, top);
                while gy <= bottom {
                    if (j % 5 == 0) == pass {
                        o.hline(x, right, gy, if pass { major } else { minor }, 0.75);
                    }
                    gy += 20.0;
                    j += 1;
                }
            }
            o.rect(outer, None, Some(WHITE), 2.0);
            o.rect(Rect { x: x + 6.0, y: top + 6.0, w: w - 12.0, h: outer.h - 12.0 }, None, Some(WHITE), 0.75);
            o.hline(x + 6.0, right - 6.0, top + head, WHITE, 0.75);
            o.hline(x + 6.0, right - 6.0, bottom - foot, WHITE, 0.75);
            titles(&mut o, x + 16.0, top + 3.0, head - 3.0, 18.0, WHITE, &f.subtitle, soft);
            put_logo(&mut o, right - 16.0, top + head / 2.0, (head - 18.0).min(30.0));
            footer_texts(&mut o, bottom - foot / 2.0 - 2.0, 11.0, WHITE, 16.0);
        }
        T::Change => {
            o.rect(outer, Some(WHITE), Some(&color), 1.5);
            o.hline(x, right, top + head, &color, 1.0);
            o.hline(x, right, bottom - foot, &color, 1.0);
            let id = if f.doc_id.is_empty() { String::new() } else { format!("Change {}", f.doc_id) };
            titles(&mut o, x + 12.0, top, head, 18.0, &color, &join(&[&id, &f.subtitle]), MUTED);
            let mut rx = right - 12.0;
            if let Some((label, bc)) = &badge {
                let r = Rect { x: rx - badge_w, y: top + head / 2.0 - 10.0, w: badge_w, h: 20.0 };
                o.rrect(r, Some(bc), None, 0.0, 10.0);
                o.text(Pt::new(r.x + r.w / 2.0, r.y + 10.0), label, 10.0, true, WHITE, Align::Center);
                rx = r.x - 12.0;
            }
            put_logo(&mut o, rx, top + head / 2.0, (head - 18.0).min(30.0));
            let who = join(&[
                &if f.author.is_empty() { String::new() } else { format!("Author: {}", f.author) },
                &if f.approver.is_empty() { String::new() } else { format!("Approved by: {}", f.approver) },
                file,
            ]);
            let fy = bottom - foot / 2.0;
            o.text(Pt::new(x + 12.0, fy), &who, 11.0, false, &color, Align::Left);
            o.text(Pt::new(x + w / 2.0, fy), &f.note, 11.0, false, &color, Align::Center);
            o.text(Pt::new(right - 12.0, fy), &footer[2], 11.0, false, &color, Align::Right);
        }
        T::Classification => {
            let (label, lc) = level(&f.level);
            o.rect(outer, Some(WHITE), Some(lc), 1.5);
            for by in [top, bottom - 22.0] {
                o.rect(Rect { x, y: by, w, h: 22.0 }, Some(lc), None, 0.0);
                o.text(Pt::new(x + w / 2.0, by + 11.0), label, 11.0, true, WHITE, Align::Center);
            }
            let hb = head - 22.0;
            titles(&mut o, x + 12.0, top + 22.0, hb, 17.0, &color, &f.subtitle, MUTED);
            put_logo(&mut o, right - 12.0, top + 22.0 + hb / 2.0, (hb - 14.0).min(30.0));
            o.hline(x, right, top + head, &tint(lc, 0.55), 0.75);
            o.hline(x, right, bottom - foot, &tint(lc, 0.55), 0.75);
            footer_texts(&mut o, bottom - 22.0 - 12.0, 10.5, &color, 12.0);
        }
        T::Site => {
            o.rect(outer, Some(WHITE), Some(&color), 1.5);
            o.hline(x, right, top + head, &color, 1.0);
            o.hline(x, right, bottom - foot, &color, 1.0);
            titles(&mut o, x + 12.0, top, head, 18.0, &color, &site_line, MUTED);
            let mut rx = right - 12.0;
            if code_w > 0.0 {
                let r = Rect { x: rx - code_w, y: top + head / 2.0 - 18.0, w: code_w, h: 36.0 };
                o.rect(r, Some(&tint(&color, 0.92)), Some(&color), 1.0);
                o.cell(Pt::new(r.x + 2.0, r.y + 2.0), "SITE", &f.doc_id, 14.0, true, &color);
                rx = r.x - 12.0;
            }
            put_logo(&mut o, rx, top + head / 2.0, (head - 18.0).min(32.0));
            // A legend of the line types used, then who and when.
            let fy = bottom - foot / 2.0;
            let mut lx = x + 12.0;
            for (k, c) in &info.legend {
                o.sample(*k, lx, fy, 26.0, c);
                let label = kind_label(*k);
                o.text(Pt::new(lx + 34.0, fy), label, 10.0, false, MUTED, Align::Left);
                lx += 26.0 + 8.0 + text_w(label, 10.0, false) + 18.0;
            }
            o.text(Pt::new(right - 12.0, fy), &join(&[&f.author, &f.note, &date, &page, file]), 10.0, false, MUTED, Align::Right);
        }
        T::Controlled => {
            o.rect(outer, Some(WHITE), Some(&color), 1.5);
            o.hline(x, right, top + head, &color, 1.0);
            o.hline(x, right, bottom - foot, &color, 1.0);
            titles(&mut o, x + 12.0, top, head, 18.0, &color, &f.subtitle, MUTED);
            // Document control table at the right of the header.
            let (tx, rh) = (right - control_w, head / 2.0);
            o.vline(tx, top, top + head, &color, 0.75);
            o.hline(tx, right, top + rh, &color, 0.5);
            for (i, row) in control.iter().enumerate() {
                let mut cx = tx;
                for (j, (caption, value)) in row.iter().enumerate() {
                    if j > 0 {
                        o.vline(cx, top + rh * i as f64, top + rh * (i + 1) as f64, &color, 0.5);
                    }
                    let c = if *caption == "CLASSIFICATION" { level(&f.level).1 } else { color.as_str() };
                    o.cell(Pt::new(cx, top + rh * i as f64 - 2.0), caption, value, 10.0, *caption == "CLASSIFICATION", c);
                    cx += control_cols[j];
                }
            }
            put_logo(&mut o, tx - 12.0, top + head / 2.0, (head - 18.0).min(30.0));
            let fy = bottom - foot / 2.0;
            o.text(Pt::new(x + 12.0, fy), &join(&["Uncontrolled when printed", &f.author]), 10.0, false, MUTED, Align::Left);
            o.text(Pt::new(x + w / 2.0, fy), &f.note, 10.0, false, &color, Align::Center);
            o.text(Pt::new(right - 12.0, fy), &join(&[file, &page]), 10.0, false, &color, Align::Right);
        }
        T::SignOff => {
            o.rect(outer, Some(WHITE), Some(&color), 1.5);
            o.hline(x, right, top + head, &color, 1.0);
            o.hline(x, right, bottom - foot, &color, 1.0);
            titles(&mut o, x + 12.0, top, head, 18.0, &color, &f.subtitle, MUTED);
            let rx = put_logo(&mut o, right - 12.0, top + head / 2.0, (head - 14.0).min(30.0));
            o.text(Pt::new(rx, top + head / 2.0), &join(&[&f.note, &page, file]), 10.0, false, MUTED, Align::Right);
            // Three signature boxes.
            let by = bottom - foot;
            let bw = w / 3.0;
            let date_only = changed.split(' ').next().unwrap_or("");
            for (i, (caption, name, when)) in
                [("PREPARED BY", &f.author, date_only), ("REVIEWED BY", &f.reviewer, ""), ("APPROVED BY", &f.approver, "")].into_iter().enumerate()
            {
                let cx = x + bw * i as f64;
                if i > 0 {
                    o.vline(cx, by, bottom, &color, 0.75);
                }
                o.cell(Pt::new(cx + 4.0, by + 2.0), caption, name, 11.0, true, &color);
                let line_y = by + 52.0;
                o.hline(cx + 10.0, cx + bw * 0.62, line_y, MUTED, 0.75);
                o.hline(cx + bw * 0.68, cx + bw - 10.0, line_y, MUTED, 0.75);
                o.text(Pt::new(cx + bw * 0.68, line_y - 7.0), when, 10.0, false, &color, Align::Left);
                o.text(Pt::new(cx + 10.0, line_y + 9.0), "Signature", 7.0, false, MUTED, Align::Left);
                o.text(Pt::new(cx + bw * 0.68, line_y + 9.0), "Date", 7.0, false, MUTED, Align::Left);
            }
        }
        T::Solution => {
            o.rect(outer, Some(WHITE), None, 0.0);
            o.rect(Rect { x, y: top, w, h: 3.0 }, Some(&color), None, 0.0);
            titles(&mut o, x + 4.0, top + 3.0, head - 3.0, 20.0, &color, &f.subtitle, MUTED);
            let mut rx = right - 4.0;
            let cy = top + 3.0 + (head - 3.0) / 2.0;
            if let Some((label, bc)) = &badge {
                let r = Rect { x: rx - badge_w, y: cy - 18.0, w: badge_w, h: 20.0 };
                o.rrect(r, Some(bc), None, 0.0, 10.0);
                o.text(Pt::new(r.x + r.w / 2.0, r.y + 10.0), label, 10.0, true, WHITE, Align::Center);
            }
            let meta = join(&[
                &if f.version.is_empty() { String::new() } else { format!("Version {}", f.version) },
                &if f.owner.is_empty() { String::new() } else { format!("Owner: {}", f.owner) },
            ]);
            o.text(Pt::new(rx, cy + 14.0), &meta, 10.0, false, MUTED, Align::Right);
            rx -= badge_w.max(text_w(&meta, 10.0, false)) + 14.0;
            put_logo(&mut o, rx, cy, (head - 22.0).min(30.0));
            o.hline(x, right, top + head, &tint(&color, 0.7), 0.75);
            o.hline(x, right, bottom - foot, &tint(&color, 0.7), 0.75);
            let who = join(&[
                &if f.author.is_empty() { String::new() } else { format!("Author: {}", f.author) },
                &if f.reviewer.is_empty() { String::new() } else { format!("Reviewed by: {}", f.reviewer) },
                file,
            ]);
            let fy = bottom - foot / 2.0 + 2.0;
            o.text(Pt::new(x + 4.0, fy), &who, 10.0, false, MUTED, Align::Left);
            o.text(Pt::new(x + w / 2.0, fy), &f.note, 10.0, false, MUTED, Align::Center);
            o.text(Pt::new(right - 4.0, fy), &footer[2], 10.0, false, MUTED, Align::Right);
        }
        T::Proposal => {
            o.rect(outer, Some(WHITE), Some(&tint(&color, 0.7)), 1.0);
            o.rect(Rect { x, y: top, w, h: 4.0 }, Some(&color), None, 0.0);
            let (cy, lh) = (top + 4.0 + 28.0, 34.0);
            // The customer's logo at the left, one's own at the right.
            if let Some(l) = logo2 {
                put_one(&mut o, Some(l), x + 16.0 + one_w(Some(l), lh), cy, lh);
            }
            put_one(&mut o, logo, right - 16.0, cy, lh);
            let ty = if has_sub { cy - 8.0 } else { cy };
            o.text(Pt::new(x + w / 2.0, ty), &f.title, 18.0, true, &color, Align::Center);
            o.text(Pt::new(x + w / 2.0, ty + 18.0), &f.subtitle, 11.0, false, MUTED, Align::Center);
            let sy = top + 60.0;
            o.rect(Rect { x, y: sy, w, h: 24.0 }, Some(&tint(&color, 0.93)), None, 0.0);
            o.text(Pt::new(x + 16.0, sy + 12.0), &customer_line, 11.0, false, &color, Align::Left);
            o.text(Pt::new(right - 16.0, sy + 12.0), &supplier_line, 11.0, false, &color, Align::Right);
            let notice = if !f.note.is_empty() {
                f.note.clone()
            } else if f.customer.is_empty() {
                "Confidential. Do not distribute.".into()
            } else {
                format!("Confidential. Prepared for {}; do not distribute.", f.customer)
            };
            o.hline(x + 16.0, right - 16.0, bottom - foot, &tint(&color, 0.75), 0.75);
            let fy = bottom - foot / 2.0;
            o.text(Pt::new(x + 16.0, fy), &notice, 9.5, false, MUTED, Align::Left);
            o.text(Pt::new(right - 16.0, fy), &join(&[&date, &page, file]), 9.5, false, MUTED, Align::Right);
        }
        T::Presentation => {
            o.rect(outer, Some(WHITE), Some("#e2e8f0"), 1.0);
            o.rect(Rect { x, y: bottom - 5.0, w, h: 5.0 }, Some(&color), None, 0.0);
            let ty = if has_sub { top + 32.0 } else { top + head / 2.0 + 2.0 };
            o.text(Pt::new(x + 28.0, ty), &f.title, 26.0, true, &color, Align::Left);
            o.text(Pt::new(x + 28.0, ty + 26.0), &f.subtitle, 14.0, false, MUTED, Align::Left);
            let fy = bottom - 5.0 - (foot - 5.0) / 2.0;
            let lx = put_one(&mut o, logo, x + 28.0 + one_w(logo, 18.0), fy, 18.0);
            let after_logo = if logo.is_some() { lx + one_w(logo, 18.0) + 20.0 } else { x + 28.0 };
            o.text(Pt::new(after_logo, fy), &join(&[&f.company, &f.author, file]), 10.0, false, MUTED, Align::Left);
            o.text(Pt::new(x + w / 2.0, fy), &f.note, 10.0, false, MUTED, Align::Center);
            o.text(Pt::new(right - 28.0, fy), &join(&[&date, &f.page]), 12.0, true, &color, Align::Right);
            put_one(&mut o, logo2, right - 28.0, top + head / 2.0, 30.0);
        }
        T::Executive => {
            o.rect(outer, Some(WHITE), Some(&color), 1.0);
            o.rect(Rect { x, y: top, w, h: head }, Some(&color), None, 0.0);
            titles(&mut o, x + 16.0, top, head, 18.0, WHITE, &f.subtitle, &tint(&color, 0.7));
            put_logo(&mut o, right - 16.0, top + head / 2.0, (head - 18.0).min(30.0));
            // The summary panel at the right.
            let px = right - PANEL;
            let foot_text = 26.0;
            o.rect(Rect { x: px, y: top + head, w: PANEL, h: outer.h - head - foot_text }, Some(&tint(&color, 0.95)), None, 0.0);
            o.vline(px, top + head, bottom - foot_text, &color, 2.0);
            o.text(Pt::new(px + 14.0, top + head + 18.0), "SUMMARY", 8.0, true, &color, Align::Left);
            for (i, line) in summary.iter().enumerate() {
                o.text(Pt::new(px + 14.0, top + head + 38.0 + 16.0 * i as f64), line, 11.0, false, "#334155", Align::Left);
            }
            o.hline(x, right, bottom - foot_text, &color, 1.0);
            footer_texts(&mut o, bottom - foot_text / 2.0, 10.5, &color, 12.0);
        }
        T::AsBuilt => {
            o.rect(outer, Some(WHITE), Some(&color), 1.5);
            o.hline(x, right, top + head, &color, 1.0);
            let drawn = if f.author.is_empty() { String::new() } else { format!("Drawn by {}", f.author) };
            titles(&mut o, x + 12.0, top, head, 18.0, &color, &join(&[&f.subtitle, &f.note, &drawn]), MUTED);
            // The stamp: red, double framed.
            let red = "#b91c1c";
            let r = Rect { x: right - 12.0 - stamp_w, y: top + head / 2.0 - 15.0, w: stamp_w, h: 30.0 };
            o.rrect(r, None, Some(red), 2.0, 3.0);
            o.rect(Rect { x: r.x + 3.0, y: r.y + 3.0, w: r.w - 6.0, h: r.h - 6.0 }, None, Some(red), 0.75);
            o.text(Pt::new(r.x + r.w / 2.0, r.y + 15.0), "AS-BUILT", 15.0, true, red, Align::Center);
            put_logo(&mut o, r.x - 14.0, top + head / 2.0, (head - 16.0).min(30.0));
            // The handover table along the bottom.
            let by = bottom - foot;
            o.hline(x, right, by, &color, 1.0);
            let cells = [
                ("INSTALLED BY", f.installer.as_str()),
                ("COMMISSIONED", f.commissioned.as_str()),
                ("ACCEPTED BY", f.approver.as_str()),
                ("LAST CHANGE", changed),
                ("PAGE", if f.page.is_empty() { file } else { f.page.as_str() }),
            ];
            let cw = w / cells.len() as f64;
            for (i, (caption, value)) in cells.iter().enumerate() {
                let cx = x + cw * i as f64;
                if i > 0 {
                    o.vline(cx, by, bottom, &color, 0.75);
                }
                o.cell(Pt::new(cx + 2.0, by + 4.0), caption, value, 11.0, false, &color);
                if *caption == "ACCEPTED BY" {
                    o.hline(cx + 10.0, cx + cw - 10.0, by + 40.0, MUTED, 0.75);
                }
            }
        }
        T::Incident => {
            o.rect(outer, Some(WHITE), Some(&color), 1.5);
            o.rect(Rect { x, y: top, w, h: 5.0 }, Some(sev_color), None, 0.0);
            o.hline(x, right, top + head, &color, 1.0);
            o.hline(x, right, bottom - foot, &color, 1.0);
            o.text(Pt::new(x + 12.0, top + 24.0), &f.title, 18.0, true, &color, Align::Left);
            let when = join(&[
                &if f.start.is_empty() { String::new() } else { format!("Start: {}", f.start) },
                &if f.end.is_empty() { String::new() } else { format!("End: {}", f.end) },
                &f.subtitle,
            ]);
            o.text(Pt::new(x + 12.0, top + 46.0), &when, 11.0, false, MUTED, Align::Left);
            if !f.services.is_empty() {
                o.text(Pt::new(x + 12.0, top + 62.0), &format!("Affected: {}", f.services), 11.0, true, sev_color, Align::Left);
            }
            let bw = text_w(&sev_label, 10.0, true) + 20.0;
            let r = Rect { x: right - 12.0 - bw, y: top + 14.0, w: bw, h: 20.0 };
            o.rrect(r, Some(sev_color), None, 0.0, 10.0);
            o.text(Pt::new(r.x + r.w / 2.0, r.y + 10.0), &sev_label, 10.0, true, WHITE, Align::Center);
            let id = if f.doc_id.is_empty() { String::new() } else { format!("Incident {}", f.doc_id) };
            o.text(Pt::new(right - 12.0, top + 46.0), &id, 11.0, true, &color, Align::Right);
            put_logo(&mut o, r.x - 12.0, top + 24.0, 26.0);
            footer_texts(&mut o, bottom - foot / 2.0, 10.5, &color, 12.0);
        }
        T::Location => {
            o.rect(outer, Some(WHITE), Some(&color), 1.5);
            o.hline(x, right, top + head, &color, 1.0);
            o.hline(x, right, bottom - foot, &color, 1.0);
            o.text(Pt::new(x + 12.0, top + 16.0), &breadcrumb(&f.location), 11.0, false, MUTED, Align::Left);
            o.text(Pt::new(x + 12.0, top + 38.0), &f.title, 18.0, true, &color, Align::Left);
            o.text(Pt::new(x + 12.0, top + 58.0), &f.subtitle, 12.0, false, MUTED, Align::Left);
            let mut rx = right - 12.0;
            if code_w > 0.0 {
                let r = Rect { x: rx - code_w, y: top + head / 2.0 - 18.0, w: code_w, h: 36.0 };
                o.rect(r, Some(&tint(&color, 0.92)), Some(&color), 1.0);
                o.cell(Pt::new(r.x + 2.0, r.y + 2.0), "LOCATION", &f.doc_id, 14.0, true, &color);
                rx = r.x - 12.0;
            }
            put_logo(&mut o, rx, top + head / 2.0, (head - 18.0).min(32.0));
            footer_texts(&mut o, bottom - foot / 2.0, 10.5, &color, 12.0);
        }
        T::Noc => {
            let (bg, line, soft) = ("#0b1220", "#1f2a44", "#94a3b8");
            // A bright accent: the frame color, unless it is the dark default.
            let accent = if color == "#1e293b" { "#22d3ee".to_string() } else { color.clone() };
            o.rrect(outer, Some(bg), Some(&accent), 1.5, 6.0);
            o.hline(x, right, top + head, line, 1.0);
            o.hline(x, right, bottom - foot, line, 1.0);
            titles(&mut o, x + 18.0, top, head, 22.0, WHITE, &f.subtitle, soft);
            let mut rx = right - 18.0;
            if let Some((label, bc)) = &badge {
                let r = Rect { x: rx - badge_w, y: top + head / 2.0 - 11.0, w: badge_w, h: 22.0 };
                o.rrect(r, Some(bc), None, 0.0, 11.0);
                o.text(Pt::new(r.x + r.w / 2.0, r.y + 11.0), label, 10.0, true, WHITE, Align::Center);
                rx = r.x - 14.0;
            }
            put_logo(&mut o, rx, top + head / 2.0, (head - 22.0).min(30.0));
            footer_texts(&mut o, bottom - foot / 2.0, 11.0, soft, 18.0);
        }
    }
    // A watermark across the diagram, over it.
    let mut overlay = Out(vec![]);
    let mark = f.watermark.trim();
    if !mark.is_empty() {
        let diag = content.w.hypot(content.h).max(200.0);
        let size = (diag * 0.7 / (mark.chars().count() as f64 * 0.62)).clamp(24.0, 220.0);
        let c = if mark.to_uppercase().contains("CONFIDENTIAL") || mark.to_uppercase().contains("RESTRICTED") { "#dc2626" } else { "#64748b" };
        let at = Pt::new(content.x + content.w / 2.0, content.y + content.h / 2.0);
        overlay.0.push(Prim::Watermark { at, text: mark.to_uppercase(), size, color: c.into(), angle: -30.0, opacity: 0.13 });
    }
    FrameLayout { outer, prims: o.0, overlay: overlay.0 }
}

// Logo

/// Largest logo file accepted (it is kept inside the diagram file).
pub const LOGO_MAX_BYTES: usize = 1024 * 1024;

/// A PNG or JPEG file as a data URI, with its size in pixels.
pub fn logo_from_file(path: &std::path::Path) -> Result<(String, f64, f64), String> {
    let bytes = std::fs::read(path).map_err(|e| format!("cannot read {}: {e}", path.display()))?;
    logo_from_bytes(&bytes)
}

/// A logo given as a file path or as a `data:` URI with base64 content
/// (as a browser reads an image).
pub fn logo_from(value: &str) -> Result<(String, f64, f64), String> {
    let Some(rest) = value.strip_prefix("data:") else { return logo_from_file(std::path::Path::new(value)) };
    let data = rest.split_once(";base64,").map(|(_, d)| d).ok_or("the logo must be base64 encoded")?;
    logo_from_bytes(&base64_decode(data).ok_or("the logo's data is not valid base64")?)
}

/// Whether `logo` is a PNG or JPEG kept inside the diagram (a data URI),
/// as every logo set in Stratagram is. Anything else, such as a web
/// address, would be fetched from elsewhere when the diagram is shown.
pub fn is_embedded_logo(logo: &str) -> bool {
    ["data:image/png;base64,", "data:image/jpeg;base64,"].iter().any(|p| logo.starts_with(p))
}

/// A PNG or JPEG image as a data URI, with its size in pixels.
pub fn logo_from_bytes(bytes: &[u8]) -> Result<(String, f64, f64), String> {
    if bytes.len() > LOGO_MAX_BYTES {
        return Err(format!("the logo is larger than {} KB", LOGO_MAX_BYTES / 1024));
    }
    let (mime, (w, h)) = if bytes.starts_with(b"\x89PNG\r\n\x1a\n") && bytes.len() >= 24 {
        let be = |i: usize| u32::from_be_bytes([bytes[i], bytes[i + 1], bytes[i + 2], bytes[i + 3]]) as f64;
        ("image/png", (be(16), be(20)))
    } else if bytes.starts_with(&[0xff, 0xd8]) {
        ("image/jpeg", jpeg_size(bytes).ok_or("cannot read the JPEG's size")?)
    } else {
        return Err("the logo must be a PNG or JPEG image".into());
    };
    if w <= 0.0 || h <= 0.0 {
        return Err("the logo has no size".into());
    }
    Ok((format!("data:{mime};base64,{}", base64_encode(bytes)), w, h))
}

/// Width and height from a JPEG's start-of-frame marker.
fn jpeg_size(b: &[u8]) -> Option<(f64, f64)> {
    let mut i = 2;
    while i + 9 < b.len() {
        if b[i] != 0xff {
            return None;
        }
        let marker = b[i + 1];
        let len = u16::from_be_bytes([b[i + 2], b[i + 3]]) as usize;
        // SOF0..SOF15, except DHT (C4), JPG (C8) and DAC (CC).
        if (0xc0..=0xcf).contains(&marker) && ![0xc4, 0xc8, 0xcc].contains(&marker) {
            let h = u16::from_be_bytes([b[i + 5], b[i + 6]]) as f64;
            let w = u16::from_be_bytes([b[i + 7], b[i + 8]]) as f64;
            return Some((w, h));
        }
        i += 2 + len;
    }
    None
}

fn base64_encode(bytes: &[u8]) -> String {
    const A: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::with_capacity(bytes.len().div_ceil(3) * 4);
    for c in bytes.chunks(3) {
        let n = (c[0] as u32) << 16 | (*c.get(1).unwrap_or(&0) as u32) << 8 | *c.get(2).unwrap_or(&0) as u32;
        for k in 0..4 {
            if k <= c.len() {
                out.push(A[(n >> (18 - 6 * k) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }
    out
}

fn base64_decode(text: &str) -> Option<Vec<u8>> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => Some(c - b'A'),
        b'a'..=b'z' => Some(c - b'a' + 26),
        b'0'..=b'9' => Some(c - b'0' + 52),
        b'+' => Some(62),
        b'/' => Some(63),
        _ => None,
    };
    let digits: Vec<u8> = text.bytes().filter(|c| !c.is_ascii_whitespace()).take_while(|&c| c != b'=').collect();
    let mut out = Vec::with_capacity(digits.len() * 3 / 4);
    for c in digits.chunks(4) {
        if c.len() == 1 {
            return None;
        }
        let mut n = 0u32;
        for (k, &d) in c.iter().enumerate() {
            n |= (value(d)? as u32) << (18 - 6 * k);
        }
        out.extend(n.to_be_bytes()[1..c.len()].iter());
    }
    Some(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_round_trips() {
        for bytes in [&b""[..], b"a", b"ab", b"abc", b"abcd", &[0, 255, 128, 7, 9]] {
            assert_eq!(base64_decode(&base64_encode(bytes)).as_deref(), Some(bytes));
        }
        assert_eq!(base64_decode("not base64!"), None);
    }

    fn texts(l: &FrameLayout) -> Vec<&str> {
        l.prims
            .iter()
            .filter_map(|p| match p {
                Prim::Text { text, .. } => Some(text.as_str()),
                _ => None,
            })
            .collect()
    }

    fn info() -> Info {
        Info { changed: "2026-10-18 14:32".into(), file: "campus.stratagram".into(), legend: vec![] }
    }

    #[test]
    fn frame_surrounds_the_content_with_its_margin() {
        let content = Rect { x: 100.0, y: 100.0, w: 600.0, h: 300.0 };
        for t in FrameTemplate::ALL {
            let f = Frame { template: *t, author: "Shin".into(), note: "Internal".into(), ..Frame::default() };
            let l = layout(&f, content, &info());
            let o = l.outer;
            assert!(o.x <= 60.0 && o.y <= 60.0 && o.x + o.w >= 740.0 && o.y + o.h >= 440.0, "{t:?}: {o:?}");
            if *t != FrameTemplate::Executive {
                assert!((o.x + o.w / 2.0 - 400.0).abs() < 1e-9, "centered on the content");
            }
            let all = texts(&l).join(" | ");
            for want in ["Network diagram", "Shin", "2026-10-18"] {
                assert!(all.contains(want), "{t:?}: {all}");
            }
            if *t != FrameTemplate::Report {
                assert!(all.contains("Internal"), "{t:?}: {all}");
            }
        }
    }

    #[test]
    fn frame_widens_for_long_texts_and_leaves_out_what_is_off() {
        let content = Rect { x: 0.0, y: 0.0, w: 50.0, h: 50.0 };
        let f = Frame { title: "A very long title for a very small diagram".into(), show_date: false, ..Frame::default() };
        let l = layout(&f, content, &info());
        assert!(l.outer.w > text_w(&f.title, 18.0, true));
        assert!(!texts(&l).iter().any(|t| t.contains("2026")));
        assert!(!texts(&l).iter().any(|t| t.contains("campus.stratagram")), "the file name is off by default");
    }

    #[test]
    fn templates_show_their_own_fields() {
        let content = Rect { x: 0.0, y: 0.0, w: 600.0, h: 300.0 };
        let f = |t| Frame {
            template: t,
            company: "ACME".into(),
            doc_id: "CR-1042".into(),
            revision: "C".into(),
            approver: "Kim".into(),
            status: "approved".into(),
            level: "confidential".into(),
            address: "Main St 1".into(),
            page: "2/3".into(),
            show_file: true,
            revisions: vec![Revision { rev: "B".into(), date: "2026-10-01".into(), change: "Added DMZ".into(), author: "Kim".into() }],
            version: "1.2".into(),
            owner: "Network team".into(),
            review_date: "2027-04-01".into(),
            reviewer: "Lee".into(),
            customer: "Globex".into(),
            summary: "Two sites joined by IPsec over the internet, with a VXLAN overlay between the data centers.".into(),
            installer: "Ops crew".into(),
            commissioned: "2026-09-30".into(),
            severity: "p1".into(),
            start: "09:12".into(),
            end: "10:40".into(),
            services: "VPN, Email".into(),
            location: "EU > Berlin / HQ > Room 2.14".into(),
            ..Frame::default()
        };
        let mut i = info();
        i.legend = vec![(LinkKind::Trunk, "#334155".into()), (LinkKind::Ipsec, "#16a34a".into())];
        let all = |t| texts(&layout(&f(t), content, &i)).join(" | ");
        use FrameTemplate as T;
        assert!(all(T::Corporate).contains("ACME"));
        assert!(all(T::Change).contains("Change CR-1042") && all(T::Change).contains("APPROVED"));
        assert!(all(T::Change).contains("Approved by: Kim"));
        assert!(all(T::Classification).contains("CONFIDENTIAL"));
        assert!(all(T::Site).contains("Main St 1") && all(T::Site).contains("802.1Q trunk") && all(T::Site).contains("IPsec tunnel"));
        assert!(all(T::Iso).contains("DRAWING NO.") && all(T::Iso).contains("CR-1042") && all(T::Iso).contains("2/3"));
        assert!(all(T::Revision).contains("Added DMZ"));
        assert!(all(T::Lines).contains("campus.stratagram") && all(T::Lines).contains("Page 2/3"));
        let has = |t, want: &[&str]| {
            let all = all(t);
            for w in want {
                assert!(all.contains(w), "{t:?} lacks {w}: {all}");
            }
        };
        has(T::Controlled, &["Uncontrolled when printed", "CR-1042", "1.2", "CONFIDENTIAL", "Network team", "2027-04-01"]);
        has(T::SignOff, &["PREPARED BY", "REVIEWED BY", "APPROVED BY", "Lee", "Kim", "Signature"]);
        has(T::Solution, &["Version 1.2", "Owner: Network team", "Reviewed by: Lee", "APPROVED"]);
        has(T::Proposal, &["Prepared for: Globex", "Prepared by: ACME"]);
        has(T::Executive, &["SUMMARY", "Two sites joined"]);
        has(T::AsBuilt, &["AS-BUILT", "Ops crew", "2026-09-30", "ACCEPTED BY"]);
        has(T::Incident, &["P1 · CRITICAL", "Incident CR-1042", "Start: 09:12", "Affected: VPN, Email"]);
        has(T::Location, &["EU  ›  Berlin  ›  HQ  ›  Room 2.14", "LOCATION"]);
        has(T::Noc, &["APPROVED"]);
        // The summary is wrapped to fit its panel.
        let l = layout(&f(T::Executive), content, &i);
        assert!(texts(&l).iter().filter(|t| t.starts_with("Two") || t.contains("VXLAN")).count() >= 1);
        assert!(!texts(&l).iter().any(|t| text_w(t, 11.0, false) > 260.0));
    }

    #[test]
    fn presentation_is_16_by_9_and_watermarks_go_over_the_diagram() {
        for content in [Rect { x: 0.0, y: 0.0, w: 300.0, h: 500.0 }, Rect { x: 0.0, y: 0.0, w: 1600.0, h: 200.0 }] {
            let f = Frame { template: FrameTemplate::Presentation, ..Frame::default() };
            let o = layout(&f, content, &info()).outer;
            assert!((o.w / o.h - 16.0 / 9.0).abs() < 1e-9, "{o:?}");
            assert!(o.x <= -40.0 && o.x + o.w >= content.w + 40.0 && o.y <= -40.0 && o.y + o.h >= content.h + 40.0);
        }
        let f = Frame { watermark: "Draft".into(), ..Frame::default() };
        let l = layout(&f, Rect { x: 0.0, y: 0.0, w: 600.0, h: 300.0 }, &info());
        assert!(matches!(&l.overlay[..], [Prim::Watermark { text, at, .. }] if text == "DRAFT" && *at == Pt::new(300.0, 150.0)));
        assert!(l.prims.iter().all(|p| !matches!(p, Prim::Watermark { .. })));
    }

    #[test]
    fn logos_from_elsewhere_are_dropped_when_a_diagram_is_opened() {
        let file = r#"{"format": "stratagram", "version": 1, "devices": [], "links": [], "layers": [
            {"id": 1, "name": "Layer 1"},
            {"id": 2, "name": "Frame", "frame": {"logo": "https://tracker.example/pixel.png", "logo_w": 5, "logo_h": 5,
                "logo2": "data:image/png;base64,AAAA", "logo2_w": 2, "logo2_h": 1}}]}"#;
        let d = crate::model::Diagram::from_json(file).unwrap();
        let f = d.layers.iter().find_map(|l| l.frame.as_ref()).unwrap();
        assert_eq!((f.logo.as_str(), f.logo_w, f.logo_h), ("", 0.0, 0.0));
        assert_eq!((f.logo2.as_str(), f.logo2_w), ("data:image/png;base64,AAAA", 2.0));
        assert!(is_embedded_logo("data:image/jpeg;base64,/9j/") && !is_embedded_logo("data:image/svg+xml;base64,PHN2Zz4="));
    }

    #[test]
    fn logos_keep_their_proportions() {
        let content = Rect { x: 0.0, y: 0.0, w: 600.0, h: 300.0 };
        let f = Frame { logo: "data:image/png;base64,AAAA".into(), logo_w: 200.0, logo_h: 100.0, ..Frame::default() };
        let l = layout(&f, content, &info());
        let img = l.prims.iter().find_map(|p| if let Prim::Image { r, .. } = p { Some(*r) } else { None }).unwrap();
        assert!((img.w / img.h - 2.0).abs() < 1e-9);
        assert!(img.y >= l.outer.y && img.x + img.w <= l.outer.x + l.outer.w);
    }

    #[test]
    fn base64_and_image_sizes() {
        assert_eq!(base64_encode(b"Man"), "TWFu");
        assert_eq!(base64_encode(b"Ma"), "TWE=");
        assert_eq!(base64_encode(b"M"), "TQ==");
        let mut jpeg = vec![0xff, 0xd8, 0xff, 0xe0, 0, 4, 0, 0];
        jpeg.extend([0xff, 0xc0, 0, 11, 8, 0, 50, 0, 120, 3, 0, 0, 0]);
        assert_eq!(jpeg_size(&jpeg), Some((120.0, 50.0)));
    }

    #[test]
    fn tints_mix_with_white() {
        assert_eq!(tint("#000000", 0.5), "#808080");
        assert_eq!(tint("#1e3a8a", 0.0), "#1e3a8a");
    }
}
