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
        title: "Add what is new in NetBox, rename what was renamed, set aside what is gone (Ctrl+Z undoes)",
        async run(app) {
            try {
                const err = app.ctl.refreshNetbox(await netboxData(), options)
                if (err) app.ctl.setStatus("Cannot refresh: " + err)
            } catch (e) {
                app.ctl.setStatus("Cannot get the site from NetBox: " + e.message)
            }
            app.changed()
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
