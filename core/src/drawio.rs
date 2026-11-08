// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Export to draw.io / diagrams.net (`.drawio`, uncompressed mxGraph XML).
//!
//! Mapping:
//! - device -> rounded rectangle vertex (an `<object>` so the device type is
//!   kept as a custom property, visible under Edit Data in draw.io)
//! - port   -> small square child vertex of its device, so it moves with it
//! - link   -> edge from port cell to port cell, with arrows, dashes,
//!   routing and label mapped to draw.io styles
//! - port name -> label near that end of its first link, or beside the port
//!   if it has no link
//! - note   -> borderless text vertex
//! - connection point -> loose edge end (sourcePoint / targetPoint)
//! - layer  -> draw.io layer (hidden layers stay hidden); cells are written
//!   in stacking order, which is draw.io's z order

use crate::geom;
use crate::model::*;
use std::collections::HashMap;
use std::fmt::Write;

/// Offset so the diagram starts near the top left of the draw.io page.
const MARGIN: f64 = 40.0;
const PORT: f64 = 2.0 * geom::PORT_HALF;
/// How far along a link a port label sits, as in the app.
const PORT_LABEL_DIST: f64 = geom::PORT_LABEL_ALONG;

fn esc(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\n', "&#xa;")
}

/// Escape text that goes into an `html=1` label (escaped once for HTML,
/// then once more as an XML attribute by the caller).
fn html_text(s: &str) -> String {
    s.replace('&', "&amp;").replace('<', "&lt;").replace('>', "&gt;")
}

fn arrow_style(h: Head) -> (&'static str, u8) {
    match h {
        Head::None => ("none", 0),
        Head::Arrow => ("block", 1),
        Head::Open => ("open", 0),
        Head::Diamond => ("diamond", 1),
        Head::Circle => ("oval", 0),
    }
}

/// Point on the port square's outer edge (relative 0..1), so links leave
/// the port away from the device, as they do in the app.
fn port_constraint(side: Side) -> (f64, f64) {
    match side {
        Side::Top => (0.5, 0.0),
        Side::Right => (1.0, 0.5),
        Side::Bottom => (0.5, 1.0),
        Side::Left => (0.0, 0.5),
    }
}

fn dev_id(id: Id) -> String {
    format!("d{id}")
}
fn port_id(id: Id) -> String {
    format!("p{id}")
}
fn link_id(id: Id) -> String {
    format!("l{id}")
}
/// A connection point's label as a borderless text box, where the app puts it.
fn point_label(out: &mut String, ctx: &Ctx, q: &Point) {
    if q.label.is_empty() {
        return;
    }
    let Ctx { doc, ox, oy, .. } = *ctx;
    let (at, align, base) = geom::point_label_anchor(doc, q);
    let size = 11.0 * q.scale;
    let (w, h) = (q.label.chars().count() as f64 * size * 0.62 + 6.0, size * 1.6);
    let x = match align {
        "left" => at.x,
        "right" => at.x - w,
        _ => at.x - w / 2.0,
    };
    let y = match base {
        "top" => at.y,
        "bottom" => at.y - h,
        _ => at.y - h / 2.0,
    };
    let _ = writeln!(
        out,
        r#"        <mxCell id="q{id}" value="{label}" style="text;html=1;align={align};verticalAlign=middle;whiteSpace=nowrap;spacing=0;fontSize={size:.1};fontColor=#334155;" vertex="1" parent="{layer}">
          <mxGeometry x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" as="geometry"/>
        </mxCell>"#,
        id = q.id,
        label = esc(&html_text(&q.label)),
        layer = ctx.parent(q.layer),
        x = x + ox,
        y = y + oy,
    );
}

fn note_id(id: Id) -> String {
    format!("n{id}")
}
fn layer_id(id: Id) -> String {
    format!("layer{id}")
}

/// What every cell writer needs.
#[derive(Clone, Copy)]
struct Ctx<'a> {
    doc: &'a Diagram,
    ox: f64,
    oy: f64,
    dirs: &'a HashMap<Id, Vec<Pt>>,
    /// Which link (and which end) carries each port's name.
    label_end: &'a HashMap<Id, (Id, bool)>,
    /// Everything in draw.io's default layer (for pasting), not in ours.
    flat: bool,
    /// Where labels sit along their lines (0 to 1), as in the app.
    spots: &'a HashMap<Id, (Pt, f64)>,
}

impl Ctx<'_> {
    /// Cell id of the layer an item goes into.
    fn parent(&self, layer: Id) -> String {
        if self.flat { "1".into() } else { layer_id(layer) }
    }
}

fn device(out: &mut String, ctx: &Ctx, d: &Device) {
    let Ctx { doc, ox, oy, label_end, .. } = *ctx;
    let fill = geom::normalize_color(&d.color).unwrap_or_else(|| "#dbeafe".into());
    let stroke = geom::darken(&fill, 0.45);
    let font = geom::text_on(&fill);
    let mut label = format!("<b>{}</b>", html_text(&d.name));
    if !d.kind.is_empty() {
        label.push_str(&format!(
            "<br><span style=\"font-size:{:.1}px;opacity:0.7\">{}</span>",
            10.0 * d.scale,
            html_text(&crate::shape::kind_label(&d.kind).to_uppercase())
        ));
    }
    // Shape and symbols in the device's own frame (unscaled, at 0,0).
    let container = doc.containers().values().any(|c| *c == d.id);
    let local = Rect { x: 0.0, y: 0.0, w: d.w / d.scale, h: d.h / d.scale };
    let mut shape = crate::shape::device_shape(&d.kind, &d.name, &local, container, d.panel.as_ref());
    crate::shape::add_cords(&mut shape, d.front().as_ref(), &local, &doc.panel_cords(d));
    let mut style = crate::shape::drawio_style(&d.kind, d.w, d.h);
    // Only a provider's tile goes along (draw.io has no other symbols), so
    // only then does the label move aside like in stratagram.
    if shape.tile.is_some() && !container {
        let (dx, dy) = shape.label;
        style.push_str(&format!("spacingLeft={:.1};spacingTop={:.1};", 2.0 * dx * d.scale, 2.0 * dy * d.scale));
    }
    let _ = writeln!(
        out,
        r#"        <object id="{id}" label="{label}" kind="{kind}" stratagram_id="{nid}">
          <mxCell style="{shape}whiteSpace=wrap;html=1;fillColor={fill};strokeColor={stroke};fontColor={font};fontSize={fs13:.1};strokeWidth={sw:.2};" vertex="1" parent="{layer}">
            <mxGeometry x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" as="geometry"/>
          </mxCell>
        </object>"#,
        id = dev_id(d.id),
        layer = ctx.parent(d.layer),
        // Deeper items (deep zoom) have proportionally smaller text and lines.
        fs13 = 13.0 * d.scale,
        sw = 1.5 * d.scale,
        label = esc(&label),
        kind = esc(&d.kind),
        shape = style,
        nid = d.id,
        x = d.x + ox,
        y = d.y + oy,
        w = d.w,
        h = d.h,
    );

    // A cloud provider's tile, as a child so it moves along.
    if let Some(t) = &shape.tile {
        let _ = writeln!(
            out,
            r##"        <mxCell id="{id}tile" value="{text}" style="rounded=1;arcSize=22;html=1;fillColor={bg};strokeColor=none;fontColor={fg};fontStyle=1;fontSize={fs:.1};resizable=0;movable=0;" vertex="1" connectable="0" parent="{id}">
          <mxGeometry x="{x:.1}" y="{y:.1}" width="{s:.1}" height="{s:.1}" as="geometry"/>
        </mxCell>"##,
            id = dev_id(d.id),
            text = esc(&html_text(t.text)),
            bg = t.bg,
            fg = t.fg,
            fs = t.font * d.scale,
            x = t.x * d.scale,
            y = t.y * d.scale,
            s = t.size * d.scale,
        );
    }

    if let Some(name) = doc.layer_badge(d) {
        // Layer name in the lower right corner, as a child so it moves along.
        let _ = writeln!(
            out,
            r##"        <mxCell id="{id}layer" value="{name}" style="text;html=1;align=right;verticalAlign=bottom;fontSize={fs9:.1};fontColor={font};textOpacity=55;spacingRight=4;spacingBottom=2;resizable=0;movable=0;pointerEvents=0;" vertex="1" connectable="0" parent="{id}">
          <mxGeometry width="{w:.1}" height="{h:.1}" as="geometry"/>
        </mxCell>"##,
            id = dev_id(d.id),
            name = esc(&html_text(name)),
            fs9 = 9.0 * d.scale,
            w = d.w,
            h = d.h,
        );
    }
    // Ports on a hidden layer are left out, like hidden items.
    for p in d.ports.iter().filter(|p| doc.port_shown(d, p)) {
        // A patch panel socket's lines meet the panel's edge (its cord runs on to the socket).
        let pos = d.port_exit(p);
        let linked = doc.links_at_port(p.id) > 0;
        // Unlinked ports carry their own name, placed like in the app.
        let (value, label_style) = if !d.name_shown(p) || label_end.contains_key(&p.id) {
            (String::new(), String::new())
        } else {
            let style = match p.side {
                Side::Top => "labelPosition=center;verticalLabelPosition=top;align=center;verticalAlign=bottom;",
                Side::Bottom => "labelPosition=center;verticalLabelPosition=bottom;align=center;verticalAlign=top;",
                Side::Left => "labelPosition=left;verticalLabelPosition=middle;align=right;verticalAlign=middle;",
                Side::Right => "labelPosition=right;verticalLabelPosition=middle;align=left;verticalAlign=middle;",
            };
            (esc(&p.name), style.to_string())
        };
        let _ = writeln!(
            out,
            r##"        <mxCell id="{id}" value="{value}" style="rounded=0;html=1;fillColor={fill};strokeColor={stroke};strokeWidth={pw:.2};resizable=0;fontSize={pf:.1};fontColor=#475569;{label_style}" vertex="1" parent="{parent}">
          <mxGeometry x="{x:.1}" y="{y:.1}" width="{s}" height="{s}" as="geometry"/>
        </mxCell>"##,
            id = port_id(p.id),
            fill = if linked { stroke.as_str() } else { "#ffffff" },
            parent = dev_id(d.id),
            // Child geometry is relative to the parent device.
            x = pos.x - d.x - PORT * d.scale / 2.0,
            y = pos.y - d.y - PORT * d.scale / 2.0,
            s = PORT * d.scale,
            pw = 1.2 * d.scale,
            pf = 10.0 * d.scale,
        );
    }
}

fn link(out: &mut String, ctx: &Ctx, l: &Link) {
    let Ctx { doc, ox, oy, dirs, label_end, .. } = *ctx;
    let Some(route) = geom::link_route(doc, l) else { return };
    let st = &l.style;
    // A bent line runs on through its joints: no arrowheads there.
    let (end, end_fill) = arrow_style(if geom::is_joint(doc, l.to) { Head::None } else { st.head_end });
    let (start, start_fill) = arrow_style(if geom::is_joint(doc, l.from) { Head::None } else { st.head_start });
    let color = geom::normalize_color(&st.color).unwrap_or_else(|| "#334155".into());
    let dash = match st.style {
        LineStyle::Solid => "",
        LineStyle::Dashed => "dashed=1;",
        LineStyle::Dotted => "dashed=1;dashPattern=1 3;",
    };
    // Port ends attach to the port cells; ends at connection points are
    // loose edge ends at that position.
    let side_of = |ep: Endpoint| doc.endpoint_pos(ep).map(|(_, s)| s).unwrap_or(Side::Right);
    let mut attach = String::new();
    let mut ends = String::new();
    let mut terminals = String::new();
    for (ep, at_source) in [(l.from, true), (l.to, false)] {
        let (pos, which, term) = if at_source { (route[0], "exit", "source") } else { (route[route.len() - 1], "entry", "target") };
        if ep.is_point() {
            let _ = write!(ends, r#"<mxPoint x="{:.1}" y="{:.1}" as="{term}Point"/>"#, pos.x + ox, pos.y + oy);
        } else {
            let (cx, cy) = port_constraint(side_of(ep));
            let _ = write!(attach, "{which}X={cx};{which}Y={cy};{which}Dx=0;{which}Dy=0;{which}Perimeter=0;");
            let _ = write!(terminals, r#" {term}="{}""#, port_id(ep.port));
        }
    }
    let routing = match st.routing {
        Routing::Straight => "",
        Routing::Orthogonal => "edgeStyle=orthogonalEdgeStyle;rounded=0;",
        // Bends are written out as waypoints below.
        Routing::Around => "",
    };
    if st.routing == Routing::Around && route.len() > 2 {
        ends.push_str(r#"<Array as="points">"#);
        for p in &route[1..route.len() - 1] {
            let _ = write!(ends, r#"<mxPoint x="{:.1}" y="{:.1}"/>"#, p.x + ox, p.y + oy);
        }
        ends.push_str("</Array>");
    }
    // Trunks as draw.io's double line, tunnels as a wide band; the type is
    // kept for importing back, and its tag goes in front of the label.
    // Something carried over something else shows as one band, as wide
    // as the outermost, in that one's dashes.
    let bands = st.bands();
    let kind = match (st.kind, bands.first()) {
        (_, Some(&(outer, extra))) => {
            let dashed = match outer {
                LinkKind::Vxlan | LinkKind::Sdwan => "dashed=1;",
                LinkKind::Capwap => "dashed=1;dashPattern=1 2;",
                _ => "",
            };
            format!(
                "shape=flexArrow;width={:.1};endSize=0;startSize=0;fillColor={color};fillOpacity={};{dashed}",
                2.0 * extra * l.scale,
                14 * bands.len()
            )
        }
        (k, None) if k.is_double() => "shape=link;".into(),
        _ => String::new(),
    } + &if st.kind == LinkKind::Plain { String::new() } else { format!("stratagramKind={};", st.kind.as_str()) }
        + &if st.over.is_empty() {
            String::new()
        } else {
            format!("stratagramOver={};", st.over.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(","))
        }
        // A hidden label stays with the line, hidden there too.
        + if l.label_hidden { "noLabel=1;" } else { "" };
    // Its type's tag once per bent line.
    let tag = if geom::shows_tag(doc, l) { st.full_tag() } else { String::new() };
    let label = match (tag.as_str(), l.label.is_empty()) {
        ("", _) => html_text(&l.label),
        (tag, true) => format!("<b>{tag}</b>"),
        (tag, false) => format!("<b>{tag}</b>&nbsp; {}", html_text(&l.label)),
    };
    // draw.io places an edge label by x from -1 (start) to 1 (end).
    let along = match ctx.spots.get(&l.id) {
        Some((_, t)) if (t - 0.5).abs() > 1e-9 => format!(r#" x="{:.2}""#, 2.0 * t - 1.0),
        _ => String::new(),
    };
    let geometry = if ends.is_empty() {
        format!(r#"<mxGeometry relative="1"{along} as="geometry"/>"#)
    } else {
        format!(r#"<mxGeometry relative="1"{along} as="geometry">{ends}</mxGeometry>"#)
    };
    let _ = writeln!(
        out,
        r##"        <mxCell id="{id}" value="{label}" style="{routing}{attach}{kind}html=1;endArrow={end};endFill={end_fill};startArrow={start};startFill={start_fill};strokeColor={color};strokeWidth={w};{dash}fontColor={color};fontSize={lf:.1};labelBackgroundColor=#ffffff;labelBorderColor={color};spacingLeft=4;spacingRight=4;spacingTop=1;spacingBottom=1;" edge="1" parent="{layer}"{terminals}>
          {geometry}
        </mxCell>"##,
        id = link_id(l.id),
        layer = ctx.parent(l.layer),
        label = esc(&label),
        w = st.width,
        lf = 11.0 * l.scale,
    );

    // Port names as edge labels near each end.
    let total: f64 = route.windows(2).map(|w| w[0].dist(w[1])).sum();
    if total < 1.0 {
        return;
    }
    for (ep, at_source) in [(l.from, true), (l.to, false)] {
        if ep.is_point() || label_end.get(&ep.port) != Some(&(l.id, at_source)) {
            continue;
        }
        let Some(d) = doc.device(ep.device) else { continue };
        let Some(port) = d.port(ep.port) else { continue };
        if !d.name_shown(port) || !doc.port_shown(d, port) {
            continue;
        }
        let Some((pos, _)) = doc.endpoint_pos(ep) else { continue };
        let (lp, align, base) = geom::port_label(d, port, dirs);
        // draw.io centers edge labels on a point along the edge; shift it
        // so the text lands where the app draws it.
        let first = if at_source { route[1].sub(route[0]) } else { route[route.len() - 2].sub(route[route.len() - 1]) };
        let along = pos.add(first.norm().scale(PORT_LABEL_DIST));
        let half_w = port.name.chars().count() as f64 * 3.1;
        let cx = match align {
            "left" => lp.x + half_w,
            "right" => lp.x - half_w,
            _ => lp.x,
        };
        let cy = match base {
            "bottom" => lp.y - 6.0,
            "top" => lp.y + 6.0,
            _ => lp.y,
        };
        let frac = (2.0 * PORT_LABEL_DIST / total).min(0.9);
        let x = if at_source { -1.0 + frac } else { 1.0 - frac };
        let _ = writeln!(
            out,
            r##"        <mxCell id="{lid}n{pid}" value="{value}" style="edgeLabel;html=1;align=center;verticalAlign=middle;resizable=0;points=[];fontSize={pf:.1};fontColor=#475569;labelBackgroundColor=none;" vertex="1" connectable="0" parent="{parent}">
          <mxGeometry x="{x:.3}" relative="1" as="geometry">
            <mxPoint x="{dx:.1}" y="{dy:.1}" as="offset"/>
          </mxGeometry>
        </mxCell>"##,
            lid = link_id(l.id),
            pid = ep.port,
            value = esc(&port.name),
            pf = 10.0 * doc.device(ep.device).map_or(1.0, |d| d.scale),
            parent = link_id(l.id),
            dx = cx - along.x,
            dy = cy - along.y,
        );
    }
}

fn note(out: &mut String, ctx: &Ctx, n: &Note) {
    let Ctx { ox, oy, .. } = *ctx;
    let r = n.rect();
    let color = geom::normalize_color(&n.color).unwrap_or_else(|| "#334155".into());
    let label = n.lines().map(html_text).collect::<Vec<_>>().join("<br>");
    let _ = writeln!(
        out,
        r##"        <mxCell id="{id}" value="{label}" style="text;html=1;align=left;verticalAlign=top;{wrap}spacing=0;fontSize={size};fontColor={color};fontStyle={style};" vertex="1" parent="{layer}">
          <mxGeometry x="{x:.1}" y="{y:.1}" width="{w:.1}" height="{h:.1}" as="geometry"/>
        </mxCell>"##,
        id = note_id(n.id),
        layer = ctx.parent(n.layer),
        label = esc(&label),
        size = n.font_size(),
        style = if n.bold { 1 } else { 0 },
        // A boxed note wraps in its box and hides what does not fit.
        wrap = if n.boxed() { "whiteSpace=wrap;overflow=hidden;" } else { "whiteSpace=nowrap;" },
        x = n.x + ox,
        y = n.y + oy,
        // Some slack: the width is only an estimate (a box is exact).
        w = if n.boxed() { r.w } else { (r.w * 1.15).ceil() },
        h = r.h.ceil(),
    );
}

/// The cells of `doc` (with layer cells unless `flat`), and its bounds.
fn cells(doc: &Diagram, flat: bool, file: &str) -> (String, Rect) {
    // The frame goes along to files (in its layer), not to the clipboard.
    let frame = if flat { None } else { crate::frame::of(doc, file) };
    // Layer backgrounds go along to files, not to the clipboard.
    let backgrounds = if flat { vec![] } else { doc.layer_backgrounds() };
    let b = doc
        .bounds()
        .into_iter()
        .chain(frame.as_ref().map(|f| f.outer))
        .chain(backgrounds.iter().map(|(_, r, _)| *r))
        .chain(geom::label_rects(doc))
        .reduce(|a, b| a.union(&b));
    let b = b.unwrap_or(Rect { x: 0.0, y: 0.0, w: 0.0, h: 0.0 });
    let (ox, oy) = (MARGIN - b.x, MARGIN - b.y);
    let dirs = geom::port_link_dirs(doc);

    // Which link (and which end) carries each port's name.
    let mut label_end: HashMap<Id, (Id, bool)> = HashMap::new();
    for l in &doc.links {
        label_end.entry(l.from.port).or_insert((l.id, true));
        label_end.entry(l.to.port).or_insert((l.id, false));
    }

    let mut cells = String::new();

    // Layers are draw.io layers; cells follow the stacking order.
    for l in doc.layers.iter().filter(|_| !flat) {
        let _ = writeln!(
            cells,
            r#"        <mxCell id="{id}" value="{name}" parent="0"{hidden}/>"#,
            id = layer_id(l.id),
            // draw.io has no sub-layers: theirs have the names above them.
            name = esc(&doc.layer_path(l.id)),
            hidden = if doc.layer_visible(l.id) { "" } else { r#" visible="0""# },
        );
    }
    if let (Some(f), Some(layer)) = (&frame, doc.frame_layer()) {
        for (i, p) in f.prims.iter().enumerate() {
            frame_cell(&mut cells, &format!("frame{i}"), &layer_id(layer), p, ox, oy);
        }
    }
    // Each layer's background first on its layer, so it is behind the rest.
    for (layer, r, bg) in &backgrounds {
        let corners = if bg.rounded { format!("rounded=1;absoluteArcSize=1;arcSize={:.1};", bg.radius * 2.0) } else { "rounded=0;".into() };
        let border = match bg.border {
            BorderStyle::None => "strokeColor=none;".to_string(),
            b => {
                let dash = match b {
                    BorderStyle::Dashed => "dashed=1;",
                    BorderStyle::Dotted => "dashed=1;dashPattern=1 3;",
                    _ => "",
                };
                format!("strokeColor={};strokeWidth={:.1};{dash}", bg.border_color, bg.border_width)
            }
        };
        let _ = writeln!(
            cells,
            r#"        <mxCell id="background{layer}" value="" style="stratagramBackground=1;{corners}html=1;fillColor={};fillOpacity={:.0};{border}" vertex="1" parent="{}">
          <mxGeometry x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" as="geometry"/>
        </mxCell>"#,
            bg.fill,
            bg.opacity,
            layer_id(*layer),
            r.x + ox,
            r.y + oy,
            r.w,
            r.h
        );
    }
    let spots = geom::label_spots(doc);
    let ctx = Ctx { doc, ox, oy, dirs: &dirs, label_end: &label_end, flat, spots: &spots };
    for it in doc.z_order() {
        match it {
            Item::Device(i) => device(&mut cells, &ctx, &doc.devices[i]),
            Item::Link(i) => link(&mut cells, &ctx, &doc.links[i]),
            Item::Note(i) => note(&mut cells, &ctx, &doc.notes[i]),
            // Connection points only exist as loose edge ends; their labels
            // are text.
            Item::Point(i) => point_label(&mut cells, &ctx, &doc.points[i]),
        }
    }
    // The watermark on a layer of its own, over everything.
    if let Some(f) = frame.as_ref().filter(|f| !f.overlay.is_empty()) {
        cells.push_str("        <mxCell id=\"watermark\" value=\"Watermark\" parent=\"0\"/>\n");
        for (i, p) in f.overlay.iter().enumerate() {
            frame_cell(&mut cells, &format!("watermark{i}"), "watermark", p, ox, oy);
        }
    }
    (cells, b)
}

/// One piece of the frame as a draw.io cell.
fn frame_cell(out: &mut String, id: &str, parent: &str, p: &crate::frame::Prim, ox: f64, oy: f64) {
    use crate::frame::{Align, Prim};
    match p {
        Prim::Rect { r, fill, stroke, width, radius, dash } => {
            let corners = if *radius > 0.0 { format!("rounded=1;absoluteArcSize=1;arcSize={:.1};", radius * 2.0) } else { "rounded=0;".into() };
            let dash = if *dash { "dashed=1;" } else { "" };
            let _ = writeln!(
                out,
                r#"        <mxCell id="{id}" value="" style="stratagramFrame=1;{corners}{dash}html=1;fillColor={};strokeColor={};strokeWidth={width:.2};" vertex="1" parent="{parent}">
          <mxGeometry x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" as="geometry"/>
        </mxCell>"#,
                fill.as_deref().unwrap_or("none"),
                stroke.as_deref().unwrap_or("none"),
                r.x + ox,
                r.y + oy,
                r.w,
                r.h
            );
        }
        Prim::Watermark { at, text, size, color, angle, opacity } => {
            let (w, h) = (text.chars().count() as f64 * size * 0.66, size * 1.4);
            let _ = writeln!(
                out,
                r#"        <mxCell id="{id}" value="{}" style="text;stratagramFrame=1;html=1;align=center;verticalAlign=middle;fontSize={size:.1};fontStyle=1;fontColor={color};textOpacity={:.0};rotation={angle};" vertex="1" parent="{parent}">
          <mxGeometry x="{:.1}" y="{:.1}" width="{w:.1}" height="{h:.1}" as="geometry"/>
        </mxCell>"#,
                esc(&html_text(text)),
                opacity * 100.0,
                at.x - w / 2.0 + ox,
                at.y - h / 2.0 + oy
            );
        }
        Prim::Image { r, data } => {
            // draw.io writes data URIs in styles without ";base64".
            let data = data.replacen(";base64,", ",", 1);
            let _ = writeln!(
                out,
                r#"        <mxCell id="{id}" value="" style="stratagramFrame=1;shape=image;imageAspect=0;image={};" vertex="1" parent="{parent}">
          <mxGeometry x="{:.1}" y="{:.1}" width="{:.1}" height="{:.1}" as="geometry"/>
        </mxCell>"#,
                esc(&data),
                r.x + ox,
                r.y + oy,
                r.w,
                r.h
            );
        }
        Prim::Line { a, b, color, width, dash } => {
            let dash = if *dash { "dashed=1;" } else { "" };
            let _ = writeln!(
                out,
                r#"        <mxCell id="{id}" style="stratagramFrame=1;{dash}endArrow=none;html=1;strokeColor={color};strokeWidth={width:.2};" edge="1" parent="{parent}">
          <mxGeometry relative="1" as="geometry"><mxPoint x="{:.1}" y="{:.1}" as="sourcePoint"/><mxPoint x="{:.1}" y="{:.1}" as="targetPoint"/></mxGeometry>
        </mxCell>"#,
                a.x + ox,
                a.y + oy,
                b.x + ox,
                b.y + oy
            );
        }
        Prim::Text { at, text, size, bold, color, align } => {
            let w = text.chars().count() as f64 * size * 0.62 + 6.0;
            let x = match align {
                Align::Left => at.x,
                Align::Center => at.x - w / 2.0,
                Align::Right => at.x - w,
            };
            let _ = writeln!(
                out,
                r#"        <mxCell id="{id}" value="{}" style="text;stratagramFrame=1;html=1;align={};verticalAlign=middle;spacing=0;fontSize={size:.1};fontStyle={};fontColor={color};" vertex="1" parent="{parent}">
          <mxGeometry x="{:.1}" y="{:.1}" width="{w:.1}" height="{:.1}" as="geometry"/>
        </mxCell>"#,
                esc(&html_text(text)),
                align.as_str(),
                if *bold { 1 } else { 0 },
                x + ox,
                at.y - size * 0.8 + oy,
                size * 1.6
            );
        }
    }
}

/// The diagram as an mxGraphModel for the clipboard: draw.io pastes it
/// (Ctrl+V) as editable shapes into the current layer, as if copied in
/// draw.io itself.
pub fn clipboard(doc: &Diagram) -> String {
    let (cells, _) = cells(doc, true, "");
    format!("<mxGraphModel><root>\n        <mxCell id=\"0\"/>\n        <mxCell id=\"1\" parent=\"0\"/>\n{cells}</root></mxGraphModel>\n")
}

/// `file` is the diagram's file name, for frames that show it.
pub fn export(doc: &Diagram, title: &str, file: &str) -> String {
    export_pages(&[(title, doc)], file)
}

/// Several diagrams (the sheets of a document) as the pages of one
/// draw.io file, each `(name, diagram)`.
pub fn export_pages(pages: &[(&str, &Diagram)], file: &str) -> String {
    let mut out = String::new();
    for (i, (title, doc)) in pages.iter().enumerate() {
        let (cells, b) = cells(doc, false, file);
        let page_w = (b.w + 2.0 * MARGIN).max(850.0).ceil();
        let page_h = (b.h + 2.0 * MARGIN).max(1100.0).ceil();
        let id = if i == 0 { "stratagram".to_string() } else { format!("stratagram-{}", i + 1) };
        out.push_str(&format!(
            r#"  <diagram id="{id}" name="{name}">
    <mxGraphModel dx="1200" dy="800" grid="1" gridSize="10" guides="1" tooltips="1" connect="1" arrows="1" fold="1" page="1" pageScale="1" pageWidth="{page_w}" pageHeight="{page_h}" math="0" shadow="0">
      <root>
        <mxCell id="0"/>
{cells}      </root>
    </mxGraphModel>
  </diagram>
"#,
            name = esc(title),
        ));
    }
    format!(
        r#"<?xml version="1.0" encoding="UTF-8"?>
<mxfile host="stratagram" agent="stratagram {ver}" version="24.0.0">
{out}</mxfile>
"#,
        ver = env!("CARGO_PKG_VERSION"),
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Diagram {
        let mut d = Diagram::default();
        let a = d.add_device("R1 <core> & co", "router", Rect { x: 100.0, y: 50.0, w: 120.0, h: 60.0 }, "#fde68a");
        let b = d.add_device("SW1", "", Rect { x: 400.0, y: 50.0, w: 120.0, h: 60.0 }, "#bfdbfe");
        let pa = d.add_port(a, Side::Right, 0.5).unwrap();
        let pb = d.add_port(b, Side::Left, 0.5).unwrap();
        d.add_port(b, Side::Bottom, 0.5).unwrap();
        d.devices[0].ports[0].name = "Gi0/1".into();
        d.devices[1].ports[1].name = "spare".into();
        let id = d.add_link(
            pa,
            pb,
            LinkStyle {
                head_end: Head::Arrow,
                head_start: Head::Diamond,
                style: LineStyle::Dashed,
                routing: Routing::Orthogonal,
                ..Default::default()
            },
        );
        d.link_mut(id).unwrap().label = "10G".into();
        d
    }

    #[test]
    fn exports_connected_model() {
        let x = export(&sample(), "Test", "");
        assert!(x.contains(r#"<mxfile host="stratagram""#));
        // Name escaped twice: HTML inside an XML attribute.
        assert!(x.contains("R1 &amp;lt;core&amp;gt; &amp;amp; co"));
        assert!(x.contains(r#"kind="router""#));
        // Edge attached to port cells, with mapped styles.
        assert!(x.contains(r#"source="p4" target="p5""#));
        assert!(x.contains("endArrow=block;endFill=1;startArrow=diamond;startFill=1;"));
        assert!(x.contains("dashed=1;"));
        assert!(x.contains("edgeStyle=orthogonalEdgeStyle"));
        assert!(x.contains(r#"value="10G""#));
        // Port name on the edge, unlinked port name on the port itself.
        assert!(x.contains(r#"id="l7n4" value="Gi0/1""#));
        assert!(x.contains(r#"id="p6" value="spare""#));
        // Ports are children of their devices; shifted to the page margin.
        assert!(x.contains(r#"parent="d2""#));
        assert!(x.contains(r#"<mxGeometry x="40.0" y="40.0" width="120.0""#));
    }

    #[test]
    fn exports_lines_to_connection_points_as_loose_ends() {
        let mut d = sample();
        let dev = d.devices[0].id;
        let port = d.add_port(dev, Side::Top, 0.5).unwrap();
        let q = d.add_point(Pt::new(60.0, -100.0));
        let l = d.add_link(port, Endpoint::point(q), LinkStyle { head_end: Head::Arrow, ..Default::default() });
        let x = export(&d, "Test", "");
        let cell = &x[x.find(&format!(r#"id="l{l}""#)).unwrap()..];
        let cell = &cell[..cell.find("</mxCell>").unwrap()];
        assert!(cell.contains(&format!(r#"source="p{}""#, port.port)));
        assert!(!cell.contains("target="));
        assert!(!cell.contains("entryX"));
        // The point is the top left of everything, so it sits at the page margin.
        assert!(cell.contains(r#"<mxPoint x="40.0" y="40.0" as="targetPoint"/>"#), "{cell}");
        assert!(!x.contains(r#"id="p0""#));
    }

    #[test]
    fn exports_layers_in_stacking_order() {
        let mut d = sample();
        let back = d.add_layer("Back");
        d.active_layer = back;
        let note = d.add_note("behind", Pt::new(0.0, 0.0));
        d.layer_mut(back).unwrap().visible = false;
        // Send the note to the bottom of the stack.
        d.set_z_order(&[note]);
        let x = export(&d, "Test", "");
        assert!(x.contains(r#"<mxCell id="layer1" value="Layer 1" parent="0"/>"#));
        assert!(x.contains(&format!(r#"<mxCell id="layer{back}" value="Back" parent="0" visible="0"/>"#)));
        assert!(x.contains(&format!(r#"vertex="1" parent="layer{back}""#)));
        // Cells are written back to front: the note before the first device.
        assert!(x.find(&format!(r#"id="n{note}""#)).unwrap() < x.find(r#"id="d2""#).unwrap());
    }

    #[test]
    fn exports_notes_as_text() {
        let mut d = sample();
        let id = d.add_note("Site A\n<core>", Pt::new(100.0, 0.0));
        d.note_mut(id).unwrap().bold = true;
        let x = export(&d, "Test", "");
        assert!(x.contains(&format!(r#"id="n{id}" value="Site A&lt;br&gt;&amp;lt;core&amp;gt;""#)));
        assert!(x.contains("fontStyle=1;"));
        // Note sits above the devices, so it now defines the top margin.
        assert!(x.contains(r#"<mxGeometry x="40.0" y="40.0" width="#));
    }
}
