// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Export to Cisco Modeling Labs: a lab topology (YAML, schema 0.2.1, which
//! CML 2.x imports) with a node per device and a link per line, so a drawn
//! network can be built and run as a lab.
//!
//! Each device type gets a CML node definition (a router an IOSv, a switch
//! an IOSvL2, a firewall an ASAv...), and each line end an interface of its
//! own named as that platform names them. Lines bent through points are one
//! link between their real ends; tunnels (VXLAN, IPsec...) are not cables
//! and are left out, as are lines ending at a free point. Text notes become
//! text annotations. What is hidden is left out, as in SVG export.

use std::collections::HashMap;
use std::fmt::Write;

use crate::geom;
use crate::model::{Device, Diagram, Endpoint, Id, LinkKind};

/// A CML platform: its node definition, and the name of its `slot`-th
/// interface (slots in use from `first_slot`).
struct Platform {
    definition: &'static str,
    first_slot: usize,
    /// Most interfaces it has (0: no limit worth checking).
    max: usize,
    name: fn(usize) -> String,
    /// Whether it takes an IOS style configuration (hostname, interface
    /// descriptions).
    ios: bool,
}

const IOSV: Platform = Platform {
    definition: "iosv",
    first_slot: 0,
    max: 16,
    name: |k| format!("GigabitEthernet0/{k}"),
    ios: true,
};
const IOSVL2: Platform = Platform {
    definition: "iosvl2",
    first_slot: 0,
    max: 16,
    name: |k| format!("GigabitEthernet{}/{}", k / 4, k % 4),
    ios: true,
};
// Slot 0 is Management0/0.
const ASAV: Platform = Platform {
    definition: "asav",
    first_slot: 1,
    max: 10,
    name: |k| format!("GigabitEthernet0/{}", k - 1),
    ios: true,
};
const UBUNTU: Platform = Platform {
    definition: "ubuntu",
    first_slot: 0,
    max: 8,
    name: |k| format!("ens{}", k + 2),
    ios: false,
};
const DESKTOP: Platform = Platform {
    definition: "desktop",
    first_slot: 0,
    max: 8,
    name: |k| format!("eth{k}"),
    ios: false,
};
const SWITCH: Platform = Platform {
    definition: "unmanaged_switch",
    first_slot: 0,
    max: 32,
    name: |k| format!("port{k}"),
    ios: false,
};
const EXTERNAL: Platform = Platform {
    definition: "external_connector",
    first_slot: 0,
    max: 1,
    name: |_| "port".into(),
    ios: false,
};

/// The platform for a device of `kind` with `links` lines.
fn platform(kind: &str, links: usize) -> &'static Platform {
    match kind {
        "router"
        | "ospf router"
        | "bgp reflector"
        | "aws transit gateway"
        | "aws direct connect gateway" => &IOSV,
        "switch" => &IOSVL2,
        "virtual switch" | "distributed virtual switch" => &SWITCH,
        "firewall" => &ASAV,
        "server" | "load balancer" => &UBUNTU,
        "client" => &DESKTOP,
        // The world outside the lab: a connector has one interface, so
        // one with several lines is a switch for them.
        "internet" | "cloud" | "aws" | "azure" | "google cloud" | "alibaba cloud"
        | "telekom cloud" | "stackit" | "cloud proxy" | "zscaler"
            if links <= 1 =>
        {
            &EXTERNAL
        }
        _ => &SWITCH,
    }
}

/// A line that is a cable in the lab: not a tunnel carried over others.
fn is_cable(kind: LinkKind, carried: bool) -> bool {
    !carried
        && !matches!(
            kind,
            LinkKind::Vxlan | LinkKind::Ipsec | LinkKind::Capwap | LinkKind::Sdwan
        )
}

/// What an export left out, for telling the user.
#[derive(Debug, Default, PartialEq)]
pub struct Skipped {
    /// Tunnels (VXLAN, IPsec...): not cables.
    pub tunnels: usize,
    /// Lines ending at a free point.
    pub loose: usize,
    /// Lines beyond what a device's platform has interfaces for.
    pub crowded: usize,
}

impl Skipped {
    pub fn summary(&self) -> String {
        let mut parts = vec![];
        let n = |c: usize, what: &str| format!("{c} {what}{}", if c == 1 { "" } else { "s" });
        if self.tunnels > 0 {
            parts.push(n(self.tunnels, "tunnel line"));
        }
        if self.loose > 0 {
            parts.push(n(self.loose, "line to a free point"));
        }
        if self.crowded > 0 {
            parts.push(n(self.crowded, "line beyond a device's interfaces"));
        }
        if parts.is_empty() {
            String::new()
        } else {
            format!("left out: {}", parts.join(", "))
        }
    }
}

/// The lab for `doc` titled `title`, and what was left out.
pub fn export(doc: &Diagram, title: &str) -> (String, Skipped) {
    let mut skipped = Skipped::default();
    let shown: Vec<&Device> = doc
        .devices
        .iter()
        .filter(|d| doc.layer_visible(d.layer))
        .collect();
    let is_shown = |id: Id| shown.iter().any(|d| d.id == id);

    // The cables: one per line, bent lines once (by their first piece),
    // between their real ends.
    let mut cables: Vec<(Endpoint, Endpoint, &crate::model::Link)> = vec![];
    for l in doc.links.iter().filter(|l| doc.link_visible(l)) {
        let bent = geom::bent_line(doc, l);
        if bent.pieces[0].0 != l.id {
            continue;
        }
        if !is_cable(l.style.kind, !l.style.over.is_empty()) {
            skipped.tunnels += 1;
            continue;
        }
        if bent.from.is_point() || bent.to.is_point() {
            skipped.loose += 1;
            continue;
        }
        if is_shown(bent.from.device)
            && is_shown(bent.to.device)
            && bent.from.device != bent.to.device
        {
            // The label of the line, wherever along a bent line it is.
            let labelled = bent
                .pieces
                .iter()
                .filter_map(|(id, _)| doc.link(*id))
                .find(|x| !x.label.is_empty())
                .unwrap_or(l);
            cables.push((bent.from, bent.to, labelled));
        }
    }

    // Platforms, and an interface per cable end.
    let mut ends: HashMap<Id, usize> = HashMap::new();
    for (a, b, _) in &cables {
        *ends.entry(a.device).or_default() += 1;
        *ends.entry(b.device).or_default() += 1;
    }
    let plat: HashMap<Id, &Platform> = shown
        .iter()
        .map(|d| {
            (
                d.id,
                platform(&d.kind, ends.get(&d.id).copied().unwrap_or(0)),
            )
        })
        .collect();
    let node_id: HashMap<Id, String> = shown
        .iter()
        .enumerate()
        .map(|(i, d)| (d.id, format!("n{i}")))
        .collect();
    // Per device: (interface id, slot, CML name, the port's own name).
    let mut ifaces: HashMap<Id, Vec<(String, usize, String, String)>> = HashMap::new();
    let mut links: Vec<(String, String, String, String, String)> = vec![];
    for (a, b, l) in &cables {
        let mut pick = |ep: &Endpoint| -> Option<String> {
            let p = plat[&ep.device];
            let list = ifaces.entry(ep.device).or_default();
            if p.max > 0 && list.len() >= p.max {
                return None;
            }
            let slot = p.first_slot + list.len();
            let id = format!("i{}", list.len());
            let port = doc
                .device(ep.device)
                .and_then(|d| d.port(ep.port))
                .map(|p| p.name.clone())
                .unwrap_or_default();
            list.push((id.clone(), slot, (p.name)(slot), port));
            Some(id)
        };
        let (Some(ia), Some(ib)) = (pick(a), pick(b)) else {
            skipped.crowded += 1;
            continue;
        };
        let name = |ep: &Endpoint, i: &str| {
            let d = doc.device(ep.device).map(|d| d.name.as_str()).unwrap_or("");
            let iface = ifaces[&ep.device]
                .iter()
                .find(|x| x.0 == i)
                .map(|x| x.2.as_str())
                .unwrap_or("");
            format!("{d}-{iface}")
        };
        let label = if l.label.is_empty() {
            format!("{}<->{}", name(a, &ia), name(b, &ib))
        } else {
            l.label.clone()
        };
        links.push((
            node_id[&a.device].clone(),
            ia,
            node_id[&b.device].clone(),
            ib,
            label,
        ));
    }

    // Positions: CML puts nodes by their centers, around the origin.
    let (mut cx, mut cy) = (0.0, 0.0);
    if !shown.is_empty() {
        cx = shown.iter().map(|d| d.x + d.w / 2.0).sum::<f64>() / shown.len() as f64;
        cy = shown.iter().map(|d| d.y + d.h / 2.0).sum::<f64>() / shown.len() as f64;
    }
    // Labels are unique in a lab.
    let mut seen: HashMap<String, usize> = HashMap::new();
    let mut s = String::new();
    // Text notes as text annotations.
    let notes: Vec<_> = doc
        .notes
        .iter()
        .filter(|n| doc.layer_visible(n.layer) && !n.text.trim().is_empty())
        .collect();
    s.push_str(if notes.is_empty() {
        "annotations: []\n"
    } else {
        "annotations:\n"
    });
    for (z, n) in notes.iter().enumerate() {
        let _ = write!(
            s,
            "  - border_color: '#00000000'\n    border_style: ''\n    color: {}\n    rotation: 0\n    text_bold: {}\n    text_content: {}\n    \
             text_font: monospace\n    text_italic: false\n    text_size: {}\n    text_unit: pt\n    thickness: 1\n    type: text\n    \
             x1: {:.1}\n    y1: {:.1}\n    z_index: {z}\n",
            quote(&n.color.to_uppercase()),
            n.bold,
            quote(&n.text),
            (n.size * 0.75).round().max(6.0),
            (n.x - cx).round(),
            (n.y - cy).round(),
        );
    }
    let _ = write!(
        s,
        "lab:\n  description: {}\n  notes: ''\n  title: {}\n  version: 0.2.1\n",
        quote("Drawn with Stratagram"),
        quote(title)
    );
    s.push_str(if links.is_empty() {
        "links: []\n"
    } else {
        "links:\n"
    });
    for (k, (n1, i1, n2, i2, label)) in links.iter().enumerate() {
        let _ = write!(
            s,
            "  - id: l{k}\n    n1: {n1}\n    n2: {n2}\n    i1: {i1}\n    i2: {i2}\n    conditioning: {{}}\n    label: {}\n",
            quote(label)
        );
    }
    s.push_str(if shown.is_empty() {
        "nodes: []\n"
    } else {
        "nodes:\n"
    });
    for d in &shown {
        let p = plat[&d.id];
        let base = if d.name.trim().is_empty() {
            p.definition.to_string()
        } else {
            d.name.trim().to_string()
        };
        let count = seen.entry(base.clone()).or_default();
        *count += 1;
        let label = if *count == 1 {
            base
        } else {
            format!("{base}-{count}")
        };
        let list = ifaces.get(&d.id).cloned().unwrap_or_default();
        let config = if p.ios {
            ios_config(&label, &list)
        } else {
            String::new()
        };
        let _ = write!(
            s,
            "  - boot_disk_size: null\n    configuration: {}\n    cpu_limit: null\n    cpus: null\n    data_volume: null\n    \
             hide_links: false\n    id: {}\n    image_definition: null\n    label: {}\n    node_definition: {}\n    ram: null\n    tags: []\n    \
             x: {}\n    y: {}\n",
            quote(&config),
            node_id[&d.id],
            quote(&label),
            p.definition,
            (d.x + d.w / 2.0 - cx).round() as i64,
            (d.y + d.h / 2.0 - cy).round() as i64,
        );
        if list.is_empty() {
            s.push_str("    interfaces: []\n");
            continue;
        }
        s.push_str("    interfaces:\n");
        for (id, slot, name, _) in &list {
            let _ = write!(
                s,
                "      - id: {id}\n        label: {}\n        slot: {slot}\n        type: physical\n",
                quote(name)
            );
        }
    }
    (s, skipped)
}

/// A start configuration for an IOS style platform: the hostname, and each
/// cabled interface described by the port's name in the drawing.
fn ios_config(label: &str, ifaces: &[(String, usize, String, String)]) -> String {
    let host: String = label
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '-' {
                c
            } else {
                '-'
            }
        })
        .collect();
    let mut c = format!("hostname {host}\n");
    for (_, _, name, port) in ifaces.iter().filter(|i| !i.3.is_empty()) {
        let _ = write!(
            c,
            "!\ninterface {name}\n description {port}\n no shutdown\n"
        );
    }
    c.push_str("!\nend\n");
    c
}

/// A YAML double-quoted string.
fn quote(s: &str) -> String {
    let mut out = String::from("\"");
    for ch in s.chars() {
        match ch {
            '"' => out.push_str("\\\""),
            '\\' => out.push_str("\\\\"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            c if (c as u32) < 0x20 => {
                let _ = write!(out, "\\x{:02x}", c as u32);
            }
            c => out.push(c),
        }
    }
    out.push('"');
    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::{LinkStyle, Rect, Side};

    #[test]
    fn a_network_becomes_a_lab() {
        let mut d = Diagram::default();
        let dev = |d: &mut Diagram, name: &str, kind: &str, x: f64| {
            d.add_device(
                name,
                kind,
                Rect {
                    x,
                    y: 0.0,
                    w: 120.0,
                    h: 60.0,
                },
                "#dbeafe",
            )
        };
        let inet = dev(&mut d, "Internet", "internet", 0.0);
        let fw = dev(&mut d, "fw 01", "firewall", 200.0);
        let rtr = dev(&mut d, "rtr-01", "router", 400.0);
        let sw = dev(&mut d, "sw-01", "switch", 600.0);
        let srv = dev(&mut d, "srv-01", "server", 800.0);
        let line = |d: &mut Diagram, a: Id, b: Id, kind: LinkKind, name: &str| {
            let pa = d.add_port(a, Side::Right, 0.5).unwrap();
            let pb = d.add_port(b, Side::Left, 0.5).unwrap();
            d.device_mut(a).unwrap().ports.last_mut().unwrap().name = name.into();
            d.add_link(
                pa,
                pb,
                LinkStyle {
                    kind,
                    ..Default::default()
                },
            )
        };
        line(&mut d, inet, fw, LinkKind::Plain, "");
        line(&mut d, inet, rtr, LinkKind::Plain, "");
        line(&mut d, fw, sw, LinkKind::Trunk, "inside");
        let bent = line(&mut d, sw, srv, LinkKind::L2, "Gi0/5");
        d.link_mut(bent).unwrap().label = "VLAN 20".into();
        d.split_link(bent, crate::model::Pt::new(700.0, 200.0));
        line(&mut d, fw, srv, LinkKind::Vxlan, "");
        let q = d.add_point(crate::model::Pt::new(0.0, 300.0));
        let p = d.add_port(rtr, Side::Bottom, 0.5).unwrap();
        d.add_link(p, Endpoint::point(q), LinkStyle::default());

        let (yaml, skipped) = export(&d, "Branch \"A\"");
        assert_eq!(
            skipped,
            Skipped {
                tunnels: 1,
                loose: 1,
                crowded: 0
            }
        );
        assert!(skipped.summary().contains("1 tunnel line"));
        // Platforms; the internet with two lines is a switch for them.
        for def in ["unmanaged_switch", "asav", "iosv", "iosvl2", "ubuntu"] {
            assert!(
                yaml.contains(&format!("node_definition: {def}\n")),
                "{def}: {yaml}"
            );
        }
        assert!(!yaml.contains("external_connector"));
        // Four cables: the bent line is one, between its real ends.
        assert_eq!(yaml.matches("  - id: l").count(), 4, "{yaml}");
        assert!(yaml.contains("label: \"VLAN 20\""));
        // Interfaces as the platforms name them: ASAv data from slot 1.
        assert!(
            yaml.contains("label: \"GigabitEthernet0/0\"\n        slot: 1"),
            "{yaml}"
        );
        assert!(yaml.contains("label: \"ens2\"\n        slot: 0"));
        // IOS style start config with the hostname and the port's name.
        assert!(
            yaml.contains(
                "hostname fw-01\\n!\\ninterface GigabitEthernet0/1\\n description inside"
            ),
            "{yaml}"
        );
        assert!(yaml.contains("title: \"Branch \\\"A\\\"\"\n  version: 0.2.1"));
    }
}
