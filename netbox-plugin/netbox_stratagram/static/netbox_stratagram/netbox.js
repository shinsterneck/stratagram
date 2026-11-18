// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// The editor on a site's (or a location's) Diagram tab: diagrams are
// saved to NetBox, and "Refresh from NetBox" brings in the site's devices
// and cables as they are now. Without a saved diagram the site is drawn
// afresh, with a sheet for each location. A location's tab shows its
// site's diagram, open on the location's sheet.

const config = JSON.parse(document.getElementById("stratagram-config").textContent)
// The editor build (its folder changes with every build, see build.sh).
const { mount } = await import(config.editorBase + "app.js")
const csrf = config.csrfToken
const savedLine = document.getElementById("stratagram-saved")
const options = JSON.stringify(config.options)

function showSaved(when, dirty) {
    const at = when ? new Date(when).toLocaleString() : null
    savedLine.textContent = (at ? "Saved in NetBox " + at : "Not saved in NetBox yet") +
        (dirty ? " · unsaved changes" : "") +
        (config.canSave ? "" : " · you may not save diagrams")
}

async function fetchJson(url, init) {
    const r = await fetch(url, Object.assign({ credentials: "same-origin", headers: { "Accept": "application/json" } }, init))
    if (!r.ok) throw new Error(r.status === 403 ? "not allowed" : (await r.text()) || r.statusText)
    return r.json()
}

async function netboxData() {
    return JSON.stringify(await fetchJson(config.dataUrl))
}

function el(tag, props = {}, ...kids) {
    const e = Object.assign(document.createElement(tag), props)
    for (const k of kids.flat()) if (k !== null && k !== undefined) e.append(k)
    return e
}

function importDialog(parent, tree) {
    return new Promise((resolve) => {
        const leaves = (n) => n.children && n.children.length ? n.children.flatMap(leaves) : [n]
        const checked = new Set()
        const open = new Set([tree.key])
        const state = (n) => {
            const all = leaves(n)
            const on = all.filter(l => checked.has(l.key)).length
            return on === 0 ? "none" : on === all.length ? "all" : "some"
        }
        const collect = (n, out) => {
            const s = state(n)
            if (s === "none") return
            if (s === "all" && n.kind === "device") { out.push(n.key); return }
            if (!n.children || !n.children.length) { if (n.kind !== "site") out.push(n.key); return }
            for (const c of n.children) collect(c, out)
        }
        const list = el("div", { className: "sg-import-tree" })
        const importButton = el("button", { className: "sg-import-go", textContent: "Import", disabled: true })
        const target = (value, label, on) => el("label", { className: "sg-import-target" },
            el("input", { type: "radio", name: "sg-import-target", value, checked: on }), label)
        const here = target("current", "Update this sheet", true)
        const fresh = target("new", "Add as a new sheet", false)
        const count = el("span", { className: "sg-import-count" })
        let focusKey = null, focusPart = "box"
        const render = () => {
            const rows = []
            const row = (n, depth) => {
                const s = state(n)
                const kids = n.children && n.children.length
                const box = el("input", { type: "checkbox", checked: s === "all", indeterminate: s === "some" })
                box.dataset.key = n.key
                box.addEventListener("change", () => {
                    for (const l of leaves(n)) box.checked ? checked.add(l.key) : checked.delete(l.key)
                    focusKey = n.key; focusPart = "box"
                    render()
                })
                const toggle = el("button", { className: "sg-import-toggle", type: "button", textContent: kids ? (open.has(n.key) ? "▾" : "▸") : "", tabIndex: kids ? 0 : -1 })
                toggle.dataset.key = n.key
                if (kids) toggle.addEventListener("click", () => { open.has(n.key) ? open.delete(n.key) : open.add(n.key); focusKey = n.key; focusPart = "toggle"; render() })
                const kind = n.kind === "device" ? (n.type || "device") : n.kind === "location" ? "location" : n.kind === "port" ? "port" : ""
                rows.push(el("div", { className: "sg-import-row", style: `padding-left: ${depth * 18}px` },
                    toggle, el("label", {}, box, el("span", { className: "sg-import-name", textContent: n.name || n.key }),
                        kind ? el("span", { className: "sg-import-kind", textContent: kind }) : null)))
                if (kids && open.has(n.key)) for (const c of n.children) row(c, depth + 1)
            }
            row(tree, 0)
            list.replaceChildren(...rows)
            if (focusKey !== null) {
                const sel = focusPart === "toggle" ? "button" : "input"
                const target = [...list.querySelectorAll(sel)].find(x => x.dataset.key === focusKey)
                if (target) target.focus()
            }
            const picked = []
            collect(tree, picked)
            const devices = []
            const walk = (n) => { if (n.kind === "device") devices.push(n); else for (const c of n.children || []) walk(c) }
            walk(tree)
            const some = devices.filter(d => state(d) !== "none").length
            importButton.disabled = picked.length === 0
            count.textContent = checked.size === 0 ? "Nothing selected"
                : state(tree) === "all" ? "Everything selected"
                : `${some} of ${devices.length} device${devices.length === 1 ? "" : "s"}`
        }
        const close = (result) => { backdrop.remove(); document.removeEventListener("keydown", keys, true); resolve(result) }
        const go = () => {
            if (importButton.disabled) return
            const items = []
            collect(tree, items)
            close({ selection: { all: state(tree) === "all", items }, newSheet: fresh.querySelector("input").checked })
        }
        const keys = (e) => {
            e.stopPropagation()
            if ((e.ctrlKey || e.metaKey) && ["a", "d", "z", "y"].includes(e.key.toLowerCase())) { e.preventDefault(); return }
            if (e.key === "Escape") { e.preventDefault(); close(null); return }
            const onOtherButton = e.target.tagName === "BUTTON" && e.target !== importButton
            if (e.key === "Enter" && !onOtherButton && !importButton.disabled) { e.preventDefault(); go() }
        }
        importButton.addEventListener("click", go)
        const cancel = el("button", { className: "sg-import-cancel", type: "button", textContent: "Cancel" })
        cancel.addEventListener("click", () => close(null))
        const dialog = el("div", { className: "sg-import-dialog", role: "dialog" },
            el("h3", { textContent: "Import from NetBox" }),
            el("p", { className: "sg-import-hint", textContent: "Choose what to bring in. A location or type brings everything under it; unfold a device to choose single interfaces. Cables come along where both ends are chosen." }),
            list,
            el("div", { className: "sg-import-targets" }, here, fresh),
            el("div", { className: "sg-import-buttons" }, count, cancel, importButton))
        const backdrop = el("div", { className: "sg-import-backdrop" }, dialog)
        backdrop.addEventListener("pointerdown", (e) => { if (e.target === backdrop) close(null) })
        document.addEventListener("keydown", keys, true)
        render()
        parent.appendChild(backdrop)
        const first = list.querySelector("input")
        if (first) first.focus()
    })
}

let saved = null
if (config.saved) {
    saved = await fetchJson(config.diagramUrl)
}
let savedAt = saved ? saved.saved : null

// NetBox's own theme (its light/dark switch) as the editor's, until the
// user picks one in the editor.
const netboxTheme = () => document.documentElement.getAttribute("data-bs-theme") === "dark" ? "deep" : "light"

const host = {
    theme: netboxTheme(),
    canNew: false,
    saveAs: false,
    initial: saved ? { json: JSON.stringify(saved.diagram), name: config.name } : null,
    title() { /* NetBox's page keeps its title */ },
    dirty(d) { showSaved(savedAt, d) },
    async save(json, name) {
        if (!config.canSave) throw new Error("you may not save diagrams in NetBox")
        const r = await fetchJson(config.diagramUrl, {
            method: "POST",
            headers: { "Content-Type": "application/json", "X-CSRFToken": csrf, "Accept": "application/json" },
            body: JSON.stringify({ diagram: JSON.parse(json) }),
        })
        savedAt = r.saved
        config.canSave = true
        // The new time, also when nothing had changed (no change of the
        // unsaved state shows it).
        showSaved(savedAt, false)
        return name
    },
    buttons: [{
        label: "Refresh from NetBox",
        title: "Choose what to bring in from NetBox, into this sheet or a new one (Ctrl+Z undoes)",
        async run(app) {
            let data
            try {
                data = await netboxData()
            } catch (e) {
                app.ctl.setStatus("Cannot get the site from NetBox: " + e.message)
                app.changed()
                return
            }
            const tree = app.ctl.netboxTree(data, options)
            if (!tree) { app.ctl.setStatus("Cannot read the site from NetBox."); app.changed(); return }
            const choice = await importDialog(app.el, JSON.parse(tree))
            if (choice) {
                const err = app.ctl.importNetboxSelection(data, options, JSON.stringify(choice.selection), choice.newSheet)
                if (err) app.ctl.setStatus("Cannot import: " + err)
                app.changed()
                if (!err && choice.newSheet) app.view.fit()
            }
            app.view.focus()
        },
    }],
}

// The editor fills the window below the tabs.
const editorEl = document.getElementById("stratagram-editor")
const fitHeight = () => {
    editorEl.style.height = Math.max(480, window.innerHeight - editorEl.getBoundingClientRect().top - window.scrollY - 16) + "px"
}
fitHeight()
window.addEventListener("resize", fitHeight)

const app = await mount(editorEl, { host })
// New lines run like the cables (orthogonal unless set up otherwise).
app.ctl.setLineRouting(config.options.routing || "orthogonal")
new MutationObserver(() => { if (!app.themePicked()) app.setTheme(netboxTheme(), false) })
    .observe(document.documentElement, { attributes: true, attributeFilter: ["data-bs-theme"] })
if (!saved) {
    // Nothing saved yet: draw the site as NetBox has it.
    try {
        const err = app.ctl.importNetbox(await netboxData(), options)
        if (err) app.ctl.setStatus(err)
    } catch (e) {
        app.ctl.setStatus("Cannot get the site from NetBox: " + e.message)
    }
    app.changed()
    app.view.fit()
}
if (config.sheet && !app.ctl.showSheetOf(config.sheet)) {
    // Saved before this location was there (or had a sheet): bring the
    // diagram up to date, which adds its sheet.
    try {
        const err = app.ctl.refreshNetbox(await netboxData(), options)
        if (err) app.ctl.setStatus("Cannot refresh: " + err)
    } catch (e) {
        app.ctl.setStatus("Cannot get the site from NetBox: " + e.message)
    }
    if (!app.ctl.showSheetOf(config.sheet)) app.ctl.setStatus("This location has no sheet in the diagram.")
}
if (config.sheet) {
    app.changed()
    app.view.fit()
}
showSaved(savedAt, app.ctl.dirty())
window.stratagram = app
