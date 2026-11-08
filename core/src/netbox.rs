// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Diagrams from NetBox: a site's devices, the interfaces with cables and
//! the cables between them, as NetBox's REST API gives them (lists of
//! devices, interfaces and cables, or the API's pages with `results`).
//!
//! Every device, port and link keeps the NetBox object it stands for in
//! `source` ("dcim.device:42", "dcim.interface:7", "dcim.cable:3"), so a
//! later [`refresh`] adds what is new, renames what was renamed and sets
//! aside what is gone, without moving anything already placed.
//!
//! Device types come from the device role (slug or name): first the
//! `kinds` given in [`Options`], then the built-in types and their short
//! forms (fw, rtr, sw, wlc...), then common role words (core, leaf, spine,
//! edge, host...). Line types come from the interfaces: 802.1Q tagged on
//! either end is a trunk, routed (with IP addresses) on both ends is layer
//! 3, access ports are layer 2. Cables to provider circuits end at a cloud
//! for the circuit, one per circuit. Patch panels (by role, or devices
//! with front ports and a role that says nothing else) get as many
//! sockets as they have front ports, of their front ports' type.
//!
//! The site is the first sheet; each of its locations gets a sheet of its
//! own ([`location_diagrams`]) with the devices in it (and in the
//! locations inside it), plus those elsewhere they are cabled to, on the
//! layer [`OUTSIDE_LAYER`]. Sheets keep what they stand for in `source`
//! ("dcim.site:1", "dcim.location:4").

use std::collections::{HashMap, HashSet};

use serde::Deserialize;
use serde_json::Value;

use crate::model::{Connector, Diagram, Endpoint, Id, LinkKind, LinkStyle, MAX_UPLINKS, PANEL_MAX_PORTS, PatchPanel, Rect, Routing, Side, Trace};

/// What NetBox has for a site.
#[derive(Deserialize, Default, Debug)]
pub struct SiteData {
    /// The site itself (its name titles the diagram).
    #[serde(default)]
    pub site: Value,
    #[serde(default, deserialize_with = "list")]
    pub devices: Vec<Value>,
    #[serde(default, deserialize_with = "list")]
    pub interfaces: Vec<Value>,
    #[serde(default, deserialize_with = "list")]
    pub cables: Vec<Value>,
    /// The site's locations ({id, name, parent}); devices name theirs in
    /// `location`.
    #[serde(default, deserialize_with = "list")]
    pub locations: Vec<Value>,
    /// Cabling from end to end through patch panels (NetBox's cable
    /// paths): the ids of its cables, one after another.
    #[serde(default)]
    pub paths: Vec<Vec<u64>>,
}

/// A list, or an API page with the list in `results`.
fn list<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Vec<Value>, D::Error> {
    Ok(match Value::deserialize(d)? {
        Value::Array(items) => items,
        Value::Object(mut page) => match page.remove("results") {
            Some(Value::Array(items)) => items,
            _ => vec![],
        },
        _ => vec![],
    })
}

/// How NetBox objects become diagram items.
#[derive(Deserialize, Default, Debug, Clone)]
#[serde(default)]
pub struct Options {
    /// Device type by role: role slug or name (any case) to a device type,
    /// before the built-in guesses.
    pub kinds: HashMap<String, String>,
    /// Also devices without any cable to another device (else only those
    /// linked to something).
    pub all_devices: bool,
    /// Each type of device on a layer of its own ("Switches", "Patch
    /// panels"...), and the cables on [`CABLES_LAYER`], to show or hide
    /// them by type.
    pub type_layers: bool,
    /// How the cables' lines run: "straight" (the default here),
    /// "orthogonal" or "around" (orthogonal, around what is in the way).
    pub routing: Routing,
    /// Device types drawn with their ports as a front, like a switch's
    /// (switches, routers, firewalls, servers); unset: switches only.
    pub fronts: Option<Vec<String>>,
}

impl Options {
    /// Whether devices of type `kind` are drawn with their ports.
    fn front_for(&self, kind: &str) -> bool {
        crate::shape::takes_ports(kind)
            && match &self.fronts {
                Some(kinds) => kinds.iter().any(|k| {
                    let k = k.trim().to_lowercase();
                    // "router" stands for the router types too.
                    k == kind || k == "router" && matches!(kind, "ospf router" | "bgp reflector")
                }),
                None => kind == "switch",
            }
    }
}

/// The layer of the cables, with devices on layers by type.
pub const CABLES_LAYER: &str = "Cables";

/// The layer for devices of type `kind`: "Switches", "Patch panels",
/// "Other devices" for those without a type.
pub fn type_layer_name(kind: &str) -> String {
    let kind = kind.trim();
    if kind.is_empty() {
        return "Other devices".into();
    }
    if kind == "internet" {
        return "Internet".into();
    }
    let plural = if kind.ends_with('s') || kind.ends_with('x') || kind.ends_with("ch") || kind.ends_with("sh") {
        format!("{kind}es")
    } else if let Some(stem) = kind.strip_suffix('y').filter(|s| !s.ends_with(['a', 'e', 'i', 'o', 'u'])) {
        format!("{stem}ies")
    } else {
        format!("{kind}s")
    };
    // Abbreviations in capitals, the rest as a sentence.
    let words: Vec<String> = plural
        .split(' ')
        .enumerate()
        .map(|(i, w)| match w {
            "aws" | "wlan" | "aci" | "ospf" | "bgp" | "ap" | "aps" => w.to_uppercase().replace("APS", "APs"),
            _ if i == 0 => w[..1].to_uppercase() + &w[1..],
            _ => w.to_string(),
        })
        .collect();
    words.join(" ")
}

impl SiteData {
    pub fn parse(json: &str) -> Result<SiteData, String> {
        serde_json::from_str(json).map_err(|e| format!("not NetBox site data: {e}"))
    }
}

// Reading NetBox's JSON

fn at<'a>(v: &'a Value, path: &[&str]) -> &'a Value {
    path.iter().fold(v, |v, k| v.get(k).unwrap_or(&Value::Null))
}

fn text(v: &Value, path: &[&str]) -> String {
    match at(v, path) {
        Value::String(s) => s.clone(),
        Value::Number(n) => n.to_string(),
        _ => String::new(),
    }
}

fn id(v: &Value) -> Option<u64> {
    v.get("id").and_then(Value::as_u64)
}

/// A choice field's value ({"value": ..., "label": ...}) or a plain string.
fn choice(v: &Value, key: &str) -> String {
    match v.get(key) {
        Some(Value::String(s)) => s.clone(),
        Some(c) => text(c, &["value"]),
        None => String::new(),
    }
}

// Device types

/// Role words NetBox installations commonly use, beyond the type names.
const ROLE_WORDS: &[(&str, &str)] = &[
    ("firewall", "firewall"),
    ("gateway", "router"),
    ("gw", "router"),
    ("edge", "router"),
    ("border", "router"),
    ("pe", "router"),
    ("ce", "router"),
    ("wan", "router"),
    ("vswitch", "virtual switch"),
    ("dvs", "distributed virtual switch"),
    ("vds", "distributed virtual switch"),
    ("dvswitch", "distributed virtual switch"),
    ("core", "switch"),
    ("distribution", "switch"),
    ("dist", "switch"),
    ("access", "switch"),
    ("leaf", "switch"),
    ("spine", "switch"),
    ("tor", "switch"),
    ("aggregation", "switch"),
    ("agg", "switch"),
    ("host", "server"),
    ("hypervisor", "server"),
    ("esx", "server"),
    ("esxi", "server"),
    ("compute", "server"),
    ("storage", "server"),
    ("nas", "server"),
    ("san", "server"),
    ("vm", "server"),
    ("workstation", "client"),
    ("pc", "client"),
    ("laptop", "client"),
    ("desktop", "client"),
    ("printer", "client"),
    ("phone", "client"),
    ("controller", "wlan controller"),
    ("balancer", "load balancer"),
    ("adc", "load balancer"),
    ("f5", "load balancer"),
];

/// The device type for a device with role `slug` / `name`.
pub fn kind_for_role(slug: &str, name: &str, opts: &Options) -> String {
    for key in [slug, name] {
        if let Some(k) = opts.kinds.iter().find(|(r, _)| !key.is_empty() && r.eq_ignore_ascii_case(key)).map(|(_, k)| k) {
            return k.clone();
        }
    }
    let known = crate::editor::known_kind;
    for key in [slug, name] {
        if let Some(k) = known(key) {
            return k.into();
        }
    }
    // Word by word, then pairs ("access point", "wireless controller").
    let words: Vec<String> = format!("{slug} {name}")
        .to_lowercase()
        .split(|c: char| !c.is_alphanumeric())
        .filter(|w| !w.is_empty())
        .map(String::from)
        .collect();
    for pair in words.windows(2) {
        if let Some(k) = known(&format!("{}{}", pair[0], pair[1])) {
            return k.into();
        }
        if pair[0] == "access" && pair[1] == "point" {
            return "access point".into();
        }
        if pair[0].starts_with("wireless") && pair[1] == "controller" {
            return "wlan controller".into();
        }
    }
    for w in &words {
        if let Some(k) = known(w) {
            return k.into();
        }
    }
    for w in &words {
        if let Some((_, k)) = ROLE_WORDS.iter().find(|(r, _)| r == w) {
            return (*k).into();
        }
    }
    String::new()
}

// Building the diagram

/// One end of a cable, as far as the diagram cares.
enum End {
    /// A port on a device: (device id, component type, component id, name,
    /// the socket of a front port on a patch panel or 0).
    Port(u64, String, u64, String, u32),
    /// A provider circuit: (circuit id, its name for the diagram).
    Circuit(u64, String),
}

fn cable_end(t: &Value) -> Option<End> {
    let kind = text(t, &["object_type"]);
    let obj = at(t, &["object"]);
    let oid = t.get("object_id").and_then(Value::as_u64).or_else(|| id(obj))?;
    match kind.as_str() {
        "dcim.interface" | "dcim.frontport" | "dcim.rearport" => {
            let dev = id(at(obj, &["device"]))?;
            let name = text(obj, &["name"]);
            let socket = match kind.as_str() {
                "dcim.frontport" => front_socket(obj, &name),
                // A patch panel's rear port: behind the socket of the front
                // port it maps to (from the plugin; none for one behind several).
                "dcim.rearport" => obj.get("position").and_then(Value::as_u64).unwrap_or(0) as u32,
                // A switch's port: only by its place (from the plugin).
                "dcim.interface" => obj.get("position").and_then(Value::as_u64).unwrap_or(0) as u32,
                _ => 0,
            };
            Some(End::Port(dev, kind, oid, name, socket))
        }
        "circuits.circuittermination" => {
            let circuit = at(obj, &["circuit"]);
            let cid = id(circuit)?;
            let provider = text(circuit, &["provider", "name"]);
            let name = text(circuit, &["cid"]);
            let name = if provider.is_empty() { name } else { format!("{provider} {name}").trim().to_string() };
            Some(End::Circuit(cid, if name.is_empty() { text(circuit, &["display"]) } else { name }))
        }
        // Console and power cabling is not part of the network drawing.
        _ => None,
    }
}

/// Which socket of its patch panel a front port is: its `position` among
/// the panel's front ports (from the plugin), else the number its name
/// ends in ("1", "Port 12", "FP-03"); 0 if neither.
fn front_socket(obj: &Value, name: &str) -> u32 {
    if let Some(n) = obj.get("position").and_then(Value::as_u64) {
        return n as u32;
    }
    let digits: String = name.chars().rev().take_while(char::is_ascii_digit).collect::<Vec<_>>().into_iter().rev().collect();
    digits.parse().unwrap_or(0)
}

/// The line type between two interfaces (NetBox interface objects).
fn link_kind(a: Option<&Value>, b: Option<&Value>) -> LinkKind {
    let ends: Vec<&Value> = [a, b].into_iter().flatten().collect();
    let mode = |v: &Value| choice(v, "mode");
    let routed = |v: &Value| v.get("count_ipaddresses").and_then(Value::as_u64).unwrap_or(0) > 0;
    if ends.iter().any(|v| matches!(mode(v).as_str(), "tagged" | "tagged-all" | "q-in-q")) {
        LinkKind::Trunk
    } else if !ends.is_empty() && ends.iter().all(|v| routed(v)) {
        LinkKind::L3
    } else if ends.iter().any(|v| mode(v) == "access") {
        LinkKind::L2
    } else {
        LinkKind::Plain
    }
}

/// A NetBox device's type, and its sockets if it is a patch panel or a
/// switch with ports. A patch panel by its role, or one with front ports
/// whose role says nothing else: as many sockets as front ports (24 if it
/// has none), of the type most of them have (`front_ports`: {count,
/// type}). A switch (or a router, firewall or server, when the options say
/// so): its physical ports, copper ones first and the others as uplinks
/// (`switch_ports`: {count, type, uplinks}).
fn kind_and_panel(v: &Value, opts: &Options) -> (String, Option<PatchPanel>) {
    let mut kind = kind_for_role(&text(v, &["role", "slug"]), &text(v, &["role", "name"]), opts);
    let fronts = at(v, &["front_ports", "count"]).as_u64().unwrap_or(0);
    if kind.is_empty() && fronts > 0 {
        kind = "patch panel".into();
    }
    if opts.front_for(&kind) {
        let count = |key: &str| at(v, &["switch_ports", key]).as_u64().unwrap_or(0) as u32;
        let (ports, uplinks) = (count("count").min(PANEL_MAX_PORTS), count("uplinks").min(MAX_UPLINKS));
        if ports == 0 {
            return (kind, None);
        }
        let connector = Connector::parse(&text(v, &["switch_ports", "type"])).unwrap_or_default();
        return (kind, Some(PatchPanel { ports, connector, uplinks }));
    }
    if kind != "patch panel" {
        return (kind, None);
    }
    let ports = if fronts > 0 { (fronts as u32).min(PANEL_MAX_PORTS) } else { PatchPanel::default().ports };
    let connector = Connector::parse(&text(v, &["front_ports", "type"])).unwrap_or_default();
    (kind, Some(PatchPanel { ports, connector, uplinks: 0 }))
}

/// The layer of a location's sheet for the devices outside the location
/// that its devices are cabled to.
pub const OUTSIDE_LAYER: &str = "Other locations";

/// The diagram for a site: its devices with their cabled interfaces as
/// ports, its cables as links. Nothing is placed yet (all in a grid): lay
/// it out afterwards.
pub fn site_diagram(data: &SiteData, opts: &Options) -> Diagram {
    let mut doc = diagram_of(data, opts, None);
    doc.name = text(&data.site, &["name"]);
    doc.source = id(&data.site).map(|i| format!("dcim.site:{i}")).unwrap_or_default();
    doc
}

/// A sheet for each of the site's locations, parents before the locations
/// inside them, by name: the devices in the location or in one inside it,
/// and on the layer [`OUTSIDE_LAYER`] the devices elsewhere they are
/// cabled to. Nothing is placed yet.
pub fn location_diagrams(data: &SiteData, opts: &Options) -> Vec<Diagram> {
    let parent = |v: &Value| id(at(v, &["parent"]));
    let locations: Vec<(u64, String, Option<u64>)> =
        data.locations.iter().filter_map(|v| Some((id(v)?, text(v, &["name"]), parent(v)))).collect();
    let known: HashSet<u64> = locations.iter().map(|l| l.0).collect();
    // Tree order: each location's children after it, by name.
    let mut order: Vec<&(u64, String, Option<u64>)> = vec![];
    fn add<'a>(
        of: Option<u64>,
        all: &'a [(u64, String, Option<u64>)],
        known: &HashSet<u64>,
        out: &mut Vec<&'a (u64, String, Option<u64>)>,
    ) {
        let mut kids: Vec<&(u64, String, Option<u64>)> =
            all.iter().filter(|l| l.2.filter(|p| known.contains(p)) == of && !out.iter().any(|o| o.0 == l.0)).collect();
        kids.sort_by(|a, b| a.1.to_lowercase().cmp(&b.1.to_lowercase()));
        for k in kids {
            out.push(k);
            add(Some(k.0), all, known, out);
        }
    }
    add(None, &locations, &known, &mut order);
    order
        .into_iter()
        .map(|&(lid, ref name, _)| {
            // The location and everything inside it.
            let mut within: HashSet<u64> = [lid].into();
            while let Some(more) = locations.iter().find(|l| !within.contains(&l.0) && l.2.is_some_and(|p| within.contains(&p))) {
                within.insert(more.0);
            }
            let mut doc = diagram_of(data, opts, Some(&within));
            doc.name = name.clone();
            doc.source = format!("dcim.location:{lid}");
            doc
        })
        .collect()
}

/// The devices of the locations `within` (all of the site's if none) and
/// their cables; devices elsewhere cabled to them go on the layer
/// [`OUTSIDE_LAYER`].
fn diagram_of(data: &SiteData, opts: &Options, within: Option<&HashSet<u64>>) -> Diagram {
    let mut doc = Diagram::default();
    // With layers by type, the first layer (where lines go) holds the cables.
    if opts.type_layers
        && let Some(l) = doc.layers.first_mut()
    {
        l.name = CABLES_LAYER.into();
    }
    let mut type_layers: HashMap<String, Id> = HashMap::new();
    let interfaces: HashMap<u64, &Value> = data.interfaces.iter().filter_map(|v| Some((id(v)?, v))).collect();
    let location: HashMap<u64, Option<u64>> = data.devices.iter().filter_map(|v| Some((id(v)?, id(at(v, &["location"]))))).collect();
    let inside = |dev: u64| within.is_none_or(|w| location.get(&dev).copied().flatten().is_some_and(|l| w.contains(&l)));
    let inside_end = |e: &End| matches!(e, End::Port(dev, ..) if inside(*dev));

    // The cables worth drawing, with both ends: here, those with an end
    // inside.
    let cables: Vec<(&Value, End, End)> = data
        .cables
        .iter()
        .filter_map(|c| {
            let first = |key: &str| c.get(key).and_then(Value::as_array).and_then(|t| t.iter().find_map(cable_end));
            Some((c, first("a_terminations")?, first("b_terminations")?))
        })
        .filter(|(_, a, b)| inside_end(a) || inside_end(b))
        .collect();
    let cabled: HashSet<u64> = cables
        .iter()
        .flat_map(|(_, a, b)| [a, b])
        .filter_map(|e| if let End::Port(d, ..) = e { Some(*d) } else { None })
        .collect();

    // Devices, in a grid until laid out.
    let mut devices: HashMap<u64, Id> = HashMap::new();
    let mut slot = 0;
    // With layers by type, each device here is on a layer of its own
    // (named after it) under its type's.
    let mut place = |doc: &mut Diagram, name: &str, kind: &str, source: String, here: bool| {
        let r = Rect { x: (slot % 6) as f64 * 200.0, y: (slot / 6) as f64 * 140.0, w: 120.0, h: 60.0 };
        slot += 1;
        let color = crate::editor::kind_color(kind).unwrap_or("#dbeafe");
        let dev = doc.add_device(name, kind, r, color);
        let layer = (opts.type_layers && here).then(|| {
            let of_type = *type_layers.entry(kind.to_string()).or_insert_with(|| {
                let lines = doc.active_layer;
                let layer = doc.add_layer(&type_layer_name(kind));
                doc.active_layer = lines;
                layer
            });
            let own = doc.add_sublayer(of_type, if name.is_empty() { &source } else { name });
            if let Some(l) = doc.layer_mut(own) {
                l.source = source.clone();
            }
            own
        });
        if let Some(d) = doc.device_mut(dev) {
            d.source = source;
            d.layer = layer.unwrap_or(d.layer);
        }
        dev
    };
    let mut outside_layer = None;
    for v in &data.devices {
        let Some(nid) = id(v) else { continue };
        let here = inside(nid);
        if !(cabled.contains(&nid) || here && opts.all_devices) {
            continue;
        }
        let name = match text(v, &["name"]) {
            n if n.is_empty() => text(v, &["display"]),
            n => n,
        };
        let (kind, panel) = kind_and_panel(v, opts);
        let dev = place(&mut doc, &name, &kind, format!("dcim.device:{nid}"), here);
        if let Some(d) = doc.device_mut(dev)
            && panel.is_some()
        {
            d.panel = panel;
            d.fit_panel();
        }
        if !here {
            let layer = *outside_layer.get_or_insert_with(|| {
                let home = doc.active_layer;
                let layer = doc.add_layer(OUTSIDE_LAYER);
                // Its name on its devices tells them apart.
                if let Some(l) = doc.layer_mut(layer) {
                    l.show_name = true;
                }
                doc.active_layer = home;
                layer
            });
            doc.device_mut(dev).expect("device").layer = layer;
        }
        devices.insert(nid, dev);
    }

    // Ports and links.
    let mut cable_links: HashMap<u64, Id> = HashMap::new();
    let mut circuits: HashMap<u64, Id> = HashMap::new();
    let mut ports: HashMap<String, Endpoint> = HashMap::new();
    for (cable, a, b) in &cables {
        let mut endpoint = |doc: &mut Diagram, e: &End| -> Option<Endpoint> {
            match e {
                End::Port(dev, kind, oid, name, socket) => {
                    let source = format!("{kind}:{oid}");
                    if let Some(ep) = ports.get(&source) {
                        return Some(*ep);
                    }
                    let device = *devices.get(dev)?;
                    let ep = doc.add_port(device, Side::Right, 0.5)?;
                    let d = doc.device_mut(device)?;
                    // A patch panel's front port, or a switch's port, sits in its
                    // socket; a rear port at the back of its socket.
                    let rear = kind == "dcim.rearport";
                    let socket = match rear {
                        true if d.rear_pos(*socket, false).is_some() => *socket,
                        false if d.socket_pos(*socket).is_some() => *socket,
                        _ => 0,
                    };
                    if let Some(p) = d.ports.iter_mut().find(|p| p.id == ep.port) {
                        p.name = name.clone();
                        p.source = source.clone();
                        p.socket = socket;
                        p.rear = rear && socket > 0;
                    }
                    ports.insert(source, ep);
                    Some(ep)
                }
                End::Circuit(cid, name) => {
                    let device = *circuits.entry(*cid).or_insert_with(|| place(doc, name, "internet", format!("circuits.circuit:{cid}"), true));
                    doc.add_port(device, Side::Bottom, 0.5)
                }
            }
        };
        let (Some(ea), Some(eb)) = (endpoint(&mut doc, a), endpoint(&mut doc, b)) else { continue };
        let iface = |e: &End| match e {
            End::Port(_, kind, oid, ..) if kind == "dcim.interface" => interfaces.get(oid).copied(),
            _ => None,
        };
        let style = LinkStyle { kind: link_kind(iface(a), iface(b)), routing: opts.routing, ..Default::default() };
        let link = doc.add_link(ea, eb, style);
        if let Some(l) = doc.link_mut(link) {
            l.label = text(cable, &["label"]);
            l.source = format!("dcim.cable:{}", id(cable).unwrap_or(0));
        }
        if let Some(c) = id(cable) {
            cable_links.insert(c, link);
        }
    }
    // The grid again with the devices' sizes (a panel is wider than a
    // slot): none may start inside another, which would be its contents.
    let (mut x, mut y, mut row) = (0.0, 0.0, 0.0_f64);
    for (i, d) in doc.devices.iter_mut().enumerate() {
        if i > 0 && i % 6 == 0 {
            (x, y, row) = (0.0, y + row + 80.0, 0.0);
        }
        (d.x, d.y) = (x, y);
        x += d.w + 80.0;
        row = row.max(d.h);
    }
    doc.traces = traces(&doc, &data.paths, &cable_links);
    // Layers by type in the order of the network (internet at the bottom
    // of the list, servers last), after the cables.
    let rank: HashMap<Id, u8> = type_layers.iter().map(|(kind, l)| (*l, crate::layout::kind_rank(kind))).collect();
    // (Each device's own layer with its type's, after it.)
    doc.layers.sort_by_key(|l| match rank.get(&l.id).or_else(|| rank.get(&l.parent)) {
        Some(r) => 1 + *r as usize,
        None if l.name == OUTSIDE_LAYER => 1000,
        None => 0,
    });
    doc
}

/// The traces along NetBox's cable `paths`, as far as their cables are
/// drawn (`links`: by cable id): each stretch of at least two of them one
/// after another, once (a path is there from both of its ends).
fn traces(doc: &Diagram, paths: &[Vec<u64>], links: &HashMap<u64, Id>) -> Vec<Trace> {
    let mut seen: HashSet<Vec<Id>> = HashSet::new();
    let mut out = vec![];
    for path in paths {
        for run in path.split(|c| !links.contains_key(c)) {
            let mut ids: Vec<Id> = run.iter().map(|c| links[c]).collect();
            if ids.len() < 2 {
                continue;
            }
            let key = if ids.first() > ids.last() { ids.iter().rev().copied().collect() } else { ids.clone() };
            if !seen.insert(key) {
                continue;
            }
            ids.dedup();
            let t = Trace { links: ids };
            if doc.is_trace(&t) {
                out.push(t);
            }
        }
    }
    out
}

// Refreshing

/// The layer for what NetBox no longer has.
pub const GONE_LAYER: &str = "Gone from NetBox";

/// What a refresh changed.
#[derive(Debug, Default, PartialEq)]
pub struct Refreshed {
    pub added_devices: usize,
    pub added_links: usize,
    pub renamed: usize,
    pub gone: usize,
    /// Patch panels and switches whose ports changed.
    pub panels: usize,
}

impl Refreshed {
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        let n = |count: usize, one: &str, many: &str| format!("{count} {}", if count == 1 { one } else { many });
        if self.added_devices > 0 {
            parts.push(n(self.added_devices, "new device", "new devices"));
        }
        if self.added_links > 0 {
            parts.push(n(self.added_links, "new link", "new links"));
        }
        if self.renamed > 0 {
            parts.push(format!("{} renamed", self.renamed));
        }
        if self.panels > 0 {
            parts.push(n(self.panels, "panel with other ports", "panels with other ports"));
        }
        if self.gone > 0 {
            parts.push(format!("{} gone (on the layer \"{GONE_LAYER}\")", self.gone));
        }
        if parts.is_empty() { "Up to date with NetBox.".into() } else { format!("From NetBox: {}.", parts.join(", ")) }
    }
}

/// Bring `doc` up to date with `fresh` (a [`site_diagram`] of the same
/// site): new devices are placed beside a device they are linked to (or
/// below everything), new ports and links added, names updated; devices
/// and links NetBox no longer has move to the layer [`GONE_LAYER`] (and
/// back if they return). Nothing else moves; drawn items stay as they are.
pub fn refresh(doc: &mut Diagram, fresh: &Diagram) -> Refreshed {
    let mut out = Refreshed::default();
    let by_source: HashMap<String, Id> =
        doc.devices.iter().filter(|d| !d.source.is_empty()).map(|d| (d.source.clone(), d.id)).collect();
    let home = doc.layers.iter().find(|l| l.frame.is_none() && l.name != GONE_LAYER && l.name != OUTSIDE_LAYER).map(|l| l.id);
    let gone_layer = doc.layers.iter().find(|l| l.name == GONE_LAYER).map(|l| l.id);
    // With layers by type, fresh devices and cables go on the layers of
    // their names (made when missing); a diagram from before them gets them
    // at its first refresh, NetBox's devices and cables moving onto them.
    let by_type = fresh.layers.iter().any(|l| l.name == CABLES_LAYER);
    let regroup = by_type && !doc.layers.iter().any(|l| l.name == CABLES_LAYER);
    let managed: HashSet<String> = fresh
        .layers
        .iter()
        .filter(|l| l.name != OUTSIDE_LAYER && l.frame.is_none() && l.source.is_empty())
        .map(|l| l.name.clone())
        .collect();
    // (None without layers by type: new items go on the active layer.)
    let layer_for = |doc: &mut Diagram, id: Id| -> Option<Id> { if by_type { layer_like(doc, fresh, id) } else { None } };
    // Devices from other locations (on a location's sheet) on their layer.
    let fresh_outside = fresh.layers.iter().find(|l| l.name == OUTSIDE_LAYER).map(|l| l.id);
    let mut outside = doc.layers.iter().find(|l| l.name == OUTSIDE_LAYER).map(|l| l.id);
    if outside.is_none() && fresh_outside.is_some_and(|o| fresh.devices.iter().any(|d| d.layer == o)) {
        let id = doc.add_layer(OUTSIDE_LAYER);
        if let Some(l) = doc.layer_mut(id) {
            l.show_name = true;
        }
        outside = Some(id);
    }

    // Devices and their ports: fresh ids to ids in `doc`.
    let mut new_ports: Vec<Endpoint> = vec![];
    let mut dev_map: HashMap<Id, Id> = HashMap::new();
    let mut port_map: HashMap<Id, Endpoint> = HashMap::new();
    let mut new_devices = vec![];
    for f in &fresh.devices {
        let out_now = fresh_outside.is_some() && Some(f.layer) == fresh_outside;
        let id = match by_source.get(&f.source) {
            Some(&id) => {
                // Moved into this location or out of it in NetBox, back from
                // the gone layer, or onto layers by type for the first time.
                // Also onto a layer of its own, from its type's.
                let layer = doc.device(id).expect("device by source").layer;
                let on = doc.layer(layer);
                let on_own = on.is_some_and(|l| !l.source.is_empty() && l.source == f.source);
                let regular = Some(layer) == home || on.is_some_and(|l| managed.contains(&l.name) || !l.source.is_empty());
                let back = gone_layer.is_some() && Some(layer) == gone_layer;
                let fl = fresh.layer(f.layer);
                let type_name = fl.and_then(|l| fresh.layer(l.parent)).map(|p| p.name.as_str());
                let to_own = by_type && fl.is_some_and(|l| !l.source.is_empty()) && !on_own && on.is_some_and(|l| Some(l.name.as_str()) == type_name);
                let moved = if out_now && (regular || back) {
                    outside
                } else if !out_now && (back || outside.is_some() && Some(layer) == outside || regroup && Some(layer) == home || to_own) {
                    layer_for(doc, f.layer).or(home)
                } else {
                    None
                };
                // Its own layer is named after it.
                if let Some(l) = doc.layers.iter_mut().find(|l| !l.source.is_empty() && l.source == f.source) {
                    l.name = f.name.clone();
                }
                let d = doc.device_mut(id).expect("device by source");
                d.layer = moved.unwrap_or(d.layer);
                if d.name != f.name {
                    d.name = f.name.clone();
                    out.renamed += 1;
                }
                // Front ports (or a switch's ports) added or removed: the
                // panel grows or shrinks. A switch drawn as a box stays one.
                let has_front = d.is_patch_panel() || d.panel.is_some() && d.front().is_some();
                if f.panel.is_some() && has_front && d.panel != f.panel {
                    d.panel = f.panel;
                    d.fit_panel();
                    out.panels += 1;
                }
                id
            }
            None => {
                let layer = if out_now { outside } else { layer_for(doc, f.layer) };
                let id = doc.add_device(&f.name, &f.kind, f.rect(), &f.color);
                let d = doc.device_mut(id).expect("new device");
                d.source = f.source.clone();
                d.panel = f.panel;
                d.layer = layer.unwrap_or(d.layer);
                new_devices.push(id);
                out.added_devices += 1;
                id
            }
        };
        dev_map.insert(f.id, id);
        for fp in &f.ports {
            let existing = doc.device(id).and_then(|d| d.ports.iter().find(|p| !fp.source.is_empty() && p.source == fp.source)).map(|p| p.id);
            let port = match existing {
                Some(port) => {
                    let p = doc.device_mut(id).and_then(|d| d.ports.iter_mut().find(|p| p.id == port)).expect("port");
                    if p.name != fp.name {
                        p.name = fp.name.clone();
                        out.renamed += 1;
                    }
                    p.socket = fp.socket;
                    p.rear = fp.rear;
                    Endpoint { device: id, port }
                }
                None => {
                    let ep = doc.add_port(id, fp.side, fp.t).expect("port on a device");
                    if let Some(p) = doc.device_mut(id).and_then(|d| d.ports.iter_mut().find(|p| p.id == ep.port)) {
                        p.name = fp.name.clone();
                        p.source = fp.source.clone();
                        p.socket = fp.socket;
                        p.rear = fp.rear;
                    }
                    new_ports.push(ep);
                    ep
                }
            };
            port_map.insert(fp.id, port);
        }
    }

    // Links.
    let mut known_links: std::collections::HashSet<String> =
        doc.links.iter().filter(|l| !l.source.is_empty()).map(|l| l.source.clone()).collect();
    for fl in &fresh.links {
        if known_links.contains(&fl.source) {
            let moves = |l: &crate::model::Link| gone_layer.is_some() && Some(l.layer) == gone_layer || regroup && Some(l.layer) == home;
            if doc.links.iter().any(|l| l.source == fl.source && moves(l)) {
                let target = layer_for(doc, fl.layer).or(home);
                for l in doc.links.iter_mut().filter(|l| l.source == fl.source && moves(l)) {
                    l.layer = target.unwrap_or(l.layer);
                }
            }
            continue;
        }
        let target = layer_for(doc, fl.layer);
        let (Some(&a), Some(&b)) = (port_map.get(&fl.from.port), port_map.get(&fl.to.port)) else { continue };
        let link = doc.add_link(a, b, fl.style.clone());
        if let Some(l) = doc.link_mut(link) {
            l.label = fl.label.clone();
            l.source = fl.source.clone();
            l.layer = target.unwrap_or(l.layer);
        }
        known_links.insert(fl.source.clone());
        out.added_links += 1;
    }

    // What NetBox no longer has: set aside on its own layer.
    let fresh_devices: std::collections::HashSet<&str> = fresh.devices.iter().map(|d| d.source.as_str()).collect();
    let fresh_links: std::collections::HashSet<&str> = fresh.links.iter().map(|l| l.source.as_str()).collect();
    let gone_devices: Vec<Id> = doc
        .devices
        .iter()
        .filter(|d| !d.source.is_empty() && !fresh_devices.contains(d.source.as_str()) && Some(d.layer) != gone_layer)
        .map(|d| d.id)
        .collect();
    let gone_links: Vec<Id> = doc
        .links
        .iter()
        .filter(|l| !l.source.is_empty() && !fresh_links.contains(l.source.as_str()) && Some(l.layer) != gone_layer)
        .map(|l| l.id)
        .collect();
    if !gone_devices.is_empty() || !gone_links.is_empty() {
        let layer = gone_layer.unwrap_or_else(|| {
            let id = doc.add_layer(GONE_LAYER);
            // Its name on its devices marks them on the drawing.
            if let Some(l) = doc.layer_mut(id) {
                l.show_name = true;
            }
            id
        });
        for id in &gone_devices {
            doc.device_mut(*id).expect("gone device").layer = layer;
        }
        for id in &gone_links {
            doc.link_mut(*id).expect("gone link").layer = layer;
        }
        out.gone = gone_devices.len() + gone_links.len();
    }

    // The cabling from end to end, as NetBox has it now.
    let by_link_source: HashMap<&str, Id> = doc.links.iter().filter(|l| !l.source.is_empty()).map(|l| (l.source.as_str(), l.id)).collect();
    let fresh_links: HashMap<Id, &str> = fresh.links.iter().map(|l| (l.id, l.source.as_str())).collect();
    let traces: Vec<Trace> = fresh
        .traces
        .iter()
        .filter_map(|t| {
            let links = t.links.iter().map(|id| fresh_links.get(id).and_then(|s| by_link_source.get(s)).copied()).collect::<Option<Vec<Id>>>()?;
            Some(Trace { links })
        })
        .filter(|t| doc.is_trace(t))
        .collect();
    doc.traces = traces;

    // Devices' own layers left empty by devices gone from NetBox go too.
    let fresh_sources: HashSet<&str> = fresh.layers.iter().filter(|l| !l.source.is_empty()).map(|l| l.source.as_str()).collect();
    let empty_own: Vec<Id> = doc
        .layers
        .iter()
        .filter(|l| !l.source.is_empty() && !fresh_sources.contains(l.source.as_str()))
        .filter(|l| doc.layer_count(l.id) == 0 && !doc.layers.iter().any(|x| x.parent == l.id) && l.id != doc.active_layer)
        .map(|l| l.id)
        .collect();
    for id in empty_own {
        doc.remove_layer(id);
    }

    place_new(doc, &new_devices);
    face_new_ports(doc, &new_ports);
    out
}

/// The layer in `doc` like `fresh`'s layer `id` (with layers by type): a
/// device's own (by what it stands for) under the layer of its type, or
/// the top layer of its name; made when missing.
fn layer_like(doc: &mut Diagram, fresh: &Diagram, id: Id) -> Option<Id> {
    let fl = fresh.layer(id)?;
    if !fl.source.is_empty() {
        if let Some(l) = doc.layers.iter().find(|l| l.source == fl.source) {
            return Some(l.id);
        }
        let parent = if fl.parent == 0 { 0 } else { layer_like(doc, fresh, fl.parent)? };
        let own = if parent == 0 {
            let active = doc.active_layer;
            let id = doc.add_layer(&fl.name);
            doc.active_layer = active;
            id
        } else {
            doc.add_sublayer(parent, &fl.name)
        };
        if let Some(l) = doc.layer_mut(own) {
            l.source = fl.source.clone();
        }
        return Some(own);
    }
    if let Some(l) = doc.layers.iter().find(|l| l.frame.is_none() && l.parent == 0 && l.name == fl.name) {
        return Some(l.id);
    }
    let active = doc.active_layer;
    let id = doc.add_layer(&fl.name);
    doc.active_layer = active;
    Some(id)
}

/// New ports on the side facing the device at their link's other end, in
/// the widest free stretch of that side (the ports already there stay).
fn face_new_ports(doc: &mut Diagram, ports: &[Endpoint]) {
    for ep in ports {
        let other = doc.links.iter().find_map(|l| {
            if l.from == *ep {
                Some(l.to.owner())
            } else if l.to == *ep {
                Some(l.from.owner())
            } else {
                None
            }
        });
        let (Some(d), Some(o)) = (doc.device(ep.device), other.and_then(|o| doc.device(o)).map(|o| o.rect())) else { continue };
        let r = d.rect();
        let (dx, dy) = (o.x + o.w / 2.0 - (r.x + r.w / 2.0), o.y + o.h / 2.0 - (r.y + r.h / 2.0));
        // Beside each other (in one row): left or right; else top or bottom.
        let side = if o.y < r.y + r.h && r.y < o.y + o.h {
            if dx > 0.0 { Side::Right } else { Side::Left }
        } else if dy > 0.0 {
            Side::Bottom
        } else {
            Side::Top
        };
        let mut taken: Vec<f64> = d.ports.iter().filter(|p| p.id != ep.port && p.side == side).map(|p| p.t).collect();
        taken.extend([0.0, 1.0]);
        taken.sort_by(f64::total_cmp);
        let t = taken.windows(2).max_by(|a, b| (a[1] - a[0]).total_cmp(&(b[1] - b[0]))).map_or(0.5, |w| (w[0] + w[1]) / 2.0);
        if let Some(p) = doc.device_mut(ep.device).and_then(|d| d.ports.iter_mut().find(|p| p.id == ep.port)) {
            (p.side, p.t) = (side, t);
        }
    }
}

/// Put each new device beside a device it is linked to, where there is
/// room; those linked to nothing placed go in a row under everything.
fn place_new(doc: &mut Diagram, new: &[Id]) {
    let placed = |doc: &Diagram, id: Id| !new.contains(&id) && doc.device(id).is_some();
    let mut fixed: Vec<Id> = doc.devices.iter().map(|d| d.id).filter(|id| placed(doc, *id)).collect();
    let mut row_x = None;
    for &id in new {
        let neighbor = doc.links.iter().find_map(|l| {
            let (a, b) = (l.from.owner(), l.to.owner());
            if a == id && fixed.contains(&b) {
                Some(b)
            } else if b == id && fixed.contains(&a) {
                Some(a)
            } else {
                None
            }
        });
        let (w, h) = doc.device(id).map(|d| (d.w, d.h)).expect("new device");
        let free = |doc: &Diagram, r: Rect| {
            fixed.iter().filter_map(|f| doc.device(*f)).all(|d| {
                let o = d.rect();
                r.x + r.w + 30.0 <= o.x || o.x + o.w + 30.0 <= r.x || r.y + r.h + 30.0 <= o.y || o.y + o.h + 30.0 <= r.y
            })
        };
        // With others already hanging below the neighbor: at the end of
        // their row.
        let row_end = neighbor.and_then(|n| {
            let o = doc.device(n)?.rect();
            let below: Vec<Rect> = doc
                .links
                .iter()
                .filter_map(|l| match (l.from.owner(), l.to.owner()) {
                    (a, b) if a == n => Some(b),
                    (a, b) if b == n => Some(a),
                    _ => None,
                })
                .filter(|&s| s != id && fixed.contains(&s))
                .filter_map(|s| doc.device(s).map(|d| d.rect()))
                .filter(|r| r.y > o.y + o.h)
                .collect();
            let top = below.iter().map(|r| r.y).fold(f64::MAX, f64::min);
            if below.is_empty() {
                return None;
            }
            // Everything in that row, siblings or not.
            let right = fixed
                .iter()
                .filter_map(|f| doc.device(*f))
                .map(|d| d.rect())
                .filter(|r| r.y < top + h && r.y + r.h > top)
                .map(|r| r.x + r.w)
                .fold(f64::MIN, f64::max);
            let r = Rect { x: ((right + 40.0) / 10.0).round() * 10.0, y: top, w, h };
            free(doc, r).then_some(r)
        });
        let spot = row_end.or_else(|| neighbor.and_then(|n| {
            let o = doc.device(n)?.rect();
            // Below, then to the sides, then above, further out each round.
            (1..12).find_map(|k| {
                let step = k as f64;
                let below = o.y + o.h + 60.0 * step;
                [
                    (o.x + (o.w - w) / 2.0, below),
                    (o.x + (o.w - w) / 2.0 + (w + 40.0) * step, below),
                    (o.x + (o.w - w) / 2.0 - (w + 40.0) * step, below),
                    (o.x + o.w + 60.0 * step, o.y + (o.h - h) / 2.0),
                    (o.x - w - 60.0 * step, o.y + (o.h - h) / 2.0),
                    (o.x + (o.w - w) / 2.0, o.y - h - 60.0 * step),
                ]
                .into_iter()
                .map(|(x, y)| Rect { x: (x / 10.0).round() * 10.0, y: (y / 10.0).round() * 10.0, w, h })
                .find(|r| free(doc, *r))
            })
        }));
        let r = spot.unwrap_or_else(|| {
            let bounds = fixed.iter().filter_map(|f| doc.device(*f)).map(|d| d.rect()).reduce(|a, b| a.union(&b));
            let (x0, y) = bounds.map_or((0.0, 0.0), |b| (b.x, b.y + b.h + 80.0));
            let x = *row_x.get_or_insert(x0);
            row_x = Some(x + w + 40.0);
            Rect { x, y, w, h }
        });
        if let Some(d) = doc.device_mut(id) {
            (d.x, d.y) = (r.x, r.y);
        }
        fixed.push(id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn iface(id: u64, dev: u64, name: &str, mode: Option<&str>, ips: u64) -> Value {
        json!({"id": id, "name": name, "device": {"id": dev, "name": format!("d{dev}")},
               "mode": mode.map(|m| json!({"value": m, "label": m})), "count_ipaddresses": ips})
    }

    fn term(id: u64, dev: u64, name: &str) -> Value {
        json!([{"object_type": "dcim.interface", "object_id": id,
                "object": {"id": id, "name": name, "device": {"id": dev, "name": format!("d{dev}")}}}])
    }

    fn site() -> Value {
        json!({
            "site": {"id": 1, "name": "Berlin"},
            "devices": {"count": 4, "results": [
                {"id": 1, "name": "fw-01", "role": {"slug": "firewall", "name": "Firewall"}},
                {"id": 2, "name": "core-01", "role": {"slug": "core-switch", "name": "Core Switch"}},
                {"id": 3, "name": "esx-01", "role": {"slug": "hypervisor", "name": "Hypervisor"}},
                {"id": 4, "name": "spare", "role": {"slug": "switch", "name": "Switch"}}
            ]},
            "interfaces": [
                iface(10, 1, "port3", None, 1), iface(20, 2, "Te1/1/1", None, 1),
                iface(21, 2, "Gi1/0/1", Some("tagged"), 0), iface(30, 3, "vmnic0", Some("tagged"), 0)
            ],
            "cables": [
                {"id": 100, "label": "uplink", "a_terminations": term(10, 1, "port3"), "b_terminations": term(20, 2, "Te1/1/1")},
                {"id": 101, "label": "", "a_terminations": term(21, 2, "Gi1/0/1"), "b_terminations": term(30, 3, "vmnic0")},
                {"id": 102, "a_terminations": [{"object_type": "dcim.interface", "object_id": 11,
                                                "object": {"id": 11, "name": "wan1", "device": {"id": 1}}}],
                 "b_terminations": [{"object_type": "circuits.circuittermination", "object_id": 7,
                                     "object": {"id": 7, "circuit": {"id": 5, "cid": "BER-INET-1", "provider": {"name": "Telekom"}}}}]},
                {"id": 103, "a_terminations": [{"object_type": "dcim.powerport", "object_id": 1, "object": {"id": 1, "device": {"id": 1}}}],
                 "b_terminations": [{"object_type": "dcim.poweroutlet", "object_id": 2, "object": {"id": 2, "device": {"id": 2}}}]}
            ]
        })
    }

    fn by_name<'a>(doc: &'a Diagram, name: &str) -> &'a crate::model::Device {
        doc.devices.iter().find(|d| d.name == name).unwrap_or_else(|| panic!("no {name}"))
    }

    #[test]
    fn patch_panels_get_their_front_ports() {
        let mut v = site();
        let devices = v["devices"]["results"].as_array_mut().unwrap();
        devices.push(json!({"id": 6, "name": "pp-a1", "role": {"slug": "cabling", "name": "Cabling"},
                            "front_ports": {"count": 48, "type": "lc-apc"}}));
        devices.push(json!({"id": 7, "name": "pp-a2", "role": {"slug": "patch-panel", "name": "Patch Panel"}}));
        let data: SiteData = serde_json::from_value(v).unwrap();
        let doc = site_diagram(&data, &Options { all_devices: true, ..Default::default() });
        let (a1, a2) = (by_name(&doc, "pp-a1"), by_name(&doc, "pp-a2"));
        assert_eq!(a1.kind, "patch panel", "front ports and a role that says nothing else");
        assert_eq!(a1.panel, Some(PatchPanel { ports: 48, connector: Connector::Lc, uplinks: 0 }));
        assert_eq!((a1.w, a1.h), crate::shape::panel_size(&a1.front().unwrap()));
        assert_eq!(a2.panel, Some(PatchPanel::default()), "by role, without front ports: 24 RJ45");
        // A switch with front ports stays a switch.
        assert_eq!(kind_and_panel(&json!({"role": {"slug": "switch"}, "front_ports": {"count": 4}}), &Options::default()).0, "switch");

        // Front ports added in NetBox: the panel grows.
        let mut doc = doc;
        let mut v = site();
        v["devices"]["results"].as_array_mut().unwrap().push(json!({"id": 6, "name": "pp-a1", "front_ports": {"count": 72, "type": "lc"}}));
        let fresh = site_diagram(&serde_json::from_value(v).unwrap(), &Options { all_devices: true, ..Default::default() });

        let r = refresh(&mut doc, &fresh);
        assert_eq!(r.panels, 1);
        assert!(r.summary().contains("1 panel with other ports"), "{}", r.summary());
        assert_eq!(by_name(&doc, "pp-a1").panel.unwrap().ports, 72);
        assert!(by_name(&doc, "pp-a1").h > a1_h(&fresh));
    }

    #[test]
    fn rear_port_cables_are_at_the_back_of_their_sockets() {
        let mut v = patched_site();
        // pp-a's rear port R1 is behind front port 1 (position 1); one behind
        // several has no position and stays on the border.
        let cables = v["cables"].as_array_mut().unwrap();
        cables.iter_mut().find(|c| c["id"] == json!(201)).unwrap()["a_terminations"][0]["object"]["position"] = json!(5);
        cables.iter_mut().find(|c| c["id"] == json!(201)).unwrap()["b_terminations"][0]["object"].as_object_mut().unwrap().remove("position");
        let doc = site_diagram(&serde_json::from_value(v).unwrap(), &Options::default());
        let (a, b) = (by_name(&doc, "pp-a"), by_name(&doc, "pp-b"));
        let r1 = a.ports.iter().find(|p| p.name == "R1").unwrap();
        assert_eq!((r1.rear, r1.socket), (true, 5));
        assert_eq!(a.port_pos(r1), a.rear_pos(5, r1.side == Side::Top).unwrap());
        let r1b = b.ports.iter().find(|p| p.name == "R1").unwrap();
        assert_eq!((r1b.rear, r1b.socket), (false, 0));
    }

    #[test]
    fn front_port_cables_plug_into_their_sockets() {
        let mut v = site();
        v["devices"]["results"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 6, "name": "pp-a1", "front_ports": {"count": 24, "type": "8p8c"}}));
        let front = |id: u64, name: &str, position: Option<u64>| {
            let mut obj = json!({"id": id, "name": name, "device": {"id": 6, "name": "pp-a1"}});
            if let Some(n) = position {
                obj["position"] = json!(n);
            }
            json!([{"object_type": "dcim.frontport", "object_id": id, "object": obj}])
        };
        let cables = v["cables"].as_array_mut().unwrap();
        cables.push(json!({"id": 110, "a_terminations": term(22, 2, "Gi1/0/2"), "b_terminations": front(60, "Port 5", Some(12))}));
        cables.push(json!({"id": 111, "a_terminations": term(23, 2, "Gi1/0/3"), "b_terminations": front(61, "FP-07", None)}));
        cables.push(json!({"id": 112, "a_terminations": term(24, 2, "Gi1/0/4"), "b_terminations": front(62, "Port 99", None)}));
        let data: SiteData = serde_json::from_value(v.clone()).unwrap();
        let doc = site_diagram(&data, &Options::default());
        let pp = by_name(&doc, "pp-a1");
        let socket = |name: &str| pp.ports.iter().find(|p| p.name == name).unwrap().socket;
        assert_eq!(socket("Port 5"), 12, "by its place among the panel's front ports");
        assert_eq!(socket("FP-07"), 7, "by the number in its name");
        assert_eq!(socket("Port 99"), 0, "past the last socket: on the border");
        let p12 = pp.ports.iter().find(|p| p.name == "Port 5").unwrap();
        assert_eq!(Some(pp.port_pos(p12)), pp.socket_pos(12));

        // Patched over to another socket in NetBox: it moves there.
        let mut doc = doc;
        v["cables"].as_array_mut().unwrap()[4]["b_terminations"] = front(60, "Port 5", Some(3));
        let fresh = site_diagram(&serde_json::from_value(v).unwrap(), &Options::default());
        refresh(&mut doc, &fresh);
        let pp = by_name(&doc, "pp-a1");
        assert_eq!(pp.ports.iter().find(|p| p.name == "Port 5").unwrap().socket, 3);
    }

    #[test]
    fn switches_get_their_ports_and_cables_their_sockets() {
        let mut v = site();
        v["devices"]["results"][1]["switch_ports"] = json!({"count": 48, "type": "1000base-t", "uplinks": 4});
        // The plugin numbers a switch's ports: copper first, then the uplinks.
        v["cables"][0]["b_terminations"][0]["object"]["position"] = json!(49);
        v["cables"][1]["a_terminations"][0]["object"]["position"] = json!(1);
        let data: SiteData = serde_json::from_value(v.clone()).unwrap();
        let doc = site_diagram(&data, &Options::default());
        let core = by_name(&doc, "core-01");
        assert_eq!((core.kind.as_str(), core.panel), ("switch", Some(PatchPanel { ports: 48, connector: Connector::Rj45, uplinks: 4 })));
        assert_eq!((core.w, core.h), crate::shape::panel_size(&core.front().unwrap()));
        let socket = |name: &str| core.ports.iter().find(|p| p.name == name).unwrap().socket;
        assert_eq!((socket("Gi1/0/1"), socket("Te1/1/1")), (1, 49));
        let gi = core.ports.iter().find(|p| p.name == "Gi1/0/1").unwrap();
        assert_eq!(Some(core.port_pos(gi)), core.socket_pos(1));
        // All SFP: SFP ports, no uplinks; without ports: a box.
        let sfp = json!({"role": {"slug": "switch"}, "switch_ports": {"count": 32, "type": "10gbase-x-sfpp", "uplinks": 0}});
        assert_eq!(kind_and_panel(&sfp, &Options::default()).1.unwrap().connector, Connector::Sfp);
        assert_eq!(kind_and_panel(&json!({"role": {"slug": "switch"}}), &Options::default()), ("switch".into(), None));
        // A router's ports stay on its border.
        let rtr = json!({"role": {"slug": "router"}, "switch_ports": {"count": 4, "type": "1000base-t"}});
        assert_eq!(kind_and_panel(&rtr, &Options::default()).1, None);

        // Refreshed into a diagram where it is a box: it stays one (the
        // ports remember their sockets, for when it shows its ports).
        let mut boxed = doc.clone();
        let id = boxed.devices.iter().find(|d| d.name == "core-01").unwrap().id;
        boxed.device_mut(id).unwrap().panel = None;
        let r = refresh(&mut boxed, &site_diagram(&serde_json::from_value(v.clone()).unwrap(), &Options::default()));
        assert_eq!((r.panels, boxed.device(id).unwrap().panel), (0, None));
        // Shown with its ports, it follows NetBox: two more uplinks.
        let mut doc = doc;
        v["devices"]["results"][1]["switch_ports"]["uplinks"] = json!(6);
        let r = refresh(&mut doc, &site_diagram(&serde_json::from_value(v).unwrap(), &Options::default()));
        assert_eq!((r.panels, by_name(&doc, "core-01").panel.unwrap().uplinks), (1, 6));
    }

    /// The site, with core-01's Gi1/0/2 cabled through two patch panels
    /// (front to rear, a trunk between them, rear to front) to esx-01.
    fn patched_site() -> Value {
        let mut v = site();
        let devices = v["devices"]["results"].as_array_mut().unwrap();
        devices.push(json!({"id": 6, "name": "pp-a", "role": {"slug": "patch-panel"}, "front_ports": {"count": 24, "type": "8p8c"}}));
        devices.push(json!({"id": 7, "name": "pp-b", "role": {"slug": "patch-panel"}, "front_ports": {"count": 24, "type": "8p8c"}}));
        let port = |kind: &str, id: u64, dev: u64, name: &str, position: u64| {
            json!([{"object_type": kind, "object_id": id,
                    "object": {"id": id, "name": name, "device": {"id": dev}, "position": position}}])
        };
        let cables = v["cables"].as_array_mut().unwrap();
        cables.push(json!({"id": 200, "a_terminations": term(22, 2, "Gi1/0/2"), "b_terminations": port("dcim.frontport", 60, 6, "1", 1)}));
        cables.push(json!({"id": 201, "a_terminations": port("dcim.rearport", 70, 6, "R1", 1), "b_terminations": port("dcim.rearport", 71, 7, "R1", 1)}));
        cables.push(json!({"id": 202, "a_terminations": port("dcim.frontport", 61, 7, "1", 1), "b_terminations": term(31, 3, "vmnic1")}));
        // From both ends, as NetBox has them.
        v["paths"] = json!([[200, 201, 202], [202, 201, 200], [101]]);
        v
    }

    #[test]
    fn wide_panels_do_not_start_around_their_neighbors() {
        // (Inside one, a device would be its contents and not laid out.)
        let data: SiteData = serde_json::from_value(patched_site()).unwrap();
        let doc = site_diagram(&data, &Options::default());
        assert!(doc.devices.iter().any(|d| d.w > 200.0), "a panel wider than a slot");
        assert_eq!(doc.containers(), HashMap::new());
    }

    #[test]
    fn devices_go_on_layers_by_type_and_cabling_shows_end_to_end() {
        let data: SiteData = serde_json::from_value(patched_site()).unwrap();
        let opts = Options { type_layers: true, ..Default::default() };
        let mut doc = site_diagram(&data, &opts);
        let names: Vec<&str> = doc.layers.iter().filter(|l| l.parent == 0).map(|l| l.name.as_str()).collect();
        assert_eq!(names, ["Cables", "Internet", "Firewalls", "Switches", "Patch panels", "Servers"], "in the order of the network");
        let layer = |doc: &Diagram, name: &str| doc.layers.iter().find(|l| l.name == name).unwrap().id;
        // Each device on a layer of its own, named after it, under its type's.
        let pp_a = by_name(&doc, "pp-a");
        assert_eq!(doc.layer_path(pp_a.layer), "Patch panels / pp-a");
        assert_eq!(doc.layer(pp_a.layer).unwrap().source, "dcim.device:6");
        assert_eq!(doc.layer_path(by_name(&doc, "core-01").layer), "Switches / core-01");
        assert_eq!(doc.layer_path(by_name(&doc, "Telekom BER-INET-1").layer), "Internet / Telekom BER-INET-1");
        let tree: Vec<String> = doc.layer_tree().iter().map(|(l, depth)| format!("{}{}", "  ".repeat(*depth), l.name)).collect();
        assert_eq!(tree[tree.len() - 5..], ["Patch panels", "  pp-a", "  pp-b", "Servers", "  esx-01"]);
        assert!(doc.links.iter().all(|l| l.layer == layer(&doc, "Cables")));
        assert_eq!(doc.traces.len(), 1, "once, and not for a single cable");
        let cable = |doc: &Diagram, n: u32| doc.links.iter().find(|l| l.source == format!("dcim.cable:{n}")).unwrap().id;
        assert_eq!(doc.traces[0].links, [cable(&doc, 200), cable(&doc, 201), cable(&doc, 202)]);

        // All shown: just the cables. Patch panels hidden: core-01 to esx-01.
        let ends = |d: &Diagram| {
            let mut e: Vec<(String, String)> = d.through_links().iter().map(|l| {
                let port = |ep: Endpoint| {
                    let dev = d.device(ep.device).unwrap();
                    format!("{} {}", dev.name, dev.port(ep.port).unwrap().name)
                };
                (port(l.from), port(l.to))
            }).collect();
            e.sort();
            e
        };
        assert!(ends(&doc).is_empty());
        let pp = layer(&doc, "Patch panels");
        doc.layer_mut(pp).unwrap().visible = false;
        assert_eq!(ends(&doc), [("core-01 Gi1/0/2".to_string(), "esx-01 vmnic1".to_string())]);
        let shown = doc.visible_only();
        let through = shown.links.iter().filter(|l| l.source.is_empty()).collect::<Vec<_>>();
        assert_eq!(through.len(), 1);
        assert!(shown.links.iter().all(|l| l.source.is_empty() || !["200", "201", "202"].iter().any(|n| l.source.ends_with(n))));
        assert!(crate::svg::export(&doc, "x.stratagram").matches("<polyline").count() >= 1);
        // Only pp-b hidden: from pp-a's rear port on to esx-01.
        doc.layer_mut(pp).unwrap().visible = true;
        let pp_b = by_name(&doc, "pp-b").id;
        let own = doc.add_layer("pp-b alone");
        doc.device_mut(pp_b).unwrap().layer = own;
        doc.layer_mut(own).unwrap().visible = false;
        assert_eq!(ends(&doc), [("pp-a R1".to_string(), "esx-01 vmnic1".to_string())]);
        // The cables hidden: no lines at all.
        let cables = layer(&doc, "Cables");
        doc.layer_mut(cables).unwrap().visible = false;
        assert!(ends(&doc).is_empty());
        // Kept in the file.
        let back = Diagram::from_json(&doc.to_json()).unwrap();
        assert_eq!(back.traces, doc.traces);
    }

    #[test]
    fn refreshing_puts_devices_on_layers_by_type() {
        let data: SiteData = serde_json::from_value(patched_site()).unwrap();
        // Imported before layers by type: all on one layer.
        let mut doc = site_diagram(&data, &Options::default());
        assert_eq!(doc.layers.len(), 1);
        assert!(doc.traces.len() == 1, "the cabling end to end comes along anyway");
        let note = doc.add_note("my note", crate::model::Pt::new(0.0, 0.0));
        let opts = Options { type_layers: true, ..Default::default() };
        refresh(&mut doc, &site_diagram(&data, &opts));
        let layer_of = |doc: &Diagram, name: &str| doc.layer_path(by_name(doc, name).layer);
        assert_eq!(layer_of(&doc, "pp-b"), "Patch panels / pp-b");
        assert_eq!(layer_of(&doc, "esx-01"), "Servers / esx-01");
        assert!(doc.links.iter().all(|l| doc.layer(l.layer).unwrap().name == "Cables"));
        assert_eq!(doc.layer(doc.note(note).unwrap().layer).unwrap().name, "Layer 1", "what was drawn by hand stays");
        // Later moved by hand: it stays there.
        let mine = doc.add_layer("Mine");
        let pp_a = by_name(&doc, "pp-a").id;
        doc.device_mut(pp_a).unwrap().layer = mine;
        let layers = doc.layers.len();
        refresh(&mut doc, &site_diagram(&data, &opts));
        assert_eq!((doc.device(pp_a).unwrap().layer, doc.layers.len()), (mine, layers));
        assert_eq!(doc.traces.len(), 1);
        // Renamed in NetBox: its layer too. Put back on its type's layer
        // (as before layers of their own): onto its own again.
        let mut v = patched_site();
        v["devices"]["results"].as_array_mut().unwrap().iter_mut().find(|d| d["id"] == json!(7)).unwrap()["name"] = json!("pp-b2");
        let pp_b = by_name(&doc, "pp-b").id;
        let panels = doc.layers.iter().find(|l| l.name == "Patch panels").unwrap().id;
        doc.device_mut(pp_b).unwrap().layer = panels;
        refresh(&mut doc, &site_diagram(&serde_json::from_value(v).unwrap(), &opts));
        assert_eq!(layer_of(&doc, "pp-b2"), "Patch panels / pp-b2");
        // A cable gone in NetBox: the cabling through it is no more.
        let mut v = patched_site();
        v["cables"].as_array_mut().unwrap().retain(|c| c["id"] != json!(201));
        v["paths"] = json!([[200], [202]]);
        refresh(&mut doc, &site_diagram(&serde_json::from_value(v).unwrap(), &opts));
        assert!(doc.traces.is_empty());
    }

    #[test]
    fn layers_are_named_after_types() {
        let names: Vec<String> = ["switch", "patch panel", "access point", "cloud proxy", "wlan controller", "", "internet", "aws transit gateway"]
            .iter()
            .map(|k| type_layer_name(k))
            .collect();
        assert_eq!(names, ["Switches", "Patch panels", "Access points", "Cloud proxies", "WLAN controllers", "Other devices", "Internet", "AWS transit gateways"]);
    }

    fn a1_h(_: &Diagram) -> f64 {
        crate::shape::panel_size(&crate::shape::Front { panel: PatchPanel { ports: 48, connector: Connector::Lc, uplinks: 0 }, switch: false }).1
    }

    #[test]
    fn roles_become_device_types() {
        let o = Options::default();
        assert_eq!(kind_for_role("firewall", "Firewall", &o), "firewall");
        assert_eq!(kind_for_role("core-switch", "Core Switch", &o), "switch");
        assert_eq!(kind_for_role("leaf", "Leaf", &o), "switch");
        assert_eq!(kind_for_role("wan-edge", "WAN Edge", &o), "router");
        assert_eq!(kind_for_role("wlc", "Wireless LAN Controller", &o), "wlan controller");
        assert_eq!(kind_for_role("ap", "Access Point", &o), "access point");
        assert_eq!(kind_for_role("hypervisor", "Hypervisor", &o), "server");
        assert_eq!(kind_for_role("patch-panel", "Patch Panel", &o), "patch panel");
        assert_eq!(kind_for_role("cabling", "Cabling", &o), "");
        let o = Options { kinds: [("patch-panel".to_string(), "switch".to_string())].into(), ..Default::default() };
        assert_eq!(kind_for_role("patch-panel", "Patch Panel", &o), "switch");
    }

    #[test]
    fn a_site_becomes_devices_ports_and_links() {
        let data: SiteData = serde_json::from_value(site()).unwrap();
        let doc = site_diagram(&data, &Options::default());
        // The uncabled spare is left out; the circuit is a cloud.
        let names: Vec<&str> = doc.devices.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["fw-01", "core-01", "esx-01", "Telekom BER-INET-1"]);
        assert_eq!(by_name(&doc, "fw-01").kind, "firewall");
        assert_eq!(by_name(&doc, "esx-01").kind, "server");
        assert_eq!(by_name(&doc, "Telekom BER-INET-1").kind, "internet");
        assert_eq!(by_name(&doc, "core-01").source, "dcim.device:2");
        // Ports named after the interfaces; power cabling left out.
        let ports: Vec<&str> = by_name(&doc, "core-01").ports.iter().map(|p| p.name.as_str()).collect();
        assert_eq!(ports, ["Te1/1/1", "Gi1/0/1"]);
        assert_eq!(doc.links.len(), 3);
        let link = |source: &str| doc.links.iter().find(|l| l.source == source).unwrap();
        assert_eq!(link("dcim.cable:100").style.kind, LinkKind::L3, "routed on both ends");
        assert_eq!(link("dcim.cable:100").label, "uplink");
        assert_eq!(link("dcim.cable:101").style.kind, LinkKind::Trunk);
        assert_eq!(link("dcim.cable:102").style.kind, LinkKind::Plain);
        // All of them, with the option.
        let doc = site_diagram(&data, &Options { all_devices: true, ..Default::default() });
        assert!(doc.devices.iter().any(|d| d.name == "spare"));
    }

    #[test]
    fn the_editor_imports_a_site_laid_out_and_refreshes_undoably() {
        let mut e = crate::editor::Editor::default();
        e.import_netbox(&site().to_string(), "").unwrap();
        assert!(e.status.starts_with("Imported 4 devices and 3 links of Berlin"), "{}", e.status);
        assert!(!e.can_undo() && e.dirty);
        // Top down: the provider, the firewall, the switch, the server.
        let y = |name: &str| by_name(&e.doc, name).y;
        assert!(y("Telekom BER-INET-1") < y("fw-01") && y("fw-01") < y("core-01") && y("core-01") < y("esx-01"));
        assert!(e.import_netbox("{\"devices\": []}", "").is_err());
        assert!(e.import_netbox(&site().to_string(), "{\"kinds\": 3}").is_err());

        // Renamed in NetBox: one undoable step.
        let mut v = site();
        v["devices"]["results"][0]["name"] = json!("fw-01a");
        e.refresh_netbox(&v.to_string(), "").unwrap();
        assert_eq!(e.status, "From NetBox: 1 renamed.");
        assert!(e.doc.devices.iter().any(|d| d.name == "fw-01a"));
        e.undo();
        assert!(e.doc.devices.iter().any(|d| d.name == "fw-01"));
        // Nothing new: nothing to undo either.
        e.refresh_netbox(&site().to_string(), "").unwrap();
        assert_eq!(e.status, "Up to date with NetBox.");
        assert!(!e.can_undo());
    }

    /// The site with locations: Building A (with Floor 1 inside) and
    /// Building B; the firewall in B, the switch in A, the server on Floor 1.
    fn site_with_locations() -> Value {
        let mut v = site();
        v["site"]["slug"] = json!("berlin");
        v["locations"] = json!([
            {"id": 1, "name": "Building A"},
            {"id": 2, "name": "Floor 1", "parent": {"id": 1}},
            {"id": 3, "name": "Building B"}
        ]);
        for (i, loc) in [(0, 3), (1, 1), (2, 2), (3, 1)] {
            v["devices"]["results"][i]["location"] = json!({"id": loc});
        }
        v
    }

    #[test]
    fn fronts_as_set_up() {
        let fw = json!({"role": {"slug": "firewall"}, "switch_ports": {"count": 6, "type": "rj45", "uplinks": 2}});
        let sw = json!({"role": {"slug": "switch"}, "switch_ports": {"count": 6, "type": "rj45", "uplinks": 2}});
        let ospf = json!({"role": {"slug": "ospf-router"}, "switch_ports": {"count": 4, "type": "rj45", "uplinks": 0}});
        let o = Options::default();
        assert!(kind_and_panel(&fw, &o).1.is_none(), "switches only, unless set up");
        assert!(kind_and_panel(&sw, &o).1.is_some());
        let o: Options = serde_json::from_str(r#"{"fronts": ["firewall", "Router"]}"#).unwrap();
        assert_eq!(kind_and_panel(&fw, &o).1.map(|p| (p.ports, p.uplinks)), Some((6, 2)));
        assert!(kind_and_panel(&ospf, &o).1.is_some(), "router types count as routers");
        assert!(kind_and_panel(&sw, &o).1.is_none(), "not listed");
    }

    #[test]
    fn cables_run_as_set_up() {
        let data: SiteData = serde_json::from_value(site()).unwrap();
        let routing = |opts: &str| -> Vec<Routing> {
            let opts: Options = serde_json::from_str(opts).unwrap();
            site_diagram(&data, &opts).links.iter().map(|l| l.style.routing).collect()
        };
        assert!(routing("{}").iter().all(|r| *r == Routing::Straight), "straight unless set");
        assert!(routing(r#"{"routing": "orthogonal"}"#).iter().all(|r| *r == Routing::Orthogonal));
        // Imported and laid out: orthogonal lines stay orthogonal (around
        // what is in the way at most), and new lines are drawn the same.
        let mut e = crate::editor::Editor::default();
        e.import_netbox(&site().to_string(), r#"{"routing": "orthogonal"}"#).unwrap();
        assert!(e.doc.links.iter().all(|l| matches!(l.style.routing, Routing::Orthogonal | Routing::Around)));
        assert_eq!(e.link_defaults.routing, Routing::Orthogonal);
    }

    #[test]
    fn each_location_gets_a_sheet() {
        let data: SiteData = serde_json::from_value(site_with_locations()).unwrap();
        let site = site_diagram(&data, &Options::default());
        assert_eq!((site.name.as_str(), site.source.as_str(), site.devices.len()), ("Berlin", "dcim.site:1", 4));
        let sheets = location_diagrams(&data, &Options::default());
        let names: Vec<&str> = sheets.iter().map(|d| d.name.as_str()).collect();
        assert_eq!(names, ["Building A", "Floor 1", "Building B"], "parents first");
        assert_eq!(sheets[1].source, "dcim.location:2");
        let outside = |d: &Diagram| d.layers.iter().find(|l| l.name == OUTSIDE_LAYER).map(|l| l.id);
        let on = |d: &Diagram, name: &str| {
            let dev = by_name(d, name);
            if Some(dev.layer) == outside(d) { "outside" } else { "inside" }
        };
        // Building A holds Floor 1's server too; the firewall it is cabled
        // to is from elsewhere.
        let a = &sheets[0];
        assert_eq!(a.devices.len(), 3);
        assert_eq!((on(a, "core-01"), on(a, "esx-01"), on(a, "fw-01")), ("inside", "inside", "outside"));
        assert!(a.layers.iter().find(|l| l.name == OUTSIDE_LAYER).unwrap().show_name);
        assert_eq!(a.links.len(), 2);
        // Floor 1: the server, and the switch it hangs off.
        let f = &sheets[1];
        assert_eq!((f.devices.len(), on(f, "esx-01"), on(f, "core-01")), (2, "inside", "outside"));
        // Building B: the firewall with its circuit, and the switch.
        let b = &sheets[2];
        assert_eq!(b.devices.len(), 3);
        assert_eq!((on(b, "fw-01"), on(b, "Telekom BER-INET-1"), on(b, "core-01")), ("inside", "inside", "outside"));
        // The uncabled spare (in Building A) only with the option.
        let all = location_diagrams(&data, &Options { all_devices: true, ..Default::default() });
        assert!(all[0].devices.iter().any(|d| d.name == "spare"));
    }

    #[test]
    fn location_sheets_are_imported_shown_and_refreshed() {
        let mut e = crate::editor::Editor::default();
        e.import_netbox(&site_with_locations().to_string(), "").unwrap();
        assert!(e.status.contains("a sheet for each of its 3 locations"), "{}", e.status);
        assert_eq!(e.sheet_count(), 4);
        assert_eq!(e.active_sheet(), 0);
        assert_eq!(e.sheet_name(0), "Berlin");
        assert!(e.show_sheet_of("dcim.location:2"));
        assert_eq!(e.sheet_name(e.active_sheet()), "Floor 1");
        assert!(!e.show_sheet_of("dcim.location:9"));
        // Laid out: the switch from elsewhere above the server.
        assert!(by_name(&e.doc, "core-01").y < by_name(&e.doc, "esx-01").y);

        // NetBox now: Floor 1 renamed, a new location with the spare cabled
        // to the switch, and the switch renamed.
        let mut v = site_with_locations();
        v["locations"][1]["name"] = json!("Floor 1 East");
        v["locations"].as_array_mut().unwrap().push(json!({"id": 4, "name": "Lab"}));
        v["devices"]["results"][3]["location"] = json!({"id": 4});
        v["devices"]["results"][1]["name"] = json!("core-01a");
        v["interfaces"].as_array_mut().unwrap().push(iface(22, 2, "Gi1/0/2", Some("access"), 0));
        v["interfaces"].as_array_mut().unwrap().push(iface(40, 4, "eth0", Some("access"), 0));
        v["cables"]
            .as_array_mut()
            .unwrap()
            .push(json!({"id": 104, "a_terminations": term(22, 2, "Gi1/0/2"), "b_terminations": term(40, 4, "eth0")}));
        e.refresh_netbox(&v.to_string(), "").unwrap();
        assert!(e.status.contains("1 location sheet renamed") && e.status.contains("1 new location sheet added"), "{}", e.status);
        assert_eq!(e.sheet_count(), 5);
        assert_eq!(e.sheet_name(e.active_sheet()), "Floor 1 East", "still in view");
        assert!(e.doc.devices.iter().any(|d| d.name == "core-01a"), "renamed on every sheet");
        assert!(e.show_sheet_of("dcim.location:4"));
        assert_eq!(e.doc.devices.len(), 2);
        // The site's sheet got the spare too; one undo step takes it all back.
        e.undo();
        assert_eq!(e.sheet_count(), 4);
        assert!(e.show_sheet_of("dcim.location:2"));
        assert_eq!(e.sheet_name(e.active_sheet()), "Floor 1");
    }

    #[test]
    fn refresh_adds_renames_and_sets_aside_without_moving() {
        let data: SiteData = serde_json::from_value(site()).unwrap();
        let mut doc = site_diagram(&data, &Options::default());
        // Arranged by hand.
        for (i, d) in doc.devices.iter_mut().enumerate() {
            (d.x, d.y) = (100.0 + 300.0 * i as f64, 500.0);
        }
        let before: Vec<(String, f64, f64)> = doc.devices.iter().map(|d| (d.source.clone(), d.x, d.y)).collect();
        assert_eq!(refresh(&mut doc, &site_diagram(&data, &Options::default())), Refreshed::default());

        // NetBox now: core-01 renamed, esx-01 and its cable gone, a new
        // server cabled to core-01.
        let mut v = site();
        v["devices"]["results"][1]["name"] = json!("core-01a");
        let devices = v["devices"]["results"].as_array_mut().unwrap();
        devices.retain(|d| d["id"] != 3);
        devices.push(json!({"id": 5, "name": "srv-02", "role": {"slug": "server"}}));
        v["interfaces"].as_array_mut().unwrap().push(iface(50, 5, "eth0", Some("access"), 0));
        v["interfaces"].as_array_mut().unwrap().push(iface(22, 2, "Gi1/0/2", Some("access"), 0));
        let cables = v["cables"].as_array_mut().unwrap();
        cables.retain(|c| c["id"] != 101);
        cables.push(json!({"id": 104, "a_terminations": term(22, 2, "Gi1/0/2"), "b_terminations": term(50, 5, "eth0")}));
        let fresh = site_diagram(&serde_json::from_value(v).unwrap(), &Options::default());

        let r = refresh(&mut doc, &fresh);
        assert_eq!(r, Refreshed { added_devices: 1, added_links: 1, renamed: 1, gone: 2, panels: 0 });
        assert!(r.summary().contains("1 new device"), "{}", r.summary());
        // Nothing that was there moved.
        for (source, x, y) in before {
            let d = doc.devices.iter().find(|d| d.source == source).unwrap();
            assert_eq!((d.x, d.y), (x, y), "{source} moved");
        }
        assert_eq!(doc.device(by_name(&doc, "core-01a").id).unwrap().source, "dcim.device:2");
        // The gone server and its cable are set aside on their own layer.
        let gone = doc.layers.iter().find(|l| l.name == GONE_LAYER).unwrap();
        assert!(gone.show_name, "gone devices are marked");
        let gone = gone.id;
        assert_eq!(by_name(&doc, "esx-01").layer, gone);
        assert_eq!(doc.links.iter().find(|l| l.source == "dcim.cable:101").unwrap().layer, gone);
        // The new server is beside its switch, clear of everything.
        let (srv, core) = (by_name(&doc, "srv-02").rect(), by_name(&doc, "core-01a").rect());
        assert!(srv.y > core.y + core.h && (srv.x + srv.w / 2.0 - (core.x + core.w / 2.0)).abs() < 400.0, "{srv:?} {core:?}");
        for d in doc.devices.iter().filter(|d| d.name != "srv-02") {
            let o = d.rect();
            assert!(srv.x + srv.w <= o.x || o.x + o.w <= srv.x || srv.y + srv.h <= o.y || o.y + o.h <= srv.y, "overlaps {}", d.name);
        }
        // The new link's ports face each other: the switch's on its bottom,
        // beside the ports already there.
        let link = doc.links.iter().find(|l| l.source == "dcim.cable:104").unwrap();
        assert_eq!(link.style.kind, LinkKind::L2);
        let core = by_name(&doc, "core-01a");
        let port = core.port(link.from.port).unwrap();
        assert_eq!(port.side, Side::Bottom);
        assert!(core.ports.iter().filter(|p| p.id != port.id && p.side == Side::Bottom).all(|p| (p.t - port.t).abs() > 0.1));
        assert_eq!(by_name(&doc, "srv-02").port(link.to.port).unwrap().side, Side::Top);
        // Refreshing again changes nothing.
        assert_eq!(refresh(&mut doc, &fresh), Refreshed::default());
    }
}
