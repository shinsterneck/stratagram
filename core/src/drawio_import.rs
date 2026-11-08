// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Read shapes copied in draw.io (or written by our own draw.io export)
//! into a diagram, for pasting.
//!
//! Mapping:
//! - vertex                 -> device (label, fill color, size; the type is
//!   taken from a `kind` property or guessed from the shape's style)
//! - small child vertex     -> port of its device (our export writes ports so)
//! - `text` vertex          -> note
//! - edge                   -> link; ends on a device get a port on the side
//!   the edge leaves from, loose ends become connection points
//! - layers and groups are flattened; edge labels and our layer-name tags
//!   are skipped

use crate::model::*;
use quick_xml::XmlVersion;
use quick_xml::events::{BytesStart, Event};
use std::collections::HashMap;

/// Whether `text` looks like draw.io data (plain, URL-encoded or compressed).
pub fn looks_like_drawio(text: &str) -> bool {
    let t = text.trim_start();
    t.starts_with("<mxGraphModel") || t.starts_with("<mxfile") || t.starts_with("%3CmxGraphModel") || t.starts_with("%3Cmxfile")
}

/// Turn clipboard text from draw.io into the mxGraphModel XML: undo URL
/// encoding, and unpack compressed `<diagram>` content.
fn model_xml(text: &str) -> Result<String, String> {
    let mut xml = text.trim().to_string();
    if xml.starts_with("%3C") {
        xml = percent_encoding::percent_decode_str(&xml).decode_utf8_lossy().into_owned();
    }
    if xml.starts_with("<mxfile") && !xml.contains("<mxGraphModel") {
        // <mxfile><diagram ...>base64(deflate(encodeURIComponent(xml)))</diagram></mxfile>
        let start = xml.find("<diagram").and_then(|i| xml[i..].find('>').map(|j| i + j + 1)).ok_or("no diagram")?;
        let end = xml[start..].find("</diagram>").map(|j| start + j).ok_or("no diagram")?;
        let packed = base64_decode(xml[start..end].trim()).ok_or("diagram is not valid base64")?;
        let mut raw = vec![];
        use std::io::Read;
        flate2::read::DeflateDecoder::new(&packed[..])
            .read_to_end(&mut raw)
            .map_err(|e| format!("cannot unpack diagram: {e}"))?;
        let encoded = String::from_utf8_lossy(&raw).into_owned();
        xml = percent_encoding::percent_decode_str(&encoded).decode_utf8_lossy().into_owned();
    }
    Ok(xml)
}

pub fn base64_decode(s: &str) -> Option<Vec<u8>> {
    let value = |c: u8| -> Option<u32> {
        Some(match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' | b'-' => 62,
            b'/' | b'_' => 63,
            _ => return None,
        } as u32)
    };
    let mut out = vec![];
    let (mut acc, mut bits) = (0u32, 0);
    for c in s.bytes().filter(|c| !c.is_ascii_whitespace() && *c != b'=') {
        acc = (acc << 6) | value(c)?;
        bits += 6;
        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }
    Some(out)
}

/// One mxCell with what we need of it.
#[derive(Default, Debug, Clone)]
struct Cell {
    id: String,
    parent: String,
    value: String,
    style: String,
    vertex: bool,
    edge: bool,
    source: String,
    target: String,
    /// Properties of a wrapping `<object>` (like our `kind`).
    props: HashMap<String, String>,
    x: f64,
    y: f64,
    w: f64,
    h: f64,
    source_point: Option<Pt>,
    target_point: Option<Pt>,
}

fn attrs(e: &BytesStart) -> HashMap<String, String> {
    e.attributes()
        .flatten()
        .filter_map(|a| {
            let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();
            a.normalized_value(XmlVersion::Implicit1_0).ok().map(|v| (key, v.into_owned()))
        })
        .collect()
}

fn num(m: &HashMap<String, String>, k: &str) -> f64 {
    m.get(k).and_then(|v| v.parse().ok()).filter(|v: &f64| v.is_finite()).unwrap_or(0.0)
}

fn parse_cells(xml: &str) -> Result<Vec<Cell>, String> {
    let mut reader = quick_xml::Reader::from_str(xml);
    let mut cells: Vec<Cell> = vec![];
    // An <object> wrapping the next mxCell: its id, label and properties.
    let mut object: Option<HashMap<String, String>> = None;
    let mut in_cell = false;
    loop {
        let ev = reader.read_event().map_err(|e| format!("invalid draw.io XML: {e}"))?;
        let (e, empty) = match &ev {
            Event::Start(e) => (e, false),
            Event::Empty(e) => (e, true),
            Event::End(e) => {
                match e.local_name().as_ref() {
                    b"mxCell" => in_cell = false,
                    b"object" | b"UserObject" => object = None,
                    _ => {}
                }
                continue;
            }
            Event::Eof => break,
            _ => continue,
        };
        let a = attrs(e);
        match e.local_name().as_ref() {
            b"object" | b"UserObject" => {
                if !empty {
                    object = Some(a);
                }
            }
            b"mxCell" => {
                let mut c = Cell {
                    id: a.get("id").cloned().unwrap_or_default(),
                    parent: a.get("parent").cloned().unwrap_or_default(),
                    value: a.get("value").cloned().unwrap_or_default(),
                    style: a.get("style").cloned().unwrap_or_default(),
                    vertex: a.get("vertex").is_some_and(|v| v == "1"),
                    edge: a.get("edge").is_some_and(|v| v == "1"),
                    source: a.get("source").cloned().unwrap_or_default(),
                    target: a.get("target").cloned().unwrap_or_default(),
                    ..Cell::default()
                };
                if let Some(o) = &object {
                    c.id = o.get("id").cloned().unwrap_or(c.id);
                    c.value = o.get("label").cloned().unwrap_or(c.value);
                    c.props = o.clone();
                }
                cells.push(c);
                in_cell = !empty;
            }
            b"mxGeometry" if in_cell => {
                if let Some(c) = cells.last_mut() {
                    (c.x, c.y, c.w, c.h) = (num(&a, "x"), num(&a, "y"), num(&a, "width"), num(&a, "height"));
                }
            }
            b"mxPoint" if in_cell => {
                let p = Pt::new(num(&a, "x"), num(&a, "y"));
                if let Some(c) = cells.last_mut() {
                    match a.get("as").map(String::as_str) {
                        Some("sourcePoint") => c.source_point = Some(p),
                        Some("targetPoint") => c.target_point = Some(p),
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    Ok(cells)
}

/// `key=value` from a draw.io style string.
fn style_value<'a>(style: &'a str, key: &str) -> Option<&'a str> {
    style.split(';').find_map(|kv| kv.split_once('=').filter(|(k, _)| *k == key).map(|(_, v)| v))
}

/// Label text: HTML line breaks become new lines, tags and entities go.
fn plain_text(html: &str) -> String {
    let mut s = html.replace("<br>", "\n").replace("<br/>", "\n").replace("<br />", "\n");
    for tag in ["</div>", "</p>"] {
        s = s.replace(tag, &format!("{tag}\n"));
    }
    let mut out = String::new();
    let mut in_tag = false;
    for ch in s.chars() {
        match ch {
            '<' => in_tag = true,
            '>' if in_tag => in_tag = false,
            _ if !in_tag => out.push(ch),
            _ => {}
        }
    }
    let out = out.replace("&nbsp;", " ").replace("&lt;", "<").replace("&gt;", ">").replace("&quot;", "\"").replace("&amp;", "&");
    out.lines().map(str::trim).filter(|l| !l.is_empty()).collect::<Vec<_>>().join("\n")
}

/// Device type from a draw.io shape style (e.g. mxgraph.cisco.routers...).
fn kind_from_style(style: &str) -> &'static str {
    let s = style.to_lowercase();
    let table: [(&[&str], &str); 12] = [
        (&["firewall"], "firewall"),
        (&["distributed_virtual_switch", "distributed virtual switch", "dvswitch", "vds"], "distributed virtual switch"),
        (&["virtual_switch", "virtual switch", "vswitch"], "virtual switch"),
        (&["router"], "router"),
        (&["load_balancer", "loadbalancer", "load balancer"], "load balancer"),
        (&["switch"], "switch"),
        (&["wireless_lan_controller", "wireless_controller", "wlan_controller", "wlc"], "wlan controller"),
        (&["access_point", "accesspoint", "wireless"], "access point"),
        (&["server", "rack"], "server"),
        (&["cloud"], "cloud"),
        (&["internet", "globe"], "internet"),
        (&["client", "laptop", "workstation", "computer", "pc"], "client"),
    ];
    for (keys, kind) in table {
        if keys.iter().any(|k| s.contains(k)) {
            return kind;
        }
    }
    ""
}

fn color(style: &str, key: &str) -> Option<String> {
    style_value(style, key).and_then(crate::geom::normalize_color)
}

fn head(style: &str, key: &str, default: Head) -> Head {
    match style_value(style, key) {
        None => default,
        Some("none") | Some("") => Head::None,
        Some("open") => Head::Open,
        Some("diamond") | Some("diamondThin") => Head::Diamond,
        Some("oval") | Some("circle") | Some("dot") => Head::Circle,
        Some(_) => Head::Arrow,
    }
}

/// Port cells are small squares inside a device.
fn is_port(c: &Cell, parent: Option<&Cell>) -> bool {
    parent.is_some_and(|p| p.vertex) && c.w <= 16.0 && c.h <= 16.0 && c.w > 0.0
}

/// Shapes from draw.io clipboard or file text, with draw.io's coordinates.
pub fn import(text: &str) -> Result<Diagram, String> {
    let xml = model_xml(text)?;
    // Our export's frame is drawn from the diagram, not part of it.
    // Our export's frame and layer backgrounds are drawn from the diagram,
    // not part of it.
    let cells: Vec<Cell> = parse_cells(&xml)?
        .into_iter()
        .filter(|c| {
            // Also the markers of exports from before the rename ("netdiag…").
            !["stratagramFrame", "stratagramBackground", "netdiagFrame", "netdiagBackground"].iter().any(|m| c.style.contains(m))
        })
        .collect();
    let by_id: HashMap<&str, &Cell> = cells.iter().map(|c| (c.id.as_str(), c)).collect();
    // Absolute position: vertices inside groups (or ports inside devices)
    // are placed relative to their parent.
    let origin = |c: &Cell| {
        let mut o = Pt::default();
        let mut p = by_id.get(c.parent.as_str()).copied();
        let mut guard = 0;
        while let Some(q) = p.filter(|q| q.vertex) {
            o = o.add(Pt::new(q.x, q.y));
            p = by_id.get(q.parent.as_str()).copied();
            guard += 1;
            if guard > 50 {
                break;
            }
        }
        o
    };

    let mut d = Diagram::default();
    let mut devices: HashMap<&str, Id> = HashMap::new();
    let mut ports: HashMap<&str, Endpoint> = HashMap::new();
    for c in cells.iter().filter(|c| c.vertex) {
        let parent = by_id.get(c.parent.as_str()).copied();
        let group = c.style.starts_with("group") || c.style.contains(";group");
        if group
            || c.style.contains("edgeLabel")
            || parent.is_some_and(|p| p.edge)
            || is_port(c, parent)
            || c.id.ends_with("layer") && parent.is_some_and(|p| p.vertex)
        {
            continue;
        }
        let at = origin(c).add(Pt::new(c.x, c.y));
        if c.style.starts_with("text;") || c.style.contains(";text;") {
            let text = plain_text(&c.value);
            if text.is_empty() {
                continue;
            }
            let id = d.add_note(&text, at);
            let n = d.note_mut(id).unwrap();
            n.size = style_value(&c.style, "fontSize").and_then(|v| v.parse().ok()).unwrap_or(n.size).clamp(NOTE_MIN_SIZE, NOTE_MAX_SIZE);
            n.color = color(&c.style, "fontColor").unwrap_or(n.color.clone());
            n.bold = style_value(&c.style, "fontStyle").and_then(|v| v.parse::<u32>().ok()).is_some_and(|f| f & 1 == 1);
            continue;
        }
        let kind = c.props.get("kind").map(String::as_str).unwrap_or_else(|| kind_from_style(&c.style)).to_string();
        let mut lines: Vec<String> = plain_text(&c.value).lines().map(String::from).collect();
        // Our export puts the type under the name, in capitals.
        if lines.len() > 1 && lines.last().is_some_and(|l| l.eq_ignore_ascii_case(&kind)) {
            lines.pop();
        }
        let name = lines.join(" ");
        let fill = color(&c.style, "fillColor")
            .or_else(|| crate::editor::kind_color(&kind).map(String::from))
            .unwrap_or_else(|| "#dbeafe".into());
        let r = Rect { x: at.x, y: at.y, w: c.w.max(MIN_W), h: c.h.max(MIN_H) };
        devices.insert(c.id.as_str(), d.add_device(&name, &kind, r, &fill));
    }
    // Ports our export wrote as small squares inside their device.
    for c in cells.iter().filter(|c| c.vertex) {
        let Some(parent) = by_id.get(c.parent.as_str()).copied() else { continue };
        let Some(&dev) = devices.get(parent.id.as_str()) else { continue };
        if !is_port(c, Some(parent)) {
            continue;
        }
        let dv = d.device(dev).unwrap();
        let center = Pt::new(dv.x + c.x + c.w / 2.0, dv.y + c.y + c.h / 2.0);
        let (side, t) = dv.nearest_side(center);
        if let Some(ep) = d.add_port(dev, side, t) {
            let name = plain_text(&c.value);
            if let Some(p) = d.device_mut(dev).and_then(|x| x.ports.iter_mut().find(|p| p.id == ep.port)) {
                p.name = name;
            }
            ports.insert(c.id.as_str(), ep);
        }
    }
    let mut links: HashMap<&str, Id> = HashMap::new();
    for c in cells.iter().filter(|c| c.edge) {
        let mut end = |term: &str, fixed: Option<Pt>, other: Option<Pt>, exit: (&str, &str)| -> Option<Endpoint> {
            if let Some(ep) = ports.get(term) {
                return Some(*ep);
            }
            if let Some(&dev) = devices.get(term) {
                let dv = d.device(dev)?.clone();
                // Where the edge leaves the shape, if draw.io says so;
                // else the side facing the other end.
                let side_t = match (style_value(&c.style, exit.0), style_value(&c.style, exit.1)) {
                    (Some(x), Some(y)) => {
                        let (x, y): (f64, f64) = (x.parse().ok()?, y.parse().ok()?);
                        Some(dv.nearest_side(Pt::new(dv.x + x * dv.w, dv.y + y * dv.h)))
                    }
                    _ => None,
                };
                let (side, t) = side_t.unwrap_or_else(|| {
                    let c = Pt::new(dv.x + dv.w / 2.0, dv.y + dv.h / 2.0);
                    let to = other.unwrap_or(c);
                    let side = facing_side(c, to);
                    (side, 0.5)
                });
                return d.add_port(dev, side, t);
            }
            fixed.map(|p| Endpoint::point(d.add_point(p)))
        };
        let center_of = |term: &str| {
            by_id.get(term).filter(|t| t.vertex).map(|t| origin(t).add(Pt::new(t.x + t.w / 2.0, t.y + t.h / 2.0)))
        };
        let (sc, tc) = (center_of(&c.source).or(c.source_point), center_of(&c.target).or(c.target_point));
        let Some(from) = end(&c.source, c.source_point, tc, ("exitX", "exitY")) else { continue };
        let Some(to) = end(&c.target, c.target_point, sc, ("entryX", "entryY")) else { continue };
        let st = &c.style;
        let style = LinkStyle {
            color: color(st, "strokeColor").unwrap_or_else(|| LinkStyle::default().color),
            width: style_value(st, "strokeWidth").and_then(|v| v.parse().ok()).filter(|w: &f64| *w > 0.0).unwrap_or(1.0).min(20.0),
            style: match (style_value(st, "dashed"), style_value(st, "dashPattern")) {
                (Some("1"), Some(p)) if p.starts_with("1 ") => LineStyle::Dotted,
                (Some("1"), _) => LineStyle::Dashed,
                _ => LineStyle::Solid,
            },
            // draw.io draws an arrow at the end unless told otherwise.
            head_end: head(st, "endArrow", Head::Arrow),
            head_start: head(st, "startArrow", Head::None),
            routing: if st.contains("orthogonalEdgeStyle") || st.contains("elbowEdgeStyle") { Routing::Orthogonal } else { Routing::Straight },
            // Written by our export; a double line is a trunk otherwise.
            kind: style_value(st, "stratagramKind")
                .or_else(|| style_value(st, "netdiagKind"))
                .and_then(LinkKind::parse)
                .unwrap_or(if style_value(st, "shape") == Some("link") { LinkKind::Trunk } else { LinkKind::Plain }),
            label_margin: DEFAULT_LABEL_MARGIN,
            over: style_value(st, "stratagramOver")
                .or_else(|| style_value(st, "netdiagOver"))
                .map(|v| v.split(',').filter_map(|k| LinkKind::parse(k.trim())).collect())
                .unwrap_or_default(),
        };
        let tag = style.full_tag();
        let id = d.add_link(from, to, style);
        // Our export puts the type's tag in front of the label.
        let label = plain_text(&c.value).replace('\n', " ");
        let label = label.strip_prefix(tag.as_str()).filter(|_| !tag.is_empty()).map_or(label.as_str(), str::trim);
        let link = d.link_mut(id).unwrap();
        link.label = label.to_string();
        link.label_hidden = style_value(st, "noLabel") == Some("1");
        links.insert(c.id.as_str(), id);
    }
    // Labels on edges: our export writes a port's name as a label near its
    // end (id "<edge>n<port>", port cell "p<port>"); other labels go to
    // the link if it has none.
    for c in cells.iter().filter(|c| c.vertex) {
        let Some(&link) = links.get(c.parent.as_str()) else { continue };
        let text = plain_text(&c.value).replace('\n', " ");
        let port = c.id.strip_prefix(&format!("{}n", c.parent)).and_then(|n| ports.get(format!("p{n}").as_str()));
        match port {
            Some(ep) => {
                if let Some(p) = d.device_mut(ep.device).and_then(|x| x.ports.iter_mut().find(|p| p.id == ep.port)) {
                    p.name = text;
                }
            }
            None => {
                if let Some(l) = d.link_mut(link).filter(|l| l.label.is_empty()) {
                    l.label = text;
                }
            }
        }
    }
    if d.devices.is_empty() && d.notes.is_empty() && d.links.is_empty() {
        return Err("no shapes found".into());
    }
    Ok(d)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_our_own_export_back() {
        let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/demo.stratagram")).unwrap();
        let demo = Diagram::from_json(&json).unwrap();
        let d = import(&crate::drawio::clipboard(&demo)).unwrap();
        assert_eq!(d.devices.len(), demo.devices.len());
        assert_eq!(d.links.len(), demo.links.len());
        let fw = d.devices.iter().find(|x| x.name == "fw-01").unwrap();
        assert_eq!((fw.kind.as_str(), fw.color.as_str(), fw.w), ("firewall", "#fecaca", 120.0));
        // Ports come back with names, on the same sides.
        let names: Vec<(&str, Side)> = fw.ports.iter().map(|p| (p.name.as_str(), p.side)).collect();
        assert!(names.contains(&("port3", Side::Bottom)) && names.contains(&("wan1", Side::Right)), "{names:?}");
        let isp = d.links.iter().find(|l| l.label == "ISP 1G").unwrap();
        assert_eq!((isp.style.style, isp.style.head_end, isp.style.color.as_str()), (LineStyle::Dashed, Head::Arrow, "#dc2626"));
        // Same layout: relative positions are kept.
        let core = d.devices.iter().find(|x| x.name == "core-sw-01").unwrap();
        assert_eq!(core.y - fw.y, 180.0);
    }

    #[test]
    fn link_types_survive_export_import_and_save() {
        let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/demo.stratagram")).unwrap();
        let mut demo = Diagram::from_json(&json).unwrap();
        let kinds = [LinkKind::Trunk, LinkKind::Ipsec, LinkKind::Capwap, LinkKind::Vxlan, LinkKind::Sdwan, LinkKind::Mpls];
        for (l, k) in demo.links.iter_mut().zip(kinds.iter().cycle()) {
            l.style.kind = *k;
        }
        let saved = Diagram::from_json(&demo.to_json()).unwrap();
        assert_eq!(saved.links, demo.links);
        let back = import(&crate::drawio::clipboard(&demo)).unwrap();
        let mut a: Vec<(String, LinkKind)> = demo.links.iter().map(|l| (l.label.clone(), l.style.kind)).collect();
        let mut b: Vec<(String, LinkKind)> = back.links.iter().map(|l| (l.label.clone(), l.style.kind)).collect();
        a.sort_by(|x, y| x.0.cmp(&y.0));
        b.sort_by(|x, y| x.0.cmp(&y.0));
        assert_eq!(a, b, "types come back, and the tags are not added to the labels");
        let svg = crate::svg::export(&demo, "");
        for k in &kinds[..demo.links.len().min(kinds.len())] {
            assert!(svg.contains(&format!(">{}</text>", k.tag())), "{k:?}");
        }
    }

    #[test]
    fn routed_link_types_survive_export_import_and_save() {
        let json = std::fs::read_to_string(concat!(env!("CARGO_MANIFEST_DIR"), "/../examples/demo.stratagram")).unwrap();
        let mut demo = Diagram::from_json(&json).unwrap();
        demo.links[0].style.kind = LinkKind::L3;
        demo.links[1].style.kind = LinkKind::L3Sub;
        demo.links[2].style.kind = LinkKind::L2;
        let saved = Diagram::from_json(&demo.to_json()).unwrap();
        assert_eq!((saved.links[0].style.kind, saved.links[1].style.kind), (LinkKind::L3, LinkKind::L3Sub));
        let back = import(&crate::drawio::clipboard(&demo)).unwrap();
        let count = |k: LinkKind| back.links.iter().filter(|l| l.style.kind == k).count();
        assert_eq!((count(LinkKind::L3), count(LinkKind::L3Sub), count(LinkKind::Trunk)), (1, 1, 0), "a double line, but not a plain trunk");
        assert_eq!(count(LinkKind::L2), 1);
        let svg = crate::svg::export(&demo, "");
        assert!(svg.contains(">L3</text>") && svg.contains(">L3 subif</text>"));
        assert!(svg.matches("<circle").count() >= 4, "two circles per routed line");
    }

    #[test]
    fn reads_exports_from_before_the_rename() {
        // Markers as netdiag wrote them.
        let xml = r#"<mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/>
            <mxCell id="f" value="" style="netdiagFrame=1;rounded=0;" vertex="1" parent="1"><mxGeometry x="0" y="0" width="900" height="600" as="geometry"/></mxCell>
            <mxCell id="a" value="A" style="rounded=1;" vertex="1" parent="1"><mxGeometry x="10" y="10" width="120" height="60" as="geometry"/></mxCell>
            <mxCell id="b" value="B" style="rounded=1;" vertex="1" parent="1"><mxGeometry x="300" y="10" width="120" height="60" as="geometry"/></mxCell>
            <mxCell id="e" value="&lt;b&gt;VXLAN over IPsec&lt;/b&gt; DCI" style="netdiagKind=vxlan;netdiagOver=ipsec;html=1;" edge="1" parent="1" source="a" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>
            </root></mxGraphModel>"#;
        let d = import(xml).unwrap();
        assert_eq!(d.devices.len(), 2, "the old frame is skipped");
        let l = &d.links[0];
        assert_eq!((l.style.kind, l.style.over.clone(), l.label.as_str()), (LinkKind::Vxlan, vec![LinkKind::Ipsec], "DCI"));
    }

    #[test]
    fn reads_shapes_copied_in_drawio() {
        // As draw.io puts it on the clipboard: URL-encoded, foreign shapes,
        // a default end arrow, a group and a loose edge end.
        let xml = r##"<mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/>
            <mxCell id="a" value="&lt;b&gt;Edge&lt;/b&gt; FW" style="shape=mxgraph.cisco.firewalls.firewall;fillColor=#FF0000;" vertex="1" parent="1"><mxGeometry x="100" y="50" width="64" height="40" as="geometry"/></mxCell>
            <mxCell id="g" value="" style="group" vertex="1" parent="1"><mxGeometry x="300" y="200" width="200" height="100" as="geometry"/></mxCell>
            <mxCell id="b" value="core" style="rounded=1;" vertex="1" parent="g"><mxGeometry x="10" y="20" width="120" height="60" as="geometry"/></mxCell>
            <mxCell id="t" value="DMZ&lt;br&gt;zone" style="text;html=1;fontSize=18;fontStyle=1;" vertex="1" parent="1"><mxGeometry x="0" y="0" width="60" height="30" as="geometry"/></mxCell>
            <mxCell id="e1" value="10G" style="edgeStyle=orthogonalEdgeStyle;dashed=1;" edge="1" parent="1" source="a" target="b"><mxGeometry relative="1" as="geometry"/></mxCell>
            <mxCell id="e2" style="endArrow=none;" edge="1" parent="1" source="b"><mxGeometry relative="1" as="geometry"><mxPoint x="700" y="250" as="targetPoint"/></mxGeometry></mxCell>
            </root></mxGraphModel>"##;
        let encoded = percent_encoding::utf8_percent_encode(xml, percent_encoding::NON_ALPHANUMERIC).to_string();
        assert!(looks_like_drawio(&encoded));
        let d = import(&encoded).unwrap();
        assert_eq!(d.devices.len(), 2, "the group itself is no device: {:?}", d.devices);
        let fw = d.devices.iter().find(|x| x.name == "Edge FW").unwrap();
        assert_eq!((fw.kind.as_str(), fw.color.as_str(), fw.w, fw.h), ("firewall", "#ff0000", 64.0, 40.0));
        let core = d.devices.iter().find(|x| x.name == "core").unwrap();
        assert_eq!((core.x, core.y), (310.0, 220.0), "inside its group");
        let n = &d.notes[0];
        assert_eq!((n.text.as_str(), n.size, n.bold), ("DMZ\nzone", 18.0, true));
        let l = d.links.iter().find(|l| l.label == "10G").unwrap();
        assert_eq!((l.style.head_end, l.style.style, l.style.routing), (Head::Arrow, LineStyle::Dashed, Routing::Orthogonal));
        assert_eq!((l.from.device, l.to.device), (fw.id, core.id));
        let loose = d.links.iter().find(|l| l.label.is_empty()).unwrap();
        assert!(loose.to.is_point() && loose.style.head_end == Head::None);
        assert_eq!(d.point(loose.to.owner()).unwrap().pos(), Pt::new(700.0, 250.0));
    }

    #[test]
    fn reads_compressed_diagrams() {
        // base64(deflate(encodeURIComponent(xml))), as in .drawio files.
        let xml = r#"<mxGraphModel><root><mxCell id="0"/><mxCell id="1" parent="0"/><mxCell id="a" value="r1" style="" vertex="1" parent="1"><mxGeometry x="1" y="2" width="80" height="40" as="geometry"/></mxCell></root></mxGraphModel>"#;
        let encoded = percent_encoding::utf8_percent_encode(xml, percent_encoding::NON_ALPHANUMERIC).to_string();
        use std::io::Write;
        let mut z = flate2::write::DeflateEncoder::new(vec![], flate2::Compression::default());
        z.write_all(encoded.as_bytes()).unwrap();
        let packed = z.finish().unwrap();
        const ABC: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
        let mut b64 = String::new();
        for chunk in packed.chunks(3) {
            let n = chunk.iter().enumerate().fold(0u32, |a, (i, b)| a | (*b as u32) << (16 - 8 * i));
            for i in 0..=chunk.len() {
                b64.push(ABC[(n >> (18 - 6 * i) & 63) as usize] as char);
            }
        }
        let file = format!(r#"<mxfile><diagram id="x" name="p">{b64}</diagram></mxfile>"#);
        let d = import(&file).unwrap();
        assert_eq!(d.devices[0].name, "r1");
        assert_eq!((d.devices[0].x, d.devices[0].y), (1.0, 2.0));
        assert!(import("hello").is_err());
    }
}
