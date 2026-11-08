// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// Stratagram in the browser: the toolbar, the diagram view and the panel
// around the editor compiled to WebAssembly (a port of qml/main.qml).
//
//   import { mount } from "./app.js"
//   const app = await mount(document.getElementById("editor"), { host })
//
// Where diagrams come from and go to is up to the page embedding the editor,
// given as `host` (see `fileHost` for the default, plain files):
//
//   host.open()              -> Promise<{json, name} | null>   (optional: no Open button)
//   host.save(json, name, as) -> Promise<string | null>       the name saved under, null if not
//   host.initial             {json, name}, the diagram to start with (optional)
//   host.canNew              show the New button (default true)
//   host.buttons             [{label, title, run(app)}], extra toolbar buttons
//   host.title(text)         show the window title (default: document.title)
//   host.dirty(bool)         unsaved changes appeared or went away (optional)
//   host.theme               "light", "deep" or "neon" until the user picks one (optional)

import init, { Stratagram } from "./pkg/stratagram_web.js"
import { DiagramView } from "./canvas.js"
import { KIND_GROUPS, Panel, h } from "./panel.js"

const LAYOUTS = [
    ["hierarchy", "Hierarchy ↓", "Layers from the internet and firewalls down to servers; redundant pairs side by side"],
    ["hierarchy-lr", "Hierarchy →", "The same, left to right"],
    ["radial", "Radial", "Rings around the most upstream device"],
    ["circle", "Circle", "All devices on one circle, neighbors together"],
    ["grid", "Grid", "Rows of equal cells, neighbors together"],
    ["force", "Force-directed", "Organic: linked devices pull together (run again to refine)"],
    ["tidy", "Tidy", "Straighten what is there: align rows and columns, center, straight lines; devices barely move"],
]

// Clipboard format for Stratagram's own copies (draw.io XML goes as text).
const NATIVE_TYPE = "application/x-stratagram"

let ready = null

export async function mount(root, options = {}) {
    ready = ready || init()
    await ready
    return new App(root, options)
}

// Settings kept in the browser, per site.
class Prefs {
    constructor(defaults) {
        this.values = Object.assign({}, defaults)
        try { Object.assign(this.values, JSON.parse(localStorage.getItem("stratagram.ui") || "{}")) } catch { /* none */ }
    }
    get(key) { return this.values[key] }
    set(key, value) {
        this.values[key] = value
        try { localStorage.setItem("stratagram.ui", JSON.stringify(this.values)) } catch { /* not kept */ }
    }
}

export class App {
    constructor(root, options) {
        this.root = root
        this.host = options.host || fileHost()
        this.ctl = new Stratagram()
        this.prefs = new Prefs({ panelVisible: true, layoutGap: 60, layoutTidyPorts: true, layoutAvoid: true,
                                 showGrid: true, snap: true })
        this.clip = null

        this.el = h("div", { className: "sg-app" })
        // The theme the user picked here last, else the host's.
        let theme = null
        try { theme = localStorage.getItem("stratagram.theme") } catch { /* none */ }
        this.theme = theme || this.host.theme || "light"
        if (!this.ctl.setTheme(this.theme)) { this.theme = "light"; this.ctl.setTheme("light") }
        this.el.dataset.theme = this.theme
        root.replaceChildren(this.el)
        this.toolbar = h("div", { className: "sg-toolbar" })
        const main = h("div", { className: "sg-main" })
        this.status = h("div", { className: "sg-status" })
        this.el.append(this.toolbar, main,
                       h("div", { className: "sg-statusline" }, this.status,
                         h("span", { className: "sg-version" }, "Stratagram " + Stratagram.version())))
        // The canvas with the sheet tabs under it.
        const stage = h("div", { className: "sg-stage" })
        main.appendChild(stage)
        this.view = new DiagramView(this, stage)
        this.view.showGrid = this.prefs.get("showGrid")
        this.sheetBar = h("div", { className: "sg-sheets" })
        stage.appendChild(this.sheetBar)
        this.forgetSheetViews()
        this.handle = h("div", { className: "sg-handle", onclick: () => this.togglePanel() })
        main.appendChild(this.handle)
        this.panel = new Panel(this, main)
        this.buildToolbar()
        this.setPanelVisible(this.prefs.get("panelVisible"))
        this.listen()

        const initial = this.host.initial
        if (initial && initial.json) this.ctl.load(initial.json, initial.name || "diagram.stratagram")
        else this.ctl.setStatus("Double-click to add a device. Drag from a device border or port to another device to connect.")
        this.ctl.setView(1, this.prefs.get("snap"))
        this.changed()
        requestAnimationFrame(() => this.view.fit())
    }

    // state

    // The editor changed (or may have): show it.
    changed() {
        const ctl = this.ctl
        this.view.setScene(ctl.scene())
        this.view.setEditBox(ctl.editBox())
        this.panel.update(ctl.selection(), ctl.layers())
        this.status.textContent = ctl.status()
        this.status.title = ctl.status()
        this.buttons.undo.disabled = !ctl.canUndo()
        this.buttons.redo.disabled = !ctl.canRedo()
        const sheets = ctl.sheets()
        if (sheets !== this.lastSheets && !this.renamingSheet) {
            this.lastSheets = sheets
            const list = JSON.parse(sheets)
            this.renderSheets(list)
            this.sheetShown(list.findIndex(x => x.active))
        }
        const title = ctl.title()
        if (title !== this.lastTitle) {
            this.lastTitle = title
            if (this.host.title) this.host.title(title)
            else document.title = title
        }
        const dirty = ctl.dirty()
        if (dirty !== this.lastDirty) {
            this.lastDirty = dirty
            if (this.host.dirty) this.host.dirty(dirty)
        }
    }

    // sheets

    // The sheet tabs: click to switch, double-click to rename in place, drag
    // sideways to reorder, × to remove (asking first if it has something on
    // it), + to add.
    renderSheets(list) {
        const ctl = this.ctl
        const tabs = list.map((sheet, i) => {
            const name = h("span", { className: "sg-sheet-name" }, sheet.name)
            const close = list.length > 1 ? h("button", {
                className: "sg-sheet-close", title: "Remove this sheet",
                onclick: (e) => {
                    e.stopPropagation()
                    if (sheet.empty || confirm(`Remove "${sheet.name}" and everything on it?\nYou can undo this with Ctrl+Z.`))
                        this.run(() => ctl.removeSheet(i))
                    this.view.focus()
                },
            }, "×") : null
            const tab = h("div", { className: "sg-sheet" + (sheet.active ? " sg-active" : ""), title: "Double-click to rename" },
                name, close)
            tab.addEventListener("click", () => {
                // (A drag ends in a click too: it only reorders.)
                if (this.sheetDragged) return
                this.run(() => ctl.switchSheet(i))
                this.view.focus()
            })
            this.dragSheet(tab, i)
            tab.addEventListener("dblclick", () => this.renameSheet(i, sheet.name, name))
            return tab
        })
        const add = h("button", { className: "sg-sheet-add", title: "Add a sheet",
                                  onclick: () => { this.run(() => ctl.addSheet()); this.view.focus() } }, "+")
        this.sheetBar.replaceChildren(...tabs, add)
    }

    // Dragging `tab` (sheet `i`) sideways onto another tab moves it to that
    // tab's place; a bar shows where it goes.
    dragSheet(tab, i) {
        let start = null, target = -1
        const tabs = () => [...this.sheetBar.querySelectorAll(".sg-sheet")]
        const mark = (j) => tabs().forEach((t, k) => {
            t.classList.toggle("sg-drop-before", k === j && j < i)
            t.classList.toggle("sg-drop-after", k === j && j > i)
        })
        tab.addEventListener("pointerdown", (e) => {
            if (e.button !== 0 || e.target.tagName === "INPUT" || e.target.tagName === "BUTTON") return
            start = e.clientX
            target = -1
        })
        tab.addEventListener("pointermove", (e) => {
            if (start === null) return
            if (target < 0 && Math.abs(e.clientX - start) < 6) return
            if (target < 0) tab.setPointerCapture(e.pointerId)
            const all = tabs()
            const over = all.findIndex(t => { const r = t.getBoundingClientRect(); return e.clientX >= r.left && e.clientX < r.right })
            target = over >= 0 ? over : (e.clientX < all[0].getBoundingClientRect().left ? 0 : all.length - 1)
            mark(target)
        })
        const end = (e) => {
            if (start === null) return
            const moved = target >= 0
            start = null
            mark(-1)
            if (moved) {
                // The click that follows the drop is not a click; after it
                // (or right away, if the tabs were redrawn) clicks count again.
                this.sheetDragged = e.type === "pointerup"
                setTimeout(() => { this.sheetDragged = false })
                if (target !== i) this.run(() => this.ctl.moveSheet(i, target))
                this.view.focus()
            }
        }
        tab.addEventListener("pointerup", end)
        tab.addEventListener("pointercancel", end)
    }

    // Rename sheet `i` in place of its name `label`.
    renameSheet(i, current, label) {
        const field = h("input", { type: "text", value: current })
        let done = false
        this.renamingSheet = true
        const finish = (save) => {
            if (done) return
            done = true
            this.renamingSheet = false
            this.lastSheets = ""   // draw the tabs again either way
            if (save) this.ctl.renameSheet(i, field.value)
            this.changed()
            this.view.focus()
        }
        field.addEventListener("keydown", (e) => {
            e.stopPropagation()
            if (e.key === "Enter") finish(true)
            else if (e.key === "Escape") finish(false)
        })
        field.addEventListener("click", (e) => e.stopPropagation())
        field.addEventListener("dblclick", (e) => e.stopPropagation())
        field.addEventListener("blur", () => finish(true))
        label.replaceWith(field)
        field.focus()
        field.select()
    }

    // Sheet `i` is shown now. Each sheet keeps its own view: back on a
    // sheet, it is where you left it; a sheet not seen yet is fitted.
    sheetShown(i) {
        if (i === this.shownSheet) return
        const v = this.view
        this.sheetViews[this.shownSheet] = { zoom: v.zoom, panX: v.panX, panY: v.panY }
        this.shownSheet = i
        const saved = this.sheetViews[i]
        if (saved) {
            v.autoFit = false
            v.panX = saved.panX
            v.panY = saved.panY
            v.setZoom(saved.zoom)
            v.repaint()
        } else {
            requestAnimationFrame(() => v.fit())
        }
    }

    // Another document: none of the views kept apply.
    forgetSheetViews() {
        this.sheetViews = {}
        this.shownSheet = 0
    }

    // The zoom changed: what is big enough to show depends on it.
    viewChanged() {
        this.ctl.setView(this.view.zoom, this.prefs.get("snap"))
        this.zoomLabel.textContent = Math.round(this.view.zoom * 100) + "%"
        this.changed()
    }

    run(f) { f(); this.changed() }

    // toolbar

    buildToolbar() {
        const ctl = this.ctl, view = this.view
        const b = (label, title, f, extra = {}) => h("button", Object.assign({ title, onclick: () => { f(); view.focus() } }, extra), label)
        const sep = () => h("span", { className: "sg-sep" })
        this.buttons = {
            undo: b("Undo", "Undo (Ctrl+Z)", () => this.run(() => ctl.undo())),
            redo: b("Redo", "Redo (Ctrl+Shift+Z)", () => this.run(() => ctl.redo())),
        }
        const canNew = this.host.canNew !== false
        const file = [
            canNew ? b("New", "A new diagram", () => this.newDiagram()) : null,
            this.host.open ? b("Open…", "Open a diagram (Ctrl+O)", () => this.open()) : null,
            b("Save", "Save (Ctrl+S)", () => this.save(false)),
            this.host.saveAs !== false ? b("Save as…", "Save under another name (Ctrl+Shift+S)", () => this.save(true)) : null,
        ]
        const exportButton = b("Export ▾", "Export and copy", () => this.menu(exportButton, [
            ["Export SVG…", "Ctrl+E", () => this.exportSvg()],
            ["Export PNG…", "Ctrl+Alt+E", () => this.exportPng()],
            ["Export draw.io…", "Ctrl+Shift+E", () => this.exportDrawio()],
            ["Export CML lab…", "", () => this.exportCml()],
            null,
            ["Copy for draw.io", "Ctrl+C", () => this.copyButton()],
            ["Copy sheet with layers and frame", "", () => this.copySheet()],
            ["Paste (from Stratagram or draw.io)", "Ctrl+V", () => this.pasteButton()],
        ]))
        const center = (f) => () => { const c = view.viewCenter(); this.run(() => f(c)) }
        const addOf = (kind) => center(c => ctl.addDeviceOf(kind, c.x, c.y))
        const addKind = b("▾", "Add a device of a type", () => this.menu(addKind, [
            ["plain device", "", addOf("")],
            ...KIND_GROUPS.flatMap(([group, kinds]) => [group, ...kinds.map(k => [k, "", addOf(k)])]),
        ]), { className: "sg-split-arrow" })
        this.layoutButton = b("Layout ▾", "Auto layout (Ctrl+L)", () => this.layoutPopup())
        const snap = h("input", { type: "checkbox", checked: this.prefs.get("snap") })
        snap.addEventListener("change", () => { this.prefs.set("snap", snap.checked); this.viewChanged() })
        this.gridBox = h("input", { type: "checkbox", checked: this.prefs.get("showGrid") })
        this.gridBox.addEventListener("change", () => this.setGrid(this.gridBox.checked))
        this.themeBox = h("select", { className: "sg-theme", title: "Theme: the colors of the diagram, its exports (SVG, PNG) and the editor.\nThe file keeps its own colors." },
                          ...[["light", "Light"], ["deep", "Dark"], ["neon", "Neon"]].map(([v, t]) => h("option", { value: v }, t)))
        this.themeBox.value = this.theme
        this.themeBox.addEventListener("change", () => this.setTheme(this.themeBox.value))
        this.quick = h("input", { type: "text", className: "sg-quick", placeholder: "Quick add (Ctrl+K): Firewall, Router, switch|my-box",
                                   title: "Comma-separated devices, then Enter.\n\"Router\" adds a router labeled Router;\n" +
                                          "\"router|edge-01\" (type, then name) adds a router labeled edge-01." })
        this.quick.addEventListener("keydown", (e) => {
            e.stopPropagation()
            if (e.key === "Enter") {
                const c = view.viewCenter()
                if (ctl.quickAdd(this.quick.value, c.x, c.y) > 0) {
                    this.quick.value = ""
                    this.changed()
                    view.revealSelection()
                    view.focus()
                }
            } else if (e.key === "Escape") { this.quick.value = ""; view.focus() }
        })
        this.deepButton = b("Zoom into", "Deep zoom (Ctrl+I): zoom into a device without limit and add things inside it", () => this.toggleDeep())
        this.zoomLabel = h("span", { className: "sg-zoom" }, "100%")
        this.panelButton = b("Panel", "Show or hide the panel (Ctrl+B)", () => this.togglePanel())
        // (Buttons the host leaves out are null.)
        const items = [
            ...file, exportButton, sep(),
            // "+ Device" adds a plain device; the arrow beside it one of a type.
            b("+ Device", "Add a device", center(c => ctl.addDevice(c.x, c.y)), { className: "sg-strong sg-split-main" }),
            addKind,
            b("+ Text", "Add a text", center(c => ctl.addNote(c.x - 20, c.y - 10)), { className: "sg-strong" }),
            b("+ Point", "Free connection point for lines that point at nothing", center(c => ctl.addPoint(c.x, c.y)), { className: "sg-strong" }),
            this.buttons.undo, this.buttons.redo,
            b("Duplicate", "Duplicate (Ctrl+D)", () => this.run(() => ctl.duplicate())),
            b("Delete", "Delete (Del)", () => this.run(() => ctl.deleteSelected())),
            sep(), this.layoutButton, sep(),
            h("label", { className: "sg-check", title: "Snap to grid" }, snap, "Snap"),
            h("label", { className: "sg-check", title: "Show the grid (Ctrl+')" }, this.gridBox, "Grid"),
            this.themeBox,
            sep(), this.quick,
            ...(this.host.buttons || []).map(x => b(x.label, x.title || "", () => x.run(this))),
            h("span", { className: "sg-grow" }),
            this.deepButton,
            b("−", "Zoom out (Ctrl+-)", () => view.zoomBy(1 / 1.2)), this.zoomLabel, b("+", "Zoom in (Ctrl++)", () => view.zoomBy(1.2)),
            b("Fit", "Fit the diagram into view (Ctrl+0)", () => view.fit()),
            sep(), this.panelButton]
        this.toolbar.append(...items.filter(Boolean))
    }

    // A dropdown menu under `anchor` (again: closes it): items are [label,
    // shortcut, action], null (a line) or a text (a heading).
    menu(anchor, items) {
        if (this.openPopup && this.popupAnchor === anchor) { this.closePopup(); return }
        const m = h("div", { className: "sg-popup sg-menu" }, items.map(i => i === null ? h("hr") :
            typeof i === "string" ? h("div", { className: "sg-menu-head" }, i) :
            h("button", { onclick: () => { close(); i[2]() } }, h("span", {}, i[0]), h("span", { className: "sg-key" }, i[1]))))
        this.popup(anchor, m)
        const close = () => this.closePopup()
    }

    popup(anchor, el) {
        this.closePopup()
        const a = anchor.getBoundingClientRect(), app = this.el.getBoundingClientRect()
        el.style.left = (a.left - app.left) + "px"
        el.style.top = (a.bottom - app.top + 2) + "px"
        this.el.appendChild(el)
        this.openPopup = el
        anchor.classList.add("sg-on")
        this.popupAnchor = anchor
        const outside = (e) => { if (!el.contains(e.target) && !anchor.contains(e.target)) this.closePopup() }
        const escape = (e) => { if (e.key === "Escape") { this.closePopup(); this.view.focus() } }
        this.popupCleanup = () => {
            document.removeEventListener("pointerdown", outside, true)
            document.removeEventListener("keydown", escape, true)
        }
        setTimeout(() => {
            document.addEventListener("pointerdown", outside, true)
            document.addEventListener("keydown", escape, true)
        })
        // Keep it inside the app (long ones scroll).
        el.style.maxHeight = Math.max(120, app.bottom - a.bottom - 12) + "px"
        const r = el.getBoundingClientRect()
        if (r.right > app.right - 4) el.style.left = Math.max(4, app.right - app.left - r.width - 4) + "px"
    }

    closePopup() {
        if (!this.openPopup) return
        this.openPopup.remove()
        this.openPopup = null
        this.popupAnchor.classList.remove("sg-on")
        this.popupCleanup()
    }

    // Auto layout: pick an arrangement; it applies at once (Ctrl+Z undoes),
    // and the popup stays open to try another.
    layoutPopup() {
        if (this.openPopup && this.popupAnchor === this.layoutButton) { this.closePopup(); return }
        const p = this.prefs
        const apply = (kind) => {
            if (this.ctl.autoLayout(kind, p.get("layoutGap"), p.get("layoutTidyPorts"), p.get("layoutAvoid"))) {
                this.changed()
                this.view.fit()
            } else this.changed()
        }
        const gap = h("input", { type: "number", min: 10, max: 400, step: 10, value: p.get("layoutGap") })
        gap.addEventListener("change", () => p.set("layoutGap", Math.min(400, Math.max(10, Number(gap.value) || 60))))
        const check = (key, label, title) => {
            const c = h("input", { type: "checkbox", checked: p.get(key) })
            c.addEventListener("change", () => p.set(key, c.checked))
            return h("label", { className: "sg-check", title }, c, label)
        }
        const el = h("div", { className: "sg-popup sg-layout" },
            h("h3", {}, "Auto layout"),
            h("p", { className: "sg-hint" }, "Arranges the selected devices, or all shown and unlocked ones when fewer than two are selected. Ctrl+Z undoes."),
            LAYOUTS.map(([kind, name, about]) => h("button", { className: "sg-choice", onclick: () => apply(kind) },
                h("b", {}, name), h("span", {}, about))),
            h("label", { className: "sg-inline" }, "Spacing", gap),
            check("layoutTidyPorts", "Move ports to face their links"),
            check("layoutAvoid", "Route lines around objects",
                  "Lines that would cross a device or text get a routing that does not (straight or orthogonal if that is enough, else around objects)"))
        this.popup(this.layoutButton, el)
    }

    // Draw in theme `name` ("light", "deep", "neon"); picked by the user it
    // is remembered in this browser (`remember`), else it is the host's.
    setTheme(name, remember = true) {
        if (!this.ctl.setTheme(name)) return
        this.theme = name
        this.el.dataset.theme = name
        if (this.themeBox) this.themeBox.value = name
        if (remember) try { localStorage.setItem("stratagram.theme", name) } catch { /* not kept */ }
        this.changed()
    }

    // Whether the user picked a theme here (the host's does not apply then).
    themePicked() {
        try { return localStorage.getItem("stratagram.theme") !== null } catch { return false }
    }

    setGrid(on) {
        this.prefs.set("showGrid", on)
        this.gridBox.checked = on
        this.view.showGrid = on
        this.view.repaint()
    }

    setPanelVisible(on) {
        this.prefs.set("panelVisible", on)
        this.panel.el.hidden = !on
        this.panelButton.classList.toggle("sg-on", on)
        this.handle.textContent = on ? "▸" : "◂"
        this.handle.title = (on ? "Hide" : "Show") + " panel (Ctrl+B)"
    }

    togglePanel() { this.setPanelVisible(!this.prefs.get("panelVisible")) }

    toggleDeep() {
        const on = !this.view.deepZoom
        this.deepButton.classList.toggle("sg-on", on)
        this.view.setDeepZoom(on)
        this.run(() => this.ctl.setDeepZoom(on))
    }

    // keys and clipboard

    listen() {
        const editable = (t) => t && (t.tagName === "INPUT" || t.tagName === "TEXTAREA" || t.tagName === "SELECT" || t.isContentEditable)
        this.keyHandler = (e) => {
            // Only while the editor has focus (or nothing else on the page does).
            const a = document.activeElement
            if (!this.el.contains(a) && a !== document.body) return
            const ctrl = e.ctrlKey || e.metaKey
            if (!ctrl) return
            const key = e.key.toLowerCase()
            const inField = editable(e.target)
            const ctl = this.ctl, view = this.view
            const act = {
                "s": () => this.save(e.shiftKey),
                "o": () => this.host.open && this.open(),
                "e": () => e.altKey ? this.exportPng() : e.shiftKey ? this.exportDrawio() : this.exportSvg(),
                "k": () => this.quick.focus(),
                "l": () => this.layoutPopup(),
                "b": () => this.togglePanel(),
                "i": () => this.toggleDeep(),
                "'": () => this.setGrid(!this.prefs.get("showGrid")),
                "0": () => view.fit(),
                "=": () => view.zoomBy(1.2),
                "+": () => view.zoomBy(1.2),
                "-": () => view.zoomBy(1 / 1.2),
            }[key]
            // These belong to text fields while one has focus.
            const canvasOnly = {
                "z": () => this.run(() => e.shiftKey ? ctl.redo() : ctl.undo()),
                "y": () => this.run(() => ctl.redo()),
                "a": () => this.run(() => ctl.selectAll()),
                "d": () => this.run(() => ctl.duplicate()),
                "]": () => this.run(() => ctl.arrange("forward")),
                "[": () => this.run(() => ctl.arrange("backward")),
            }[key]
            const f = act || (!inField ? canvasOnly : null)
            if (!f) return
            e.preventDefault()
            e.stopPropagation()
            f()
        }
        document.addEventListener("keydown", this.keyHandler)

        // Copy, cut and paste go through the browser's clipboard events, which
        // may write and read it without asking.
        const onCanvas = (e) => !editable(e.target) && (this.el.contains(e.target) || e.target === document.body)
        this.copyHandler = (e) => {
            if (!onCanvas(e)) return
            const cut = e.type === "cut"
            const xml = this.ctl.copyDrawio()
            if (xml === "") { if (cut) this.run(() => this.ctl.setStatus("Select something to cut.")); return }
            const native = this.ctl.copyNative()
            this.clip = { xml, native }
            e.clipboardData.setData("text/plain", xml)
            try { e.clipboardData.setData(NATIVE_TYPE, native) } catch { /* the copy in memory serves */ }
            e.preventDefault()
            if (cut) this.run(() => { this.ctl.deleteSelected(); this.ctl.setStatus("Cut: paste it with Ctrl+V, here or in draw.io.") })
            else this.changed()
        }
        this.pasteHandler = (e) => {
            if (!onCanvas(e)) return
            const text = e.clipboardData.getData("text/plain")
            let native = ""
            try { native = e.clipboardData.getData(NATIVE_TYPE) } catch { /* none */ }
            if (!native && this.clip && this.clip.xml === text) native = this.clip.native
            e.preventDefault()
            this.paste(native, text)
        }
        document.addEventListener("copy", this.copyHandler)
        document.addEventListener("cut", this.copyHandler)
        document.addEventListener("paste", this.pasteHandler)
        this.unloadHandler = (e) => { if (this.ctl.dirty()) { e.preventDefault(); e.returnValue = "" } }
        window.addEventListener("beforeunload", this.unloadHandler)
    }

    // Stop listening to the page (when the editor is taken off it).
    destroy() {
        document.removeEventListener("keydown", this.keyHandler)
        document.removeEventListener("copy", this.copyHandler)
        document.removeEventListener("cut", this.copyHandler)
        document.removeEventListener("paste", this.pasteHandler)
        window.removeEventListener("beforeunload", this.unloadHandler)
        this.root.replaceChildren()
    }

    paste(native, text) {
        const r = this.view.viewRect()
        this.run(() => this.ctl.paste(native, text, r.x, r.y, r.w, r.h))
        this.view.focus()
    }

    // The menu's Copy: the clipboard API, as there is no copy event.
    async copyButton() {
        const xml = this.ctl.copyDrawio()
        if (xml === "") return
        this.clip = { xml, native: this.ctl.copyNative() }
        try {
            await navigator.clipboard.writeText(xml)
            this.run(() => this.ctl.setStatus("Copied: paste it with Ctrl+V, here or in draw.io."))
        } catch {
            this.run(() => this.ctl.setStatus("The browser did not allow copying; use Ctrl+C."))
        }
    }

    // The whole sheet with its layers and frame, as text: pasted into
    // Stratagram (here, in another window, in the app), they come along.
    async copySheet() {
        const json = this.ctl.copySheet()
        this.clip = { xml: json, native: json }
        try {
            await navigator.clipboard.writeText(json)
            this.changed()
            return
        } catch { /* not allowed, or not on https: the older way */ }
        const area = h("textarea", { value: json, style: "position:fixed;left:-9999px;top:0" })
        document.body.append(area)
        area.select()
        const ok = document.execCommand("copy")
        area.remove()
        if (ok) this.changed()
        else this.run(() => this.ctl.setStatus("The browser did not allow copying."))
        this.view.focus()
    }

    async pasteButton() {
        try {
            const text = await navigator.clipboard.readText()
            this.paste(this.clip && this.clip.xml === text ? this.clip.native : "", text)
        } catch {
            this.run(() => this.ctl.setStatus("The browser did not allow reading the clipboard; use Ctrl+V."))
        }
    }

    // files

    // Go on only when unsaved changes may be dropped.
    discardOk() {
        return !this.ctl.dirty() || confirm("The diagram has unsaved changes. Discard them?")
    }

    newDiagram() {
        if (!this.discardOk()) return
        this.forgetSheetViews()
        this.run(() => this.ctl.newDiagram())
        this.view.fit()
    }

    async open() {
        if (!this.discardOk()) return
        const got = await this.host.open()
        if (!got) return
        this.forgetSheetViews()
        const err = this.ctl.load(got.json, got.name)
        this.changed()
        if (err === "") this.view.fit()
    }

    // The base name for exports.
    baseName() {
        return (this.ctl.fileName() || "diagram").replace(/\.(stratagram|netdiag|json)$/i, "")
    }

    async save(as) {
        const name = this.ctl.fileName() || "diagram.stratagram"
        try {
            const saved = await this.host.save(this.ctl.toJson(), name, as)
            if (saved) this.ctl.savedAs(saved)
        } catch (e) {
            this.ctl.setStatus("Not saved: " + (e && e.message ? e.message : e))
        }
        this.changed()
    }

    exportSvg() {
        download(this.ctl.exportSvg(), this.baseName() + ".svg", "image/svg+xml")
        this.run(() => this.ctl.setStatus("Exported " + this.baseName() + ".svg"))
    }

    exportDrawio() {
        download(this.ctl.exportDrawio(this.baseName()), this.baseName() + ".drawio", "application/xml")
        this.run(() => this.ctl.setStatus("Exported " + this.baseName() + ".drawio"))
    }

    // Cisco Modeling Labs: the sheet in view as a lab to import.
    exportCml() {
        const yaml = this.ctl.exportCml(this.baseName())
        download(yaml, this.baseName() + ".yaml", "application/x-yaml")
        const left = this.ctl.status()
        this.run(() => this.ctl.setStatus("Exported " + this.baseName() + ".yaml" + (left ? "; " + left : "")))
    }

    // PNG: the SVG export drawn at twice its size (sharp on high resolution
    // screens and in documents).
    async exportPng(scale = 2) {
        const svg = this.ctl.exportSvg()
        const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }))
        try {
            const img = new Image()
            await new Promise((ok, fail) => { img.onload = ok; img.onerror = () => fail(new Error("cannot draw the SVG")); img.src = url })
            const canvas = document.createElement("canvas")
            canvas.width = Math.ceil(img.naturalWidth * scale)
            canvas.height = Math.ceil(img.naturalHeight * scale)
            const ctx = canvas.getContext("2d")
            ctx.scale(scale, scale)
            ctx.drawImage(img, 0, 0)
            const blob = await new Promise(ok => canvas.toBlob(ok, "image/png"))
            download(blob, this.baseName() + ".png", "image/png")
            this.run(() => this.ctl.setStatus("Exported " + this.baseName() + ".png"))
        } catch (e) {
            this.run(() => this.ctl.setStatus("PNG export failed: " + e.message))
        } finally {
            URL.revokeObjectURL(url)
        }
    }
}

// Save `data` (text or a Blob) as a file the browser downloads.
export function download(data, name, type) {
    const blob = data instanceof Blob ? data : new Blob([data], { type })
    const url = URL.createObjectURL(blob)
    const a = h("a", { href: url, download: name })
    document.body.appendChild(a)
    a.click()
    a.remove()
    setTimeout(() => URL.revokeObjectURL(url), 1000)
}

// The default host: diagrams are files on this computer. Where the browser
// can save to a chosen file (Chrome, Edge), Save writes it again; elsewhere
// every save is a download.
export function fileHost() {
    let handle = null
    const types = [{ description: "Network diagrams", accept: { "application/json": [".stratagram", ".netdiag", ".json"] } }]
    return {
        async open() {
            if (window.showOpenFilePicker) {
                try {
                    const [h] = await window.showOpenFilePicker({ types })
                    const file = await h.getFile()
                    handle = h
                    return { json: await file.text(), name: file.name }
                } catch (e) {
                    if (e.name === "AbortError") return null
                    throw e
                }
            }
            return new Promise((resolve) => {
                const input = h("input", { type: "file", accept: ".stratagram,.netdiag,.json" })
                input.addEventListener("change", async () => {
                    const file = input.files[0]
                    handle = null
                    resolve(file ? { json: await file.text(), name: file.name } : null)
                })
                input.addEventListener("cancel", () => resolve(null))
                input.click()
            })
        },
        async save(json, name, as) {
            const suggested = name.replace(/\.(netdiag|json)$/i, "").replace(/(\.stratagram)?$/i, ".stratagram")
            if (window.showSaveFilePicker) {
                try {
                    if (as || !handle) handle = await window.showSaveFilePicker({ suggestedName: suggested, types: types.slice(0, 1) })
                    const w = await handle.createWritable()
                    await w.write(json)
                    await w.close()
                    return handle.name
                } catch (e) {
                    if (e.name === "AbortError") return null
                    throw e
                }
            }
            // (As JSON, browsers may show it instead of saving it.)
            download(json, suggested, "application/octet-stream")
            return suggested
        },
    }
}
