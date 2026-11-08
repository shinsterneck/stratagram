// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// The diagram view: draws the scene computed in Rust and forwards mouse and
// keyboard input in world coordinates (a port of qml/DiagramCanvas.qml).
// Pan: middle/right drag or Space+drag. Zoom: mouse wheel.

// The theme's colors around the diagram (scene.ui, see core/src/theme.rs),
// set on every paint; the diagram's own colors come translated already.
let ACCENT = "#2563eb"
let GOOD = "#16a34a"
let BACKGROUND = "#f8fafc"
let PAPER = "#ffffff"
let INK = "#334155"
let MUTED = "#475569"
let GRID = ["#eef2f6", "#e1e7ef"]
let GLOW = false

function setTheme(ui) {
    if (!ui) return
    ACCENT = ui.accent; GOOD = ui.good; BACKGROUND = ui.canvas; PAPER = ui.paper
    INK = ui.ink; MUTED = ui.muted; GRID = ui.grid; GLOW = ui.glow
}

// Neon: lines and outlines glow in their own color.
function glow(ctx, color, on) {
    if (!GLOW) return
    ctx.shadowColor = on ? color : "transparent"
    ctx.shadowBlur = on ? 7 : 0
}
const FONT = "system-ui, -apple-system, 'Segoe UI', Roboto, 'Noto Sans', sans-serif"

// Editor cursor codes (Qt's cursor shapes) as CSS cursors.
const CURSORS = { 0: "default", 2: "crosshair", 8: "nwse-resize", 9: "move", 13: "pointer" }

export class DiagramView {
    // `app` gives the controller (`app.ctl`), `app.changed()` after every
    // change, and `app.zoomChanged()` for the zoom display.
    constructor(app, parent) {
        this.app = app
        this.ctl = app.ctl
        this.zoom = 1
        this.panX = 40
        this.panY = 40
        this.scene = { devices: [], links: [], notes: [], points: [], order: [], guides: [], spacings: [] }
        this.showGrid = true
        this.deepZoom = false
        // While true the view refits whenever it is resized, so a fit
        // requested before the page has its final size still lands right.
        this.autoFit = false
        this.panning = null
        this.spaceHeld = false
        this.leftDown = false
        this.cursor = 0
        // Boxed notes: scroll position by id and what the last paint found.
        this.noteScroll = {}
        this.noteInfo = {}
        this.thumbDrag = null
        this.images = new Map()

        this.el = document.createElement("div")
        this.el.className = "sg-view"
        this.canvas = document.createElement("canvas")
        this.canvas.tabIndex = 0
        this.canvas.className = "sg-canvas"
        this.el.appendChild(this.canvas)
        parent.appendChild(this.el)
        this.ctx = this.canvas.getContext("2d")
        this.makeEditor()

        // (Resizing inside the observer's callback would make it report again.)
        new ResizeObserver(() => requestAnimationFrame(() => this.resized())).observe(this.el)
        this.listen()
    }

    // view

    get width() { return this.el.clientWidth }
    get height() { return this.el.clientHeight }

    // World size of one grid step at this zoom (finer at deeper levels);
    // the same rule as the level of new items (Editor::level).
    get gridLevel() {
        return this.deepZoom ? Math.pow(8, -Math.floor(Math.log(this.zoom / 0.75) / Math.log(8) + 1e-9)) : 1
    }

    resized() {
        const dpr = window.devicePixelRatio || 1
        this.canvas.width = Math.max(1, Math.round(this.width * dpr))
        this.canvas.height = Math.max(1, Math.round(this.height * dpr))
        this.canvas.style.width = this.width + "px"
        this.canvas.style.height = this.height + "px"
        if (this.autoFit) this.fit()
        this.repaint()
    }

    setScene(json) {
        this.scene = JSON.parse(json)
        this.repaint()
    }

    repaint() {
        if (this.frameRequested) return
        this.frameRequested = true
        requestAnimationFrame(() => { this.frameRequested = false; this.paint() })
    }

    toWorld(mx, my) {
        return { x: (mx - this.panX) / this.zoom, y: (my - this.panY) / this.zoom }
    }

    viewCenter() { return this.toWorld(this.width / 2, this.height / 2) }

    // The visible area in world coordinates.
    viewRect() {
        const a = this.toWorld(0, 0), b = this.toWorld(this.width, this.height)
        return { x: a.x, y: a.y, w: b.x - a.x, h: b.y - a.y }
    }

    setZoom(z) {
        if (z === this.zoom) return
        this.zoom = z
        // What is big enough to show depends on the zoom.
        this.app.viewChanged()
    }

    zoomAt(factor, mx, my) {
        this.autoFit = false
        const nz = this.deepZoom ? Math.max(1e-3, Math.min(1e7, this.zoom * factor))
                                 : Math.max(0.2, Math.min(4.0, this.zoom * factor))
        const w = this.toWorld(mx, my)
        this.panX = mx - w.x * nz
        this.panY = my - w.y * nz
        this.setZoom(nz)
        this.repaint()
    }

    zoomBy(factor) { this.zoomAt(factor, this.width / 2, this.height / 2) }

    setDeepZoom(on) {
        this.deepZoom = on
        if (!on) this.zoomBy(1)
        this.repaint()
    }

    fit() {
        this.autoFit = true
        const b = this.scene.bounds
        if (!b) { this.panX = 40; this.panY = 40; this.setZoom(1); this.repaint(); return }
        const m = 60
        if (this.width <= 2 * m || this.height <= 2 * m) return  // not laid out yet
        let z = Math.min((this.width - 2 * m) / Math.max(b.w, 1), (this.height - 2 * m) / Math.max(b.h, 1), 1.5)
        z = Math.max(0.2, z)
        this.panX = (this.width - b.w * z) / 2 - b.x * z
        this.panY = (this.height - b.h * z) / 2 - b.y * z
        this.setZoom(z)
        this.repaint()
    }

    // Pan (keeping the zoom) so the selected devices are in view.
    revealSelection() {
        const sel = this.scene.devices.filter(d => d.sel)
        if (sel.length === 0) return
        const x0 = Math.min(...sel.map(d => d.x)), y0 = Math.min(...sel.map(d => d.y))
        const x1 = Math.max(...sel.map(d => d.x + d.w)), y1 = Math.max(...sel.map(d => d.y + d.h))
        const m = 30, z = this.zoom
        if (x0 * z + this.panX >= m && y0 * z + this.panY >= m &&
            x1 * z + this.panX <= this.width - m && y1 * z + this.panY <= this.height - m) return
        this.autoFit = false
        this.panX = this.width / 2 - (x0 + x1) / 2 * z
        this.panY = this.height / 2 - (y0 + y1) / 2 * z
        this.repaint()
    }

    focus() { this.canvas.focus({ preventScroll: true }) }

    // painting

    paint() {
        const ctx = this.ctx
        const dpr = window.devicePixelRatio || 1
        ctx.setTransform(dpr, 0, 0, dpr, 0, 0)
        setTheme(this.scene.ui)
        ctx.fillStyle = BACKGROUND
        ctx.fillRect(0, 0, this.width, this.height)
        this.paintGrid(ctx)
        this.paintScene(ctx)
        this.placeEditor()
    }

    paintGrid(ctx) {
        const step = 10 * this.gridLevel * this.zoom
        if (!this.showGrid || step < 5) return
        const { width, height, panX, panY } = this
        const x0 = ((panX % step) + step) % step
        const y0 = ((panY % step) + step) % step
        const majorEvery = 5
        const ix0 = Math.round((x0 - panX) / step)
        const iy0 = Math.round((y0 - panY) / step)
        ctx.lineWidth = 1
        for (let pass = 0; pass < 2; pass++) {
            ctx.beginPath()
            ctx.strokeStyle = GRID[pass]
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

    paintScene(ctx) {
        const s = this.scene
        ctx.save()
        ctx.translate(this.panX, this.panY)
        ctx.scale(this.zoom, this.zoom)
        ctx.lineJoin = "round"

        // The frame (border, header and footer) under everything.
        if (s.frame) drawFrame(ctx, s.frame, this)
        this.noteInfo = {}
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
        // order (hidden layers are not in the scene). Items deeper in (deep
        // zoom) have a scale below 1: they are drawn in a frame scaled down
        // around their corner, and fade in as they get big enough to read.
        for (const [kind, i] of s.order) {
            if (kind === "l") {
                const l = s.links[i]
                ctx.globalAlpha = l.a
                drawLink(ctx, l)
            } else if (kind === "d") {
                const d = s.devices[i]
                ctx.globalAlpha = d.a
                inFrame(ctx, d.x, d.y, d.s, () => drawDevice(ctx, this.localDevice(d), s.target))
            } else if (kind === "p") {
                const p = s.points[i]
                ctx.globalAlpha = p.a
                inFrame(ctx, p.x, p.y, p.s, () => this.drawPoint(ctx, p, s.target))
            } else {
                const n = s.notes[i]
                ctx.globalAlpha = n.a
                inFrame(ctx, n.x, n.y, n.s, () => this.drawNote(ctx, localNote(n)))
            }
        }
        ctx.globalAlpha = 1
        ctx.setLineDash([])
        // The frame's watermark, over the diagram.
        if (s.frameOver && s.frameOver.length > 0) drawFrame(ctx, s.frameOver, this)

        const z = this.zoom
        // Alignment guides (Ctrl+drag), one screen pixel wide at any zoom.
        if (s.guides.length > 0) {
            ctx.lineWidth = 1 / z
            ctx.setLineDash([5 / z, 3 / z])
            ctx.strokeStyle = "#e11d48"
            ctx.beginPath()
            for (const g of s.guides) { ctx.moveTo(g[0], g[1]); ctx.lineTo(g[2], g[3]) }
            ctx.stroke()
            ctx.setLineDash([])
        }

        // Equal spacing (Ctrl+drag): dimension lines with end ticks and the
        // distance, at a constant screen size.
        if (s.spacings.length > 0) {
            const px = 1 / z
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
            ctx.font = font(10 * px)
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
            ctx.setLineDash([7 * s.level, 5 * s.level])
            ctx.strokeStyle = ACCENT
            ctx.stroke()
            drawHeads(ctx, pv.heads, ACCENT, Math.max(1.5 * s.level, pv.width))
            ctx.beginPath()
            ctx.arc(pv.src.x, pv.src.y, 4 * s.level, 0, 2 * Math.PI)
            ctx.fillStyle = ACCENT
            ctx.fill()
        }

        // The socket a line dropped at its back now would be behind.
        if (s.socketLit) {
            const o = s.socketLit, m = 2.5 * s.level
            ctx.setLineDash([])
            ctx.fillStyle = "rgba(22,163,74,0.2)"
            ctx.fillRect(o.x - o.hw - m, o.y - o.hh - m, 2 * (o.hw + m), 2 * (o.hh + m))
            ctx.lineWidth = 1.5 * s.level
            ctx.strokeStyle = GOOD
            ctx.strokeRect(o.x - o.hw - m, o.y - o.hh - m, 2 * (o.hw + m), 2 * (o.hh + m))
        }

        if (s.ghost) {
            ctx.setLineDash([])
            ctx.beginPath()
            ctx.arc(s.ghost.x, s.ghost.y, 6 * s.level, 0, 2 * Math.PI)
            ctx.fillStyle = "rgba(22,163,74,0.2)"
            ctx.fill()
            ctx.lineWidth = 1.5 * s.level
            ctx.strokeStyle = GOOD
            ctx.stroke()
            const g3 = 3 * s.level
            ctx.beginPath()
            ctx.moveTo(s.ghost.x - g3, s.ghost.y); ctx.lineTo(s.ghost.x + g3, s.ghost.y)
            ctx.moveTo(s.ghost.x, s.ghost.y - g3); ctx.lineTo(s.ghost.x, s.ghost.y + g3)
            ctx.stroke()
        }

        if (s.handle) {
            ctx.setLineDash([])
            ctx.fillStyle = PAPER
            const hs = s.handle.s
            ctx.fillRect(s.handle.x - 4 * hs, s.handle.y - 4 * hs, 8 * hs, 8 * hs)
            ctx.lineWidth = 1.5 * hs
            ctx.strokeStyle = ACCENT
            ctx.strokeRect(s.handle.x - 4 * hs, s.handle.y - 4 * hs, 8 * hs, 8 * hs)
        }

        if (s.rubber) {
            const r = s.rubber
            ctx.fillStyle = "rgba(37,99,235,0.09)"
            ctx.fillRect(r.x, r.y, r.w, r.h)
            ctx.lineWidth = 1 / z
            ctx.setLineDash([4 / z, 3 / z])
            ctx.strokeStyle = ACCENT
            ctx.strokeRect(r.x, r.y, r.w, r.h)
            ctx.setLineDash([])
        }
        ctx.restore()
        // What the line under the pointer connects, at screen size.
        if (s.tip && s.tip.lines.length > 0) drawTip(ctx, s.tip, this)
    }

    // In deep zoom, decorations stop growing past 1.5x their size.
    capFor(sc) { return this.deepZoom ? Math.min(1, 1.5 / (sc * this.zoom)) : 1 }

    // A device's coordinates in its own frame (see inFrame).
    localDevice(d) {
        const sc = d.s
        const k = this.capFor(sc)
        if (sc === 1) return k === 1 ? d : Object.assign({}, d, { k })
        const lx = (x) => d.x + (x - d.x) / sc, ly = (y) => d.y + (y - d.y) / sc
        const ports = d.ports.map(p => Object.assign({}, p, { x: lx(p.x), y: ly(p.y), lx: lx(p.lx), ly: ly(p.ly) }))
        return Object.assign({}, d, { w: d.w / sc, h: d.h / sc, ports, k })
    }

    // A free connection point: a small ring, only on screen (exports show
    // just the line ending there).
    drawPoint(ctx, p, target) {
        const hot = target === p.id
        const k = this.capFor(p.s)
        if (p.sel || hot) {
            ctx.beginPath()
            ctx.arc(p.x, p.y, 9 * k, 0, 2 * Math.PI)
            ctx.fillStyle = hot ? "rgba(22,163,74,0.25)" : "rgba(37,99,235,0.25)"
            ctx.fill()
        }
        ctx.setLineDash([])
        ctx.beginPath()
        ctx.arc(p.x, p.y, 4 * k, 0, 2 * Math.PI)
        ctx.fillStyle = p.sel ? ACCENT : PAPER
        ctx.fill()
        ctx.lineWidth = 1.5 * k
        ctx.strokeStyle = hot ? GOOD : (p.sel ? ACCENT : MUTED)
        ctx.stroke()
        // Its label, beyond the end of its line.
        if (p.label !== "" && !p.editing) {
            ctx.font = font(11)
            ctx.fillStyle = INK
            ctx.textAlign = p.align
            ctx.textBaseline = p.base
            ctx.fillText(p.label, p.x + p.ldx, p.y + p.ldy)
        }
    }

    // A boxed note: wrapped to its box, scrolled within it, with a scroll
    // bar when the text is taller than the box.
    drawBoxedNote(ctx, n) {
        // About 8 screen pixels wide, at any zoom.
        const sbw = 9 / (n.s * this.zoom)
        let lines = wrapText(ctx, n.text, n.w)
        const over = lines.length * n.lh > n.h + 0.5
        if (over) lines = wrapText(ctx, n.text, Math.max(n.w - sbw, n.size))
        const contentH = lines.length * n.lh
        const maxOff = Math.max(0, contentH - n.h)
        const off = Math.min(maxOff, Math.max(0, this.noteScroll[n.id] || 0))
        this.noteScroll[n.id] = off
        this.noteInfo[n.id] = { x: n.x, y: n.y, w: n.w * n.s, h: n.h * n.s, s: n.s, sbw: sbw * n.s,
                                over, contentH, viewH: n.h, maxOff }
        if (n.sel) {
            roundRect(ctx, n.x - 4, n.y - 3, n.w + 8, n.h + 6, 4)
            ctx.fillStyle = "rgba(37,99,235,0.12)"
            ctx.fill()
            ctx.lineWidth = 1.2
            ctx.strokeStyle = ACCENT
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

    drawNote(ctx, n) {
        ctx.font = font(n.size, n.bold)
        if (n.box) { this.drawBoxedNote(ctx, n); return }
        const lines = n.lines
        if (n.sel) {
            let w = 0
            for (const line of lines) w = Math.max(w, ctx.measureText(line).width)
            roundRect(ctx, n.x - 4, n.y - 3, Math.max(w, n.size) + 8, lines.length * n.lh + 6, 4)
            ctx.fillStyle = "rgba(37,99,235,0.12)"
            ctx.fill()
            ctx.lineWidth = 1.2
            ctx.strokeStyle = ACCENT
            ctx.stroke()
        }
        if (n.editing) return
        ctx.fillStyle = n.color
        ctx.textAlign = "left"
        ctx.textBaseline = "middle"
        for (let i = 0; i < lines.length; i++) ctx.fillText(lines[i], n.x, n.y + (i + 0.5) * n.lh)
    }

    // A logo (a data URI) once loaded; loading it repaints.
    image(data) {
        let img = this.images.get(data)
        if (!img) {
            img = new Image()
            img.onload = () => this.repaint()
            img.src = data
            this.images.set(data, img)
        }
        return img.complete && img.naturalWidth > 0 ? img : null
    }

    // The boxed note whose text overflows under (x, y) in world units, and
    // whether the point is on its scroll bar.
    scrollableNoteAt(x, y) {
        for (const id in this.noteInfo) {
            const i = this.noteInfo[id]
            if (i.over && x >= i.x && x <= i.x + i.w && y >= i.y && y <= i.y + i.h)
                return { id, info: i, onBar: x >= i.x + i.w - i.sbw }
        }
        return null
    }

    scrollNote(id, by) {
        const i = this.noteInfo[id]
        this.noteScroll[id] = Math.min(i.maxOff, Math.max(0, (this.noteScroll[id] || 0) + by))
        this.repaint()
    }

    // input

    listen() {
        const c = this.canvas
        const at = (e) => {
            const r = c.getBoundingClientRect()
            return { x: e.clientX - r.left, y: e.clientY - r.top }
        }
        c.addEventListener("contextmenu", (e) => e.preventDefault())
        c.addEventListener("pointerdown", (e) => {
            this.focus()
            // The view stays put from the first touch on: a refit between
            // press and release would turn a click into a drag.
            this.autoFit = false
            const m = at(e)
            this.pressAt = null
            c.setPointerCapture(e.pointerId)
            if (e.button === 0 && !this.spaceHeld) {
                const w = this.toWorld(m.x, m.y)
                // A boxed note's scroll bar: drag it to scroll the text.
                const hit = this.scrollableNoteAt(w.x, w.y)
                if (hit && hit.onBar) {
                    this.thumbDrag = { id: hit.id, startY: m.y, startOff: this.noteScroll[hit.id] || 0 }
                    return
                }
                this.leftDown = true
                // Where it went down, on the screen and in the diagram.
                this.pressAt = { x: e.clientX, y: e.clientY, world: w }
                this.ctl.press(w.x, w.y, e.shiftKey, e.ctrlKey || e.metaKey)
                this.app.changed()
            } else {
                this.autoFit = false
                this.panning = { x: m.x, y: m.y, panX: this.panX, panY: this.panY }
                this.updateCursor()
            }
            e.preventDefault()
        })
        c.addEventListener("pointermove", (e) => {
            const m = at(e)
            // A click is no drag (which would bend a clicked line) until the
            // mouse moves a bit on the screen, whatever the page does
            // meanwhile (browsers report tiny moves, layouts shift).
            if (this.pressAt) {
                if (Math.hypot(e.clientX - this.pressAt.x, e.clientY - this.pressAt.y) < 3) return
                this.pressAt = null
            }
            if (this.thumbDrag) {
                const t = this.thumbDrag, i = this.noteInfo[t.id]
                if (!i) return
                // The thumb moves with the mouse: the text by the ratio of
                // its height to the box.
                this.noteScroll[t.id] = t.startOff
                this.scrollNote(t.id, (m.y - t.startY) / (i.s * this.zoom) * (i.contentH / i.viewH))
                return
            }
            if (this.panning) {
                this.panX = this.panning.panX + m.x - this.panning.x
                this.panY = this.panning.panY + m.y - this.panning.y
                this.repaint()
                return
            }
            const w = this.toWorld(m.x, m.y)
            if (this.leftDown) {
                this.ctl.drag(w.x, w.y, e.ctrlKey || e.metaKey)
                this.app.changed()
            } else {
                this.cursor = this.ctl.hover(w.x, w.y)
                this.updateCursor()
                this.app.changed()
            }
        })
        // Nothing pointed at any more (no line tip left behind).
        c.addEventListener("pointerleave", () => {
            if (this.leftDown) return
            this.ctl.hoverOut()
            this.app.changed()
        })
        c.addEventListener("pointerup", (e) => {
            const m = at(e)
            const click = this.pressAt
            this.pressAt = null
            if (this.thumbDrag) { this.thumbDrag = null; return }
            if (this.leftDown && e.button === 0) {
                this.leftDown = false
                const w = click ? click.world : this.toWorld(m.x, m.y)
                this.cursor = this.ctl.release(w.x, w.y)
                this.app.changed()
            } else if (this.panning) {
                this.panning = null
            }
            this.updateCursor()
        })
        c.addEventListener("dblclick", (e) => {
            if (e.button !== 0) return
            const m = at(e)
            const w = this.toWorld(m.x, m.y)
            this.ctl.doubleClick(w.x, w.y)
            this.app.changed()
        })
        c.addEventListener("wheel", (e) => {
            e.preventDefault()
            const m = at(e)
            // Pixels as Qt counts them: 120 per notch, positive away from you.
            const dy = -e.deltaY * (e.deltaMode === 1 ? 40 : e.deltaMode === 2 ? 800 : 1.2)
            // Over a boxed note with more text than fits: scroll its text
            // (Ctrl+wheel zooms anyway).
            if (!e.ctrlKey) {
                const w = this.toWorld(m.x, m.y)
                const hit = this.scrollableNoteAt(w.x, w.y)
                if (hit) { this.scrollNote(hit.id, -dy * 0.5 / (hit.info.s * this.zoom)); return }
            }
            this.zoomAt(Math.pow(1.0015, dy), m.x, m.y)
        }, { passive: false })
        c.addEventListener("keydown", (e) => this.keyDown(e))
        c.addEventListener("keyup", (e) => {
            if (e.key === " ") { this.spaceHeld = false; this.updateCursor(); e.preventDefault() }
        })
        c.addEventListener("blur", () => { this.spaceHeld = false; this.updateCursor() })
    }

    updateCursor() {
        this.canvas.style.cursor = this.panning ? "grabbing" : this.spaceHeld ? "grab" : (CURSORS[this.cursor] || "default")
    }

    keyDown(e) {
        if (e.ctrlKey || e.metaKey || e.altKey) return  // shortcuts: the app's
        const step = e.shiftKey ? 1 : 10
        const ctl = this.ctl
        switch (e.key) {
        case "F2":
        case "Enter": ctl.beginEditSelection(); break
        case "Delete":
        case "Backspace": ctl.deleteSelected(); break
        case "Escape": this.leftDown = false; ctl.cancel(); break
        case "ArrowLeft": ctl.nudge(-step, 0); break
        case "ArrowRight": ctl.nudge(step, 0); break
        case "ArrowUp": ctl.nudge(0, -step); break
        case "ArrowDown": ctl.nudge(0, step); break
        case " ": if (!e.repeat) { this.spaceHeld = true; this.updateCursor() } break
        // Stacking order, as in Inkscape.
        case "Home": ctl.arrange("front"); break
        case "End": ctl.arrange("back"); break
        case "PageUp": ctl.arrange("forward"); break
        case "PageDown": ctl.arrange("backward"); break
        default:
            // Typing on a selected item starts editing its text, replacing it.
            if (e.key.length === 1 && ctl.beginEditSelection()) {
                this.app.changed()
                this.field.value = e.key
                this.field.setSelectionRange(1, 1)
                this.fitEditor()
                break
            }
            return
        }
        e.preventDefault()
        this.app.changed()
    }

    // in-place editor
    // Rust decides what is being edited and where (world coordinates); the
    // field sits over the canvas at screen size, its text scaled with the
    // zoom to match the canvas text it replaces.

    makeEditor() {
        this.edit = null
        this.editKey = ""
        const frame = document.createElement("div")
        frame.className = "sg-edit"
        frame.hidden = true
        const field = document.createElement("textarea")
        field.spellcheck = false
        field.wrap = "off"
        frame.appendChild(field)
        this.el.appendChild(frame)
        this.editFrame = frame
        this.field = field
        // Resized by its corner handle: the note takes that size.
        this.userSize = null
        new ResizeObserver(() => requestAnimationFrame(() => {
            if (!this.edit || this.settingSize) return
            const w = field.offsetWidth, h = field.offsetHeight
            if (this.lastSize && (Math.abs(w - this.lastSize.w) > 1 || Math.abs(h - this.lastSize.h) > 1)) {
                this.userSize = { w, h }
                this.lastSize = { w, h }
                this.applyEditorStyle()
                this.placeEditor()
            }
        })).observe(field)
        field.addEventListener("keydown", (e) => {
            e.stopPropagation()
            if (e.key === "Enter" && !(this.edit && this.edit.multi && e.shiftKey)) { e.preventDefault(); this.commitEdit() }
            else if (e.key === "Escape") { e.preventDefault(); this.cancelEdit() }
            else if (e.key === "Tab") { e.preventDefault(); this.commitEdit() }
        })
        field.addEventListener("input", () => this.fitEditor())
        field.addEventListener("blur", () => { if (this.edit) this.commitEdit() })
    }

    // The editor's place and size follow the edit box from Rust.
    setEditBox(json) {
        const b = json === "" ? null : JSON.parse(json)
        this.edit = b
        if (!b) {
            this.editKey = ""
            this.editFrame.hidden = true
            return
        }
        if (b.key !== this.editKey) {
            this.editKey = b.key
            const k = this.zoom * (b.s || 1)
            // A boxed note opens at its box; anything else fits its text.
            this.userSize = b.bw > 0 ? { w: b.bw * k + this.chrome(k), h: b.bh * k + this.chrome(k) } : null
            this.resizedByUser = false
            this.field.value = b.text
            this.editFrame.hidden = false
            this.fitEditor()
            this.field.focus({ preventScroll: true })
            this.field.select()
        }
        this.placeEditor()
    }

    // Border and inner margin around the text, both sides together.
    chrome(k) { return 2 * this.pad(k) + 3 }
    pad(k) { return 6 * Math.min(k, 3) }

    applyEditorStyle() {
        const b = this.edit
        if (!b) return
        const k = this.zoom * (b.s || 1)
        const f = this.field
        f.style.font = font(Math.max(1, b.size * k), b.bold)
        f.style.lineHeight = "1.25"
        f.style.padding = this.pad(k) + "px"
        f.style.textAlign = b.align === "center" ? "center" : b.align === "right" ? "right" : "left"
        // Text wraps in a fixed box: a boxed note, or after resizing one.
        const wraps = b.multi && this.userSize !== null
        f.wrap = wraps ? "soft" : "off"
        f.style.resize = b.multi ? "both" : "none"
    }

    // Size the field to its text (up to what the view shows), or to the
    // size its handle was dragged to.
    fitEditor() {
        const b = this.edit
        if (!b) return
        this.applyEditorStyle()
        const k = this.zoom * (b.s || 1)
        const f = this.field
        const maxW = Math.max(80, this.width - 24), maxH = Math.max(40, this.height - 24)
        let w, h
        if (this.userSize) {
            w = this.userSize.w
            h = this.userSize.h
        } else {
            const ctx = this.ctx
            ctx.save()
            ctx.font = font(Math.max(1, b.size * k), b.bold)
            const lines = f.value.split("\n")
            const textW = Math.max(...lines.map(l => ctx.measureText(l).width))
            ctx.restore()
            w = Math.max(b.minw * k, textW + 2 * this.pad(k)) + 8
            h = Math.max(b.size * k, lines.length * b.size * k * 1.25) + 2 * this.pad(k) + 4
        }
        this.settingSize = true
        f.style.width = Math.min(w, maxW) + "px"
        f.style.height = Math.min(h, maxH) + "px"
        this.lastSize = { w: f.offsetWidth, h: f.offsetHeight }
        this.settingSize = false
        this.placeEditor()
    }

    // Anchored where the text is on the canvas, but kept in view. A note's
    // text starts at its corner: the margin goes outside it.
    placeEditor() {
        const b = this.edit
        if (!b || this.editFrame.hidden) return
        const f = this.field
        const k = this.zoom * (b.s || 1)
        const w = f.offsetWidth, h = f.offsetHeight
        const inset = b.inset ? this.chrome(k) / 2 : 0
        const ax = this.panX + b.x * this.zoom - inset - (b.align === "center" ? w / 2 : b.align === "right" ? w : 0)
        const ay = this.panY + b.y * this.zoom - inset - (b.base === "middle" ? h / 2 : b.base === "bottom" ? h : 0)
        this.editFrame.style.left = Math.max(4, Math.min(ax, this.width - w - 4)) + "px"
        this.editFrame.style.top = Math.max(4, Math.min(ay, this.height - h - 4)) + "px"
    }

    commitEdit() {
        const b = this.edit
        if (!b) return
        this.editKey = ""
        this.edit = null
        // A note gets the box its editor was resized to; otherwise it keeps
        // its own size.
        const k = this.zoom * (b.s || 1)
        const sized = b.multi && this.userSize !== null
        const w = sized ? (this.field.offsetWidth - this.chrome(k)) / k : 0
        const h = sized ? (this.field.offsetHeight - this.chrome(k)) / k : 0
        this.ctl.commitEdit(this.field.value, w, h)
        this.editFrame.hidden = true
        this.focus()
        this.app.changed()
    }

    cancelEdit() {
        this.editKey = ""
        this.edit = null
        this.ctl.cancelEdit()
        this.editFrame.hidden = true
        this.focus()
        this.app.changed()
    }
}

// drawing

function font(size, bold) {
    return (bold ? "bold " : "") + size + "px " + FONT
}

function polyline(ctx, pts) {
    ctx.beginPath()
    ctx.moveTo(pts[0], pts[1])
    for (let i = 2; i < pts.length; i += 2) ctx.lineTo(pts[i], pts[i + 1])
}

// Dashes for a line `w` wide (lengths in pixels, as in SVG).
function applyDash(ctx, style, w) {
    if (style === "dashed") ctx.setLineDash([4 * w + 2, 2.5 * w + 2])
    else if (style === "dotted") ctx.setLineDash([w, 1.5 * w + 1])
    else ctx.setLineDash([])
}

function drawHeads(ctx, heads, color, width) {
    ctx.setLineDash([])
    for (const h of heads) {
        if (h.t === "circle") {
            ctx.beginPath()
            ctx.arc(h.x, h.y, h.r, 0, 2 * Math.PI)
            ctx.fillStyle = PAPER
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

// A line's tip: a box of `tip.lines` (the first one bold) beside the
// pointer, at screen size, kept inside the view.
function drawTip(ctx, tip, view) {
    const sx = view.panX + tip.x * view.zoom, sy = view.panY + tip.y * view.zoom
    const pad = 7, lh = 17
    let w = 0
    tip.lines.forEach((t, i) => {
        ctx.font = font(12, i === 0)
        w = Math.max(w, ctx.measureText(t).width)
    })
    w += 2 * pad
    const h = tip.lines.length * lh + 2 * pad - 4
    let x = sx + 14, y = sy + 18
    if (x + w > view.width - 4) x = Math.max(4, sx - 14 - w)
    if (y + h > view.height - 4) y = Math.max(4, sy - 12 - h)
    ctx.save()
    ctx.setLineDash([])
    ctx.shadowColor = "rgba(0,0,0,0.25)"
    ctx.shadowBlur = 8
    roundRect(ctx, x, y, w, h, 5)
    ctx.globalAlpha = 0.94
    ctx.fillStyle = INK
    ctx.fill()
    ctx.globalAlpha = 1
    ctx.shadowBlur = 0
    ctx.fillStyle = PAPER
    ctx.textAlign = "left"
    ctx.textBaseline = "top"
    tip.lines.forEach((t, i) => {
        ctx.font = font(12, i === 0)
        ctx.fillText(t, x + pad, y + pad + i * lh)
    })
    ctx.restore()
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

function localNote(n) {
    return n.s === 1 ? n : Object.assign({}, n, { size: n.size / n.s, lh: n.lh / n.s, w: n.w / n.s, h: n.h / n.s })
}

function drawLink(ctx, l) {
    if (l.pts.length < 4) return
    // Tunnels and what carries the line: tinted bands, outermost first (the
    // tints add up inward), their ends cut along the device borders; the
    // edge solid, or dashed by type (VXLAN, CAPWAP, SD-WAN).
    for (let i = 0; i < l.bands.length; i++) {
        const b = l.bands[i]
        if (b.pts.length < 6) continue
        polyline(ctx, b.pts)
        ctx.closePath()
        if (i === 0) {
            ctx.fillStyle = PAPER
            ctx.fill()
        }
        const base = ctx.globalAlpha
        ctx.globalAlpha = base * 0.14
        ctx.fillStyle = l.color
        ctx.fill()
        ctx.globalAlpha = base
        ctx.setLineDash(b.dash || [])
        ctx.lineWidth = l.s
        ctx.strokeStyle = l.color
        ctx.stroke()
        ctx.setLineDash([])
    }
    if (l.sel || l.drop) {
        // Selected, or where the dragged device would be inserted: over the
        // bands, as wide as the outermost.
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
        ctx.strokeStyle = l.color
        glow(ctx, l.color, true)
        ctx.stroke()
        glow(ctx, l.color, false)
        ctx.lineWidth = 3 * l.s
        ctx.strokeStyle = PAPER
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
            const r = l.beadR * 0.9
            ctx.fillStyle = l.color
            ctx.fillRect(l.beads[i] - r, l.beads[i + 1] - r, 2 * r, 2 * r)
            continue
        }
        ctx.beginPath()
        ctx.arc(l.beads[i], l.beads[i + 1], l.beadR, 0, 2 * Math.PI)
        ctx.fillStyle = PAPER
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
            ctx.font = font(10, true)
            const tagw = l.tag !== "" ? ctx.measureText(l.tag).width : 0
            ctx.font = font(11)
            const labw = label !== "" ? ctx.measureText(label).width : 0
            const gap = tagw > 0 && labw > 0 ? 6 : 0
            const tw = tagw + gap + labw + 10
            const x0 = l.lx - tw / 2
            roundRect(ctx, x0, l.ly - 8.5, tw, 17, 3)
            ctx.fillStyle = PAPER
            ctx.fill()
            ctx.lineWidth = l.sel ? 1.5 : 0.8
            ctx.strokeStyle = l.sel ? ACCENT : l.color
            ctx.setLineDash([])
            ctx.stroke()
            ctx.fillStyle = l.color
            ctx.textAlign = "left"
            ctx.textBaseline = "middle"
            if (tagw > 0) {
                ctx.font = font(10, true)
                ctx.fillText(l.tag, x0 + 5, l.ly + 0.5)
            }
            ctx.font = font(11)
            if (labw > 0) ctx.fillText(label, x0 + 5 + tagw + gap, l.ly + 0.5)
        })
    }
    ctx.setLineDash([])
}

function drawDevice(ctx, d, target) {
    // k shrinks decorations (text, borders, ports) of a box you are zoomed
    // far into, so they stay readable instead of huge.
    const k = d.k || 1
    // The outline depends on the kind (see core/src/shape.rs).
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
            ctx.font = font(t.size)
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
        ctx.font = font(t.font, true)
        ctx.textAlign = "center"
        ctx.textBaseline = "middle"
        ctx.fillText(t.text, t.x + t.size / 2, t.y + t.size / 2 + t.font * 0.04)
        ctx.restore()
        tracePath(ctx, d.shape)
    }
    ctx.lineWidth = (d.sel ? 2 : 1.5) * k
    ctx.strokeStyle = d.sel ? ACCENT : d.stroke
    glow(ctx, d.stroke, true)
    ctx.stroke()
    glow(ctx, d.stroke, false)
    if (target === d.id) {
        roundRect(ctx, d.x - 3 * k, d.y - 3 * k, d.w + 6 * k, d.h + 6 * k, 7 * k)
        ctx.lineWidth = 2 * k
        ctx.setLineDash([5 * k, 4 * k])
        ctx.strokeStyle = GOOD
        ctx.stroke()
        ctx.setLineDash([])
    }

    ctx.fillStyle = d.text
    if (k < 1) {
        // Zoomed into it: its name as a small header in the corner.
        ctx.textAlign = "left"
        ctx.textBaseline = "top"
        ctx.font = font(13 * k, true)
        if (!d.editing) ctx.fillText(d.name + (d.kind !== "" ? "  ·  " + d.kindLabel.toUpperCase() : ""), d.x + 10 * k, d.y + 8 * k)
    } else {
        ctx.textAlign = "center"
        ctx.textBaseline = "middle"
        // A device with contents has its name at the top, out of their way.
        const cx = d.x + d.w / 2 + d.ldx
        const cy = d.container ? d.y + 14 + (d.kind === "" ? 0 : 6) : d.y + d.h / 2 + d.ldy
        ctx.font = font(13, true)
        if (d.kind === "") {
            if (!d.editing) ctx.fillText(d.name, cx, cy)
        } else {
            if (!d.editing) ctx.fillText(d.name, cx, cy - 6)
            ctx.font = font(10)
            const base = ctx.globalAlpha
            ctx.globalAlpha = base * 0.7
            ctx.fillText(d.kindLabel.toUpperCase(), cx, cy + 9)
            ctx.globalAlpha = base
        }
    }

    // Layer name in the lower right corner, when its layer shows names.
    if (d.badge !== "") {
        ctx.font = font(9 * k)
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
                ctx.strokeStyle = ACCENT
                ctx.stroke()
            }
        }
        ctx.fillStyle = on ? ACCENT : (p.linked ? d.stroke : PAPER)
        ctx.fillRect(p.x - r, p.y - r, 2 * r, 2 * r)
        ctx.lineWidth = 1.2 * k
        ctx.strokeStyle = on ? ACCENT : d.stroke
        ctx.strokeRect(p.x - r, p.y - r, 2 * r, 2 * r)
        if (p.name !== "" && !p.editing) {
            ctx.font = font(10 * k)
            ctx.fillStyle = MUTED
            ctx.textAlign = p.align
            ctx.textBaseline = p.base
            ctx.fillText(p.name, p.lx, p.ly)
        }
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

// Rectangles, lines and texts laid out by core/src/frame.rs.
function drawFrame(ctx, prims, view) {
    ctx.setLineDash([])
    for (const p of prims) {
        if (p.t === "rect") {
            if (p.radius > 0) roundRect(ctx, p.x, p.y, p.w, p.h, p.radius)
            else { ctx.beginPath(); ctx.rect(p.x, p.y, p.w, p.h) }
            if (p.fill) { ctx.fillStyle = p.fill; ctx.fill() }
            if (p.stroke) {
                ctx.lineWidth = p.width
                ctx.strokeStyle = p.stroke
                if (p.dash) ctx.setLineDash([4, 3])
                ctx.stroke()
                ctx.setLineDash([])
            }
        } else if (p.t === "line") {
            ctx.beginPath()
            ctx.moveTo(p.pts[0], p.pts[1])
            ctx.lineTo(p.pts[2], p.pts[3])
            ctx.lineWidth = p.width
            ctx.strokeStyle = p.color
            if (p.dash) ctx.setLineDash([5, 3])
            ctx.stroke()
            ctx.setLineDash([])
        } else if (p.t === "watermark") {
            ctx.save()
            ctx.translate(p.x, p.y)
            ctx.rotate(p.angle * Math.PI / 180)
            ctx.globalAlpha = ctx.globalAlpha * p.opacity
            ctx.font = font(p.size, true)
            ctx.fillStyle = p.color
            ctx.textAlign = "center"
            ctx.textBaseline = "middle"
            ctx.fillText(p.text, 0, 0)
            ctx.restore()
        } else if (p.t === "image") {
            const img = view.image(p.data)
            if (img) ctx.drawImage(img, p.x, p.y, p.w, p.h)
        } else {
            ctx.font = font(p.size, p.bold)
            ctx.fillStyle = p.color
            ctx.textAlign = p.align
            ctx.textBaseline = "middle"
            ctx.fillText(p.text, p.x, p.y)
        }
    }
}
