// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Automatic layouts for devices. Works on plain boxes and edges, so it
//! knows nothing about the editor; positions come back as top left corners
//! with an arbitrary origin, and the caller moves them into place.

use crate::model::Pt;
use std::collections::VecDeque;
use std::f64::consts::TAU;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layout {
    /// Layers from the most "upstream" device (internet, firewall...) down.
    Hierarchy,
    /// The same, left to right.
    HierarchyLr,
    /// Rings around a hub.
    Radial,
    Circle,
    Grid,
    /// Spring embedder: linked devices pull together, all push apart.
    Force,
    /// Clean up what is there: rows and columns aligned, devices centered
    /// over what they link to, overlaps cleared, with small moves only.
    Tidy,
}

impl Layout {
    pub fn parse(s: &str) -> Option<Layout> {
        Some(match s {
            "hierarchy" => Layout::Hierarchy,
            "hierarchy-lr" => Layout::HierarchyLr,
            "radial" => Layout::Radial,
            "circle" => Layout::Circle,
            "grid" => Layout::Grid,
            "force" => Layout::Force,
            "tidy" => Layout::Tidy,
            _ => return None,
        })
    }

    pub fn name(self) -> &'static str {
        match self {
            Layout::Hierarchy => "hierarchy",
            Layout::HierarchyLr => "left-to-right hierarchy",
            Layout::Radial => "radial",
            Layout::Circle => "circle",
            Layout::Grid => "grid",
            Layout::Force => "force-directed",
            Layout::Tidy => "tidy",
        }
    }
}

#[derive(Clone, Debug)]
pub struct Node {
    pub w: f64,
    pub h: f64,
    /// Current top left corner (the force layout starts from here).
    pub pos: Pt,
    /// Lower is further upstream: becomes the root of hierarchies.
    pub rank: u8,
    /// Device type; devices of one type sharing a neighbor are peers (a
    /// redundant pair) and share a row in hierarchies.
    pub kind: String,
    /// Position of the device's layer in the layer list. In hierarchies,
    /// later layers go below earlier ones, never on the same row.
    pub layer: usize,
}

/// How far upstream a device type sits in a typical network.
pub fn kind_rank(kind: &str) -> u8 {
    match kind {
        "internet" | "cloud" | "aws" | "azure" | "google cloud" | "alibaba cloud" | "telekom cloud" | "stackit" | "cloud proxy"
        | "zscaler" | "aws transit gateway" => 0,
        "aws direct connect gateway" => 1,
        "firewall" => 1,
        "router" | "ospf router" | "bgp reflector" => 2,
        "load balancer" | "aci l3out" => 3,
        "switch" | "virtual switch" | "distributed virtual switch" => 4,
        "patch panel" => 5,
        "wlan controller" => 4,
        "access point" | "standalone ap" | "managed ap" => 5,
        "server" | "client" => 6,
        _ => 5,
    }
}

/// Top left corners for `nodes`, linked by `edges` (index pairs).
#[cfg(test)]
pub fn arrange(kind: Layout, nodes: &[Node], edges: &[(usize, usize)], gap: f64) -> Vec<Pt> {
    arrange_with_labels(kind, nodes, edges, &[], gap)
}

/// A link's label, for keeping room for it: its box and the free line
/// wanted on each side.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct LabelRoom {
    pub w: f64,
    pub h: f64,
    pub margin: f64,
}

impl LabelRoom {
    /// Room across x and y: the box plus the margin on both sides.
    fn total(&self) -> Pt {
        if self.w <= 0.0 && self.h <= 0.0 {
            return Pt::default();
        }
        Pt::new(self.w + 2.0 * self.margin, self.h + 2.0 * self.margin)
    }
}

/// [`arrange`] with the label of each edge (`rooms[i]` for `edges[i]`):
/// linked devices are kept far enough apart for it and its margin.
pub fn arrange_with_labels(kind: Layout, nodes: &[Node], edges: &[(usize, usize)], rooms: &[LabelRoom], gap: f64) -> Vec<Pt> {
    let g = Graph::new(nodes, edges, rooms);
    // Layouts that do not look at single links keep at least that much
    // room everywhere.
    let roomy = g.rooms.values().map(|r| r.total()).fold(gap, |m, r| m.max(r.x).max(r.y));
    let centers = match kind {
        Layout::Hierarchy => g.pack(|c| g.hierarchy(c, gap, false), gap, false),
        Layout::HierarchyLr => g.pack(|c| g.hierarchy(c, gap, true), gap, true),
        Layout::Radial => g.pack(|c| g.radial(c, roomy), gap, false),
        Layout::Circle => g.circle(roomy),
        Layout::Grid => g.grid(roomy),
        Layout::Force => g.pack(|c| g.force(c, gap), gap, false),
        // Where the devices are, so nothing is packed or moved as a whole.
        Layout::Tidy => g.tidy(gap),
    };
    centers.iter().zip(nodes).map(|(c, n)| Pt::new(c.x - n.w / 2.0, c.y - n.h / 2.0)).collect()
}

/// Depth below the BFS root, and the node it was reached from.
type Reached = (usize, Option<usize>);

struct Graph<'a> {
    nodes: &'a [Node],
    adj: Vec<Vec<usize>>,
    /// The labels on the links between two nodes (the largest of them),
    /// keyed by (smaller, larger) node index.
    rooms: std::collections::HashMap<(usize, usize), LabelRoom>,
}

impl<'a> Graph<'a> {
    fn new(nodes: &'a [Node], edges: &[(usize, usize)], rooms: &[LabelRoom]) -> Self {
        let mut adj = vec![vec![]; nodes.len()];
        let mut room_of: std::collections::HashMap<(usize, usize), LabelRoom> = std::collections::HashMap::new();
        for (e, &(a, b)) in edges.iter().enumerate() {
            if a == b || a >= nodes.len() || b >= nodes.len() {
                continue;
            }
            if !adj[a].contains(&b) {
                adj[a].push(b);
                adj[b].push(a);
            }
            if let Some(r) = rooms.get(e).filter(|r| r.w > 0.0 || r.h > 0.0) {
                let slot = room_of.entry((a.min(b), a.max(b))).or_default();
                *slot = LabelRoom { w: slot.w.max(r.w), h: slot.h.max(r.h), margin: slot.margin.max(r.margin) };
            }
        }
        Graph { nodes, adj, rooms: room_of }
    }

    /// Room for the labels on the links between `a` and `b` (zero if none),
    /// across x and y, margins included.
    fn room(&self, a: usize, b: usize) -> Pt {
        self.label(a, b).total()
    }

    fn label(&self, a: usize, b: usize) -> LabelRoom {
        self.rooms.get(&(a.min(b), a.max(b))).copied().unwrap_or_default()
    }

    fn diag(&self, i: usize) -> f64 {
        self.nodes[i].w.hypot(self.nodes[i].h)
    }

    /// Most upstream node of `members`: lowest rank, then most links.
    fn root(&self, members: &[usize]) -> usize {
        *members
            .iter()
            .min_by_key(|&&i| (self.nodes[i].layer, self.nodes[i].rank, usize::MAX - self.adj[i].len(), i))
            .unwrap()
    }

    /// Connected components, each in BFS order from its root.
    fn components(&self) -> Vec<Vec<usize>> {
        let mut seen = vec![false; self.nodes.len()];
        let mut out = vec![];
        for start in 0..self.nodes.len() {
            if seen[start] {
                continue;
            }
            // Collect the component, then walk it again from its root.
            let mut members = vec![];
            let mut queue = VecDeque::from([start]);
            seen[start] = true;
            while let Some(i) = queue.pop_front() {
                members.push(i);
                for &j in &self.adj[i] {
                    if !seen[j] {
                        seen[j] = true;
                        queue.push_back(j);
                    }
                }
            }
            let (order, _) = self.bfs(self.root(&members));
            out.push(order);
        }
        // Big components first, so packing reads naturally.
        out.sort_by_key(|c| std::cmp::Reverse(c.len()));
        out
    }

    /// BFS order from `root` and each reached node's (depth, parent).
    fn bfs(&self, root: usize) -> (Vec<usize>, Vec<Option<Reached>>) {
        let mut info = vec![None; self.nodes.len()];
        info[root] = Some((0, None));
        let mut order = vec![];
        let mut queue = VecDeque::from([root]);
        while let Some(i) = queue.pop_front() {
            order.push(i);
            let depth = info[i].unwrap().0;
            // Visit upstream neighbors first, for a stable, sensible order.
            let mut next: Vec<usize> = self.adj[i].iter().copied().filter(|j| info[*j].is_none()).collect();
            next.sort_by_key(|&j| (self.nodes[j].rank, j));
            for j in next {
                info[j] = Some((depth + 1, Some(i)));
                queue.push_back(j);
            }
        }
        (order, info)
    }

    /// Lay out each component with `place` (centers for its members) and set
    /// the components side by side (or one under another for `stack`),
    /// aligned at the top (or left).
    fn pack(&self, place: impl Fn(&[usize]) -> Vec<(usize, Pt)>, gap: f64, stack: bool) -> Vec<Pt> {
        let mut out = vec![Pt::default(); self.nodes.len()];
        let mut cursor = 0.0;
        for comp in self.components() {
            let placed = place(&comp);
            let (mut x0, mut y0, mut x1) = (f64::MAX, f64::MAX, f64::MIN);
            let mut y1 = f64::MIN;
            for &(i, c) in &placed {
                let n = &self.nodes[i];
                x0 = x0.min(c.x - n.w / 2.0);
                x1 = x1.max(c.x + n.w / 2.0);
                y0 = y0.min(c.y - n.h / 2.0);
                y1 = y1.max(c.y + n.h / 2.0);
            }
            let shift = if stack { Pt::new(-x0, cursor - y0) } else { Pt::new(cursor - x0, -y0) };
            for (i, c) in placed {
                out[i] = c.add(shift);
            }
            cursor += if stack { y1 - y0 } else { x1 - x0 } + 2.0 * gap;
        }
        out
    }

    /// Layered layout of one component: BFS depth from its root gives the
    /// layer, barycenter sweeps reduce crossings, then nodes slide toward
    /// their neighbors without overlapping.
    /// Peer groups: devices of the same type with a neighbor in common, like
    /// two firewalls of an HA pair on one switch. Returns a group id per node.
    fn peer_groups(&self, comp: &[usize]) -> Vec<usize> {
        let mut parent: Vec<usize> = (0..self.nodes.len()).collect();
        fn find(parent: &mut [usize], i: usize) -> usize {
            let mut r = i;
            while parent[r] != r {
                r = parent[r];
            }
            parent[i] = r;
            r
        }
        // Neighbors of `a` other than `b`.
        let others = |a: usize, b: usize| {
            let mut v: Vec<usize> = self.adj[a].iter().copied().filter(|&x| x != b).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        for &i in comp {
            let ns = &self.adj[i];
            for (a, &j) in ns.iter().enumerate() {
                for &k in &ns[a + 1..] {
                    let (nj, nk) = (&self.nodes[j], &self.nodes[k]);
                    if j == k || nj.kind != nk.kind || nj.layer != nk.layer {
                        continue;
                    }
                    // Two of a type on a shared neighbor are peers when that
                    // neighbor is of another type (an HA pair on its switch),
                    // or they are not linked to each other (two access
                    // switches on one core). Linked to each other, they are
                    // peers only with the same other neighbors (a core pair):
                    // a core switch and the access switch below it are not.
                    let peers = self.nodes[i].kind != nj.kind || !self.adj[j].contains(&k) || others(j, k) == others(k, j);
                    if peers {
                        let (rj, rk) = (find(&mut parent, j), find(&mut parent, k));
                        parent[rj] = rk;
                    }
                }
            }
        }
        (0..self.nodes.len()).map(|i| find(&mut parent, i)).collect()
    }

    /// Row of each node: BFS depth over peer groups from the root's group,
    /// so peers share a row and links never skip a row.
    fn group_depths(&self, comp: &[usize], group: &[usize]) -> Vec<usize> {
        let mut members: std::collections::HashMap<usize, Vec<usize>> = std::collections::HashMap::new();
        for &i in comp {
            members.entry(group[i]).or_default().push(i);
        }
        let mut gdepth: std::collections::HashMap<usize, usize> = std::collections::HashMap::new();
        // From the root, and from every other device as far upstream when
        // that is the internet, a cloud, a firewall or a router: two
        // internet uplinks each with their own router and firewall make two
        // columns side by side, not one long chain through both.
        let (root_layer, root_rank) = (self.nodes[comp[0]].layer, self.nodes[comp[0]].rank);
        let mut queue = VecDeque::new();
        for &i in comp {
            let top = i == comp[0] || (root_rank <= 2 && self.nodes[i].layer == root_layer && self.nodes[i].rank == root_rank);
            if top && !gdepth.contains_key(&group[i]) {
                gdepth.insert(group[i], 0);
                queue.push_back(group[i]);
            }
        }
        while let Some(g) = queue.pop_front() {
            let d = gdepth[&g];
            for &m in &members[&g] {
                for &j in &self.adj[m] {
                    if let std::collections::hash_map::Entry::Vacant(e) = gdepth.entry(group[j]) {
                        e.insert(d + 1);
                        queue.push_back(group[j]);
                    }
                }
            }
        }
        // Rows: by layer first (later layers below), then by depth.
        let key = |i: usize| (self.nodes[i].layer, gdepth[&group[i]]);
        let mut keys: Vec<(usize, usize)> = comp.iter().map(|&i| key(i)).collect();
        keys.sort_unstable();
        keys.dedup();
        let mut depth = vec![0; self.nodes.len()];
        for &i in comp {
            depth[i] = keys.binary_search(&key(i)).unwrap();
        }
        depth
    }

    fn hierarchy(&self, comp: &[usize], gap: f64, horizontal: bool) -> Vec<(usize, Pt)> {
        let group = self.peer_groups(comp);
        let depths = self.group_depths(comp, &group);
        let depth = |i: usize| depths[i];
        let layers_n = comp.iter().map(|&i| depth(i)).max().unwrap_or(0) + 1;
        let mut layers: Vec<Vec<usize>> = vec![vec![]; layers_n];
        for &i in comp {
            layers[depth(i)].push(i);
        }
        // Extent along the layer, and across it.
        let along = |i: usize| if horizontal { self.nodes[i].h } else { self.nodes[i].w };
        let across = |i: usize| if horizontal { self.nodes[i].w } else { self.nodes[i].h };

        // Crossing reduction: order each layer by the mean index of its
        // neighbors in the layer before (down sweep) or after (up sweep).
        let mut index = vec![0.0; self.nodes.len()];
        let reindex = |layers: &Vec<Vec<usize>>, index: &mut Vec<f64>| {
            for layer in layers {
                for (k, &i) in layer.iter().enumerate() {
                    index[i] = k as f64;
                }
            }
        };
        reindex(&layers, &mut index);
        for sweep in 0..8 {
            let down = sweep % 2 == 0;
            let range: Vec<usize> = if down { (1..layers_n).collect() } else { (0..layers_n.saturating_sub(1)).rev().collect() };
            for l in range {
                let other = if down { l - 1 } else { l + 1 };
                let keyed: Vec<(f64, usize)> = layers[l]
                    .iter()
                    .map(|&i| {
                        let ns: Vec<f64> = self.adj[i].iter().filter(|&&j| depth(j) == other).map(|&j| index[j]).collect();
                        let key = if ns.is_empty() { index[i] } else { ns.iter().sum::<f64>() / ns.len() as f64 };
                        (key, i)
                    })
                    .collect();
                // Peers move as a block, at their mean position, so a
                // redundant pair stays side by side.
                let group_key = |g: usize| {
                    let ks: Vec<f64> = keyed.iter().filter(|(_, j)| group[*j] == g).map(|(k, _)| *k).collect();
                    ks.iter().sum::<f64>() / ks.len() as f64
                };
                let gk: Vec<f64> = keyed.iter().map(|(_, i)| group_key(group[*i])).collect();
                let mut order: Vec<usize> = (0..keyed.len()).collect();
                order.sort_by(|&a, &b| {
                    gk[a]
                        .total_cmp(&gk[b])
                        .then(group[keyed[a].1].cmp(&group[keyed[b].1]))
                        .then(keyed[a].0.total_cmp(&keyed[b].0))
                        .then(index[keyed[a].1].total_cmp(&index[keyed[b].1]))
                });
                layers[l] = order.into_iter().map(|k| keyed[k].1).collect();
                reindex(&layers, &mut index);
            }
        }

        // Positions along each layer: packed first, then pulled toward the
        // mean of linked nodes in neighboring layers, keeping order and gaps.
        // Space between neighbors in a layer: the gap, or more when a
        // labeled link runs between them.
        let space = |a: usize, b: usize| {
            let r = self.room(a, b);
            gap.max(if horizontal { r.y } else { r.x })
        };
        let mut pos = vec![0.0; self.nodes.len()];
        for layer in &layers {
            let mut c = 0.0;
            for (k, &i) in layer.iter().enumerate() {
                if k > 0 {
                    c += (along(layer[k - 1]) + along(i)) / 2.0 + space(layer[k - 1], i);
                }
                pos[i] = c;
            }
            let mid = c / 2.0;
            for &i in layer {
                pos[i] -= mid;
            }
        }
        for sweep in 0..12 {
            let down = sweep % 2 == 0;
            let order: Vec<usize> = if down { (0..layers_n).collect() } else { (0..layers_n).rev().collect() };
            for l in order {
                let layer = &layers[l];
                let want: Vec<f64> = layer
                    .iter()
                    .map(|&i| {
                        let ns: Vec<f64> =
                            self.adj[i].iter().filter(|&&j| depth(j) != l).map(|&j| pos[j]).collect();
                        if ns.is_empty() { pos[i] } else { ns.iter().sum::<f64>() / ns.len() as f64 }
                    })
                    .collect();
                // Keep the gaps, then shift the layer back so on average
                // every node is where it wanted to be.
                let mut placed = want.clone();
                for k in 1..layer.len() {
                    let min = placed[k - 1] + (along(layer[k - 1]) + along(layer[k])) / 2.0 + space(layer[k - 1], layer[k]);
                    placed[k] = placed[k].max(min);
                }
                let drift = want.iter().zip(&placed).map(|(w, p)| w - p).sum::<f64>() / layer.len() as f64;
                for (k, &i) in layer.iter().enumerate() {
                    pos[i] = placed[k] + drift;
                }
            }
        }

        // Layers one after another, with room between them for the links
        // (and for the labels on the links to the next layer).
        let mut out = vec![];
        let mut offset = 0.0;
        for (l, layer) in layers.iter().enumerate() {
            let thick = layer.iter().map(|&i| across(i)).fold(0.0, f64::max);
            for &i in layer {
                let c = offset + thick / 2.0;
                out.push((i, if horizontal { Pt::new(c, pos[i]) } else { Pt::new(pos[i], c) }));
            }
            // Labels on the links to the next layer sit in the gap: where
            // they would overlap side by side they stack, so the gap holds
            // as many as overlap at once, plus the margin.
            let mut spans: Vec<(f64, f64, f64, f64)> = vec![];
            for &i in layer {
                for &j in self.adj[i].iter().filter(|&&j| depth(j) == l + 1) {
                    let r = self.label(i, j);
                    if r.w <= 0.0 && r.h <= 0.0 {
                        continue;
                    }
                    let (len, thick) = if horizontal { (r.h, r.w) } else { (r.w, r.h) };
                    let mid = (pos[i] + pos[j]) / 2.0;
                    spans.push((mid - len / 2.0, mid + len / 2.0, thick, r.margin));
                }
            }
            let mut labels: f64 = 0.0;
            for &(a0, a1, _, _) in &spans {
                let at_once: Vec<&(f64, f64, f64, f64)> = spans.iter().filter(|s| s.0 <= a1 && a0 <= s.1).collect();
                let stack: f64 = at_once.iter().map(|s| s.2 + 4.0).sum::<f64>() - 4.0;
                let margin = at_once.iter().map(|s| s.3).fold(0.0, f64::max);
                labels = labels.max(stack + 2.0 * margin);
            }
            offset += thick + (1.5 * gap).max(labels);
        }
        out
    }

    /// Rings around the root; every subtree gets a wedge sized by its
    /// number of leaves, so branches stay together.
    fn radial(&self, comp: &[usize], gap: f64) -> Vec<(usize, Pt)> {
        let root = comp[0];
        let (order, info) = self.bfs(root);
        let n = self.nodes.len();
        let mut children = vec![vec![]; n];
        for &i in &order {
            if let Some((_, Some(p))) = info[i] {
                children[p].push(i);
            }
        }
        let mut leaves = vec![1.0; n];
        for &i in order.iter().rev() {
            if !children[i].is_empty() {
                leaves[i] = children[i].iter().map(|c| leaves[*c]).sum();
            }
        }
        // Ring radii: far enough apart, and wide enough for their nodes.
        let max_depth = order.iter().map(|&i| info[i].unwrap().0).max().unwrap_or(0);
        let step = comp.iter().map(|&i| self.diag(i)).fold(0.0, f64::max) + gap;
        let mut radius = vec![0.0; max_depth + 1];
        for d in 1..=max_depth {
            let ring: f64 = order.iter().filter(|&&i| info[i].unwrap().0 == d).map(|&i| self.diag(i) + gap).sum();
            radius[d] = (radius[d - 1] + step).max(ring / TAU);
        }
        let mut wedge = vec![(0.0, TAU); n];
        let mut out = vec![(root, Pt::default())];
        for &i in &order {
            let (mut start, mut span) = wedge[i];
            // Below the root, children stay within the angle where their
            // ring is still outward of this node (else a chain's leaves
            // would wrap back around the hub).
            let d = info[i].unwrap().0;
            if d > 0 && d < max_depth {
                let cap = 2.0 * (radius[d] / radius[d + 1]).clamp(-1.0, 1.0).acos();
                if span > cap {
                    start += (span - cap) / 2.0;
                    span = cap;
                }
            }
            let mut a = start;
            for &c in &children[i] {
                let share = span * leaves[c] / leaves[i];
                wedge[c] = (a, share);
                let angle = a + share / 2.0 - TAU / 4.0;
                let r = radius[info[c].unwrap().0];
                out.push((c, Pt::new(r * angle.cos(), r * angle.sin())));
                a += share;
            }
        }
        out
    }

    /// Every node on one circle, neighbors next to each other.
    fn circle(&self, gap: f64) -> Vec<Pt> {
        let order: Vec<usize> = self.components().concat();
        let n = order.len();
        let mut out = vec![Pt::default(); self.nodes.len()];
        if n == 1 {
            return out;
        }
        let perimeter: f64 = order.iter().map(|&i| self.diag(i) + gap).sum();
        let r = (perimeter / TAU).max(order.iter().map(|&i| self.diag(i)).fold(0.0, f64::max));
        for (k, &i) in order.iter().enumerate() {
            let a = TAU * k as f64 / n as f64 - TAU / 4.0;
            out[i] = Pt::new(r * a.cos(), r * a.sin());
        }
        out
    }

    /// Rows of equal cells, neighbors close together.
    fn grid(&self, gap: f64) -> Vec<Pt> {
        let order: Vec<usize> = self.components().concat();
        let cols = (order.len() as f64).sqrt().ceil().max(1.0) as usize;
        let cw = self.nodes.iter().map(|n| n.w).fold(0.0, f64::max) + gap;
        let ch = self.nodes.iter().map(|n| n.h).fold(0.0, f64::max) + gap;
        let mut out = vec![Pt::default(); self.nodes.len()];
        for (k, &i) in order.iter().enumerate() {
            out[i] = Pt::new((k % cols) as f64 * cw, (k / cols) as f64 * ch);
        }
        out
    }

    /// Fruchterman-Reingold on one component, from the current positions
    /// (so running it again refines), then overlaps pushed apart.
    fn force(&self, comp: &[usize], gap: f64) -> Vec<(usize, Pt)> {
        let n = comp.len();
        let mut p: Vec<Pt> =
            comp.iter().map(|&i| Pt::new(self.nodes[i].pos.x + self.nodes[i].w / 2.0, self.nodes[i].pos.y + self.nodes[i].h / 2.0)).collect();
        // Start from a circle if positions say nothing (all in one spot).
        let spread = p.iter().map(|q| q.dist(p[0])).fold(0.0, f64::max);
        if spread < 1.0 && n > 1 {
            let r = comp.iter().map(|&i| self.diag(i) + gap).sum::<f64>() / TAU;
            for (k, q) in p.iter_mut().enumerate() {
                let a = TAU * k as f64 / n as f64;
                *q = Pt::new(r * a.cos(), r * a.sin());
            }
        }
        // Index within the component, for the edges.
        let local = |i: usize| comp.iter().position(|&c| c == i);
        let k = comp.iter().map(|&i| self.diag(i)).sum::<f64>() / n.max(1) as f64 + gap;
        let mut temp = k * 2.0;
        for _ in 0..300 {
            let mut disp = vec![Pt::default(); n];
            for i in 0..n {
                for j in i + 1..n {
                    let mut d = p[i].sub(p[j]);
                    if d.len() < 0.01 {
                        // Same spot: nudge apart deterministically.
                        d = Pt::new((i as f64 - j as f64) * 0.1, 0.1);
                    }
                    let f = d.norm().scale(k * k / d.len());
                    disp[i] = disp[i].add(f);
                    disp[j] = disp[j].sub(f);
                }
            }
            for i in 0..n {
                for j in self.adj[comp[i]].iter().filter_map(|&j| local(j)) {
                    if j > i {
                        let d = p[i].sub(p[j]);
                        let f = d.norm().scale(d.len() * d.len() / k);
                        disp[i] = disp[i].sub(f);
                        disp[j] = disp[j].add(f);
                    }
                }
            }
            // A little gravity keeps separate groups from drifting off.
            let center = p.iter().fold(Pt::default(), |a, q| a.add(*q)).scale(1.0 / n as f64);
            for i in 0..n {
                let d = disp[i].add(center.sub(p[i]).scale(0.05));
                p[i] = p[i].add(d.norm().scale(d.len().min(temp)));
            }
            temp = (temp * 0.98).max(k * 0.01);
        }
        self.stretch(comp, &mut p);
        self.separate(comp, &mut p, gap / 2.0);
        comp.iter().copied().zip(p).collect()
    }

    /// Tidy up the current arrangement (returns centers, like the others):
    /// devices at nearly one height share a row; a device and its one linked
    /// device in the next row (each other's nearest) line up in a column; a
    /// device fanning out to several that hang on it alone is centered over
    /// (or under) them; overlaps are pushed apart evenly with room for the
    /// labels in between. The whole stays centered where it was, so tidying
    /// again changes nothing.
    fn tidy(&self, gap: f64) -> Vec<Pt> {
        let n = self.nodes.len();
        if n == 0 {
            return vec![];
        }
        let orig: Vec<Pt> = self.nodes.iter().map(|d| Pt::new(d.pos.x + d.w / 2.0, d.pos.y + d.h / 2.0)).collect();
        let mut c = orig.clone();
        let row_tol = self.nodes.iter().map(|d| d.h).sum::<f64>() / n as f64 * 0.75;
        // Rows: nodes whose y is within `row_tol` of their row's mean.
        let mut order: Vec<usize> = (0..n).collect();
        order.sort_by(|&a, &b| c[a].y.total_cmp(&c[b].y));
        let mut rows: Vec<Vec<usize>> = vec![];
        for i in order {
            match rows.last_mut() {
                Some(g) if c[i].y - g.iter().map(|&j| c[j].y).sum::<f64>() / g.len() as f64 <= row_tol => g.push(i),
                _ => rows.push(vec![i]),
            }
        }
        let mut row_of = vec![0; n];
        for (k, g) in rows.iter().enumerate() {
            let y = g.iter().map(|&i| c[i].y).sum::<f64>() / g.len() as f64;
            for &i in g {
                c[i].y = y;
                row_of[i] = k;
            }
        }
        let next = |i: usize, dk: isize| -> Vec<usize> {
            let mut v: Vec<usize> =
                self.adj[i].iter().copied().filter(|&j| row_of[j] as isize == row_of[i] as isize + dk).collect();
            v.sort_unstable();
            v.dedup();
            v
        };
        // Fans: linked to several in the next row that hang on this one alone.
        let fan = |i: usize, dk: isize| -> Option<Vec<usize>> {
            let kids = next(i, dk);
            (kids.len() >= 2 && kids.iter().all(|&j| next(j, -dk) == [i])).then_some(kids)
        };
        let fans: Vec<(usize, Vec<usize>)> =
            (0..n).filter_map(|i| fan(i, 1).or_else(|| fan(i, -1)).map(|kids| (i, kids))).collect();
        let is_fan = |i: usize| fans.iter().any(|(f, _)| *f == i);

        // Columns: pairs in neighboring rows, linked and each other's nearest
        // (fans excepted), without crossing other pairs.
        let mut block: Vec<usize> = (0..n).collect();
        fn root(b: &mut [usize], mut i: usize) -> usize {
            while b[i] != i {
                b[i] = b[b[i]];
                i = b[i];
            }
            i
        }
        let nearest = |i: usize, dk: isize| {
            next(i, dk).into_iter().filter(|&j| !is_fan(j)).min_by(|&a, &b| {
                (c[a].x - c[i].x).abs().total_cmp(&(c[b].x - c[i].x).abs()).then(a.cmp(&b))
            })
        };
        for row in rows.iter().skip(1) {
            let mut pairs: Vec<(usize, usize)> = row
                .iter()
                .filter(|&&v| !is_fan(v))
                .filter_map(|&v| nearest(v, -1).filter(|&u| nearest(u, 1) == Some(v)).map(|u| (u, v)))
                .collect();
            pairs.sort_by(|a, b| (c[a.0].x - c[a.1].x).abs().total_cmp(&(c[b.0].x - c[b.1].x).abs()));
            let mut kept: Vec<(usize, usize)> = vec![];
            for (u, v) in pairs {
                if kept.iter().all(|&(a, b)| (c[a].x < c[u].x) == (c[b].x < c[v].x)) {
                    kept.push((u, v));
                    let (ru, rv) = (root(&mut block, u), root(&mut block, v));
                    block[rv] = ru;
                }
            }
        }
        let block: Vec<usize> = (0..n).map(|i| root(&mut block, i)).collect();
        // A column stands at its members' mean.
        let mut bx: Vec<f64> = vec![0.0; n];
        for (b, x) in bx.iter_mut().enumerate() {
            let m: Vec<usize> = (0..n).filter(|&i| block[i] == b).collect();
            if !m.is_empty() {
                *x = m.iter().map(|&i| orig[i].x).sum::<f64>() / m.len() as f64;
            }
        }

        let min_gap = (gap * 0.5).max(20.0);
        let x_of = |bx: &[f64], i: usize| bx[block[i]];
        for round in 0..4 {
            // Fans centered, the deepest first so nested ones follow.
            if round < 3 {
                let mut fans = fans.clone();
                fans.sort_by_key(|(f, _)| std::cmp::Reverse(row_of[*f]));
                for (f, kids) in &fans {
                    bx[block[*f]] = kids.iter().map(|&j| x_of(&bx, j)).sum::<f64>() / kids.len() as f64;
                }
            }
            // Overlaps in a row: both sides give way evenly.
            for _ in 0..200 {
                let mut moved = false;
                for g in &rows {
                    let mut g = g.clone();
                    g.sort_by(|&a, &b| x_of(&bx, a).total_cmp(&x_of(&bx, b)).then(orig[a].x.total_cmp(&orig[b].x)));
                    for k in 1..g.len() {
                        let (a, b) = (g[k - 1], g[k]);
                        let need = (self.nodes[a].w + self.nodes[b].w) / 2.0 + min_gap.max(self.room(a, b).x);
                        let short = need - (x_of(&bx, b) - x_of(&bx, a));
                        if short > 0.01 {
                            bx[block[a]] -= short / 2.0;
                            bx[block[b]] += short / 2.0;
                            moved = true;
                        }
                    }
                }
                if !moved {
                    break;
                }
            }
        }
        for (i, q) in c.iter_mut().enumerate() {
            q.x = x_of(&bx, i);
        }
        // Whatever is still too close (columns pulling against each other)
        // is pushed apart along its row.
        for g in &rows {
            let mut g = g.clone();
            g.sort_by(|&a, &b| c[a].x.total_cmp(&c[b].x));
            for k in 1..g.len() {
                let (a, b) = (g[k - 1], g[k]);
                let need = c[a].x + (self.nodes[a].w + self.nodes[b].w) / 2.0 + min_gap.max(self.room(a, b).x);
                c[b].x = c[b].x.max(need);
            }
        }
        // Rows apart: each starts below the one above, with room for the
        // labels on the links between them.
        let mut shift = 0.0;
        for k in 0..rows.len() {
            rows[k].iter().for_each(|&i| c[i].y += shift);
            if k + 1 < rows.len() {
                let bottom = rows[k].iter().map(|&i| c[i].y + self.nodes[i].h / 2.0).fold(f64::MIN, f64::max);
                let top = rows[k + 1].iter().map(|&i| c[i].y + shift - self.nodes[i].h / 2.0).fold(f64::MAX, f64::min);
                let labels = rows[k]
                    .iter()
                    .flat_map(|&i| self.adj[i].iter().filter(|&&j| row_of[j] == k + 1).map(move |&j| (i, j)))
                    .map(|(i, j)| self.room(i, j).y)
                    .fold(0.0, f64::max);
                let need = bottom + min_gap.max(labels);
                if top < need {
                    shift += need - top;
                }
            }
        }
        // Where it was: on average, nothing moved.
        let d = orig.iter().zip(&c).fold(Pt::default(), |s, (o, q)| s.add(o.sub(*q))).scale(1.0 / n as f64);
        c.iter_mut().for_each(|q| *q = q.add(d));
        c
    }

    /// Move linked nodes apart along their link until the stretch of line
    /// between their boxes holds the link's label with its margin.
    fn stretch(&self, comp: &[usize], p: &mut [Pt]) {
        if self.rooms.is_empty() {
            return;
        }
        // From the center to where a ray in direction `u` leaves the box.
        let exit = |n: &Node, u: Pt| (n.w / 2.0 / u.x.abs().max(1e-9)).min(n.h / 2.0 / u.y.abs().max(1e-9));
        for _ in 0..50 {
            let mut moved = false;
            for i in 0..comp.len() {
                for j in i + 1..comp.len() {
                    let r = self.room(comp[i], comp[j]);
                    if r.x <= 0.0 && r.y <= 0.0 {
                        continue;
                    }
                    let d = p[j].sub(p[i]);
                    let u = d.norm();
                    let free = d.len() - exit(&self.nodes[comp[i]], u) - exit(&self.nodes[comp[j]], u);
                    // The label box's length along the line.
                    let need = r.x * u.x.abs() + r.y * u.y.abs();
                    if free + 0.5 < need {
                        let push = u.scale((need - free) / 2.0);
                        p[i] = p[i].sub(push);
                        p[j] = p[j].add(push);
                        moved = true;
                    }
                }
            }
            if !moved {
                break;
            }
        }
    }

    /// Push overlapping boxes (with `margin` around them) apart along the
    /// axis that needs the smaller move.
    fn separate(&self, comp: &[usize], p: &mut [Pt], margin: f64) {
        for _ in 0..100 {
            let mut moved = false;
            for i in 0..p.len() {
                for j in i + 1..p.len() {
                    let (a, b) = (&self.nodes[comp[i]], &self.nodes[comp[j]]);
                    let d = p[j].sub(p[i]);
                    let ox = (a.w + b.w) / 2.0 + margin - d.x.abs();
                    let oy = (a.h + b.h) / 2.0 + margin - d.y.abs();
                    if ox <= 0.0 || oy <= 0.0 {
                        continue;
                    }
                    moved = true;
                    let push = if ox < oy {
                        Pt::new(ox / 2.0 * if d.x >= 0.0 { 1.0 } else { -1.0 }, 0.0)
                    } else {
                        Pt::new(0.0, oy / 2.0 * if d.y >= 0.0 { 1.0 } else { -1.0 })
                    };
                    p[i] = p[i].sub(push);
                    p[j] = p[j].add(push);
                }
            }
            if !moved {
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn boxes(ranks: &[u8]) -> Vec<Node> {
        ranks.iter().map(|&rank| Node { w: 120.0, h: 60.0, pos: Pt::default(), rank, kind: format!("kind{rank}"), layer: 0 }).collect()
    }

    fn overlap(a: Pt, b: Pt) -> bool {
        (a.x - b.x).abs() < 120.0 && (a.y - b.y).abs() < 60.0
    }

    fn no_overlaps(p: &[Pt]) {
        for i in 0..p.len() {
            for j in i + 1..p.len() {
                assert!(!overlap(p[i], p[j]), "{i} and {j} overlap: {:?} {:?}", p[i], p[j]);
            }
        }
    }

    /// internet - fw - core switch - three servers, plus a lone box.
    fn network() -> (Vec<Node>, Vec<(usize, usize)>) {
        let nodes = boxes(&[0, 1, 4, 6, 6, 6, 5]);
        (nodes, vec![(3, 2), (1, 0), (2, 1), (2, 4), (5, 2)])
    }

    #[test]
    fn hierarchy_puts_upstream_devices_on_top() {
        let (nodes, edges) = network();
        let p = arrange(Layout::Hierarchy, &nodes, &edges, 60.0);
        no_overlaps(&p);
        assert!(p[0].y < p[1].y && p[1].y < p[2].y && p[2].y < p[3].y);
        assert_eq!(p[3].y, p[4].y);
        assert_eq!(p[4].y, p[5].y);
        // The single chain is straight above the middle server.
        assert!((p[0].x - p[4].x).abs() < 1.0 && (p[2].x - p[4].x).abs() < 1.0, "{p:?}");
        // The lone box sits beside the component, at the top.
        assert!(p[6].x > p.iter().take(6).map(|q| q.x).fold(f64::MIN, f64::max));
        assert_eq!(p[6].y, p[0].y);
        let lr = arrange(Layout::HierarchyLr, &nodes, &edges, 60.0);
        no_overlaps(&lr);
        assert!(lr[0].x < lr[1].x && lr[1].x < lr[2].x && lr[2].x < lr[3].x);
    }

    #[test]
    fn redundant_pairs_share_a_row_with_the_shared_device_centered_below() {
        // internet; fw-a and fw-b (HA link between them) both on it and on
        // one switch; two servers on the switch.
        let mut nodes = boxes(&[0, 1, 1, 4, 6, 6]);
        let edges = vec![(0, 1), (0, 2), (1, 2), (1, 3), (2, 3), (3, 4), (3, 5)];
        let check = |p: &[Pt]| {
            no_overlaps(p);
            assert_eq!(p[1].y, p[2].y, "firewalls side by side: {p:?}");
            assert!((p[1].x - p[2].x).abs() <= 120.0 + 60.0 + 0.01, "next to each other");
            assert!(p[0].y < p[1].y && p[1].y < p[3].y && p[3].y < p[4].y);
            let mid = (p[1].x + p[2].x) / 2.0;
            assert!((p[3].x - mid).abs() < 1.0, "switch centered under the pair: {p:?}");
            assert!((p[0].x - mid).abs() < 1.0, "internet centered above it");
        };
        check(&arrange(Layout::Hierarchy, &nodes, &edges, 60.0));
        // Only one firewall on the internet: the shared switch still makes
        // them peers.
        let edges = vec![(0, 1), (1, 2), (1, 3), (2, 3), (3, 4), (3, 5)];
        let p = arrange(Layout::Hierarchy, &nodes, &edges, 60.0);
        no_overlaps(&p);
        assert_eq!(p[1].y, p[2].y);
        assert!(p[2].y < p[3].y);
        // Left to right: same column.
        let lr = arrange(Layout::HierarchyLr, &nodes, &edges, 60.0);
        assert_eq!(lr[1].x, lr[2].x);
        // Same type in a chain (core switch -> access switch) is not a pair.
        nodes[2].kind = "kind4".into();
        let chain = vec![(0, 1), (1, 3), (3, 2), (2, 4), (2, 5)];
        let p = arrange(Layout::Hierarchy, &nodes, &chain, 60.0);
        assert!(p[3].y < p[2].y, "access switch below core: {p:?}");
    }

    #[test]
    fn switch_tiers_get_their_own_rows() {
        // A firewall on two core switches (linked to each other), each core
        // switch on both access switches.
        let nodes = boxes(&[1, 4, 4, 4, 4]);
        let edges = vec![(0, 1), (0, 2), (1, 2), (1, 3), (1, 4), (2, 3), (2, 4)];
        let p = arrange(Layout::Hierarchy, &nodes, &edges, 60.0);
        no_overlaps(&p);
        assert_eq!(p[1].y, p[2].y, "core pair side by side: {p:?}");
        assert_eq!(p[3].y, p[4].y, "access pair side by side");
        assert!(p[0].y < p[1].y && p[1].y < p[3].y, "access below core: {p:?}");
        // The same without the firewall: the core pair still goes on top.
        let nodes = boxes(&[4, 4, 4, 4]);
        let edges = vec![(0, 1), (0, 2), (0, 3), (1, 2), (1, 3)];
        let p = arrange(Layout::Hierarchy, &nodes, &edges, 60.0);
        assert_eq!(p[0].y, p[1].y);
        assert_eq!(p[2].y, p[3].y);
        assert!(p[0].y < p[2].y, "{p:?}");
    }

    #[test]
    fn later_layers_go_below_earlier_ones() {
        // Four switches in a ring: on one layer they could share a row; the
        // two on the second layer go below the two on the first.
        let mut nodes = boxes(&[4, 4, 4, 4]);
        nodes[2].layer = 1;
        nodes[3].layer = 1;
        let edges = vec![(0, 2), (0, 3), (1, 2), (1, 3)];
        let p = arrange(Layout::Hierarchy, &nodes, &edges, 60.0);
        no_overlaps(&p);
        assert_eq!(p[0].y, p[1].y);
        assert_eq!(p[2].y, p[3].y);
        assert!(p[0].y < p[2].y, "{p:?}");
        // Even when the second layer holds the most upstream device.
        let mut nodes = boxes(&[4, 0]);
        nodes[1].layer = 1;
        let p = arrange(Layout::Hierarchy, &nodes, &[(0, 1)], 60.0);
        assert!(p[0].y < p[1].y, "{p:?}");
    }

    #[test]
    fn labels_get_room_with_their_margin() {
        // Router on top of a switch, a labeled link between them, and a
        // second switch beside the first one, linked to it with a label.
        let nodes = boxes(&[2, 4, 6]);
        let edges = vec![(0, 1), (1, 2)];
        let plain = arrange(Layout::Hierarchy, &nodes, &edges, 40.0);
        // 17 high label + 2 x 50 margin between the rows; 200 wide (with
        // margins) between the switch and the server.
        let rooms = vec![LabelRoom { w: 30.0, h: 17.0, margin: 50.0 }, LabelRoom { w: 150.0, h: 0.0, margin: 25.0 }];
        let p = arrange_with_labels(Layout::Hierarchy, &nodes, &edges, &rooms, 40.0);
        assert!(p[1].y - (p[0].y + 60.0) >= 117.0 - 0.01, "rows apart: {p:?}");
        assert!(p[1].y - p[0].y > plain[1].y - plain[0].y);
        // Left to right, the width counts.
        let lr = arrange_with_labels(Layout::HierarchyLr, &nodes, &edges, &rooms, 40.0);
        assert!(lr[2].x - (lr[1].x + 120.0) >= 200.0 - 0.01, "{lr:?}");
        // Force-directed: the free line between linked boxes holds the label.
        let f = arrange_with_labels(Layout::Force, &nodes, &edges, &rooms, 40.0);
        let c = |i: usize| Pt::new(f[i].x + 60.0, f[i].y + 30.0);
        let u = c(2).sub(c(1)).norm();
        let exit = (60.0 / u.x.abs().max(1e-9)).min(30.0 / u.y.abs().max(1e-9));
        let free = c(1).dist(c(2)) - 2.0 * exit;
        assert!(free + 1.0 >= 200.0 * u.x.abs() + 50.0 * u.y.abs(), "free {free}: {f:?}");
    }

    #[test]
    fn tidy_straightens_without_moving_much() {
        // A router roughly above two switches, and a server roughly under
        // each switch; all a little off.
        let mut nodes = boxes(&[2, 4, 4, 6, 6, 6]);
        let at = [(205.0, 12.0), (52.0, 160.0), (371.0, 148.0), (47.0, 309.0), (379.0, 296.0), (402.0, 300.0)];
        for (n, (x, y)) in nodes.iter_mut().zip(at) {
            n.pos = Pt::new(x, y);
        }
        let edges = vec![(0, 1), (0, 2), (1, 3), (2, 4), (2, 5)];
        let p = arrange(Layout::Tidy, &nodes, &edges, 60.0);
        no_overlaps(&p);
        // Rows and columns line up.
        assert_eq!(p[1].y, p[2].y);
        assert!(p[3].y == p[4].y && p[4].y == p[5].y);
        assert_eq!(p[1].x, p[3].x, "server under its switch");
        // The router is centered over its two switches.
        assert!(((p[0].x + 60.0) - (p[1].x + p[2].x + 120.0) / 2.0).abs() < 1e-9, "{p:?}");
        // Nothing went far, except the two servers that sat on each other.
        for i in 0..4 {
            assert!(p[i].dist(nodes[i].pos) < 60.0, "{i} moved from {:?} to {:?}", nodes[i].pos, p[i]);
        }
        assert!(p[5].x >= p[4].x + 120.0 + 30.0, "pushed off its neighbor: {p:?}");
        assert!((p[2].x - (p[4].x + p[5].x) / 2.0).abs() < 1e-9, "switch centered over its servers: {p:?}");
        // Tidy again: nothing moves.
        let mut again = nodes.clone();
        again.iter_mut().zip(&p).for_each(|(n, q)| n.pos = *q);
        let q = arrange(Layout::Tidy, &again, &edges, 60.0);
        assert!(p.iter().zip(&q).all(|(a, b)| a.dist(*b) < 1e-6), "{p:?}\n{q:?}");
    }

    #[test]
    fn tidy_lines_up_redundant_pairs_and_stays_put() {
        // internet over two routers, each over a firewall over a switch;
        // both switches linked to both servers (crosswise). Switches are
        // off to the right of their firewalls, as drawn by hand.
        let mut nodes = boxes(&[0, 2, 2, 3, 3, 4, 4, 6, 6]);
        let at = [(140.0, 0.0), (40.0, 200.0), (265.0, 200.0), (40.0, 380.0), (265.0, 380.0), (145.0, 540.0), (370.0, 540.0), (145.0, 780.0), (370.0, 780.0)];
        for (n, (x, y)) in nodes.iter_mut().zip(at) {
            n.pos = Pt::new(x, y);
        }
        let edges = vec![(0, 1), (0, 2), (1, 2), (1, 3), (2, 4), (3, 4), (3, 5), (4, 6), (5, 7), (5, 8), (6, 7), (6, 8)];
        let mut p = arrange(Layout::Tidy, &nodes, &edges, 60.0);
        no_overlaps(&p);
        // Each side is one straight column; the internet over the middle.
        for (a, b) in [(1, 3), (3, 5), (5, 7), (2, 4), (4, 6), (6, 8)] {
            assert!((p[a].x - p[b].x).abs() < 1e-9, "{a} over {b}: {p:?}");
        }
        assert!((p[0].x - (p[1].x + p[2].x) / 2.0).abs() < 1e-9, "{p:?}");
        // Tidying again and again neither moves nor spreads anything.
        for _ in 0..5 {
            let mut again = nodes.clone();
            again.iter_mut().zip(&p).for_each(|(n, q)| n.pos = *q);
            let q = arrange(Layout::Tidy, &again, &edges, 60.0);
            assert!(p.iter().zip(&q).all(|(a, b)| a.dist(*b) < 1e-6), "{p:?}\n{q:?}");
            p = q;
        }
        for i in 0..nodes.len() {
            assert!(p[i].dist(nodes[i].pos) < 80.0, "{i} moved from {:?} to {:?}", nodes[i].pos, p[i]);
        }
    }

    #[test]
    fn hierarchy_untangles_crossings() {
        // Two parents whose children were listed crosswise.
        let nodes = boxes(&[0, 2, 2, 6, 6, 6, 6]);
        let edges = vec![(0, 1), (0, 2), (1, 5), (1, 6), (2, 3), (2, 4)];
        let p = arrange(Layout::Hierarchy, &nodes, &edges, 40.0);
        let left_parent = if p[1].x < p[2].x { 1 } else { 2 };
        let kids: [usize; 2] = if left_parent == 1 { [5, 6] } else { [3, 4] };
        let others: [usize; 2] = if left_parent == 1 { [3, 4] } else { [5, 6] };
        assert!(kids.iter().all(|&k| others.iter().all(|&o| p[k].x < p[o].x)), "{p:?}");
    }

    #[test]
    fn radial_circle_grid_and_force_do_not_overlap() {
        let (nodes, edges) = network();
        for kind in [Layout::Radial, Layout::Circle, Layout::Grid, Layout::Force] {
            let p = arrange(kind, &nodes, &edges, 40.0);
            assert_eq!(p.len(), nodes.len());
            assert!(p.iter().all(|q| q.x.is_finite() && q.y.is_finite()), "{kind:?}");
            no_overlaps(&p);
        }
        // Radial: the internet box is the hub, the switch's servers are on
        // the outermost ring, all equally far out.
        let p = arrange(Layout::Radial, &nodes, &edges, 40.0);
        let c = |i: usize| Pt::new(p[i].x + 60.0, p[i].y + 30.0);
        let r = |i: usize| c(i).dist(c(0));
        assert!(r(1) < r(2) && r(2) < r(3));
        assert!((r(3) - r(4)).abs() < 0.01 && (r(4) - r(5)).abs() < 0.01);
        // A chain keeps heading outward: the servers hang beyond the switch,
        // not wrapped back level with the hub.
        assert!((3..6).all(|i| c(i).y > c(2).y), "{p:?}");
        // Force-directed: the lone box is packed beside the rest, not
        // pushed far away.
        let f = arrange(Layout::Force, &nodes, &edges, 40.0);
        let right = (0..6).map(|i| f[i].x + 120.0).fold(f64::MIN, f64::max);
        assert!((f[6].x - right - 80.0).abs() < 0.01, "{f:?}");
        // Grid: 7 boxes make 3 columns.
        let g = arrange(Layout::Grid, &nodes, &edges, 40.0);
        let mut xs: Vec<i64> = g.iter().map(|q| q.x as i64).collect();
        xs.sort();
        xs.dedup();
        assert_eq!(xs.len(), 3);
    }

    #[test]
    fn force_keeps_linked_devices_closer_than_unlinked() {
        let mut nodes = boxes(&[5; 6]);
        for (i, n) in nodes.iter_mut().enumerate() {
            n.pos = Pt::new((i % 3) as f64 * 400.0, (i / 3) as f64 * 400.0);
        }
        // Two triangles.
        let edges = vec![(0, 1), (1, 2), (2, 0), (3, 4), (4, 5), (5, 3)];
        let p = arrange(Layout::Force, &nodes, &edges, 40.0);
        no_overlaps(&p);
        let d = |a: usize, b: usize| p[a].dist(p[b]);
        let inside = [d(0, 1), d(1, 2), d(3, 4), d(4, 5)].into_iter().fold(0.0, f64::max);
        let across = [d(0, 3), d(1, 4), d(2, 5)].into_iter().fold(f64::MAX, f64::min);
        assert!(inside < across, "inside {inside} across {across}");
        // Same input, same output.
        assert_eq!(p, arrange(Layout::Force, &nodes, &edges, 40.0));
    }
}
