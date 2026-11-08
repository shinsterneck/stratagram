// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

import QtQuick
import QtQuick.Controls
import com.stratagram

// Renders the scene computed in Rust and forwards mouse input in world
// coordinates. Pan: middle/right drag or Space+drag. Zoom: mouse wheel.
Item {
    id: root

    required property DiagramController ctl

    property real zoom: 1.0
    property real panX: 40
    property real panY: 40
    property var scene: JSON.parse(ctl.scene)

    property bool panning: false
    property bool spaceHeld: false
    property real panStartX: 0
    property real panStartY: 0
    property real panOrigX: 0
    property real panOrigY: 0
    // While true the view refits whenever the canvas is resized, so a fit
    // requested before the window has its final size still lands correctly.
    // Cleared as soon as the user pans or zooms.
    property bool autoFit: false
    // Grid lines in the background (snapping works either way).
    property bool showGrid: true
    onShowGridChanged: grid.requestPaint()
    // Deep zoom: no zoom limit; the grid follows the zoom level.
    property bool deepZoom: false
    onDeepZoomChanged: { if (!deepZoom) zoomBy(1); repaintAll() }
    // World size of one grid step at this zoom (finer at deeper levels).
    // Same rule as the level of new items (Editor::level).
    readonly property real gridLevel: deepZoom ? Math.pow(8, -Math.floor(Math.log(zoom / 0.75) / Math.log(8) + 1e-9)) : 1

    // The theme's colors around the diagram (scene.ui, see core/src/theme.rs);
    // the diagram's own colors come translated already.
    readonly property var ui: scene.ui || ({})
    readonly property color background: ui.canvas || "#f8fafc"
    readonly property color accent: ui.accent || "#2563eb"
    readonly property color good: ui.good || "#16a34a"
    readonly property string paper: ui.paper || "#ffffff"
    readonly property string ink: ui.ink || "#334155"
    readonly property string muted: ui.muted || "#475569"
    readonly property var gridColors: ui.grid || ["#eef2f6", "#e1e7ef"]
    readonly property bool glowing: ui.glow === true
    readonly property bool darkTheme: ui.dark === true
    onBackgroundChanged: grid.requestPaint()

    // Neon: lines and outlines glow in their own color.
    function glow(ctx, color, on) {
        if (!glowing) return
        ctx.shadowColor = on ? color : "transparent"
        ctx.shadowBlur = on ? 7 : 0
    }

    clip: true
    focus: true

    // What is big enough to show depends on the zoom.
    onZoomChanged: { ctl.zoom = zoom; ctl.viewChanged(); repaintAll() }
    onPanXChanged: repaintAll()
    onPanYChanged: repaintAll()
    onWidthChanged: { if (autoFit) fit(); repaintAll() }
    onHeightChanged: { if (autoFit) fit(); repaintAll() }
    onSceneChanged: diagram.requestPaint()

    function repaintAll() { grid.requestPaint(); diagram.requestPaint() }

    function toWorld(mx, my) {
        return Qt.point((mx - panX) / zoom, (my - panY) / zoom)
    }

    function viewCenter() {
        return toWorld(width / 2, height / 2)
    }

    function zoomAt(factor, mx, my) {
        autoFit = false
        const nz = deepZoom ? Math.max(1e-3, Math.min(1e7, zoom * factor)) : Math.max(0.2, Math.min(4.0, zoom * factor))
        const w = toWorld(mx, my)
        zoom = nz
        panX = mx - w.x * nz
        panY = my - w.y * nz
    }

    function zoomBy(factor) { zoomAt(factor, width / 2, height / 2) }

    function fit() {
        autoFit = true
        const b = scene.bounds
        if (!b) { zoom = 1; panX = 40; panY = 40; return }
        const m = 60
        if (width <= 2 * m || height <= 2 * m) return  // not laid out yet
        let z = Math.min((width - 2 * m) / Math.max(b.w, 1), (height - 2 * m) / Math.max(b.h, 1), 1.5)
        z = Math.max(0.2, z)
        zoom = z
        panX = (width - b.w * z) / 2 - b.x * z
        panY = (height - b.h * z) / 2 - b.y * z
    }

    // Pan (keeping the zoom) so the selected devices are in view.
    function revealSelection() {
        const sel = scene.devices.filter(d => d.sel)
        if (sel.length === 0) return
        const x0 = Math.min(...sel.map(d => d.x)), y0 = Math.min(...sel.map(d => d.y))
        const x1 = Math.max(...sel.map(d => d.x + d.w)), y1 = Math.max(...sel.map(d => d.y + d.h))
        const m = 30
        if (x0 * zoom + panX >= m && y0 * zoom + panY >= m &&
            x1 * zoom + panX <= width - m && y1 * zoom + panY <= height - m) return
        autoFit = false
        panX = width / 2 - (x0 + x1) / 2 * zoom
        panY = height / 2 - (y0 + y1) / 2 * zoom
    }

    // grid
    Canvas {
        id: grid
        anchors.fill: parent
        onPaint: {
            const ctx = getContext("2d")
            ctx.reset()
            ctx.fillStyle = root.background
            ctx.fillRect(0, 0, width, height)
            const step = 10 * root.gridLevel * root.zoom
            if (!root.showGrid || step < 5) return
            const x0 = ((root.panX % step) + step) % step
            const y0 = ((root.panY % step) + step) % step
            const majorEvery = 5
            const ix0 = Math.round((x0 - root.panX) / step)
            const iy0 = Math.round((y0 - root.panY) / step)
            ctx.lineWidth = 1
            for (let pass = 0; pass < 2; pass++) {
                ctx.beginPath()
                ctx.strokeStyle = root.gridColors[pass]
                for (let x = x0, i = ix0; x < width; x += step, i++) {
                    const major = (((i % majorEvery) + majorEvery) % majorEvery) === 0
                    if (major === (pass === 1)) { ctx.moveTo(Math.round(x) + 0.5, 0); ctx.lineTo(Math.round(x) + 0.5, height) }
                }
                for (let y = y0, j = iy0; y < height; y += step, j++) {
                    const major = (((j % majorEvery) + majorEvery) % majorEvery) === 0
                    if (major === (pass === 1)) { ctx.moveTo(0, Math.round(y) + 0.5); ctx.lineTo(width, Math.round(y) + 0.5) }
                }
                ctx.stroke()
            }
        }
    }

    // diagram
    Canvas {
        id: diagram
        anchors.fill: parent
        onPaint: root.paintScene(getContext("2d"))
        onImageLoaded: requestPaint()
    }

    function polyline(ctx, pts) {
        ctx.beginPath()
        ctx.moveTo(pts[0], pts[1])
        for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1])
    }

    // Qt's Context2D measures dash lengths in multiples of the line width;
    // take pixel lengths (as in SVG) and convert.
    function dashPx(ctx, pattern, lineWidth) {
        ctx.setLineDash(pattern.map(v => v / Math.max(lineWidth, 0.01)))
    }

    // Dashes for a line `w` wide, stroked `lw` wide (a double line is wider).
    function applyDash(ctx, style, w, lw) {
        lw = lw === undefined ? w : lw
        if (style === "dashed") dashPx(ctx, [4 * w + 2, 2.5 * w + 2], lw)
        else if (style === "dotted") dashPx(ctx, [w, 1.5 * w + 1], lw)
        else ctx.setLineDash([])
    }

    function drawHeads(ctx, heads, color, width) {
        ctx.setLineDash([])
        for (const h of heads) {
            if (h.t === "circle") {
                ctx.beginPath()
                ctx.arc(h.x, h.y, h.r, 0, 2 * Math.PI)
                ctx.fillStyle = root.paper
                ctx.fill()
                ctx.lineWidth = Math.max(1.5, width)
                ctx.strokeStyle = color
                ctx.stroke()
            } else if (h.t === "fill") {
                polyline(ctx, h.pts)
                ctx.closePath()
                ctx.fillStyle = color
                ctx.fill()
                ctx.lineWidth = 1
                ctx.strokeStyle = color
                ctx.stroke()
            } else {
                polyline(ctx, h.pts)
                ctx.lineWidth = width
                ctx.lineCap = "round"
                ctx.strokeStyle = color
                ctx.stroke()
                ctx.lineCap = "butt"
            }
        }
    }

    // Trace path data made of M, L, C and Z commands (absolute coordinates).
    function tracePath(ctx, data) {
        const t = data.split(" ")
        ctx.beginPath()
        for (let i = 0; i < t.length;) {
            const c = t[i++]
            if (c === "M") { ctx.moveTo(+t[i], +t[i + 1]); i += 2 }
            else if (c === "L") { ctx.lineTo(+t[i], +t[i + 1]); i += 2 }
            else if (c === "C") { ctx.bezierCurveTo(+t[i], +t[i + 1], +t[i + 2], +t[i + 3], +t[i + 4], +t[i + 5]); i += 6 }
            else if (c === "Z") ctx.closePath()
        }
    }

    function roundRect(ctx, x, y, w, h, r) {
        ctx.beginPath()
        ctx.moveTo(x + r, y)
        ctx.arcTo(x + w, y, x + w, y + h, r)
        ctx.arcTo(x + w, y + h, x, y + h, r)
        ctx.arcTo(x, y + h, x, y, r)
        ctx.arcTo(x, y, x + w, y, r)
        ctx.closePath()
    }

    // Draw with `draw` in a frame scaled by `sc` around (ox, oy).
    function inFrame(ctx, ox, oy, sc, draw) {
        if (sc === 1) { draw(); return }
        ctx.save()
        ctx.translate(ox, oy)
        ctx.scale(sc, sc)
        ctx.translate(-ox, -oy)
        draw()
        ctx.restore()
    }

    // A device's coordinates in its own frame (see inFrame).
    // In deep zoom, decorations stop growing past 1.5x their size.
    function capFor(sc) { return deepZoom ? Math.min(1, 1.5 / (sc * zoom)) : 1 }

    function localDevice(d) {
        const sc = d.s
        const k = capFor(sc)
        if (sc === 1) return k === 1 ? d : Object.assign({}, d, { k: k })
        const lx = (x) => d.x + (x - d.x) / sc, ly = (y) => d.y + (y - d.y) / sc
        const ports = d.ports.map(p => Object.assign({}, p, { x: lx(p.x), y: ly(p.y), lx: lx(p.lx), ly: ly(p.ly) }))
        return Object.assign({}, d, { w: d.w / sc, h: d.h / sc, ports: ports, k: k })
    }

    function localNote(n) {
        return n.s === 1 ? n : Object.assign({}, n, { size: n.size / n.s, lh: n.lh / n.s, w: n.w / n.s, h: n.h / n.s })
    }

    function drawLink(ctx, l) {
        if (l.pts.length < 4) return
        // Tunnels and what carries the line: tinted bands, outermost first
        // (the tints add up inward), their ends cut along the device
        // borders; the edge solid, or dashed by type (VXLAN, CAPWAP, SD-WAN).
        for (let i = 0; i < l.bands.length; i++) {
            const b = l.bands[i]
            if (b.pts.length < 6) continue
            polyline(ctx, b.pts)
            ctx.closePath()
            if (i === 0) {
                ctx.fillStyle = root.paper
                ctx.fill()
            }
            const base = ctx.globalAlpha
            ctx.globalAlpha = base * 0.14
            ctx.fillStyle = l.color
            ctx.fill()
            ctx.globalAlpha = base
            if (b.dash) dashPx(ctx, b.dash, l.s)
            else ctx.setLineDash([])
            ctx.lineWidth = l.s
            ctx.strokeStyle = l.color
            ctx.stroke()
            ctx.setLineDash([])
        }
        if (l.sel || l.drop) {
            // Selected, or where the dragged device would be inserted: over
            // the bands, as wide as the outermost.
            ctx.setLineDash([])
            polyline(ctx, l.pts)
            ctx.lineWidth = Math.max(l.width, l.bandW) + (l.drop ? 12 : 8) * l.s
            ctx.strokeStyle = l.drop ? "rgba(22,163,74,0.45)" : "rgba(37,99,235,0.38)"
            ctx.stroke()
        }
        polyline(ctx, l.pts)
        applyDash(ctx, l.style, l.width)
        if (l.kind === "trunk" || l.kind === "l3sub") {
            // A double line: two strokes of the line's width with a gap.
            ctx.lineWidth = 2 * l.width + 3 * l.s
            applyDash(ctx, l.style, l.width, ctx.lineWidth)
            ctx.strokeStyle = l.color
            glow(ctx, l.color, true)
            ctx.stroke()
            glow(ctx, l.color, false)
            ctx.lineWidth = 3 * l.s
            applyDash(ctx, l.style, l.width, ctx.lineWidth)
            ctx.strokeStyle = root.paper
            ctx.stroke()
        } else {
            ctx.lineWidth = l.width
            ctx.strokeStyle = l.color
            glow(ctx, l.color, true)
            ctx.stroke()
            glow(ctx, l.color, false)
        }
        if (l.ticks.length > 0) {
            // MPLS: strokes across the line, like a label stack.
            ctx.setLineDash([])
            ctx.beginPath()
            for (let i = 0; i + 3 < l.ticks.length; i += 4) {
                ctx.moveTo(l.ticks[i], l.ticks[i + 1])
                ctx.lineTo(l.ticks[i + 2], l.ticks[i + 3])
            }
            ctx.lineWidth = Math.max(l.width * 0.75, 1 * l.s)
            ctx.strokeStyle = l.color
            ctx.stroke()
        }
        // Ends: open circles when routed (L3), filled squares when switched (L2).
        for (let i = 0; i + 1 < l.beads.length; i += 2) {
            ctx.setLineDash([])
            if (l.beadSquare) {
                // Switched (L2): filled squares for the ports.
                const r = l.beadR * 0.9
                ctx.fillStyle = l.color
                ctx.fillRect(l.beads[i] - r, l.beads[i + 1] - r, 2 * r, 2 * r)
                continue
            }
            ctx.beginPath()
            ctx.arc(l.beads[i], l.beads[i + 1], l.beadR, 0, 2 * Math.PI)
            ctx.fillStyle = root.paper
            ctx.fill()
            ctx.lineWidth = Math.max(1.2 * l.s, l.width * 0.7)
            ctx.strokeStyle = l.color
            ctx.stroke()
        }
        drawHeads(ctx, l.heads, l.color, l.width)
        const label = l.editing ? "" : l.label
        if (label !== "" || (l.tag !== "" && !l.editing)) {
            // The type's tag in bold, then the label.
            inFrame(ctx, l.lx, l.ly, l.s, () => {
                ctx.font = "bold 10px sans-serif"
                const tagw = l.tag !== "" ? ctx.measureText(l.tag).width : 0
                ctx.font = "11px sans-serif"
                const labw = label !== "" ? ctx.measureText(label).width : 0
                const gap = tagw > 0 && labw > 0 ? 6 : 0
                const tw = tagw + gap + labw + 10
                const x0 = l.lx - tw / 2
                roundRect(ctx, x0, l.ly - 8.5, tw, 17, 3)
                ctx.fillStyle = root.paper
                ctx.fill()
                ctx.lineWidth = l.sel ? 1.5 : 0.8
                ctx.strokeStyle = l.sel ? accent : l.color
                ctx.setLineDash([])
                ctx.stroke()
                ctx.fillStyle = l.color
                ctx.textAlign = "left"
                ctx.textBaseline = "middle"
                if (tagw > 0) {
                    ctx.font = "bold 10px sans-serif"
                    ctx.fillText(l.tag, x0 + 5, l.ly + 0.5)
                }
                ctx.font = "11px sans-serif"
                if (labw > 0) ctx.fillText(label, x0 + 5 + tagw + gap, l.ly + 0.5)
            })
        }
        ctx.setLineDash([])
    }

    function drawDevice(ctx, d, target) {
        // k shrinks decorations (text, borders, ports) of a box you are
        // zoomed far into, so they stay readable instead of huge.
        const k = d.k || 1
        // The outline depends on the kind (see src/shape.rs).
        tracePath(ctx, d.shape)
        if (d.sel) {
            ctx.lineWidth = 8 * k
            ctx.strokeStyle = "rgba(37,99,235,0.2)"
            ctx.stroke()
        }
        ctx.fillStyle = d.fill
        ctx.fill()
        // Faint inner lines (bricks, globe...), left out when zoomed into it.
        if (d.detail !== "" && k === 1) {
            tracePath(ctx, d.detail)
            const base = ctx.globalAlpha
            ctx.globalAlpha = base * 0.3
            ctx.lineWidth = 1
            ctx.strokeStyle = d.stroke
            ctx.stroke()
            ctx.globalAlpha = base
            tracePath(ctx, d.shape)
        }
        // The symbol identifying its kind (router arrows, switch ports...).
        if (k === 1 && (d.mark !== "" || d.solid !== "")) {
            ctx.save()
            ctx.strokeStyle = d.stroke
            ctx.fillStyle = d.stroke
            ctx.lineCap = "round"
            ctx.lineJoin = "round"
            if (d.mark !== "") {
                tracePath(ctx, d.mark)
                ctx.lineWidth = 1.8
                ctx.stroke()
            }
            if (d.solid !== "") {
                tracePath(ctx, d.solid)
                ctx.fill()
                ctx.lineWidth = 0.6
                ctx.stroke()
            }
            ctx.restore()
            tracePath(ctx, d.shape)
        }
        // Small texts of the shape (a patch panel's port numbers).
        if (k === 1 && d.texts && d.texts.length > 0) {
            ctx.save()
            ctx.fillStyle = d.stroke
            // A halo in the device's color keeps a number readable over a cord.
            ctx.strokeStyle = d.fill
            ctx.lineJoin = "round"
            ctx.textAlign = "center"
            ctx.textBaseline = "middle"
            for (const t of d.texts) {
                ctx.font = t.size + "px sans-serif"
                ctx.lineWidth = t.size * 0.4
                ctx.strokeText(t.text, t.x, t.y)
                ctx.fillText(t.text, t.x, t.y)
            }
            ctx.restore()
            tracePath(ctx, d.shape)
        }
        // A cloud provider's tile with its monogram.
        if (k === 1 && d.tile) {
            const t = d.tile
            ctx.save()
            roundRect(ctx, t.x, t.y, t.size, t.size, t.size * 0.22)
            ctx.fillStyle = t.bg
            ctx.fill()
            ctx.fillStyle = t.fg
            ctx.font = "bold " + t.font + "px sans-serif"
            ctx.textAlign = "center"
            ctx.textBaseline = "middle"
            ctx.fillText(t.text, t.x + t.size / 2, t.y + t.size / 2 + t.font * 0.04)
            ctx.restore()
            tracePath(ctx, d.shape)
        }
        ctx.lineWidth = (d.sel ? 2 : 1.5) * k
        ctx.strokeStyle = d.sel ? accent : d.stroke
        glow(ctx, d.stroke, true)
        ctx.stroke()
        glow(ctx, d.stroke, false)
        if (target === d.id) {
            roundRect(ctx, d.x - 3 * k, d.y - 3 * k, d.w + 6 * k, d.h + 6 * k, 7 * k)
            ctx.lineWidth = 2 * k
            dashPx(ctx, [5 * k, 4 * k], 2 * k)
            ctx.strokeStyle = good
            ctx.stroke()
            ctx.setLineDash([])
        }

        ctx.fillStyle = d.text
        if (k < 1) {
            // Zoomed into it: its name as a small header in the corner.
            ctx.textAlign = "left"
            ctx.textBaseline = "top"
            ctx.font = "bold " + (13 * k) + "px sans-serif"
            if (!d.editing) ctx.fillText(d.name + (d.kind !== "" ? "  ·  " + d.kindLabel.toUpperCase() : ""), d.x + 10 * k, d.y + 8 * k)
        } else {
            ctx.textAlign = "center"
            ctx.textBaseline = "middle"
            // A device with contents has its name at the top, out of their way.
            const cx = d.x + d.w / 2 + d.ldx
            const cy = d.container ? d.y + 14 + (d.kind === "" ? 0 : 6) : d.y + d.h / 2 + d.ldy
            ctx.font = "bold 13px sans-serif"
            if (d.kind === "") {
                if (!d.editing) ctx.fillText(d.name, cx, cy)
            } else {
                if (!d.editing) ctx.fillText(d.name, cx, cy - 6)
                ctx.font = "10px sans-serif"
                const base = ctx.globalAlpha
                ctx.globalAlpha = base * 0.7
                ctx.fillText(d.kindLabel.toUpperCase(), cx, cy + 9)
                ctx.globalAlpha = base
            }
        }

        // Layer name in the lower right corner, when its layer shows names.
        if (d.badge !== "") {
            ctx.font = (9 * k) + "px sans-serif"
            ctx.fillStyle = d.text
            const base = ctx.globalAlpha
            ctx.globalAlpha = base * 0.55
            ctx.textAlign = "right"
            ctx.textBaseline = "bottom"
            ctx.fillText(d.badge, d.x + d.w - 4 * k, d.y + d.h - 3 * k)
            ctx.globalAlpha = base
        }

        for (const p of d.ports) {
            // Selected, or at an end of the selected line: in the accent color.
            const on = p.sel || p.lit
            const r = (p.hot || on ? 5.5 : 4) * k
            if (on || p.hot) {
                ctx.beginPath()
                ctx.arc(p.x, p.y, (p.lit && !p.sel ? 10 : 9) * k, 0, 2 * Math.PI)
                ctx.fillStyle = on ? "rgba(37,99,235,0.25)" : "rgba(245,158,11,0.25)"
                ctx.fill()
                if (p.lit) {
                    ctx.lineWidth = 1.5 * k
                    ctx.strokeStyle = accent
                    ctx.stroke()
                }
            }
            ctx.fillStyle = on ? accent : (p.linked ? d.stroke : root.paper)
            ctx.fillRect(p.x - r, p.y - r, 2 * r, 2 * r)
            ctx.lineWidth = 1.2 * k
            ctx.strokeStyle = on ? accent : d.stroke
            ctx.strokeRect(p.x - r, p.y - r, 2 * r, 2 * r)
            if (p.name !== "" && !p.editing) {
                ctx.font = (10 * k) + "px sans-serif"
                ctx.fillStyle = root.muted
                ctx.textAlign = p.align
                ctx.textBaseline = p.base
                ctx.fillText(p.name, p.lx, p.ly)
            }
        }
    }

    // A free connection point: a small ring, only on screen (exports show
    // just the line ending there).
    function drawPoint(ctx, p, target) {
        const hot = target === p.id
        const k = capFor(p.s)
        if (p.sel || hot) {
            ctx.beginPath()
            ctx.arc(p.x, p.y, 9 * k, 0, 2 * Math.PI)
            ctx.fillStyle = hot ? "rgba(22,163,74,0.25)" : "rgba(37,99,235,0.25)"
            ctx.fill()
        }
        ctx.setLineDash([])
        ctx.beginPath()
        ctx.arc(p.x, p.y, 4 * k, 0, 2 * Math.PI)
        ctx.fillStyle = p.sel ? accent : root.paper
        ctx.fill()
        ctx.lineWidth = 1.5 * k
        ctx.strokeStyle = hot ? good : (p.sel ? accent : root.muted)
        ctx.stroke()
        // Its label, beyond the end of its line.
        if (p.label !== "" && !p.editing) {
            ctx.font = "11px sans-serif"
            ctx.fillStyle = root.ink
            ctx.textAlign = p.align
            ctx.textBaseline = p.base
            ctx.fillText(p.label, p.x + p.ldx, p.y + p.ldy)
        }
    }

    // `text` broken into lines no wider than `maxW` (new lines kept; words
    // longer than a line are split), with the context's current font.
    function wrapText(ctx, text, maxW) {
        const out = []
        for (const para of text.split("\n")) {
            let line = ""
            for (let word of para.split(" ")) {
                while (ctx.measureText(word).width > maxW && word.length > 1) {
                    let n = word.length - 1
                    while (n > 1 && ctx.measureText(word.slice(0, n)).width > maxW) n--
                    if (line !== "") { out.push(line); line = "" }
                    out.push(word.slice(0, n))
                    word = word.slice(n)
                }
                const next = line === "" ? word : line + " " + word
                if (line !== "" && ctx.measureText(next).width > maxW) { out.push(line); line = word }
                else line = next
            }
            out.push(line)
        }
        return out
    }

    // Scroll position of boxed notes (by id, in the note's own units) and
    // what the last paint found about them, for the wheel and scroll bar.
    // View state only: files and exports show a note from its top.
    property var noteScroll: ({})
    property var noteInfo: ({})
    property var thumbDrag: null

    // A boxed note: wrapped to its box, scrolled within it, with a scroll
    // bar when the text is taller than the box.
    function drawBoxedNote(ctx, n) {
        // 8 screen pixels wide, at any zoom.
        const sbw = 9 / (n.s * root.zoom)
        let lines = wrapText(ctx, n.text, n.w)
        const over = lines.length * n.lh > n.h + 0.5
        if (over) lines = wrapText(ctx, n.text, Math.max(n.w - sbw, n.size))
        const contentH = lines.length * n.lh
        const maxOff = Math.max(0, contentH - n.h)
        const off = Math.min(maxOff, Math.max(0, noteScroll[n.id] || 0))
        noteScroll[n.id] = off
        noteInfo[n.id] = { x: n.x, y: n.y, w: n.w * n.s, h: n.h * n.s, s: n.s, sbw: sbw * n.s,
                           over: over, contentH: contentH, viewH: n.h, maxOff: maxOff }
        if (n.sel) {
            roundRect(ctx, n.x - 4, n.y - 3, n.w + 8, n.h + 6, 4)
            ctx.fillStyle = "rgba(37,99,235,0.12)"
            ctx.fill()
            ctx.lineWidth = 1.2
            ctx.strokeStyle = accent
            ctx.stroke()
        }
        if (n.editing) return
        ctx.save()
        ctx.beginPath()
        ctx.rect(n.x, n.y, n.w, n.h)
        ctx.clip()
        ctx.fillStyle = n.color
        ctx.textAlign = "left"
        ctx.textBaseline = "middle"
        for (let i = 0; i < lines.length; i++) {
            const y = n.y - off + (i + 0.5) * n.lh
            if (y > n.y - n.lh && y < n.y + n.h + n.lh) ctx.fillText(lines[i], n.x, y)
        }
        ctx.restore()
        if (over) {
            const tx = n.x + n.w - sbw + sbw * 0.2, tw = sbw * 0.6
            ctx.fillStyle = "rgba(100,116,139,0.15)"
            roundRect(ctx, tx, n.y, tw, n.h, tw / 2)
            ctx.fill()
            const th = Math.max(n.h * n.h / contentH, sbw * 2)
            const ty = n.y + (n.h - th) * (maxOff > 0 ? off / maxOff : 0)
            ctx.fillStyle = "rgba(71,85,105,0.65)"
            roundRect(ctx, tx, ty, tw, th, tw / 2)
            ctx.fill()
        }
    }

    // The boxed note whose text overflows under (x, y) in world units, and
    // whether the point is on its scroll bar.
    function scrollableNoteAt(x, y) {
        for (const id in noteInfo) {
            const i = noteInfo[id]
            if (i.over && x >= i.x && x <= i.x + i.w && y >= i.y && y <= i.y + i.h)
                return { id: id, info: i, onBar: x >= i.x + i.w - i.sbw }
        }
        return null
    }

    function scrollNote(id, by) {
        const i = noteInfo[id]
        noteScroll[id] = Math.min(i.maxOff, Math.max(0, (noteScroll[id] || 0) + by))
        diagram.requestPaint()
    }

    function drawNote(ctx, n) {
        ctx.font = (n.bold ? "bold " : "") + n.size + "px sans-serif"
        if (n.box) { drawBoxedNote(ctx, n); return }
        const lines = n.lines
        if (n.sel) {
            let w = 0
            for (const line of lines) w = Math.max(w, ctx.measureText(line).width)
            const bw = n.box ? n.w : Math.max(w, n.size)
            const bh = n.box ? n.h : lines.length * n.lh
            roundRect(ctx, n.x - 4, n.y - 3, bw + 8, bh + 6, 4)
            ctx.fillStyle = "rgba(37,99,235,0.12)"
            ctx.fill()
            ctx.lineWidth = 1.2
            ctx.strokeStyle = accent
            ctx.stroke()
        }
        if (n.editing) return
        ctx.fillStyle = n.color
        ctx.textAlign = "left"
        ctx.textBaseline = "middle"
        for (let i = 0; i < lines.length; i++)
            ctx.fillText(lines[i], n.x, n.y + (i + 0.5) * n.lh)
    }

    // Rectangles, lines and texts laid out by src/frame.rs.
    function drawFrame(ctx, prims) {
        ctx.setLineDash([])
        for (const p of prims) {
            if (p.t === "rect") {
                if (p.radius > 0) roundRect(ctx, p.x, p.y, p.w, p.h, p.radius)
                else { ctx.beginPath(); ctx.rect(p.x, p.y, p.w, p.h) }
                if (p.fill) { ctx.fillStyle = p.fill; ctx.fill() }
                if (p.stroke) {
                    ctx.lineWidth = p.width
                    ctx.strokeStyle = p.stroke
                    if (p.dash) dashPx(ctx, [4, 3], p.width)
                    ctx.stroke()
                    ctx.setLineDash([])
                }
            } else if (p.t === "line") {
                ctx.beginPath()
                ctx.moveTo(p.pts[0], p.pts[1])
                ctx.lineTo(p.pts[2], p.pts[3])
                ctx.lineWidth = p.width
                ctx.strokeStyle = p.color
                if (p.dash) dashPx(ctx, [5, 3], p.width)
                ctx.stroke()
                ctx.setLineDash([])
            } else if (p.t === "watermark") {
                ctx.save()
                ctx.translate(p.x, p.y)
                ctx.rotate(p.angle * Math.PI / 180)
                ctx.globalAlpha = ctx.globalAlpha * p.opacity
                ctx.font = "bold " + p.size + "px sans-serif"
                ctx.fillStyle = p.color
                ctx.textAlign = "center"
                ctx.textBaseline = "middle"
                ctx.fillText(p.text, 0, 0)
                ctx.restore()
            } else if (p.t === "image") {
                // The logo, a data URI: drawn once loaded (which repaints).
                if (diagram.isImageLoaded(p.data)) ctx.drawImage(p.data, p.x, p.y, p.w, p.h)
                else if (!diagram.isImageLoading(p.data) && !diagram.isImageError(p.data)) diagram.loadImage(p.data)
            } else {
                ctx.font = (p.bold ? "bold " : "") + p.size + "px sans-serif"
                ctx.fillStyle = p.color
                ctx.textAlign = p.align
                ctx.textBaseline = "middle"
                ctx.fillText(p.text, p.x, p.y)
            }
        }
    }

    function paintScene(ctx) {
        ctx.reset()
        ctx.clearRect(0, 0, width, height)
        const s = scene
        ctx.save()
        ctx.translate(panX, panY)
        ctx.scale(zoom, zoom)
        ctx.lineJoin = "round"

        // The frame (border, header and footer) under everything.
        if (s.frame) drawFrame(ctx, s.frame)
        noteInfo = ({})
        // Layer backgrounds, under everything on the layers.
        for (const bg of (s.backgrounds || [])) {
            if (bg.radius > 0) roundRect(ctx, bg.x, bg.y, bg.w, bg.h, bg.radius)
            else { ctx.beginPath(); ctx.rect(bg.x, bg.y, bg.w, bg.h) }
            ctx.globalAlpha = bg.opacity
            ctx.fillStyle = bg.fill
            ctx.fill()
            ctx.globalAlpha = 1
            if (bg.border !== "none") {
                ctx.lineWidth = bg.width
                ctx.strokeStyle = bg.stroke
                applyDash(ctx, bg.border, bg.width)
                ctx.stroke()
                ctx.setLineDash([])
            }
        }

        // Links, devices and notes back to front, in their global stacking
        // order (hidden layers are not in the scene).
        // Items deeper in (deep zoom) have a scale below 1: they are drawn
        // in a frame scaled down around their corner, and fade in as they
        // get big enough to read.
        for (const [kind, i] of s.order) {
            if (kind === "l") {
                const l = s.links[i]
                ctx.globalAlpha = l.a
                drawLink(ctx, l)
            } else if (kind === "d") {
                const d = s.devices[i]
                ctx.globalAlpha = d.a
                inFrame(ctx, d.x, d.y, d.s, () => drawDevice(ctx, localDevice(d), s.target))
            } else if (kind === "p") {
                const p = s.points[i]
                ctx.globalAlpha = p.a
                inFrame(ctx, p.x, p.y, p.s, () => drawPoint(ctx, p, s.target))
            } else {
                const n = s.notes[i]
                ctx.globalAlpha = n.a
                inFrame(ctx, n.x, n.y, n.s, () => drawNote(ctx, localNote(n)))
            }
        }
        ctx.globalAlpha = 1
        ctx.setLineDash([])
        // The frame's watermark, over the diagram.
        if (s.frameOver && s.frameOver.length > 0) drawFrame(ctx, s.frameOver)

        // Alignment guides (Ctrl+drag), one screen pixel wide at any zoom.
        if (s.guides.length > 0) {
            ctx.lineWidth = 1 / zoom
            dashPx(ctx, [5 / zoom, 3 / zoom], 1 / zoom)
            ctx.strokeStyle = "#e11d48"
            ctx.beginPath()
            for (const g of s.guides) { ctx.moveTo(g[0], g[1]); ctx.lineTo(g[2], g[3]) }
            ctx.stroke()
            ctx.setLineDash([])
        }

        // Equal spacing (Ctrl+drag): dimension lines with end ticks and the
        // distance, drawn at a constant screen size. Row and column gaps are
        // axis aligned; a diagonal pair is measured center to center.
        if (s.spacings.length > 0) {
            const px = 1 / zoom
            const col = "#db2777"
            ctx.setLineDash([])
            ctx.lineWidth = 1.2 * px
            ctx.strokeStyle = col
            ctx.beginPath()
            for (const g of s.spacings) {
                const len = Math.max(Math.hypot(g[2] - g[0], g[3] - g[1]), 0.001)
                const nx = -(g[3] - g[1]) / len * 5 * px, ny = (g[2] - g[0]) / len * 5 * px
                ctx.moveTo(g[0], g[1]); ctx.lineTo(g[2], g[3])
                ctx.moveTo(g[0] - nx, g[1] - ny); ctx.lineTo(g[0] + nx, g[1] + ny)
                ctx.moveTo(g[2] - nx, g[3] - ny); ctx.lineTo(g[2] + nx, g[3] + ny)
            }
            ctx.stroke()
            ctx.font = (10 * px) + "px sans-serif"
            ctx.textAlign = "center"
            ctx.textBaseline = "middle"
            for (const g of s.spacings) {
                const label = String(Math.round(Math.hypot(g[2] - g[0], g[3] - g[1])))
                const cx = (g[0] + g[2]) / 2, cy = (g[1] + g[3]) / 2
                const tw = ctx.measureText(label).width + 8 * px
                roundRect(ctx, cx - tw / 2, cy - 7 * px, tw, 14 * px, 3 * px)
                ctx.fillStyle = col
                ctx.fill()
                ctx.fillStyle = "#fff"
                ctx.fillText(label, cx, cy + 0.5 * px)
            }
        }

        if (s.preview) {
            const pv = s.preview
            polyline(ctx, pv.pts)
            ctx.lineWidth = Math.max(1.5 * s.level, pv.width)
            dashPx(ctx, [7 * s.level, 5 * s.level], ctx.lineWidth)
            ctx.strokeStyle = accent
            ctx.stroke()
            drawHeads(ctx, pv.heads, accent, Math.max(1.5 * s.level, pv.width))
            ctx.beginPath()
            ctx.arc(pv.src.x, pv.src.y, 4 * s.level, 0, 2 * Math.PI)
            ctx.fillStyle = accent
            ctx.fill()
        }

        // The socket a line dropped at its back now would be behind.
        if (s.socketLit) {
            const o = s.socketLit, m = 2.5 * s.level
            ctx.setLineDash([])
            ctx.fillStyle = "rgba(22,163,74,0.2)"
            ctx.fillRect(o.x - o.hw - m, o.y - o.hh - m, 2 * (o.hw + m), 2 * (o.hh + m))
            ctx.lineWidth = 1.5 * s.level
            ctx.strokeStyle = good
            ctx.strokeRect(o.x - o.hw - m, o.y - o.hh - m, 2 * (o.hw + m), 2 * (o.hh + m))
        }

        if (s.ghost) {
            ctx.setLineDash([])
            ctx.beginPath()
            ctx.arc(s.ghost.x, s.ghost.y, 6 * s.level, 0, 2 * Math.PI)
            ctx.fillStyle = "rgba(22,163,74,0.2)"
            ctx.fill()
            ctx.lineWidth = 1.5 * s.level
            ctx.strokeStyle = good
            ctx.stroke()
            const g3 = 3 * s.level
            ctx.beginPath()
            ctx.moveTo(s.ghost.x - g3, s.ghost.y); ctx.lineTo(s.ghost.x + g3, s.ghost.y)
            ctx.moveTo(s.ghost.x, s.ghost.y - g3); ctx.lineTo(s.ghost.x, s.ghost.y + g3)
            ctx.stroke()
        }

        if (s.handle) {
            ctx.setLineDash([])
            ctx.fillStyle = root.paper
            const hs = s.handle.s
            ctx.fillRect(s.handle.x - 4 * hs, s.handle.y - 4 * hs, 8 * hs, 8 * hs)
            ctx.lineWidth = 1.5 * hs
            ctx.strokeStyle = accent
            ctx.strokeRect(s.handle.x - 4 * hs, s.handle.y - 4 * hs, 8 * hs, 8 * hs)
        }

        if (s.rubber) {
            const r = s.rubber
            ctx.fillStyle = "rgba(37,99,235,0.09)"
            ctx.fillRect(r.x, r.y, r.w, r.h)
            ctx.lineWidth = 1 / zoom
            dashPx(ctx, [4 / zoom, 3 / zoom], 1 / zoom)
            ctx.strokeStyle = accent
            ctx.strokeRect(r.x, r.y, r.w, r.h)
            ctx.setLineDash([])
        }
        ctx.restore()
        // What the line under the pointer connects, at screen size.
        if (s.tip && s.tip.lines.length > 0) drawTip(ctx, s.tip)
    }

    // A line's tip: a box of `tip.lines` (the first one bold) beside the
    // pointer, at screen size, kept inside the view.
    function drawTip(ctx, tip) {
        const sx = panX + tip.x * zoom, sy = panY + tip.y * zoom
        const pad = 7, lh = 17
        let w = 0
        for (let i = 0; i < tip.lines.length; i++) {
            ctx.font = (i === 0 ? "bold " : "") + "12px sans-serif"
            w = Math.max(w, ctx.measureText(tip.lines[i]).width)
        }
        w += 2 * pad
        const h = tip.lines.length * lh + 2 * pad - 4
        let x = sx + 14, y = sy + 18
        if (x + w > diagram.width - 4) x = Math.max(4, sx - 14 - w)
        if (y + h > diagram.height - 4) y = Math.max(4, sy - 12 - h)
        ctx.save()
        ctx.setLineDash([])
        roundRect(ctx, x, y, w, h, 5)
        ctx.globalAlpha = 0.94
        ctx.fillStyle = root.ink
        ctx.fill()
        ctx.globalAlpha = 1
        ctx.fillStyle = root.paper
        ctx.textAlign = "left"
        ctx.textBaseline = "top"
        for (let i = 0; i < tip.lines.length; i++) {
            ctx.font = (i === 0 ? "bold " : "") + "12px sans-serif"
            ctx.fillText(tip.lines[i], x + pad, y + pad + i * lh)
        }
        ctx.restore()
    }

    // input
    MouseArea {
        id: mouse
        anchors.fill: parent
        acceptedButtons: Qt.LeftButton | Qt.MiddleButton | Qt.RightButton
        hoverEnabled: true
        cursorShape: root.panning ? Qt.ClosedHandCursor : (root.spaceHeld ? Qt.OpenHandCursor : root.ctl.cursor)
        property bool leftDown: false

        onPressed: (m) => {
            root.forceActiveFocus()
            if (m.button === Qt.LeftButton && !root.spaceHeld) {
                const w = root.toWorld(m.x, m.y)
                // A boxed note's scroll bar: drag it to scroll the text.
                const hit = root.scrollableNoteAt(w.x, w.y)
                if (hit && hit.onBar) {
                    root.thumbDrag = { id: hit.id, startY: m.y, startOff: root.noteScroll[hit.id] || 0 }
                    return
                }
                leftDown = true
                root.ctl.press(w.x, w.y, !!(m.modifiers & Qt.ShiftModifier), !!(m.modifiers & Qt.ControlModifier))
            } else {
                root.panning = true
                root.autoFit = false
                root.panStartX = m.x; root.panStartY = m.y
                root.panOrigX = root.panX; root.panOrigY = root.panY
            }
        }
        onPositionChanged: (m) => {
            if (root.thumbDrag) {
                const t = root.thumbDrag, i = root.noteInfo[t.id]
                if (!i) return
                // The thumb moves with the mouse: the text by the ratio of
                // its height to the box.
                const by = (m.y - t.startY) / (i.s * root.zoom) * (i.contentH / i.viewH)
                root.noteScroll[t.id] = t.startOff
                root.scrollNote(t.id, by)
                return
            }
            if (root.panning) {
                root.panX = root.panOrigX + m.x - root.panStartX
                root.panY = root.panOrigY + m.y - root.panStartY
                return
            }
            const w = root.toWorld(m.x, m.y)
            if (leftDown) root.ctl.drag(w.x, w.y, !!(m.modifiers & Qt.ControlModifier))
            else root.ctl.hover(w.x, w.y)
        }
        onReleased: (m) => {
            if (root.thumbDrag) { root.thumbDrag = null; return }
            if (leftDown && m.button === Qt.LeftButton) {
                leftDown = false
                const w = root.toWorld(m.x, m.y)
                root.ctl.release(w.x, w.y)
            } else if (root.panning) {
                root.panning = false
            }
        }
        // Nothing pointed at any more (no line tip left behind).
        onExited: if (!leftDown) root.ctl.hoverOut()
        onDoubleClicked: (m) => {
            if (m.button !== Qt.LeftButton) return
            const w = root.toWorld(m.x, m.y)
            root.ctl.doubleClick(w.x, w.y)
        }
        onWheel: (wh) => {
            const dy = wh.angleDelta.y !== 0 ? wh.angleDelta.y : wh.pixelDelta.y
            // Over a boxed note with more text than fits: scroll its text
            // (Ctrl+wheel zooms anyway).
            if (!(wh.modifiers & Qt.ControlModifier)) {
                const w = root.toWorld(wh.x, wh.y)
                const hit = root.scrollableNoteAt(w.x, w.y)
                if (hit) {
                    root.scrollNote(hit.id, -dy * 0.5 / (hit.info.s * root.zoom))
                    return
                }
            }
            root.zoomAt(Math.pow(1.0015, dy), wh.x, wh.y)
        }
    }

    // in-place editor
    // Rust decides what is being edited and where (world coordinates); the
    // field is drawn in screen space and scaled with the zoom so its text
    // matches the canvas text it replaces.
    readonly property var editBox: ctl.editBox === "" ? null : JSON.parse(ctl.editBox)
    property string editKey: ""

    onEditBoxChanged: {
        if (!editBox) { editKey = ""; return }
        if (editBox.key !== editKey) {
            editKey = editBox.key
            // A boxed note opens at its box; anything else fits its text.
            // (The frame's own `k` may not have caught up with the new box
            // yet, so it is worked out here.)
            const k = root.zoom * (editBox.s || 1)
            const chrome = 2 * 6 * Math.min(k, 3) + 3
            editFrame.resized = false
            editFrame.userW = editBox.bw > 0 ? editBox.bw * k + chrome : 0
            editFrame.userH = editBox.bh > 0 ? editBox.bh * k + chrome : 0
            editField.text = editBox.text
            editField.forceActiveFocus()
            editField.selectAll()
        }
    }

    function commitEdit() {
        if (!editBox) return
        editKey = ""
        // A note gets the box its editor was resized to; otherwise it keeps
        // its own size.
        const f = editFrame
        const sized = editBox.multi && f.resized
        ctl.commitEdit(editField.text, sized ? (f.boxW - f.chrome) / f.k : 0, sized ? (f.boxH - f.chrome) / f.k : 0)
        root.forceActiveFocus()
    }

    function cancelEdit() {
        editKey = ""
        ctl.cancelEdit()
        root.forceActiveFocus()
    }

    Rectangle {
        id: editFrame
        visible: root.editBox !== null
        // Drawn at screen size (so its scroll bars and handle stay a normal
        // size at any zoom); the text inside is scaled like the canvas text.
        // Sizes in the box are nominal; `s` is the item's scale (smaller for
        // items inside other items at deep zoom).
        readonly property var b: root.editBox
        readonly property real k: root.zoom * (b && b.s ? b.s : 1)
        readonly property real pad: 6 * Math.min(k, 3)
        // Border and inner margin around the text, on both sides together.
        readonly property real chrome: 2 * pad + 3
        // Text wraps in a fixed box: a boxed note, or after resizing one.
        readonly property bool wraps: b !== null && b.multi && (userW > 0 || b.bw > 0)
        // Grows with the text, up to what the view shows, unless resized
        // with the corner handle (0: not resized).
        property real userW: 0
        property real userH: 0
        // The handle was dragged: the note takes this size.
        property bool resized: false
        readonly property real maxW: Math.max(80, root.width - 24)
        readonly property real maxH: Math.max(40, root.height - 24)
        readonly property real fitW: b ? Math.max(b.minw * k, editField.contentWidth + 2 * pad) : 0
        readonly property real fitH: b ? Math.max(b.size * k, editField.contentHeight) + 2 * pad : 0
        readonly property real boxW: Math.min(userW > 0 ? userW : fitW + 4, maxW)
        readonly property real boxH: Math.min(userH > 0 ? userH : fitH + 4, maxH)
        width: boxW
        height: boxH
        // Anchored where the text is on the canvas, but kept in view.
        // A note's text starts at its corner: the margin goes outside it.
        readonly property real inset: b && b.inset ? chrome / 2 : 0
        readonly property real anchorX: b ? root.panX + b.x * root.zoom - inset - (b.align === "center" ? boxW / 2 : b.align === "right" ? boxW : 0) : 0
        readonly property real anchorY: b ? root.panY + b.y * root.zoom - inset - (b.base === "middle" ? boxH / 2 : b.base === "bottom" ? boxH : 0) : 0
        x: Math.max(4, Math.min(anchorX, root.width - boxW - 4))
        y: Math.max(4, Math.min(anchorY, root.height - boxH - 4))
        radius: 3
        color: root.paper
        border.color: root.accent
        border.width: 1.5

        ScrollView {
            id: editScroll
            anchors.fill: parent
            anchors.margins: 1.5
            clip: true
            // Wrapped text is as wide as the box; otherwise as wide as it is.
            contentWidth: editFrame.wraps ? availableWidth : editField.implicitWidth
            // Shown whenever the text does not fit (not only while scrolling).
            ScrollBar.horizontal.policy: !editFrame.wraps && editField.implicitWidth > width + 0.5 ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff
            ScrollBar.vertical.policy: editField.implicitHeight > height + 0.5 ? ScrollBar.AlwaysOn : ScrollBar.AlwaysOff

            // TextArea rather than a single line field so notes can span
            // several lines (Shift+Enter); other texts are joined into one
            // line on commit. In a ScrollView it scrolls to the cursor.
            TextArea {
                id: editField
                padding: editFrame.pad
                wrapMode: editFrame.wraps ? TextEdit.Wrap : TextEdit.NoWrap
                background: null
                verticalAlignment: editFrame.wraps ? TextEdit.AlignTop : TextEdit.AlignVCenter
                horizontalAlignment: !editFrame.b || editFrame.b.align === "center" ? TextEdit.AlignHCenter
                                   : editFrame.b.align === "right" ? TextEdit.AlignRight : TextEdit.AlignLeft
                font.pixelSize: Math.max(1, (editFrame.b ? editFrame.b.size : 12) * editFrame.k)
                font.bold: editFrame.b ? editFrame.b.bold : false
                color: root.darkTheme ? "#e2e8f0" : "#0f172a"
                selectByMouse: true
                selectionColor: root.darkTheme ? "#1e3a8a" : "#bfdbfe"
                selectedTextColor: root.darkTheme ? "#ffffff" : "#0f172a"
                function onEnter(e) {
                    if (editFrame.b && editFrame.b.multi && (e.modifiers & Qt.ShiftModifier)) e.accepted = false
                    else root.commitEdit()
                }
                Keys.onReturnPressed: (e) => onEnter(e)
                Keys.onEnterPressed: (e) => onEnter(e)
                Keys.onEscapePressed: root.cancelEdit()
                Keys.onTabPressed: root.commitEdit()
                onActiveFocusChanged: if (!activeFocus && root.editBox && !editResize.pressed) root.commitEdit()
            }
        }

        // Resize handle in the lower right corner.
        Rectangle {
            width: 12; height: 12
            anchors.right: parent.right
            anchors.bottom: parent.bottom
            color: "transparent"
            Canvas {
                anchors.fill: parent
                onPaint: {
                    const ctx = getContext("2d")
                    ctx.reset()
                    ctx.strokeStyle = root.accent
                    ctx.lineWidth = 1.2
                    ctx.beginPath()
                    ctx.moveTo(11, 3); ctx.lineTo(3, 11)
                    ctx.moveTo(11, 7); ctx.lineTo(7, 11)
                    ctx.stroke()
                }
            }
            MouseArea {
                id: editResize
                anchors.fill: parent
                anchors.margins: -3
                cursorShape: Qt.SizeFDiagCursor
                property point start
                property size startSize
                onPressed: (m) => {
                    start = mapToItem(root, m.x, m.y)
                    startSize = Qt.size(editFrame.boxW, editFrame.boxH)
                }
                onPositionChanged: (m) => {
                    editFrame.resized = true
                    const p = mapToItem(root, m.x, m.y)
                    editFrame.userW = Math.max(60, Math.min(startSize.width + p.x - start.x, root.width - editFrame.x - 4))
                    editFrame.userH = Math.max(24, Math.min(startSize.height + p.y - start.y, root.height - editFrame.y - 4))
                }
                onReleased: editField.forceActiveFocus()
            }
        }
    }

    Keys.onPressed: (e) => {
        if (editField.activeFocus) return   // keys the text field did not use
        const step = (e.modifiers & Qt.ShiftModifier) ? 1 : 10
        switch (e.key) {
        case Qt.Key_F2:
        case Qt.Key_Return:
        case Qt.Key_Enter: ctl.beginEditSelection(); break
        case Qt.Key_Delete:
        case Qt.Key_Backspace: ctl.deleteSelected(); break
        case Qt.Key_Escape: mouse.leftDown = false; ctl.cancel(); break
        case Qt.Key_Left: ctl.nudge(-step, 0); break
        case Qt.Key_Right: ctl.nudge(step, 0); break
        case Qt.Key_Up: ctl.nudge(0, -step); break
        case Qt.Key_Down: ctl.nudge(0, step); break
        case Qt.Key_Space: if (!e.isAutoRepeat) spaceHeld = true; break
        // Stacking order, as in Inkscape.
        case Qt.Key_Home: ctl.arrange("front"); break
        case Qt.Key_End: ctl.arrange("back"); break
        case Qt.Key_PageUp: ctl.arrange("forward"); break
        case Qt.Key_PageDown: ctl.arrange("backward"); break
        default:
            // Typing on a selected item starts editing its text, replacing it.
            if (e.text.length === 1 && e.text >= " " && !(e.modifiers & (Qt.ControlModifier | Qt.AltModifier | Qt.MetaModifier))
                    && ctl.beginEditSelection()) {
                editField.text = e.text
                editField.cursorPosition = editField.text.length
                break
            }
            return
        }
        e.accepted = true
    }
    Keys.onReleased: (e) => {
        if (e.key === Qt.Key_Space && !e.isAutoRepeat) { spaceHeld = false; e.accepted = true }
    }
}
