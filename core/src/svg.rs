// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Export the whole diagram as a standalone SVG.

use crate::geom::{self, HeadShape};
use crate::model::*;
use std::collections::HashMap;
use std::fmt::Write;

const MARGIN: f64 = 40.0;
const FONT: &str = "DejaVu Sans, Helvetica, Arial, sans-serif";

fn esc(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;").replace('"', "&quot;")
}

fn pts_attr(pts: &[Pt]) -> String {
    pts.iter().map(|p| format!("{:.1},{:.1}", p.x, p.y)).collect::<Vec<_>>().join(" ")
}

pub fn dash_array(style: LineStyle, width: f64) -> Option<String> {
    match style {
        LineStyle::Solid => None,
        LineStyle::Dashed => Some(format!("{:.1} {:.1}", 4.0 * width + 2.0, 2.5 * width + 2.0)),
        LineStyle::Dotted => Some(format!("{:.1} {:.1}", width, 1.5 * width + 1.0)),
    }
}

/// [`export`] in `theme`'s colors.
pub fn export_themed(doc: &Diagram, file: &str, theme: crate::theme::Theme) -> String {
    theme.recolor_svg(&export(doc, file))
}

/// Hidden layers are left out.
/// `file` is the diagram's file name, for frames that show it.
pub fn export(doc: &Diagram, file: &str) -> String {
    let doc = &doc.visible_only();
    // With a frame, the page is the frame plus a little room.
    let frame = crate::frame::of(doc, file);
    let b = match &frame {
        Some(f) => f.outer.inflate(10.0),
        None => doc
            .bounds()
            .into_iter()
            .map(|b| b.inflate(MARGIN))
            .chain(doc.layer_backgrounds().into_iter().map(|(_, r, _)| r.inflate(10.0)))
            .chain(geom::label_rects(doc).into_iter().map(|r| r.inflate(10.0)))
            .reduce(|a, b| a.union(&b))
            .unwrap_or(Rect { x: 0.0, y: 0.0, w: 200.0, h: 100.0 }.inflate(MARGIN)),
    };
    let mut s = String::new();
    let _ = writeln!(
        s,
        r##"<svg xmlns="http://www.w3.org/2000/svg" viewBox="{:.1} {:.1} {:.1} {:.1}" width="{:.0}" height="{:.0}" font-family="{FONT}">"##,
        b.x, b.y, b.w, b.h, b.w, b.h
    );
    let _ = writeln!(s, r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="#ffffff"/>"##, b.x, b.y, b.w, b.h);
    for p in frame.iter().flat_map(|f| &f.prims) {
        frame_prim(&mut s, p);
    }

    // Layer backgrounds, under everything on the layers.
    for (_, r, bg) in doc.layer_backgrounds() {
        let stroke = if bg.border == BorderStyle::None {
            r#" stroke="none""#.to_string()
        } else {
            let dash = match bg.border {
                BorderStyle::Dashed => format!(r#" stroke-dasharray="{:.1} {:.1}""#, 4.0 * bg.border_width + 2.0, 2.5 * bg.border_width + 2.0),
                BorderStyle::Dotted => format!(r#" stroke-dasharray="{:.1} {:.1}""#, bg.border_width, 1.5 * bg.border_width + 1.0),
                _ => String::new(),
            };
            format!(r#" stroke="{}" stroke-width="{:.1}"{dash}"#, esc(&bg.border_color), bg.border_width)
        };
        let _ = writeln!(
            s,
            r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.1}" fill="{}" fill-opacity="{:.2}"{stroke}/>"##,
            r.x,
            r.y,
            r.w,
            r.h,
            if bg.rounded { bg.radius } else { 0.0 },
            esc(&bg.fill),
            bg.opacity / 100.0
        );
    }
    // Labels slide along their lines where they would overlap.
    let spots = geom::label_spots(doc);
    // Back to front in stacking order.
    let dirs = geom::port_link_dirs(doc);
    for it in doc.z_order() {
        match it {
            Item::Link(i) => link(&mut s, doc, &doc.links[i], &spots),
            // Items deeper in (deep zoom) are drawn at full size in a group
            // scaled down around their corner, like on screen.
            Item::Device(i) => {
                let d = &doc.devices[i];
                let local = Device { w: d.w / d.scale, h: d.h / d.scale, scale: 1.0, ..d.clone() };
                scaled(&mut s, Pt::new(d.x, d.y), d.scale, |s| device(s, doc, &local, &dirs));
            }
            Item::Note(i) => {
                let n = &doc.notes[i];
                let local = Note { scale: 1.0, ..n.clone() };
                scaled(&mut s, Pt::new(n.x, n.y), n.scale, |s| note(s, &local));
            }
            // Connection points are invisible (the line just ends there);
            // their labels show.
            Item::Point(i) => {
                let q = &doc.points[i];
                if !q.label.is_empty() {
                    let (at, align, base) = geom::point_label_anchor(doc, q);
                    let size = 11.0 * q.scale;
                    let anchor = match align {
                        "left" => "start",
                        "right" => "end",
                        _ => "middle",
                    };
                    let y = match base {
                        "top" => at.y + size * 0.8,
                        "bottom" => at.y - size * 0.2,
                        _ => at.y + size * 0.35,
                    };
                    let _ = writeln!(
                        s,
                        r##"<text x="{:.1}" y="{y:.1}" font-size="{size:.1}" text-anchor="{anchor}" fill="#334155">{}</text>"##,
                        at.x,
                        esc(&q.label)
                    );
                }
            }
        }
    }
    for p in frame.iter().flat_map(|f| &f.overlay) {
        frame_prim(&mut s, p);
    }
    s.push_str("</svg>\n");
    s
}

fn frame_prim(s: &mut String, p: &crate::frame::Prim) {
    use crate::frame::{Align, Prim};
    match p {
        Prim::Rect { r, fill, stroke, width, radius, dash } => {
            let dash = if *dash { r#" stroke-dasharray="4 3""# } else { "" };
            let _ = writeln!(
                s,
                r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{radius:.1}" fill="{}" stroke="{}" stroke-width="{width:.2}"{dash}/>"##,
                r.x,
                r.y,
                r.w,
                r.h,
                fill.as_deref().unwrap_or("none"),
                stroke.as_deref().unwrap_or("none")
            );
        }
        Prim::Line { a, b, color, width, dash } => {
            let dash = if *dash { r#" stroke-dasharray="5 3""# } else { "" };
            let _ = writeln!(
                s,
                r##"<line x1="{:.1}" y1="{:.1}" x2="{:.1}" y2="{:.1}" stroke="{color}" stroke-width="{width:.2}"{dash}/>"##,
                a.x, a.y, b.x, b.y
            );
        }
        Prim::Watermark { at, text, size, color, angle, opacity } => {
            let _ = writeln!(
                s,
                r##"<text x="{:.1}" y="{:.1}" font-size="{size:.1}" font-weight="bold" text-anchor="middle" fill="{color}" fill-opacity="{opacity}" transform="rotate({angle} {:.1} {:.1})">{}</text>"##,
                at.x,
                at.y + size * 0.35,
                at.x,
                at.y,
                esc(text)
            );
        }
        Prim::Image { r, data } => {
            let _ = writeln!(
                s,
                r##"<image x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" preserveAspectRatio="none" href="{}"/>"##,
                r.x,
                r.y,
                r.w,
                r.h,
                esc(data)
            );
        }
        Prim::Text { at, text, size, bold, color, align } => {
            let anchor = match align {
                Align::Left => "start",
                Align::Center => "middle",
                Align::Right => "end",
            };
            let weight = if *bold { r#" font-weight="bold""# } else { "" };
            let _ = writeln!(
                s,
                r##"<text x="{:.1}" y="{:.1}" font-size="{size:.1}"{weight} text-anchor="{anchor}" fill="{color}">{}</text>"##,
                at.x,
                at.y + size * 0.35,
                esc(text)
            );
        }
    }
}

fn link(s: &mut String, doc: &Diagram, l: &Link, spots: &HashMap<Id, (Pt, f64)>) {
    let Some(route) = geom::link_route(doc, l) else { return };
    // A bent line runs on through its joints: no ends there, and its bands
    // go once along the whole of it, with the piece drawn first.
    let open = (!geom::is_joint(doc, l.from), !geom::is_joint(doc, l.to));
    let g = geom::link_geometry_ends(&route, &l.style, l.scale, open);
    let bent = geom::bent_line(doc, l);
    let band_route = if bent.pieces.len() > 1 {
        let z: Vec<Id> = doc.z_order().into_iter().map(|it| doc.item_id(it)).collect();
        let rank = |id: Id| z.iter().position(|x| *x == id).unwrap_or(usize::MAX);
        if geom::band_piece(&bent, rank) == l.id { geom::bent_route(doc, &bent) } else { None }
    } else {
        Some(route.clone())
    };
    let c = esc(&l.style.color);
    let dash = dash_array(l.style.style, l.style.width)
        .map(|d| format!(r##" stroke-dasharray="{d}""##))
        .unwrap_or_default();
    let pts = pts_attr(&g.line);
    let (w, kind) = (l.style.width, l.style.kind);
    // Tunnels and what carries the line: tinted bands, outermost first,
    // their ends cut along the device borders; the edge solid, or dashed by
    // type (VXLAN, CAPWAP, SD-WAN).
    let bands = if band_route.is_some() { l.style.bands() } else { vec![] };
    for (i, (k, extra)) in bands.into_iter().enumerate() {
        let band = pts_attr(&geom::tunnel_band_between(doc, bent.from, bent.to, band_route.as_deref().unwrap_or(&route), w / 2.0 + extra * l.scale));
        let edge_dash = geom::band_dash(k)
            .map(|(a, b)| format!(r#" stroke-dasharray="{:.1} {:.1}""#, a * l.scale, b * l.scale))
            .unwrap_or_default();
        if i == 0 {
            let _ = writeln!(s, r##"<polygon points="{band}" fill="#ffffff" stroke="none"/>"##);
        }
        let _ = writeln!(
            s,
            r##"<polygon points="{band}" fill="{c}" fill-opacity="0.14" stroke="{c}" stroke-width="{:.1}"{edge_dash}/>"##,
            l.scale
        );
    }
    if kind.is_double() {
        // A double line: two strokes of the line's width with a gap.
        let _ = writeln!(
            s,
            r##"<polyline points="{pts}" fill="none" stroke="{c}" stroke-width="{:.1}" stroke-linejoin="round"{dash}/>"##,
            2.0 * w + 3.0 * l.scale
        );
        let _ = writeln!(
            s,
            r##"<polyline points="{pts}" fill="none" stroke="#ffffff" stroke-width="{:.1}" stroke-linejoin="round"{dash}/>"##,
            3.0 * l.scale
        );
    } else {
        let _ = writeln!(s, r##"<polyline points="{pts}" fill="none" stroke="{c}" stroke-width="{w:.1}" stroke-linejoin="round"{dash}/>"##);
    }
    for b in &g.beads {
        // Switched (L2): filled squares near both ends for the ports.
        if kind == LinkKind::L2 {
            let r = geom::BEAD_RADIUS * l.scale * 0.9 + w / 2.0;
            let _ = writeln!(
                s,
                r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{c}"/>"##,
                b.x - r,
                b.y - r,
                2.0 * r,
                2.0 * r
            );
            continue;
        }
        // Routed (L3): open circles near both ends for the IP interfaces.
        let _ = writeln!(
            s,
            r##"<circle cx="{:.1}" cy="{:.1}" r="{:.1}" fill="#ffffff" stroke="{c}" stroke-width="{:.1}"/>"##,
            b.x,
            b.y,
            geom::BEAD_RADIUS * l.scale + w / 2.0,
            (w * 0.7).max(1.2 * l.scale)
        );
    }
    if !g.ticks.is_empty() {
        // MPLS: strokes across the line, like a label stack.
        let d: String = g.ticks.iter().map(|(a, b)| format!("M{:.1} {:.1}L{:.1} {:.1}", a.x, a.y, b.x, b.y)).collect();
        let _ = writeln!(s, r##"<path d="{d}" fill="none" stroke="{c}" stroke-width="{:.1}"/>"##, (w * 0.75).max(l.scale));
    }
    for h in &g.heads {
        match h {
            HeadShape::Filled(p) => {
                let _ = writeln!(s, r##"<polygon points="{}" fill="{c}" stroke="{c}" stroke-width="1" stroke-linejoin="round"/>"##, pts_attr(p));
            }
            HeadShape::Chevron(p) => {
                let _ = writeln!(
                    s,
                    r##"<polyline points="{}" fill="none" stroke="{c}" stroke-width="{:.1}" stroke-linejoin="round" stroke-linecap="round"/>"##,
                    pts_attr(p),
                    l.style.width
                );
            }
            HeadShape::Circle { c: ctr, r } => {
                let _ = writeln!(
                    s,
                    r##"<circle cx="{:.1}" cy="{:.1}" r="{:.1}" fill="#ffffff" stroke="{c}" stroke-width="{:.1}"/>"##,
                    ctr.x,
                    ctr.y,
                    r,
                    l.style.width.max(1.5)
                );
            }
        }
    }
    let tag = if geom::shows_tag(doc, l) { l.style.full_tag() } else { String::new() };
    if !l.label_hidden && (!l.label.is_empty() || !tag.is_empty()) {
        let at = spots.get(&l.id).map_or(g.label_pos, |s| s.0);
        scaled(s, at, l.scale, |s| label(s, l, &tag, at, &c));
    }
}

/// Write with `draw` inside a group scaled by `scale` around `at`.
fn scaled(s: &mut String, at: Pt, scale: f64, draw: impl FnOnce(&mut String)) {
    if scale == 1.0 {
        draw(s);
        return;
    }
    let _ = writeln!(
        s,
        r#"<g transform="translate({:.3} {:.3}) scale({}) translate({:.3} {:.3})">"#,
        at.x, at.y, scale, -at.x, -at.y
    );
    draw(s);
    s.push_str("</g>\n");
}

/// The link's label in a box, after its type's tag (`tag`) in bold.
fn label(s: &mut String, l: &Link, tag: &str, p: Pt, c: &str) {
    let tag_w = tag.chars().count() as f64 * 6.4;
    let label_w = l.label.chars().count() as f64 * 6.6;
    let gap = if tag_w > 0.0 && label_w > 0.0 { 6.0 } else { 0.0 };
    let w = tag_w + gap + label_w + 10.0;
    let x0 = p.x - w / 2.0;
    let _ = writeln!(
        s,
        r##"<rect x="{x0:.1}" y="{:.1}" width="{w:.1}" height="17" rx="3" fill="#ffffff" stroke="{c}" stroke-width="0.8"/>"##,
        p.y - 8.5
    );
    if !tag.is_empty() {
        let _ = writeln!(
            s,
            r##"<text x="{:.1}" y="{:.1}" font-size="10" font-weight="bold" fill="{c}">{tag}</text>"##,
            x0 + 5.0,
            p.y + 3.5
        );
    }
    if !l.label.is_empty() {
        let _ = writeln!(
            s,
            r##"<text x="{:.1}" y="{:.1}" font-size="11" fill="{c}">{}</text>"##,
            x0 + 5.0 + tag_w + gap,
            p.y + 4.0,
            esc(&l.label)
        );
    }
}

fn device(s: &mut String, doc: &Diagram, d: &Device, dirs: &HashMap<Id, Vec<Pt>>) {
    let fill = esc(&d.color);
    let stroke = geom::darken(&d.color, 0.45);
    let text = geom::text_on(&d.color);
    // A device with contents has its name at the top, out of their way.
    let container = doc.containers().values().any(|c| *c == d.id);
    let mut shape = crate::shape::device_shape(&d.kind, &d.name, &d.rect(), container, d.panel.as_ref());
    crate::shape::add_cords(&mut shape, d.front().as_ref(), &d.rect(), &doc.panel_cords(d));
    let _ = writeln!(s, r##"<path d="{}" fill="{fill}" stroke="{stroke}" stroke-width="1.5"/>"##, shape.outline);
    if !shape.detail.is_empty() {
        let _ = writeln!(
            s,
            r##"<path d="{}" fill="none" stroke="{stroke}" stroke-width="1" stroke-opacity="0.3"/>"##,
            shape.detail
        );
    }
    if !shape.mark.is_empty() {
        let _ = writeln!(
            s,
            r##"<path d="{}" fill="none" stroke="{stroke}" stroke-width="1.8" stroke-linecap="round" stroke-linejoin="round"/>"##,
            shape.mark
        );
    }
    if !shape.solid.is_empty() {
        let _ = writeln!(s, r##"<path d="{}" fill="{stroke}" stroke="{stroke}" stroke-width="0.6" stroke-linejoin="round"/>"##, shape.solid);
    }
    for t in &shape.texts {
        let _ = writeln!(
            s,
            // A halo in the device's color under it keeps a number readable
            // over a cord (as a text of its own: paint-order is not
            // understood everywhere).
            concat!(
                r##"<text x="{x:.1}" y="{y:.1}" font-size="{size:.1}" text-anchor="middle" fill="{fill}" stroke="{fill}" stroke-width="{halo:.1}" stroke-linejoin="round">{text}</text>"##,
                r##"<text x="{x:.1}" y="{y:.1}" font-size="{size:.1}" text-anchor="middle" fill="{stroke}">{text}</text>"##
            ),
            x = t.x,
            y = t.y + t.size * 0.36,
            size = t.size,
            halo = t.size * 0.4,
            text = esc(&t.text),
            fill = fill,
            stroke = stroke
        );
    }
    if let Some(t) = &shape.tile {
        let _ = writeln!(
            s,
            r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" rx="{:.1}" fill="{}"/>"##,
            t.x,
            t.y,
            t.size,
            t.size,
            t.size * 0.22,
            t.bg
        );
        let _ = writeln!(
            s,
            r##"<text x="{:.1}" y="{:.1}" font-size="{:.1}" font-weight="bold" text-anchor="middle" fill="{}">{}</text>"##,
            t.x + t.size / 2.0,
            t.y + t.size / 2.0 + t.font * 0.36,
            t.font,
            t.fg,
            esc(t.text)
        );
    }
    let cx = d.x + d.w / 2.0 + shape.label.0;
    let cy = match (container, d.kind.is_empty()) {
        (true, true) => d.y + crate::editor::CONTAINER_NAME_Y,
        (true, false) => d.y + crate::editor::CONTAINER_NAME_Y + 5.5,
        _ => d.y + d.h / 2.0 + shape.label.1,
    };
    if d.kind.is_empty() {
        let _ = writeln!(
            s,
            r##"<text x="{cx:.1}" y="{:.1}" font-size="13" font-weight="bold" text-anchor="middle" fill="{text}">{}</text>"##,
            cy + 4.5,
            esc(&d.name)
        );
    } else {
        let _ = writeln!(
            s,
            r##"<text x="{cx:.1}" y="{:.1}" font-size="13" font-weight="bold" text-anchor="middle" fill="{text}">{}</text>"##,
            cy - 1.0,
            esc(&d.name)
        );
        let _ = writeln!(
            s,
            r##"<text x="{cx:.1}" y="{:.1}" font-size="10" text-anchor="middle" fill="{text}" fill-opacity="0.7">{}</text>"##,
            cy + 13.0,
            esc(&crate::shape::kind_label(&d.kind).to_uppercase())
        );
    }
    if let Some(name) = doc.layer_badge(d) {
        let _ = writeln!(
            s,
            r##"<text x="{:.1}" y="{:.1}" font-size="9" text-anchor="end" fill="{text}" fill-opacity="0.55">{}</text>"##,
            d.x + d.w - 4.0,
            d.y + d.h - 4.0,
            esc(name)
        );
    }
    // Ports on a hidden layer are left out, with their names.
    for p in d.ports.iter().filter(|p| doc.port_shown(d, p)) {
        // Where its lines start: on a patch panel's edge for a socket.
        let pos = d.port_exit(p);
        let h = geom::PORT_HALF;
        let linked = doc.links_at_port(p.id) > 0;
        // A socket with a line shows its plug and cord instead of the box.
        // (A socket's back is at the panel's edge, drawn like a port there.)
        let socket = if p.rear { None } else { d.socket_pos(p.socket) };
        if !(socket.is_some() && linked) {
            let at = socket.unwrap_or(pos);
            let _ = writeln!(
            s,
            r##"<rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" fill="{}" stroke="{stroke}" stroke-width="1.2"/>"##,
                at.x - h,
                at.y - h,
            2.0 * h,
            2.0 * h,
            if linked { stroke.as_str() } else { "#ffffff" }
        );
        }
        if d.name_shown(p) {
            let (lp, align, base) = geom::port_label(d, p, dirs);
            let anchor = if align == "right" { "end" } else { "start" };
            let y = match base {
                "top" => lp.y + 9.0,
                "middle" => lp.y + 3.5,
                _ => lp.y,
            };
            let _ = writeln!(
                s,
                r##"<text x="{:.1}" y="{y:.1}" font-size="10" text-anchor="{anchor}" fill="#475569">{}</text>"##,
                lp.x,
                esc(&p.name)
            );
        }
    }
}

fn note(s: &mut String, n: &Note) {
    let weight = if n.bold { r#" font-weight="bold""# } else { "" };
    let lh = n.size * NOTE_LINE_HEIGHT;
    // A boxed note is wrapped to its box and cut at its edges.
    let clip = if n.boxed() {
        let r = n.rect();
        let _ = writeln!(
            s,
            r##"<clipPath id="clip-n{}"><rect x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}"/></clipPath>"##,
            n.id,
            r.x,
            r.y,
            r.w,
            r.h
        );
        format!(r#" clip-path="url(#clip-n{})""#, n.id)
    } else {
        String::new()
    };
    let _ = writeln!(
        s,
        r##"<text font-size="{:.1}"{weight} fill="{}" xml:space="preserve"{clip}>"##,
        n.size,
        esc(&n.color)
    );
    for (i, line) in n.shown_lines().iter().enumerate() {
        // Baseline of a line box of height `lh` with the text centered in it.
        let y = n.y + i as f64 * lh + (lh - n.size) / 2.0 + n.size * 0.8;
        let _ = writeln!(s, r##"<tspan x="{:.1}" y="{y:.1}">{}</tspan>"##, n.x, esc(line));
    }
    s.push_str("</text>\n");
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exports_escaped_svg() {
        let mut d = Diagram::default();
        let a = d.add_device("R1 <core>", "router", Rect { x: 0.0, y: 0.0, w: 120.0, h: 60.0 }, "#fde68a");
        let b = d.add_device("SW&1", "", Rect { x: 300.0, y: 0.0, w: 120.0, h: 60.0 }, "#bfdbfe");
        let pa = d.add_port(a, Side::Right, 0.5).unwrap();
        let pb = d.add_port(b, Side::Left, 0.5).unwrap();
        d.add_link(pa, pb, LinkStyle { head_end: Head::Arrow, style: LineStyle::Dashed, ..Default::default() });
        let svg = export(&d, "");
        assert!(svg.starts_with("<svg"));
        assert!(svg.contains("R1 &lt;core&gt;"));
        assert!(svg.contains("SW&amp;1"));
        assert!(svg.contains("stroke-dasharray"));
        assert!(svg.contains("<polygon"));
        d.add_note("DMZ\n<10.0.0.0/24>", Pt::new(0.0, 100.0));
        let svg = export(&d, "");
        assert!(svg.contains(">DMZ</tspan>"));
        assert!(svg.contains(">&lt;10.0.0.0/24&gt;</tspan>"));
    }

    #[test]
    fn leaves_out_hidden_layers_and_follows_stacking() {
        let mut d = Diagram::default();
        d.add_device("front", "", Rect { x: 0.0, y: 0.0, w: 120.0, h: 60.0 }, "#fde68a");
        let back = d.add_device("back", "", Rect { x: 50.0, y: 20.0, w: 120.0, h: 60.0 }, "#bfdbfe");
        d.set_z_order(&[back]);
        let svg = export(&d, "");
        assert!(svg.find(">back<").unwrap() < svg.find(">front<").unwrap());
        let hidden = d.add_layer("hidden");
        d.active_layer = hidden;
        d.add_device("secret", "", Rect { x: 900.0, y: 900.0, w: 120.0, h: 60.0 }, "#bfdbfe");
        d.layer_mut(hidden).unwrap().visible = false;
        let svg = export(&d, "");
        assert!(!svg.contains("secret"));
        // The page only covers what is shown.
        assert!(svg.contains(r#"width="250" height="160""#));
    }
}
