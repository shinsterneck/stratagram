// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

//! Derived geometry shared by the on-screen renderer and the SVG exporter:
//! link routes, arrowhead shapes and label anchors.

use crate::model::*;
use std::collections::HashMap;

/// Length of the straight stub that leaves a port before an orthogonal route bends.
pub const STUB: f64 = 18.0;
/// Half the edge length of the little port square.
pub const PORT_HALF: f64 = 4.0;

/// Route from port `a` (leaving through `side_a`) to `b`. `side_b` is `None`
/// while previewing a link that ends at the mouse cursor.
#[cfg(test)]
pub fn route(a: Pt, side_a: Side, b: Pt, side_b: Option<Side>, routing: Routing) -> Vec<Pt> {
    route_scaled(a, side_a, b, side_b, routing, 1.0)
}

/// [`route`] for a link drawn at `scale` (shorter stubs when smaller).
pub fn route_scaled(a: Pt, side_a: Side, b: Pt, side_b: Option<Side>, routing: Routing, scale: f64) -> Vec<Pt> {
    route_through(a, side_a, b, side_b, routing, scale, None)
}

/// [`route_scaled`], with the middle stretch of an orthogonal route whose
/// ends leave the same way (up or down, left or right) at `mid` instead
/// of halfway.
fn route_through(a: Pt, side_a: Side, b: Pt, side_b: Option<Side>, routing: Routing, scale: f64, mid: Option<f64>) -> Vec<Pt> {
    if routing == Routing::Straight {
        return vec![a, b];
    }
    let a1 = a.add(side_a.normal().scale(STUB * scale));
    let b1 = match side_b {
        Some(s) => b.add(s.normal().scale(STUB * scale)),
        None => b,
    };
    let mut pts = vec![a, a1];
    let b_horizontal = side_b.map(|s| s.is_horizontal());
    match (side_a.is_horizontal(), b_horizontal) {
        (true, Some(true)) | (true, None) => {
            let mx = mid.unwrap_or((a1.x + b1.x) / 2.0);
            pts.push(Pt::new(mx, a1.y));
            pts.push(Pt::new(mx, b1.y));
        }
        (false, Some(false)) | (false, None) => {
            let my = mid.unwrap_or((a1.y + b1.y) / 2.0);
            pts.push(Pt::new(a1.x, my));
            pts.push(Pt::new(b1.x, my));
        }
        (true, Some(false)) => pts.push(Pt::new(b1.x, a1.y)),
        (false, Some(true)) => pts.push(Pt::new(a1.x, b1.y)),
    }
    pts.push(b1);
    pts.push(b);
    simplify(pts)
}

/// Remove duplicate and collinear interior points.
fn simplify(pts: Vec<Pt>) -> Vec<Pt> {
    let mut out: Vec<Pt> = Vec::with_capacity(pts.len());
    for p in pts {
        if out.last().is_some_and(|q| q.dist(p) < 0.01) {
            continue;
        }
        if out.len() >= 2 {
            let a = out[out.len() - 2];
            let b = out[out.len() - 1];
            let cross = (b.x - a.x) * (p.y - b.y) - (b.y - a.y) * (p.x - b.x);
            let dot = (b.x - a.x) * (p.x - b.x) + (b.y - a.y) * (p.y - b.y);
            if cross.abs() < 1e-6 && dot >= 0.0 {
                out.pop();
            }
        }
        out.push(p);
    }
    out
}

/// Clearance kept between a line routed around objects and their boxes.
pub const CLEARANCE: f64 = 12.0;
/// Extra cost of a bend, in pixels of length: fewer bends beat slightly
/// shorter lines.
const BEND_COST: f64 = 40.0;

/// Boxes lines should not cross: shown devices and texts.
pub fn obstacles(d: &Diagram) -> Vec<Rect> {
    let devices = d.devices.iter().filter(|x| d.layer_visible(x.layer)).map(|x| x.rect());
    let notes = d.notes.iter().filter(|x| d.layer_visible(x.layer)).map(|x| x.rect());
    devices.chain(notes).collect()
}

/// Obstacles for one link: like [`obstacles`], but without the devices
/// its ends lie in, since a line from inside a container has to leave it.
pub fn link_obstacles(d: &Diagram, l: &Link) -> Vec<Rect> {
    let mut skip = d.enclosing(l.from.owner());
    skip.extend(d.enclosing(l.to.owner()));
    let devices = d.devices.iter().filter(|x| d.layer_visible(x.layer) && !skip.contains(&x.id)).map(|x| x.rect());
    let notes = d.notes.iter().filter(|x| d.layer_visible(x.layer)).map(|x| x.rect());
    devices.chain(notes).collect()
}

/// Whether segment `p`-`q` passes through the inside of `r` (touching or
/// running along its border does not count).
pub fn segment_hits_rect(p: Pt, q: Pt, r: &Rect) -> bool {
    let r = r.inflate(-1.0);
    if r.w <= 0.0 || r.h <= 0.0 {
        return false;
    }
    // Liang-Barsky clipping against the rectangle.
    let d = q.sub(p);
    let (mut t0, mut t1) = (0.0_f64, 1.0_f64);
    for (pk, qk) in [(-d.x, p.x - r.x), (d.x, r.x + r.w - p.x), (-d.y, p.y - r.y), (d.y, r.y + r.h - p.y)] {
        if pk.abs() < 1e-12 {
            if qk < 0.0 {
                return false;
            }
        } else {
            let t = qk / pk;
            if pk < 0.0 {
                t0 = t0.max(t);
            } else {
                t1 = t1.min(t);
            }
            if t0 > t1 {
                return false;
            }
        }
    }
    t1 - t0 > 1e-9
}

/// Whether a route passes through any of `obstacles`.
pub fn crosses(route: &[Pt], obstacles: &[Rect]) -> bool {
    route.windows(2).any(|w| obstacles.iter().any(|r| segment_hits_rect(w[0], w[1], r)))
}

/// How many times routes `p` and `q` cross each other: segments meeting
/// inside both of them, running along each other counting once per
/// stretch (meeting at an end of either does not count).
pub fn route_crossings(p: &[Pt], q: &[Pt]) -> usize {
    const EPS: f64 = 0.5;
    let mut n = 0;
    for s in p.windows(2) {
        for t in q.windows(2) {
            let (a, b, c, e) = (s[0], s[1], t[0], t[1]);
            let r = b.sub(a);
            let u = e.sub(c);
            let den = r.x * u.y - r.y * u.x;
            let ca = c.sub(a);
            if den.abs() < 1e-9 {
                // Parallel: on top of each other for a stretch?
                if (ca.x * r.y - ca.y * r.x).abs() > EPS * r.x.hypot(r.y).max(1e-9) {
                    continue;
                }
                let len = r.x.hypot(r.y);
                if len < 1e-9 {
                    continue;
                }
                let along = |p: Pt| (p.sub(a).x * r.x + p.sub(a).y * r.y) / len;
                let (lo, hi) = (along(c).min(along(e)), along(c).max(along(e)));
                if hi.min(len) - lo.max(0.0) > EPS {
                    n += 1;
                }
                continue;
            }
            let t1 = (ca.x * u.y - ca.y * u.x) / den;
            let t2 = (ca.x * r.y - ca.y * r.x) / den;
            let (l1, l2) = (r.x.hypot(r.y), u.x.hypot(u.y));
            let inner = |t: f64, len: f64| t * len > EPS && (1.0 - t) * len > EPS;
            if inner(t1, l1) && inner(t2, l2) {
                n += 1;
            }
        }
    }
    n
}

/// Give line `i` of `d` the first routing that keeps it clear of
/// `obstacles`: its own, then straight, then orthogonal, else around
/// them (an orthogonal line stays orthogonal, around if need be).
/// Returns the routing it gets.
pub fn route_clear_of(d: &mut Diagram, i: usize, obstacles: &[Rect]) -> Routing {
    let own = d.links[i].style.routing;
    let tries: &[Routing] = if own == Routing::Orthogonal { &[Routing::Orthogonal] } else { &[own, Routing::Straight, Routing::Orthogonal] };
    let mut pick = Routing::Around;
    for &r in tries {
        d.links[i].style.routing = r;
        if link_route(d, &d.links[i]).is_some_and(|pts| !crosses(&pts, obstacles)) {
            pick = r;
            break;
        }
    }
    d.links[i].style.routing = pick;
    pick
}

/// Orthogonal route from `a` (leaving through `side_a`) to `b` (entered
/// through `side_b`) that keeps clear of `obstacles`: a shortest path with
/// few bends over lanes along the obstacles' edges. None if there is no way.
#[cfg(test)]
pub fn route_around(a: Pt, side_a: Side, b: Pt, side_b: Side, obstacles: &[Rect]) -> Option<Vec<Pt>> {
    route_around_scaled(a, side_a, b, side_b, obstacles, 1.0)
}

/// [`route_around`] for a link drawn at `scale`.
pub fn route_around_scaled(a: Pt, side_a: Side, b: Pt, side_b: Side, obstacles: &[Rect], scale: f64) -> Option<Vec<Pt>> {
    use std::cmp::Reverse;
    use std::collections::BinaryHeap;

    let a1 = a.add(side_a.normal().scale(STUB * scale));
    let b1 = b.add(side_b.normal().scale(STUB * scale));
    // Obstacles with clearance; where that would swallow a stub end (boxes
    // very close together), keep just the box itself.
    let inside = |r: &Rect, p: Pt| p.x > r.x && p.x < r.x + r.w && p.y > r.y && p.y < r.y + r.h;
    let obs: Vec<Rect> = obstacles
        .iter()
        .map(|r| {
            let big = r.inflate(CLEARANCE * scale);
            if inside(&big, a1) || inside(&big, b1) { *r } else { big }
        })
        .collect();
    if obs.iter().any(|r| inside(r, a1) || inside(r, b1)) {
        return None;
    }
    let lanes = |extra: [f64; 2], lo: fn(&Rect) -> f64, hi: fn(&Rect) -> f64| {
        let mut v: Vec<f64> = obs.iter().flat_map(|r| [lo(r), hi(r)]).chain(extra).collect();
        v.sort_by(f64::total_cmp);
        v.dedup_by(|x, y| (*x - *y).abs() < 0.01);
        v
    };
    let xs = lanes([a1.x, b1.x], |r| r.x, |r| r.x + r.w);
    let ys = lanes([a1.y, b1.y], |r| r.y, |r| r.y + r.h);
    let (nx, ny) = (xs.len(), ys.len());
    let find = |v: &[f64], x: f64| v.iter().position(|y| (y - x).abs() < 0.01);
    let (sx, sy, tx, ty) = (find(&xs, a1.x)?, find(&ys, a1.y)?, find(&xs, b1.x)?, find(&ys, b1.y)?);

    // Lanes cover every obstacle edge, so a step between neighboring lanes
    // is either wholly inside an obstacle or wholly outside it.
    let free = |x0: f64, y0: f64, x1: f64, y1: f64| {
        !obs.iter().any(|r| {
            let (lx, hx) = (x0.min(x1), x0.max(x1));
            let (ly, hy) = (y0.min(y1), y0.max(y1));
            lx < r.x + r.w - 0.01 && hx > r.x + 0.01 && ly < r.y + r.h - 0.01 && hy > r.y + 0.01
                || (lx == hx && lx > r.x + 0.01 && lx < r.x + r.w - 0.01 && ly < r.y + r.h - 0.01 && hy > r.y + 0.01)
                || (ly == hy && ly > r.y + 0.01 && ly < r.y + r.h - 0.01 && lx < r.x + r.w - 0.01 && hx > r.x + 0.01)
        })
    };
    // Directions: 0 right, 1 down, 2 left, 3 up.
    const STEP: [(isize, isize); 4] = [(1, 0), (0, 1), (-1, 0), (0, -1)];
    let dir_of = |s: Side| match s {
        Side::Right => 0,
        Side::Bottom => 1,
        Side::Left => 2,
        Side::Top => 3,
    };
    let start_dir = dir_of(side_a);
    // Arriving at b1 we then head into b, against its side's normal.
    let end_dir = (dir_of(side_b) + 2) % 4;
    let idx = |x: usize, y: usize, d: usize| (y * nx + x) * 4 + d;
    let mut best = vec![f64::INFINITY; nx * ny * 4];
    let mut prev = vec![usize::MAX; nx * ny * 4];
    let mut heap = BinaryHeap::new();
    let key = |c: f64| Reverse((c * 100.0) as u64);
    let h = |x: usize, y: usize| (xs[x] - b1.x).abs() + (ys[y] - b1.y).abs();
    best[idx(sx, sy, start_dir)] = 0.0;
    heap.push((key(h(sx, sy)), idx(sx, sy, start_dir)));
    let mut done = None;
    while let Some((_, state)) = heap.pop() {
        let (d, cell) = (state % 4, state / 4);
        let (x, y) = (cell % nx, cell / nx);
        let cost = best[state];
        if x == tx && y == ty {
            let total = cost + if d == end_dir { 0.0 } else { BEND_COST * scale };
            if done.is_none_or(|(c, _)| total < c) {
                done = Some((total, state));
            }
            continue;
        }
        if done.is_some_and(|(c, _)| cost + h(x, y) >= c) {
            break;
        }
        for (nd, (dx, dy)) in STEP.iter().enumerate() {
            if nd == (d + 2) % 4 {
                continue; // no U-turns on the spot
            }
            let (Some(x2), Some(y2)) = (x.checked_add_signed(*dx), y.checked_add_signed(*dy)) else { continue };
            if x2 >= nx || y2 >= ny || !free(xs[x], ys[y], xs[x2], ys[y2]) {
                continue;
            }
            let step = (xs[x2] - xs[x]).abs() + (ys[y2] - ys[y]).abs();
            let c = cost + step + if nd == d { 0.0 } else { BEND_COST * scale };
            let next = idx(x2, y2, nd);
            if c < best[next] - 1e-9 {
                best[next] = c;
                prev[next] = state;
                heap.push((key(c + h(x2, y2)), next));
            }
        }
    }
    let (_, mut state) = done?;
    let mut path = vec![];
    loop {
        let cell = state / 4;
        path.push(Pt::new(xs[cell % nx], ys[cell / nx]));
        if prev[state] == usize::MAX {
            break;
        }
        state = prev[state];
    }
    path.reverse();
    let mut pts = vec![a];
    pts.extend(path);
    pts.push(b);
    Some(simplify(pts))
}

/// Where a stored link starts and ends, and which way it leaves each end.
fn link_ends(d: &Diagram, l: &Link) -> Option<(Pt, Side, Pt, Side)> {
    let (a, mut sa) = d.endpoint_pos(l.from)?;
    let (b, mut sb) = d.endpoint_pos(l.to)?;
    // Connection points have no side: leave toward the other end.
    if l.from.is_point() {
        sa = facing_side(a, b);
    }
    if l.to.is_point() {
        sb = facing_side(b, a);
    }
    Some((a, sa, b, sb))
}

/// The middle stretch of an orthogonal line whose ends both leave up or
/// down (or both left or right): whether it runs across (horizontal),
/// where it is by default (halfway), its extent along it, the room it can
/// move in, and its order among lines sharing it (see [`lane`]).
#[derive(Debug, Clone, Copy)]
struct Stretch {
    across: bool,
    mid: f64,
    lo: f64,
    hi: f64,
    room: (f64, f64),
    order: f64,
}

fn stretch(l: &Link, a: Pt, sa: Side, b: Pt, sb: Side) -> Option<Stretch> {
    if l.style.routing != Routing::Orthogonal || sa.is_horizontal() != sb.is_horizontal() {
        return None;
    }
    let a1 = a.add(sa.normal().scale(STUB * l.scale));
    let b1 = b.add(sb.normal().scale(STUB * l.scale));
    // Along the stretch (x for one across) and the way the ends go (y).
    let across = !sa.is_horizontal();
    let along = |p: Pt| -> (f64, f64) { if across { (p.x, p.y) } else { (p.y, p.x) } };
    let ((pa, qa), (pb, qb)) = (along(a1), along(b1));
    if (pa - pb).abs() < 0.5 {
        return None; // straight through, no stretch
    }
    // From the end it starts at (the upper or left one): lines turning
    // toward higher coordinates go first from the far end of the group,
    // the others from the near end, so they do not cross.
    let (start, end) = if qa <= qb { (pa, pb) } else { (pb, pa) };
    let order = if end > start { -start } else { start };
    Some(Stretch { across, mid: (qa + qb) / 2.0, lo: pa.min(pb), hi: pa.max(pb), room: (qa.min(qb), qa.max(qb)), order })
}

/// Lines running side by side, the middle stretches of orthogonal lines
/// at the same place, each get a lane of their own: where the stretch of
/// `l` goes. None when it has no such stretch, or no other line shares it.
fn lane(d: &Diagram, l: &Link, own: &Stretch) -> Option<f64> {
    let overlaps = |s: &Stretch| s.across == own.across && (s.mid - own.mid).abs() < 1.0 && s.lo < own.hi - 0.5 && own.lo < s.hi - 0.5;
    let mut group: Vec<(f64, Id)> = d
        .links
        .iter()
        .filter(|m| m.id != l.id && m.style.routing == Routing::Orthogonal && d.link_visible(m))
        .filter_map(|m| {
            let (a, sa, b, sb) = link_ends(d, m)?;
            stretch(m, a, sa, b, sb).filter(overlaps).map(|s| (s.order, m.id))
        })
        .collect();
    if group.is_empty() {
        return None;
    }
    group.push((own.order, l.id));
    group.sort_by(|x, y| x.0.total_cmp(&y.0).then(x.1.cmp(&y.1)));
    let k = group.iter().position(|g| g.1 == l.id)? as f64;
    let n = group.len() as f64;
    // Lanes a little apart, within the room between the ends' stubs.
    let gap = (LANE_GAP * l.scale).min((own.room.1 - own.room.0) / (n + 1.0));
    Some(own.mid + (k - (n - 1.0) / 2.0) * gap)
}

/// Space between lines running side by side in lanes.
pub const LANE_GAP: f64 = 10.0;

/// Full untrimmed route of a stored link.
pub fn link_route(d: &Diagram, l: &Link) -> Option<Vec<Pt>> {
    let (a, sa, b, sb) = link_ends(d, l)?;
    if let Some(own) = stretch(l, a, sa, b, sb) {
        let mid = lane(d, l, &own);
        return Some(route_through(a, sa, b, Some(sb), l.style.routing, l.scale, mid));
    }
    if l.style.routing == Routing::Around {
        // Its own devices count too, so the line never doubles back
        // through them; with no way around, fall back to orthogonal.
        if let Some(r) = route_around_scaled(a, sa, b, sb, &link_obstacles(d, l), l.scale) {
            return Some(r);
        }
    }
    Some(route_scaled(a, sa, b, Some(sb), l.style.routing, l.scale))
}

#[derive(Debug, Clone, PartialEq)]
pub enum HeadShape {
    /// Closed polygon, filled with the line color.
    Filled(Vec<Pt>),
    /// Open polyline (chevron).
    Chevron(Vec<Pt>),
    /// Circle outline filled with the background color.
    Circle { c: Pt, r: f64 },
}

/// Arrowhead length for a line of `width` drawn at `scale`.
pub fn head_len(width: f64, scale: f64) -> f64 {
    9.0 * scale + 2.0 * width
}

/// Build the decoration for a line end. `tip` is the end point, `from` the
/// previous point of the route. Returns the shape and how far the line
/// itself should be shortened so it does not poke through the decoration.
pub fn head_shape(kind: Head, tip: Pt, from: Pt, width: f64, scale: f64) -> Option<(HeadShape, f64)> {
    let dir = tip.sub(from).norm();
    let n = dir.perp();
    let l = head_len(width, scale);
    let hw = l * 0.42;
    match kind {
        Head::None => None,
        Head::Arrow => {
            let base = tip.sub(dir.scale(l));
            Some((HeadShape::Filled(vec![tip, base.add(n.scale(hw)), base.sub(n.scale(hw))]), l * 0.8))
        }
        Head::Open => {
            let base = tip.sub(dir.scale(l));
            Some((HeadShape::Chevron(vec![base.add(n.scale(hw)), tip, base.sub(n.scale(hw))]), width * 0.5))
        }
        Head::Diamond => {
            let mid = tip.sub(dir.scale(l * 0.75));
            let back = tip.sub(dir.scale(l * 1.5));
            Some((
                HeadShape::Filled(vec![tip, mid.add(n.scale(hw * 0.85)), back, mid.sub(n.scale(hw * 0.85))]),
                l * 1.4,
            ))
        }
        Head::Circle => {
            let r = l * 0.38;
            Some((HeadShape::Circle { c: tip.sub(dir.scale(r)), r }, 2.0 * r))
        }
    }
}

/// Shorten the end of a polyline by `by`, never past its last segment.
pub fn trim_end(pts: &mut [Pt], by: f64) {
    let n = pts.len();
    if n < 2 || by <= 0.0 {
        return;
    }
    let a = pts[n - 2];
    let b = pts[n - 1];
    let seg = a.dist(b);
    let cut = by.min((seg - 0.5).max(0.0));
    pts[n - 1] = b.sub(b.sub(a).norm().scale(cut));
}

pub fn trim_start(pts: &mut [Pt], by: f64) {
    pts.reverse();
    trim_end(pts, by);
    pts.reverse();
}

/// Point halfway along the polyline, measured by length.
pub fn midpoint(pts: &[Pt]) -> Pt {
    let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    let mut left = total / 2.0;
    for w in pts.windows(2) {
        let s = w[0].dist(w[1]);
        if s >= left && s > 0.0 {
            return w[0].add(w[1].sub(w[0]).scale(left / s));
        }
        left -= s;
    }
    pts.first().copied().unwrap_or_default()
}

/// The dash pattern (at scale 1) of a band's edge for a type carried in it:
/// dashed for VXLAN, dotted for CAPWAP, long dashes for SD-WAN, solid
/// (none) for the rest.
pub fn band_dash(kind: LinkKind) -> Option<(f64, f64)> {
    match kind {
        LinkKind::Vxlan => Some((6.0, 4.0)),
        LinkKind::Capwap => Some((1.5, 2.5)),
        LinkKind::Sdwan => Some((10.0, 4.0)),
        _ => None,
    }
}

/// The point `t` (0 to 1) of the way along `pts`, by length.
pub fn point_along(pts: &[Pt], t: f64) -> Pt {
    let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    let mut left = total * t.clamp(0.0, 1.0);
    for w in pts.windows(2) {
        let s = w[0].dist(w[1]);
        if s >= left && s > 0.0 {
            return w[0].add(w[1].sub(w[0]).scale(left / s));
        }
        left -= s;
    }
    pts.last().copied().unwrap_or_default()
}

/// How far along `pts` (0 to 1, by length) the point nearest to `p` is.
pub fn nearest_along(pts: &[Pt], p: Pt) -> f64 {
    let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    if total < 1e-9 {
        return 0.5;
    }
    let (mut best, mut best_d, mut walked) = (0.0, f64::INFINITY, 0.0);
    for w in pts.windows(2) {
        let (a, b) = (w[0], w[1]);
        let len = a.dist(b);
        let ab = b.sub(a);
        let s = if len < 1e-9 { 0.0 } else { (((p.x - a.x) * ab.x + (p.y - a.y) * ab.y) / (len * len)).clamp(0.0, 1.0) };
        let q = a.add(ab.scale(s));
        let d = q.dist(p);
        if d < best_d {
            best_d = d;
            best = (walked + s * len) / total;
        }
        walked += len;
    }
    best
}

/// Direction of `pts` at `t` of the way along it.
fn direction_along(pts: &[Pt], t: f64) -> Pt {
    let total: f64 = pts.windows(2).map(|w| w[0].dist(w[1])).sum();
    let mut left = total * t.clamp(0.0, 1.0);
    for w in pts.windows(2) {
        let s = w[0].dist(w[1]);
        if s >= left && s > 0.0 {
            return w[1].sub(w[0]).norm();
        }
        left -= s;
    }
    Pt::new(1.0, 0.0)
}

/// A link's label box as drawn (its type's tag in bold, then the label):
/// width and height, or nothing when it shows neither.
pub fn label_box(l: &Link) -> Option<(f64, f64)> {
    if l.label_hidden {
        return None;
    }
    let tag = l.style.full_tag();
    if tag.is_empty() && l.label.is_empty() {
        return None;
    }
    let (tag_w, label_w) = (tag.chars().count() as f64 * 6.4, l.label.chars().count() as f64 * 6.6);
    let gap = if tag_w > 0.0 && label_w > 0.0 { 6.0 } else { 0.0 };
    Some(((tag_w + gap + label_w + 10.0) * l.scale, 17.0 * l.scale))
}

/// Where each shown link's label goes, and how far along its line (0 to
/// 1). A label sits in the middle of its line unless that overlaps a label
/// placed before it; then it slides along its line to a free spot, keeping
/// its label margin to the ends where it can.
pub fn label_spots(d: &Diagram) -> HashMap<Id, (Pt, f64)> {
    const TRIES: [f64; 11] = [0.5, 0.4, 0.6, 0.3, 0.7, 0.2, 0.8, 0.35, 0.65, 0.25, 0.75];
    // Labels placed by hand first, where they were put; the others make
    // room for them.
    let mut links: Vec<&Link> = d.links.iter().filter(|l| d.link_visible(l)).collect();
    links.sort_by_key(|l| (l.label_at.is_none(), l.id));
    let mut placed: Vec<Rect> = vec![];
    let mut out = HashMap::new();
    for l in links {
        let (Some((w, h)), Some(route)) = (label_box_in(d, l), link_route(d, l)) else { continue };
        let total: f64 = route.windows(2).map(|s| s[0].dist(s[1])).sum();
        let at = |t: f64| {
            let p = point_along(&route, t);
            Rect { x: p.x - w / 2.0, y: p.y - h / 2.0, w, h }
        };
        // Clear of the ends by half the box along the line plus the margin.
        let fits = |t: f64| {
            let u = direction_along(&route, t);
            let need = (w * u.x.abs() + h * u.y.abs()) / 2.0 + l.style.label_margin * l.scale;
            t * total >= need && (1.0 - t) * total >= need
        };
        let free = |t: f64| !placed.iter().any(|r: &Rect| overlaps(&r.inflate(3.0), &at(t)));
        // Best a free spot clear of the ends, else any free spot.
        let t = l.label_at.map(|t| t.clamp(0.0, 1.0)).unwrap_or_else(|| {
            TRIES
                .into_iter()
                .find(|&t| (t == 0.5 || fits(t)) && free(t))
                .or_else(|| TRIES.into_iter().find(|&t| free(t)))
                .unwrap_or(0.5)
        });
        placed.push(at(t));
        out.insert(l.id, (point_along(&route, t), t));
    }
    out
}

fn overlaps(a: &Rect, b: &Rect) -> bool {
    a.x < b.x + b.w && b.x < a.x + a.w && a.y < b.y + b.h && b.y < a.y + a.h
}

/// A drawn link, already trimmed for its end decorations.
#[derive(Debug, Clone)]
pub struct LinkGeometry {
    pub line: Vec<Pt>,
    pub heads: Vec<HeadShape>,
    pub label_pos: Pt,
    /// Short strokes across the line, like a label stack (MPLS).
    pub ticks: Vec<(Pt, Pt)>,
    /// Centers of the small open circles near each end of a routed (layer
    /// 3) line, marking its IP interfaces.
    pub beads: Vec<Pt>,
}

pub fn link_geometry(route: &[Pt], style: &LinkStyle, scale: f64) -> LinkGeometry {
    link_geometry_ends(route, style, scale, (true, true))
}

/// [`link_geometry`] with only the ends that are `open` (start, end)
/// decorated: an end at a joint of a bent line (see [`is_joint`]) gets no
/// arrowhead and no end mark, as the line runs on through it.
pub fn link_geometry_ends(route: &[Pt], style: &LinkStyle, scale: f64, open: (bool, bool)) -> LinkGeometry {
    let mut line = route.to_vec();
    let mut heads = vec![];
    let n = route.len();
    if n >= 2 {
        if let Some((shape, trim)) = head_shape(style.head_end, route[n - 1], route[n - 2], style.width, scale).filter(|_| open.1) {
            trim_end(&mut line, trim);
            heads.push(shape);
        }
        if let Some((shape, trim)) = head_shape(style.head_start, route[0], route[1], style.width, scale).filter(|_| open.0) {
            trim_start(&mut line, trim);
            heads.push(shape);
        }
    }
    let ticks = if style.kind == LinkKind::Mpls { cross_ticks(&line, 14.0 * scale, 4.0 * scale + style.width / 2.0) } else { vec![] };
    let mut beads = if style.kind.has_end_marks() { near_ends(&line, BEAD_OFFSET * scale) } else { vec![] };
    if beads.len() == 2 {
        // near_ends gives the start's mark, then the end's.
        if !open.1 {
            beads.pop();
        }
        if !open.0 {
            beads.remove(0);
        }
    }
    LinkGeometry { label_pos: midpoint(route), line, heads, ticks, beads }
}

// Bent lines

/// Whether `ep` is a joint: a free point where exactly two lines of one
/// type meet. A line bent through connection points is stored as pieces
/// meeting at such points, and is drawn as one line running through them.
pub fn is_joint(d: &Diagram, ep: Endpoint) -> bool {
    if !ep.is_point() {
        return false;
    }
    let mut at = d.links.iter().filter(|l| l.from == ep || l.to == ep);
    match (at.next(), at.next(), at.next()) {
        (Some(a), Some(b), None) => a.id != b.id && a.style.kind == b.style.kind && a.style.over == b.style.over,
        _ => false,
    }
}

/// A line bent through joints: its pieces in order along it, each with
/// whether it runs backwards (to to from), and its two outer ends.
#[derive(Debug, Clone, PartialEq)]
pub struct BentLine {
    pub pieces: Vec<(Id, bool)>,
    pub from: Endpoint,
    pub to: Endpoint,
}

/// The whole bent line the piece `l` belongs to (just `l` if it has no
/// joints).
pub fn bent_line(d: &Diagram, l: &Link) -> BentLine {
    let ends = |id: Id, rev: bool| d.link(id).map(|x| if rev { (x.to, x.from) } else { (x.from, x.to) });
    let mut pieces: std::collections::VecDeque<(Id, bool)> = std::collections::VecDeque::from([(l.id, false)]);
    let mut seen = std::collections::HashSet::from([l.id]);
    // Back from its start, then on from its end.
    for forward in [false, true] {
        loop {
            let (id, rev) = if forward { *pieces.back().unwrap() } else { *pieces.front().unwrap() };
            let Some((a, b)) = ends(id, rev) else { break };
            let at = if forward { b } else { a };
            if !is_joint(d, at) {
                break;
            }
            let Some(next) = d.links.iter().find(|x| x.id != id && (x.from == at || x.to == at)) else { break };
            if !seen.insert(next.id) {
                break;
            }
            // Oriented to run on from (or into) the joint.
            let rev = if forward { next.to == at } else { next.from == at };
            if forward {
                pieces.push_back((next.id, rev));
            } else {
                pieces.push_front((next.id, rev));
            }
        }
    }
    let pieces: Vec<(Id, bool)> = pieces.into_iter().collect();
    let from = ends(pieces[0].0, pieces[0].1).map_or(l.from, |e| e.0);
    let to = ends(pieces.last().unwrap().0, pieces.last().unwrap().1).map_or(l.to, |e| e.1);
    BentLine { pieces, from, to }
}

/// The piece of a bent line that draws its bands: the one drawn first
/// (lowest `rank` in the stacking order), so the band lies under every
/// piece's line.
pub fn band_piece(b: &BentLine, rank: impl Fn(Id) -> usize) -> Id {
    b.pieces.iter().map(|p| p.0).min_by_key(|id| (rank(*id), *id)).unwrap_or(b.pieces[0].0)
}

/// The route of a whole bent line, from its start to its end.
pub fn bent_route(d: &Diagram, b: &BentLine) -> Option<Vec<Pt>> {
    let mut pts: Vec<Pt> = vec![];
    for &(id, rev) in &b.pieces {
        let mut r = link_route(d, d.link(id)?)?;
        if rev {
            r.reverse();
        }
        if pts.last().is_some_and(|p| r.first().is_some_and(|q| p.dist(*q) < 0.01)) {
            r.remove(0);
        }
        pts.extend(r);
    }
    Some(pts)
}

/// Whether piece `l` shows its bent line's type tag: the piece with label
/// text (the first one), else the first piece. Lines without joints do.
pub fn shows_tag(d: &Diagram, l: &Link) -> bool {
    let b = bent_line(d, l);
    if b.pieces.len() < 2 {
        return true;
    }
    let with_label = b.pieces.iter().find(|(id, _)| d.link(*id).is_some_and(|x| !x.label.is_empty() && !x.label_hidden));
    with_label.unwrap_or(&b.pieces[0]).0 == l.id
}

/// [`label_box`] as drawn in `d`: without the type tag on the pieces of a
/// bent line that leave it to another.
pub fn label_box_in(d: &Diagram, l: &Link) -> Option<(f64, f64)> {
    if shows_tag(d, l) {
        return label_box(l);
    }
    let bare = Link { style: LinkStyle { kind: LinkKind::Plain, over: vec![], ..l.style.clone() }, ..l.clone() };
    label_box(&bare)
}

/// The outline of a tunnel drawn `half` wide on each side of `route` (the
/// band around an IPsec or VXLAN line). Where an end meets a device, the
/// band is cut along that device's border instead of square to the line,
/// so a line arriving at an angle still reaches the border with both
/// corners. Corners are mitered.
pub fn tunnel_band(d: &Diagram, l: &Link, route: &[Pt], half: f64) -> Vec<Pt> {
    tunnel_band_between(d, l.from, l.to, route, half)
}

/// [`tunnel_band`] for a route from `from` to `to` (a whole bent line).
pub fn tunnel_band_between(d: &Diagram, from: Endpoint, to: Endpoint, route: &[Pt], half: f64) -> Vec<Pt> {
    let mut pts: Vec<Pt> = vec![];
    for p in route {
        if pts.last().is_none_or(|q: &Pt| q.dist(*p) > 1e-6) {
            pts.push(*p);
        }
    }
    if pts.len() < 2 {
        return vec![];
    }
    let dirs: Vec<Pt> = pts.windows(2).map(|w| w[1].sub(w[0]).norm()).collect();
    // Offset of each point to the left of the line (miter at corners).
    let offsets: Vec<Pt> = (0..pts.len())
        .map(|i| {
            let n_in = dirs[i.saturating_sub(1)].perp();
            let n_out = dirs[i.min(dirs.len() - 1)].perp();
            let sum = n_in.add(n_out);
            let k = 1.0 + n_in.x * n_out.x + n_in.y * n_out.y;
            if k < 0.25 { n_out.scale(half) } else { sum.scale(half / k) }
        })
        .collect();
    let mut left: Vec<Pt> = pts.iter().zip(&offsets).map(|(p, o)| p.add(*o)).collect();
    let mut right: Vec<Pt> = pts.iter().zip(&offsets).map(|(p, o)| p.sub(*o)).collect();
    // Slide the end corners along their edge onto the border line.
    let border = |ep: Endpoint| if ep.is_point() { None } else { d.endpoint_pos(ep).map(|(_, side)| side.normal()) };
    let last = pts.len() - 1;
    for (i, along, ep) in [(0, dirs[0], from), (last, dirs[last - 1], to)] {
        let Some(n) = border(ep) else { continue };
        let across = along.x * n.x + along.y * n.y;
        if across.abs() < 0.2 {
            continue; // running along the border: nothing sensible to cut
        }
        for side in [&mut left, &mut right] {
            let off = side[i].sub(pts[i]);
            let t = -(off.x * n.x + off.y * n.y) / across;
            side[i] = side[i].add(along.scale(t.clamp(-4.0 * half, 4.0 * half)));
        }
    }
    right.reverse();
    left.extend(right);
    left
}

/// How far from each end of a routed line its circles sit.
pub const BEAD_OFFSET: f64 = 12.0;
/// Radius of those circles (before scaling), plus half the line width.
pub const BEAD_RADIUS: f64 = 3.5;

/// The points `d` along `line` from its start and from its end (none when
/// the line is too short to hold both apart).
fn near_ends(line: &[Pt], d: f64) -> Vec<Pt> {
    let total: f64 = line.windows(2).map(|w| w[0].dist(w[1])).sum();
    if line.len() < 2 || total < 3.0 * d {
        return vec![];
    }
    let at = |pts: &mut dyn Iterator<Item = (Pt, Pt)>| {
        let mut left = d;
        for (a, b) in pts {
            let len = a.dist(b);
            if len >= left && len > 1e-9 {
                return Some(a.add(b.sub(a).scale(left / len)));
            }
            left -= len;
        }
        None
    };
    let fwd = at(&mut line.windows(2).map(|w| (w[0], w[1])));
    let back = at(&mut line.windows(2).rev().map(|w| (w[1], w[0])));
    fwd.into_iter().chain(back).collect()
}

/// Strokes `half` long on each side across `line`, every `step` along it,
/// kept clear of its ends.
fn cross_ticks(line: &[Pt], step: f64, half: f64) -> Vec<(Pt, Pt)> {
    let total: f64 = line.windows(2).map(|w| w[0].dist(w[1])).sum();
    let n = ((total - 2.0 * step) / step).floor();
    if n < 0.0 {
        return vec![];
    }
    // Spread evenly, centered along the line.
    let first = (total - n * step) / 2.0;
    let mut ticks = vec![];
    let (mut at, mut walked) = (first, 0.0);
    for w in line.windows(2) {
        let len = w[0].dist(w[1]);
        if len < 1e-9 {
            continue;
        }
        let u = w[1].sub(w[0]).scale(1.0 / len);
        while at <= walked + len + 1e-9 && at <= total - first + 1e-9 {
            let c = w[0].add(u.scale(at - walked));
            let off = u.perp().scale(half);
            ticks.push((c.sub(off), c.add(off)));
            at += step;
        }
        walked += len;
    }
    ticks
}

/// Interface names: how far along a vertical line from the port, how far
/// beside it, and how far above a horizontal line.
pub const PORT_LABEL_ALONG: f64 = 11.0;
pub const PORT_LABEL_SIDE: f64 = 8.0;
pub const PORT_LABEL_ABOVE: f64 = 9.0;

/// Where to put a port's name: a short way along its (first) link, beside
/// the line, like interface labels in usual network diagrams. Unlinked ports
/// use the outward normal. `link_dirs` are the directions in which attached
/// links leave the port. Returns (anchor, align, baseline).
/// Where the name of port `p` of `dev` goes ([`port_label_anchor`] at
/// the start of its lines). A patch panel's or switch's socket has it
/// outside the panel's edge, beside its cord, whichever way its lines go.
pub fn port_label(dev: &Device, p: &Port, dirs: &HashMap<Id, Vec<Pt>>) -> (Pt, &'static str, &'static str) {
    let socket = dev.socket_pos(p.socket).is_some();
    let link_dirs = if socket { &[][..] } else { dirs.get(&p.id).map(|v| v.as_slice()).unwrap_or(&[]) };
    port_label_anchor(dev.port_exit(p), dev.port_side(p), link_dirs, dev.scale)
}

pub fn port_label_anchor(pos: Pt, side: Side, link_dirs: &[Pt], scale: f64) -> (Pt, &'static str, &'static str) {
    let u = link_dirs.first().map(|d| d.norm()).unwrap_or(side.normal());
    // Unit normal pointing right (or up for vertical normals).
    let mut n = u.perp();
    if n.x < -1e-6 || (n.x.abs() < 1e-6 && n.y > 0.0) {
        n = n.scale(-1.0);
    }
    if n.x > 0.3 {
        // Mostly vertical or diagonal line: label to its right, close to
        // the port, clear of an arrowhead there.
        (pos.add(u.scale(PORT_LABEL_ALONG * scale)).add(n.scale(PORT_LABEL_SIDE * scale)), "left", "middle")
    } else if u.x >= 0.0 {
        // Mostly horizontal line heading right: label above, starting beside
        // the port, high enough to clear the port and an arrowhead.
        (Pt::new(pos.x + 9.0 * scale, pos.y - PORT_LABEL_ABOVE * scale), "left", "bottom")
    } else {
        (Pt::new(pos.x - 9.0 * scale, pos.y - PORT_LABEL_ABOVE * scale), "right", "bottom")
    }
}

/// Where a connection point's label goes: just beyond the end of its
/// line(s), continuing their direction (a line ending at a labeled point
/// reads like an arrow pointing at the text); to the right of a point
/// without lines. Returns (anchor, align, baseline).
pub fn point_label_anchor(d: &Diagram, q: &Point) -> (Pt, &'static str, &'static str) {
    let here = Endpoint::point(q.id);
    let mut toward = Pt::default();
    for l in d.links.iter().filter(|l| l.from == here || l.to == here) {
        let Some(pts) = link_route(d, l) else { continue };
        let n = pts.len();
        if n < 2 {
            continue;
        }
        let dir = if l.from == here { pts[1].sub(pts[0]) } else { pts[n - 2].sub(pts[n - 1]) };
        toward = toward.add(dir.norm());
    }
    // Away from the lines.
    let out = if toward.len() < 1e-6 { Pt::new(1.0, 0.0) } else { toward.norm().scale(-1.0) };
    let at = q.pos().add(out.scale(9.0 * q.scale));
    if out.x.abs() >= out.y.abs() {
        (at, if out.x >= 0.0 { "left" } else { "right" }, "middle")
    } else {
        (at, "center", if out.y >= 0.0 { "top" } else { "bottom" })
    }
}

/// The box a text of `chars` characters at `size` takes when anchored at
/// `at` with `align` and `base` (as the canvas draws it). The width is an
/// estimate on the wide side (no font metrics here).
fn text_rect(at: Pt, chars: usize, size: f64, align: &str, base: &str) -> Rect {
    let (w, h) = (chars as f64 * size * 0.62, size * 1.3);
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
    Rect { x, y, w, h }
}

/// Where the texts outside the boxes are: point labels, line labels,
/// interface names, and device names wider than their device. Hidden
/// layers count as in `d` (pass a diagram of what is shown).
pub fn label_rects(d: &Diagram) -> Vec<Rect> {
    let mut out = vec![];
    for q in d.points.iter().filter(|q| !q.label.is_empty()) {
        let (at, align, base) = point_label_anchor(d, q);
        out.push(text_rect(at, q.label.chars().count(), 11.0 * q.scale, align, base));
    }
    let spots = label_spots(d);
    for l in &d.links {
        if let (Some(&(c, _)), Some((w, h))) = (spots.get(&l.id), label_box(l)) {
            out.push(Rect { x: c.x - w / 2.0, y: c.y - h / 2.0, w, h });
        }
    }
    let dirs = port_link_dirs(d);
    for dev in &d.devices {
        for p in dev.ports.iter().filter(|p| dev.name_shown(p)) {
            let (at, align, base) = port_label(dev, p, &dirs);
            out.push(text_rect(at, p.name.chars().count(), 10.0 * dev.scale, align, base));
        }
        // A long name runs over the sides of its box.
        let name = text_rect(Pt::new(dev.x + dev.w / 2.0, dev.y + dev.h / 2.0), dev.name.chars().count(), 13.0 * dev.scale, "center", "middle");
        if name.w > dev.w {
            out.push(name);
        }
    }
    out
}

/// Directions in which links leave each port.
pub fn port_link_dirs(d: &Diagram) -> std::collections::HashMap<Id, Vec<Pt>> {
    let mut m: std::collections::HashMap<Id, Vec<Pt>> = std::collections::HashMap::new();
    for l in &d.links {
        let Some(pts) = link_route(d, l) else { continue };
        let n = pts.len();
        if n < 2 {
            continue;
        }
        let k = 1;
        if !l.from.is_point() {
            m.entry(l.from.port).or_default().push(pts[k].sub(pts[0]));
        }
        if !l.to.is_point() {
            m.entry(l.to.port).or_default().push(pts[n - 1 - k].sub(pts[n - 1]));
        }
    }
    m
}

// Colors

pub fn parse_hex(c: &str) -> Option<(u8, u8, u8)> {
    let h = c.trim().strip_prefix('#')?;
    let v = match h.len() {
        6 => u32::from_str_radix(h, 16).ok()?,
        3 => {
            let s: String = h.chars().flat_map(|ch| [ch, ch]).collect();
            u32::from_str_radix(&s, 16).ok()?
        }
        _ => return None,
    };
    Some(((v >> 16) as u8, (v >> 8) as u8, v as u8))
}

pub fn normalize_color(c: &str) -> Option<String> {
    parse_hex(c).map(|(r, g, b)| format!("#{r:02x}{g:02x}{b:02x}"))
}

/// Darker shade for device borders.
pub fn darken(c: &str, k: f64) -> String {
    let (r, g, b) = parse_hex(c).unwrap_or((200, 200, 200));
    let f = |v: u8| ((v as f64) * (1.0 - k)).round().clamp(0.0, 255.0) as u8;
    format!("#{:02x}{:02x}{:02x}", f(r), f(g), f(b))
}

/// Black or white text, whichever reads better on `c`.
pub fn text_on(c: &str) -> &'static str {
    let (r, g, b) = parse_hex(c).unwrap_or((255, 255, 255));
    let lum = 0.2126 * r as f64 + 0.7152 * g as f64 + 0.0722 * b as f64;
    if lum > 140.0 { "#0f172a" } else { "#ffffff" }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether two axis-aligned routes cross (meet away from their ends).
    fn routes_cross(p: &[Pt], q: &[Pt]) -> bool {
        let inside = |v: f64, a: f64, b: f64| v > a.min(b) + 0.5 && v < a.max(b) - 0.5;
        p.windows(2).any(|s| {
            q.windows(2).any(|t| {
                let (s_h, t_h) = ((s[0].y - s[1].y).abs() < 1e-6, (t[0].y - t[1].y).abs() < 1e-6);
                match (s_h, t_h) {
                    (true, false) => inside(t[0].x, s[0].x, s[1].x) && inside(s[0].y, t[0].y, t[1].y),
                    (false, true) => inside(s[0].x, t[0].x, t[1].x) && inside(t[0].y, s[0].y, s[1].y),
                    // Running along each other on the same line.
                    (true, true) => (s[0].y - t[0].y).abs() < 0.5 && s[0].x.max(s[1].x).min(t[0].x.max(t[1].x)) - s[0].x.min(s[1].x).max(t[0].x.min(t[1].x)) > 0.5,
                    (false, false) => (s[0].x - t[0].x).abs() < 0.5 && s[0].y.max(s[1].y).min(t[0].y.max(t[1].y)) - s[0].y.min(s[1].y).max(t[0].y.min(t[1].y)) > 0.5,
                }
            })
        })
    }

    #[test]
    fn a_line_standing_in_for_hidden_devices_is_routed_clear_of_the_rest() {
        // A switch cabled through a patch panel (on a layer of its own) to
        // a server, straight down; a NAS stands where the line would go
        // with the panel hidden.
        let mut d = Diagram::default();
        let dev = |d: &mut Diagram, x: f64, y: f64| d.add_device("d", "server", Rect { x, y, w: 100.0, h: 40.0 }, "#eee");
        let sw = dev(&mut d, 0.0, 0.0);
        let pp = dev(&mut d, 0.0, 200.0);
        let srv = dev(&mut d, 0.0, 400.0);
        let nas = dev(&mut d, 0.0, 300.0);
        let panels = d.add_layer("Patch panels");
        d.device_mut(pp).unwrap().layer = panels;
        let port = |d: &mut Diagram, id: Id, side: Side| d.add_port(id, side, 0.5).unwrap();
        let (a, p1, p2, b) = (port(&mut d, sw, Side::Bottom), port(&mut d, pp, Side::Top), port(&mut d, pp, Side::Bottom), port(&mut d, srv, Side::Top));
        let l1 = d.add_link(a, p1, LinkStyle::default());
        let l2 = d.add_link(p2, b, LinkStyle::default());
        d.traces = vec![Trace { links: vec![l1, l2] }];
        // Shown: the panel, with the NAS beside the cable's way (moved out of it).
        d.device_mut(nas).unwrap().x = 300.0;
        assert_eq!(d.visible_only().links.len(), 2);
        // The NAS in the way, the panel hidden: one line, around the NAS.
        d.device_mut(nas).unwrap().x = 0.0;
        d.layer_mut(panels).unwrap().visible = false;
        let shown = d.visible_only();
        let through: Vec<&Link> = shown.links.iter().filter(|l| l.from == a && l.to == b).collect();
        assert_eq!(through.len(), 1);
        assert_ne!(through[0].style.routing, Routing::Straight, "rerouted");
        let route = link_route(&shown, through[0]).unwrap();
        assert!(!crosses(&route, &[shown.device(nas).unwrap().rect()]), "clear of the NAS: {route:?}");
        // Without anything in the way it stays as its cable is.
        d.device_mut(nas).unwrap().x = 300.0;
        let shown = d.visible_only();
        assert_eq!(shown.links.iter().find(|l| l.from == a && l.to == b).unwrap().style.routing, Routing::Straight);
    }

    #[test]
    fn orthogonal_lines_side_by_side_get_lanes_of_their_own() {
        // A patch panel's four ports close together at its bottom, cabled
        // to four servers spread out below (one to the left, three right).
        let mut d = Diagram::default();
        let pp = d.add_device("pp", "patch panel", Rect { x: 100.0, y: 0.0, w: 400.0, h: 40.0 }, "#eee");
        let ortho = LinkStyle { routing: Routing::Orthogonal, ..Default::default() };
        let mut links = vec![];
        for (k, x) in [0.0, 200.0, 400.0, 600.0].into_iter().enumerate() {
            let srv = d.add_device("srv", "server", Rect { x, y: 200.0, w: 100.0, h: 50.0 }, "#eee");
            let a = d.add_port(pp, Side::Bottom, 0.3 + 0.04 * k as f64).unwrap();
            let b = d.add_port(srv, Side::Top, 0.5).unwrap();
            links.push(d.add_link(a, b, ortho.clone()));
        }
        let routes: Vec<Vec<Pt>> = links.iter().map(|id| link_route(&d, d.link(*id).unwrap()).unwrap()).collect();
        let mids: Vec<f64> = routes.iter().map(|r| r[2].y).collect();
        // Those whose stretches overlap (the three to the right) are a lane apart.
        assert!((mids[2] - mids[3]).abs() >= 9.0 && (mids[1] - mids[2]).abs() + (mids[1] - mids[3]).abs() >= 9.0, "{mids:?}");
        assert!(mids.iter().all(|m| *m > 40.0 + STUB && *m < 200.0 - STUB), "between the stubs: {mids:?}");
        for i in 0..routes.len() {
            for j in i + 1..routes.len() {
                assert!(!routes_cross(&routes[i], &routes[j]), "lines {i} and {j} cross: {routes:?}");
            }
        }
        // On its own, a line keeps its stretch halfway.
        d.links.retain(|l| l.id == links[1]);
        assert_eq!(link_route(&d, d.link(links[1]).unwrap()).unwrap()[2].y, (40.0 + STUB + 200.0 - STUB) / 2.0);
    }

    #[test]
    fn orthogonal_routes_are_axis_aligned() {
        for sa in Side::ALL {
            for sb in Side::ALL {
                let pts = route(Pt::new(0.0, 0.0), sa, Pt::new(200.0, 130.0), Some(sb), Routing::Orthogonal);
                for w in pts.windows(2) {
                    let dx = (w[0].x - w[1].x).abs();
                    let dy = (w[0].y - w[1].y).abs();
                    assert!(dx < 1e-9 || dy < 1e-9, "{sa:?}->{sb:?}: diagonal segment {w:?}");
                }
                assert_eq!(pts[0], Pt::new(0.0, 0.0));
                assert_eq!(*pts.last().unwrap(), Pt::new(200.0, 130.0));
            }
        }
    }

    #[test]
    fn arrow_trims_line() {
        let style = LinkStyle { head_end: Head::Arrow, ..Default::default() };
        let g = link_geometry(&[Pt::new(0.0, 0.0), Pt::new(100.0, 0.0)], &style, 1.0);
        assert!(g.line[1].x < 100.0);
        assert_eq!(g.heads.len(), 1);
    }

    #[test]
    fn tunnel_ends_are_cut_along_the_device_border() {
        let mut d = Diagram::default();
        let dev = d.add_device("fw", "", Rect { x: 0.0, y: 0.0, w: 100.0, h: 60.0 }, "#ffffff");
        let port = d.add_port(dev, Side::Right, 0.5).unwrap();
        // Leaves the right border (x = 100) at about 34 degrees, to a free point.
        let q = d.add_point(Pt::new(300.0, 160.0));
        let id = d.add_link(port, Endpoint::point(q), LinkStyle { kind: LinkKind::Ipsec, ..Default::default() });
        let l = d.link(id).unwrap().clone();
        let route = link_route(&d, &l).unwrap();
        let band = tunnel_band(&d, &l, &route, 7.0);
        assert_eq!(band.len(), 4);
        // Both corners at the device end lie on its border...
        assert!((band[0].x - 100.0).abs() < 1e-9 && (band[3].x - 100.0).abs() < 1e-9, "{band:?}");
        assert!((band[0].y - band[3].y).abs() > 14.0, "the cut is slanted, wider than the band");
        // ...while the end at the point is square to the line, 14 wide.
        assert!((band[1].dist(band[2]) - 14.0).abs() < 1e-9);
    }

    #[test]
    fn routed_lines_get_a_circle_near_each_end() {
        let style = |kind| LinkStyle { kind, ..Default::default() };
        let route = [Pt::new(0.0, 0.0), Pt::new(0.0, 50.0), Pt::new(100.0, 50.0)];
        for kind in [LinkKind::L3, LinkKind::L3Sub] {
            let g = link_geometry(&route, &style(kind), 1.0);
            assert_eq!(g.beads, vec![Pt::new(0.0, BEAD_OFFSET), Pt::new(100.0 - BEAD_OFFSET, 50.0)], "{kind:?}");
            assert!(kind.is_double() == (kind == LinkKind::L3Sub));
        }
        assert!(link_geometry(&route, &style(LinkKind::Plain), 1.0).beads.is_empty());
        assert_eq!(link_geometry(&route, &style(LinkKind::L2), 1.0).beads.len(), 2, "switched ports too");
        // Too short to hold two circles apart: none.
        assert!(link_geometry(&[Pt::new(0.0, 0.0), Pt::new(30.0, 0.0)], &style(LinkKind::L3), 1.0).beads.is_empty());
    }

    #[test]
    fn colors() {
        assert_eq!(normalize_color("#ABC").as_deref(), Some("#aabbcc"));
        assert_eq!(normalize_color("nope"), None);
        assert_eq!(text_on("#000000"), "#ffffff");
        assert_eq!(text_on("#ffffff"), "#0f172a");
    }

    #[test]
    fn segment_rect_hits() {
        let r = Rect { x: 0.0, y: 0.0, w: 100.0, h: 50.0 };
        assert!(segment_hits_rect(Pt::new(-10.0, 25.0), Pt::new(110.0, 25.0), &r));
        assert!(segment_hits_rect(Pt::new(-10.0, -10.0), Pt::new(50.0, 25.0), &r), "ends inside");
        assert!(!segment_hits_rect(Pt::new(-10.0, 0.0), Pt::new(110.0, 0.0), &r), "along the border");
        assert!(!segment_hits_rect(Pt::new(50.0, 50.0), Pt::new(50.0, 90.0), &r), "leaving from the border");
        assert!(!segment_hits_rect(Pt::new(-10.0, 60.0), Pt::new(110.0, 120.0), &r));
    }

    #[test]
    fn routes_around_a_box_in_the_way() {
        // a's right side to c's left side, with b right in between.
        let a = Rect { x: 0.0, y: 0.0, w: 120.0, h: 60.0 };
        let b = Rect { x: 200.0, y: 0.0, w: 120.0, h: 60.0 };
        let c = Rect { x: 400.0, y: 0.0, w: 120.0, h: 60.0 };
        let obstacles = [a, b, c];
        let (from, to) = (Pt::new(120.0, 30.0), Pt::new(400.0, 30.0));
        assert!(crosses(&[from, to], &obstacles), "the straight line goes through b");
        let r = route_around(from, Side::Right, to, Side::Left, &obstacles).unwrap();
        assert!(!crosses(&r, &obstacles), "{r:?}");
        assert_eq!((r[0], *r.last().unwrap()), (from, to));
        // Orthogonal, and only the few bends needed to go over or under b.
        assert!(r.windows(2).all(|w| w[0].x == w[1].x || w[0].y == w[1].y));
        assert!(r.len() <= 8, "{r:?}");
        // Clear of b by the clearance.
        let gap = r.iter().map(|p| if p.y < 0.0 { -p.y } else { p.y - 60.0 }).fold(f64::MIN, f64::max);
        assert!(gap >= CLEARANCE - 0.01, "{r:?}");
        // Leaving downward from a's bottom and entering c from the top works too.
        let r = route_around(Pt::new(60.0, 60.0), Side::Bottom, Pt::new(460.0, 0.0), Side::Top, &obstacles).unwrap();
        assert!(!crosses(&r, &obstacles), "{r:?}");
    }
}
