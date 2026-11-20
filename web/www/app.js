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

import { language, languages, languageEvents, setLanguage, tr } from "./i18n.js"
import { translate, translateStatus } from "./locales/messages.mjs"
import init, { Stratagram } from "./pkg/stratagram_web.js"
import { DiagramView } from "./canvas.js"
import { KIND_GROUPS, Panel, h } from "./panel.js"

const LAYOUTS = [
    ["hierarchy", "layout.hierarchy", "layout.hierarchyHint"],
    ["hierarchy-lr", "layout.hierarchyLR", "layout.hierarchyLRHint"],
    ["radial", "layout.radial", "layout.radialHint"],
    ["circle", "layout.circle", "layout.circleHint"],
    ["grid", "layout.grid", "layout.gridHint"],
    ["force", "layout.force", "layout.forceHint"],
    ["tidy", "layout.tidy", "layout.tidyHint"],
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
        this.ctl = new Stratagram(language)
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
        this.el.lang = language
        this.ctl.setLanguage(language)
        this.buildToolbar()
        this.languageChanged = () => {
            this.closePopup()
            const hadFocus = this.languageBox === document.activeElement
            const quick = this.quick.value
            this.el.lang = language
            this.ctl.setLanguage(language)
            this.buildToolbar()
            this.quick.value = quick
            this.deepButton.classList.toggle("sg-on", this.view.deepZoom)
            this.zoomLabel.textContent = Math.round(this.view.zoom * 100) + "%"
            this.setPanelVisible(this.prefs.get("panelVisible"))
            this.panel.render()
            this.panel.renderLayers()
            this.lastSheets = null
            this.changed()
            this.view.repaint()
            if (hadFocus) this.languageBox.focus()
        }
        languageEvents.addEventListener("change", this.languageChanged)
        this.setPanelVisible(this.prefs.get("panelVisible"))
        this.listen()

        const initial = this.host.initial
        if (initial && initial.json) this.ctl.load(initial.json, initial.name || "diagram.stratagram")
        else this.setStatus("status.welcome")
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
        const status = ctl.status()
        if (this.uiStatus && this.uiStatus.text !== status) this.uiStatus = null
        this.status.textContent = this.uiStatus ? tr(this.uiStatus.key, this.uiStatus.values) : translateStatus(language, status)
        this.status.title = this.status.textContent
        this.buttons.undo.disabled = !ctl.canUndo()
        this.buttons.redo.disabled = !ctl.canRedo()
        const sheets = ctl.sheets()
        if (sheets !== this.lastSheets && !this.renamingSheet) {
            this.lastSheets = sheets
            const list = JSON.parse(sheets)
            this.renderSheets(list)
            this.sheetShown(list.findIndex(x => x.active))
        }
        const title = ctl.fileName() ? ctl.title() : tr(ctl.dirty() ? "file.untitledDirty" : "file.untitled")
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
                className: "sg-sheet-close", title: tr("sheet.removeHint"),
                onclick: (e) => {
                    e.stopPropagation()
                    if (sheet.empty || confirm(tr("confirm.removeSheet", { name: sheet.name })))
                        this.run(() => ctl.removeSheet(i))
                    this.view.focus()
                },
            }, "×") : null
            const tab = h("div", { className: "sg-sheet" + (sheet.active ? " sg-active" : ""), title: tr("sheet.renameHint") },
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
        const add = h("button", { className: "sg-sheet-add", title: tr("sheet.addHint"),
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

    setStatus(key, values) {
        const text = translate("en", key, values)
        this.uiStatus = { key, values, text }
        this.ctl.setStatus(text)
    }

    run(f) { f(); this.changed() }

    // toolbar

    buildToolbar() {
        const ctl = this.ctl, view = this.view
        const b = (label, title, f, extra = {}) => h("button", Object.assign({ title, onclick: () => { f(); view.focus() } }, extra), label)
        const sep = () => h("span", { className: "sg-sep" })
        this.buttons = {
            undo: b(tr("edit.undo"), tr("edit.undoHint"), () => this.run(() => ctl.undo())),
            redo: b(tr("edit.redo"), tr("edit.redoHint"), () => this.run(() => ctl.redo())),
        }
        const canNew = this.host.canNew !== false
        const file = [
            canNew ? b(tr("file.new"), tr("file.newHint"), () => this.newDiagram()) : null,
            this.host.open ? b(tr("file.open"), tr("file.openHint"), () => this.open()) : null,
            b(tr("file.save"), tr("file.saveHint"), () => this.save(false)),
            this.host.saveAs !== false ? b(tr("file.saveAs"), tr("file.saveAsHint"), () => this.save(true)) : null,
        ]
        const exportButton = b(tr("file.exportMenu"), tr("file.exportHint"), () => this.menu(exportButton, [
            [tr("file.exportSvg"), "Ctrl+E", () => this.exportSvg()],
            [tr("file.exportPng"), "Ctrl+Alt+E", () => this.exportPng()],
            [tr("file.exportDrawio"), "Ctrl+Shift+E", () => this.exportDrawio()],
            [tr("file.exportCml"), "", () => this.exportCml()],
            null,
            [tr("file.copyDrawio"), "Ctrl+C", () => this.copyButton()],
            [tr("file.copySheet"), "", () => this.copySheet()],
            [tr("file.paste"), "Ctrl+V", () => this.pasteButton()],
        ]))
        const center = (f) => () => { const c = view.viewCenter(); this.run(() => f(c)) }
        const addOf = (kind) => center(c => ctl.addDeviceOf(kind, c.x, c.y))
        const addKind = b("▾", tr("add.typeHint"), () => this.menu(addKind, [
            [tr("menu.plainDevice"), "", addOf("")],
            ...KIND_GROUPS.flatMap(([group, kinds]) => [tr(group), ...kinds.map(k => [tr("device." + k.replace(/ /g, "_")), "", addOf(k)])]),
        ]), { className: "sg-split-arrow" })
        this.layoutButton = b(tr("layout.menu"), tr("layout.shortcut"), () => this.layoutPopup())
        const snap = h("input", { type: "checkbox", checked: this.prefs.get("snap") })
        snap.addEventListener("change", () => { this.prefs.set("snap", snap.checked); this.viewChanged() })
        this.gridBox = h("input", { type: "checkbox", checked: this.prefs.get("showGrid") })
        this.gridBox.addEventListener("change", () => this.setGrid(this.gridBox.checked))
        this.themeBox = h("select", { className: "sg-theme", title: tr("view.themeHint") },
                          ...[["light", tr("view.light")], ["deep", tr("view.dark")], ["neon", tr("view.neon")]].map(([v, t]) => h("option", { value: v }, t)))
        this.themeBox.value = this.theme
        this.themeBox.addEventListener("change", () => this.setTheme(this.themeBox.value))
        this.languageBox = h("select", { className: "sg-theme", title: tr("ui.language"), "aria-label": tr("ui.language") },
            Object.entries(languages).map(([value, name]) => h("option", { value, lang: value }, name)))
        this.languageBox.value = language
        this.languageBox.addEventListener("change", () => setLanguage(this.languageBox.value))
        this.quick = h("input", { type: "text", className: "sg-quick", placeholder: tr("quickAdd.placeholder"),
                                   title: tr("quickAdd.tooltip") })
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
        this.deepButton = b(tr("view.deep"), tr("view.deepHint"), () => this.toggleDeep())
        this.zoomLabel = h("span", { className: "sg-zoom" }, "100%")
        this.panelButton = b(tr("view.panel"), tr("view.panelHint"), () => this.togglePanel())
        // (Buttons the host leaves out are null.)
        const items = [
            ...file, exportButton, sep(),
            // "+ Device" adds a plain device; the arrow beside it one of a type.
            b(tr("add.device"), tr("add.deviceHint"), center(c => ctl.addDevice(c.x, c.y)), { className: "sg-strong sg-split-main" }),
            addKind,
            b(tr("add.text"), tr("add.textHint"), center(c => ctl.addNote(c.x - 20, c.y - 10)), { className: "sg-strong" }),
            b(tr("add.point"), tr("add.pointHint"), center(c => ctl.addPoint(c.x, c.y)), { className: "sg-strong" }),
            this.buttons.undo, this.buttons.redo,
            b(tr("edit.duplicate"), tr("edit.duplicateHint"), () => this.run(() => ctl.duplicate())),
            b(tr("edit.delete"), tr("edit.deleteHint"), () => this.run(() => ctl.deleteSelected())),
            sep(), this.layoutButton, sep(),
            h("label", { className: "sg-check", title: tr("view.snapHint") }, snap, tr("view.snap")),
            h("label", { className: "sg-check", title: tr("view.gridHint") }, this.gridBox, tr("layout.grid")),
            this.themeBox, this.languageBox,
            sep(), this.quick,
            ...(this.host.buttons || []).map(x => b(x.label, x.title || "", () => x.run(this))),
            h("span", { className: "sg-grow" }),
            this.deepButton,
            b("−", tr("view.zoomOut"), () => view.zoomBy(1 / 1.2)), this.zoomLabel, b("+", tr("view.zoomIn"), () => view.zoomBy(1.2)),
            b(tr("view.fit"), tr("view.fitHint"), () => view.fit()),
            sep(), this.panelButton]
        this.toolbar.replaceChildren(...items.filter(Boolean))
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

    contextMenu(clientX, clientY, world) {
        if (!this.ctl.contextSelect(world.x, world.y)) { this.changed(); return }
        this.changed()
        const sel = JSON.parse(this.ctl.selection())
        const type = sel.type
        const multi = type === "multi"
        const devices = type === "device" || (multi && sel.devices > 0)
        const kind = type === "device" ? sel.kind : (multi && sel.dev && !(sel.dev.kind === null || sel.dev.kind === undefined) ? sel.dev.kind : null)
        const knownKind = KIND_GROUPS.some(([, kinds]) => kinds.includes(kind))
        const kindLabel = kind ? (knownKind ? tr("device." + kind.replace(/ /g, "_")) : kind) : tr("menu.plainDevice")
        const layers = JSON.parse(this.ctl.layers()).filter(l => !l.frame)
        const at = { getBoundingClientRect: () => ({ left: clientX, right: clientX, top: clientY, bottom: clientY }),
                     classList: { add() {}, remove() {} }, contains: () => false }
        const m = h("div", { className: "sg-popup sg-menu sg-context" })
        const act = (f) => () => { this.closePopup(); this.run(f); this.view.focus() }
        const item = (label, key, f, on) => h("button", { onclick: f }, h("span", {}, (on ? "✓ " : "") + label), h("span", { className: "sg-key" }, key))
        const fit = () => {
            const app = this.el.getBoundingClientRect()
            m.style.maxHeight = (app.height - 8) + "px"
            const r = m.getBoundingClientRect()
            let top = clientY - app.top + 2
            if (top + r.height > app.height - 4) top = Math.max(4, app.height - r.height - 4)
            m.style.top = top + "px"
        }
        const show = (items) => { m.replaceChildren(...items); m.scrollTop = 0; if (m.isConnected) fit() }
        const back = () => item(tr("menu.back"), "", () => show(main()))
        const types = () => [back(), h("hr"),
            item(tr("menu.plainDevice"), "", act(() => this.ctl.setProp("device:kind", "")), kind === ""),
            ...KIND_GROUPS.flatMap(([group, kinds]) => [h("div", { className: "sg-menu-head" }, tr(group)),
                ...kinds.map(k => item(tr("device." + k.replace(/ /g, "_")), "", act(() => this.ctl.setProp("device:kind", k)), k === kind))])]
        const moves = () => [back(), h("hr"),
            ...layers.map(l => item("\u00a0\u00a0".repeat(l.depth) + l.name + (l.shown ? "" : tr("value.hiddenSuffix")) + (l.held ? tr("value.lockedSuffix") : ""), "",
                act(() => this.ctl.setProp("layer", String(l.id))), l.id === sel.layer))]
        const main = () => [
            devices ? item(tr("property.type") + (kind !== null ? ": " + kindLabel : ""), "▸", () => show(types())) : null,
            item(tr("menu.moveToLayer"), "▸", () => show(moves())),
            h("hr"),
            (type === "device" || type === "note" || (multi && sel.devices + sel.notes > 0))
                ? item(tr("edit.clone"), "Ctrl+D", act(() => this.ctl.duplicate())) : null,
            item(tr("edit.delete"), "Del", act(() => this.ctl.deleteSelected())),
        ].filter(Boolean)
        show(main())
        this.popup(at, m)
        fit()
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
            return h("label", { className: "sg-check", title: title && tr(title) }, c, tr(label))
        }
        const el = h("div", { className: "sg-popup sg-layout" },
            h("h3", {}, tr("layout.title")),
            h("p", { className: "sg-hint" }, tr("layout.instructions")),
            LAYOUTS.map(([kind, name, about]) => h("button", { className: "sg-choice", onclick: () => apply(kind) },
                h("b", {}, tr(name)), h("span", {}, tr(about)))),
            h("label", { className: "sg-inline" }, tr("layout.spacing"), gap),
            check("layoutTidyPorts", "layout.orientPorts"),
            check("layoutAvoid", "layout.avoidObjects",
                  "layout.avoidObjectsHint"))
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
        this.handle.title = tr(on ? "view.hidePanel" : "view.showPanel")
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
            if (xml === "") { if (cut) this.run(() => this.setStatus("clipboard.selectToCut")); return }
            const native = this.ctl.copyNative()
            this.clip = { xml, native }
            e.clipboardData.setData("text/plain", xml)
            try { e.clipboardData.setData(NATIVE_TYPE, native) } catch { /* the copy in memory serves */ }
            e.preventDefault()
            if (cut) this.run(() => { this.ctl.deleteSelected(); this.setStatus("clipboard.cut") })
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
        languageEvents.removeEventListener("change", this.languageChanged)
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
            this.run(() => this.setStatus("clipboard.copied"))
        } catch {
            this.run(() => this.setStatus("clipboard.copyShortcut"))
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
        else this.run(() => this.setStatus("clipboard.copyDenied"))
        this.view.focus()
    }

    async pasteButton() {
        try {
            const text = await navigator.clipboard.readText()
            this.paste(this.clip && this.clip.xml === text ? this.clip.native : "", text)
        } catch {
            this.run(() => this.setStatus("clipboard.pasteDenied"))
        }
    }

    // files

    // Go on only when unsaved changes may be dropped.
    discardOk() {
        return !this.ctl.dirty() || confirm(tr("file.discardConfirm"))
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
        if (got) this.showDiagram(got)
    }

    // Open a diagram handed over from elsewhere ({json, name}), e.g. a file
    // the system opens with the installed app; false if the user keeps the
    // unsaved one.
    openFile(got) {
        if (!this.discardOk()) return false
        this.showDiagram(got)
        return true
    }

    showDiagram(got) {
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
            this.setStatus("status.saveFailed", { error: e.message || String(e) })
        }
        this.changed()
    }

    exportSvg() {
        download(this.ctl.exportSvg(), this.baseName() + ".svg", "image/svg+xml")
        this.run(() => this.setStatus("status.exported", { name: this.baseName() + ".svg" }))
    }

    exportDrawio() {
        download(this.ctl.exportDrawio(this.baseName()), this.baseName() + ".drawio", "application/xml")
        this.run(() => this.setStatus("status.exported", { name: this.baseName() + ".drawio" }))
    }

    // Cisco Modeling Labs: the sheet in view as a lab to import.
    exportCml() {
        const yaml = this.ctl.exportCml(this.baseName())
        download(yaml, this.baseName() + ".yaml", "application/x-yaml")
        const left = this.ctl.status()
        this.run(() => this.setStatus(left ? "status.exportedWithDetails" : "status.exported", { name: this.baseName() + ".yaml", details: left }))
    }

    // PNG: the SVG export drawn at twice its size (sharp on high resolution
    // screens and in documents).
    async exportPng(scale = 2) {
        const svg = this.ctl.exportSvg()
        const url = URL.createObjectURL(new Blob([svg], { type: "image/svg+xml" }))
        try {
            const img = new Image()
            await new Promise((ok, fail) => { img.onload = ok; img.onerror = () => fail(new Error(tr("error.svg"))); img.src = url })
            const canvas = document.createElement("canvas")
            canvas.width = Math.ceil(img.naturalWidth * scale)
            canvas.height = Math.ceil(img.naturalHeight * scale)
            const ctx = canvas.getContext("2d")
            ctx.scale(scale, scale)
            ctx.drawImage(img, 0, 0)
            const blob = await new Promise(ok => canvas.toBlob(ok, "image/png"))
            download(blob, this.baseName() + ".png", "image/png")
            this.run(() => this.setStatus("status.exported", { name: this.baseName() + ".png" }))
        } catch (e) {
            this.run(() => this.setStatus("status.pngFailed", { error: e.message }))
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
    const types = () => [{ description: tr("file.filterName"), accept: { "application/json": [".stratagram", ".netdiag", ".json"] } }]
    return {
        async open() {
            if (window.showOpenFilePicker) {
                try {
                    const [h] = await window.showOpenFilePicker({ types: types() })
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
        // Save writes to this file from now on (one opened from the system).
        useHandle(h) {
            handle = h
        },
        async save(json, name, as) {
            const suggested = name.replace(/\.(netdiag|json)$/i, "").replace(/(\.stratagram)?$/i, ".stratagram")
            if (window.showSaveFilePicker) {
                try {
                    if (as || !handle) handle = await window.showSaveFilePicker({ suggestedName: suggested, types: types() })
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
