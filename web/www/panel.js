// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// The properties panel: an editor for the current selection, with the layer
// list pinned underneath (a port of qml/PropertiesPanel.qml).

// Device types by category, as in the desktop app. "" is a plain device.
export const KIND_GROUPS = [
    ["Routing & switching", ["router", "ospf router", "bgp reflector", "switch", "load balancer"]],
    ["Security", ["firewall", "cloud proxy", "zscaler"]],
    ["Wireless", ["access point", "standalone ap", "managed ap", "wlan controller"]],
    ["Servers & clients", ["server", "client"]],
    ["Virtualization", ["virtual switch", "distributed virtual switch"]],
    ["Data center", ["aci l3out"]],
    ["Cabling", ["patch panel"]],
    ["Internet & cloud", ["internet", "cloud", "aws", "aws transit gateway", "aws direct connect gateway", "azure", "google cloud", "alibaba cloud", "telekom cloud", "stackit"]],
]

const FRAME_GROUPS = [
    ["Basic", [["lines", "lines"], ["banner", "banner"], ["minimal", "minimal"], ["card", "card"]]],
    ["Business", [["corporate", "corporate"], ["report", "report page"], ["presentation", "presentation (16:9)"],
                  ["executive", "executive summary"], ["proposal", "customer proposal"]]],
    ["Technical drawing", [["titleblock", "title block"], ["iso", "ISO title block"], ["revision", "revision block"],
                           ["blueprint", "blueprint"], ["asbuilt", "as-built"]]],
    ["Documentation & governance", [["controlled", "controlled document"], ["signoff", "sign-off"],
                                    ["solution", "solution design"], ["change", "change request"],
                                    ["classification", "classification banner"]]],
    ["Operations", [["site", "site sheet"], ["location", "data center location"], ["incident", "incident report"],
                    ["noc", "NOC display"]]],
]

const DEVICE_COLORS = ["#dbeafe", "#bfdbfe", "#bbf7d0", "#fde68a", "#fecaca", "#ddd6fe",
                       "#e0f2fe", "#ccfbf1", "#fef3c7", "#e5e7eb", "#1e3a8a", "#334155"]
const LINK_COLORS = ["#334155", "#0f172a", "#2563eb", "#16a34a", "#dc2626", "#ea580c", "#9333ea", "#0891b2", "#94a3b8"]
const FRAME_COLORS = ["#1e293b", "#000000", "#1e3a8a", "#2563eb", "#0f766e", "#16a34a", "#b91c1c", "#ea580c", "#7c3aed", "#e20074"]
const BG_FILLS = ["#e0f2fe", "#dbeafe", "#dcfce7", "#fef9c3", "#ffedd5", "#fee2e2", "#f3e8ff", "#f1f5f9", "#e5e7eb", "#ffffff"]
const BG_BORDERS = ["#7dd3fc", "#2563eb", "#16a34a", "#ca8a04", "#ea580c", "#dc2626", "#9333ea", "#64748b", "#334155", "#000000"]
const HEADS = ["none", "arrow", "open", "diamond", "circle"]
const STYLES = ["solid", "dashed", "dotted"]
const ROUTINGS = [["straight", "straight"], ["orthogonal", "orthogonal"], ["around", "around objects"]]
const SIDES = ["top", "right", "bottom", "left"]
const LINK_KINDS = [["plain", "plain"], ["trunk", "trunk (802.1Q)"], ["ipsec", "IPsec tunnel"], ["vxlan", "VXLAN tunnel"],
                    ["sdwan", "SD-WAN"], ["mpls", "MPLS"], ["l2", "Layer 2 (switched)"], ["l3", "Layer 3 (routed)"],
                    ["l3sub", "L3 subinterface (802.1Q)"], ["capwap", "CAPWAP tunnel (AP to WLC)"]]

const HELP = `
<b>Double-click</b> empty space to add a device.<br><br>
<b>Drag from a device border</b> (or a port) onto another device to connect them. Ports are created where you start and end.<br><br>
<b>Double-click a border</b> to add a port.<br><br>
<b>Drag a port along the border</b> to move it (Shift+drag moves it from anywhere); pull a port with lines off the device
to turn it into a free point (the lines stay), or pull an unused port away to draw a link.<br><br>
<b>Double-click</b> a device, port, link or text to edit it in place, or select it and press F2, Enter or just start typing.<br><br>
<b>Quick add</b> (Ctrl+K): type devices into the toolbar field, comma-separated, and press Enter. <i>Firewall, Router</i>
adds a firewall and a router; <i>firewall|my-box</i> adds a firewall labeled my-box.<br><br>
<b>+ Text</b> adds a free text annotation; Shift+Enter starts a new line.<br><br>
<b>Drop a device on a line</b> (it lights up green) to put it in between; deleting the device joins the line again when
both halves have the same type and label.<br><br>
<b>Drag a line</b> to bend it: a connection point appears where you grab it and the line runs through it. Delete the
point to straighten the line again.<br><br>
<b>Lines to nowhere</b>: drop a line on empty space and it ends at a free connection point (a small ring, not exported).
Drag the ring to move the end; drop it on a device border (or port) to attach the lines there. <b>+ Point</b> adds one on its own.<br><br>
<b>Drag on empty space</b> to select several items, <b>Ctrl+click</b> to add to the selection. With several selected,
this panel edits them all at once, including numbered renaming (<i>sw-##</i> gives sw-01, sw-02…).<br><br>
<b>Hold Ctrl while dragging</b> to line things up with nearby boxes and ports, and to snap to equal spacing.<br><br>
<b>Layers</b> (bottom of this panel): new items go into the highlighted layer. Tick a layer to show or hide it; the
padlock makes it read only; the tag shows its name on its devices.<br><br>
<b>Arrange</b>: Home to front, Page Up forward, Page Down backward, End to back.<br><br>
<b>Zoom into</b> (Ctrl+I): zoom into a device without limit and add things inside it.<br><br>
<b>Layout ▾</b> (Ctrl+L) arranges devices automatically. Ctrl+Z undoes.<br><br>
<b>Pan</b> with middle/right drag or Space+drag, <b>zoom</b> with the wheel.<br><br>
<b>Ctrl+C</b> copies the selection for draw.io and for pasting here; <b>Ctrl+V</b> pastes Stratagram copies, draw.io
shapes or text; Ctrl+X cuts.<br><br>
Arrow keys nudge, Del deletes, Ctrl+D duplicates.`

// A DOM element: h("div", {className: "x", onclick: f}, child, "text", ...).
// What removing layer `l` asks, naming what goes with it: the layers under
// it and the items on them, those there are; "" with nothing on or under it.
export function removeQuestion(l, layers) {
    const under = layers.filter(x => x.path.startsWith(l.path + " / "))
    const items = l.count + under.reduce((n, x) => n + x.count, 0)
    const n = (k, one) => `${k} ${one}${k === 1 ? "" : "s"}`
    const subs = under.length ? n(under.length, "layer") + " under it" : ""
    const on = items ? n(items, "item") + (under.length ? " on them" : " on it") : ""
    if (!subs && !on) return ""
    return `Remove "${l.name}" and ` + (subs && on ? `the ${subs} and the ${on}` : `the ${subs || on}`) + "?"
}

export function h(tag, props = {}, ...kids) {
    const e = document.createElement(tag)
    for (const [k, v] of Object.entries(props || {})) {
        if (v === undefined || v === null || v === false) continue
        if (k.startsWith("on")) e.addEventListener(k.slice(2), v)
        else if (k === "style" && typeof v === "object") Object.assign(e.style, v)
        else if (k in e && k !== "list") e[k] = v
        else e.setAttribute(k, v === true ? "" : v)
    }
    for (const kid of kids.flat()) {
        if (kid === null || kid === undefined || kid === false) continue
        e.appendChild(typeof kid === "string" || typeof kid === "number" ? document.createTextNode(String(kid)) : kid)
    }
    return e
}

const mixed = (v) => v === null || v === undefined
const plural = (n, one, many) => n + " " + (n === 1 ? one : many)

function select(options, value, onPick, extra = {}) {
    // options: [value, label] pairs, or [group, [[value, label], ...]] with groups.
    const s = h("select", extra)
    for (const o of options) {
        if (Array.isArray(o[1])) {
            const g = h("optgroup", { label: o[0] })
            for (const [v, l] of o[1]) g.appendChild(h("option", { value: v }, l))
            s.appendChild(g)
        } else {
            s.appendChild(h("option", { value: o[0] }, o[1]))
        }
    }
    if (value === null) s.appendChild(h("option", { value: "", disabled: true, hidden: true }, "(mixed)"))
    s.value = value === null ? "" : value
    s.addEventListener("change", () => onPick(s.value))
    return s
}

function number(value, min, max, step, onSet, fallback) {
    const i = h("input", { type: "number", min, max, step, value: mixed(value) ? "" : Math.round(value * 100) / 100,
                           placeholder: mixed(value) ? "mixed" : "" })
    i.addEventListener("change", () => {
        const v = Number(i.value)
        if (i.value !== "" && Number.isFinite(v)) onSet(Math.min(max, Math.max(min, v)))
        else i.value = mixed(value) ? "" : value ?? fallback
    })
    return i
}

function text(value, onEdit, extra = {}) {
    const i = h("input", Object.assign({ type: "text", value: value ?? "" }, extra))
    i.addEventListener("input", () => onEdit(i.value))
    return i
}

function textarea(value, onEdit, extra = {}) {
    const t = h("textarea", Object.assign({ value: value ?? "", rows: 4 }, extra))
    t.addEventListener("input", () => onEdit(t.value))
    return t
}

function checkbox(label, checked, onToggle, extra = {}) {
    const c = h("input", { type: "checkbox", checked: !!checked })
    c.addEventListener("change", () => onToggle(c.checked))
    return h("label", Object.assign({ className: "sg-check" }, extra), c, label)
}

// Swatches plus a hex field.
function colorPicker(value, swatches, onPick) {
    const field = h("input", { type: "text", value: value || "", placeholder: "#rrggbb", className: "sg-hex" })
    field.addEventListener("change", () => {
        let t = field.value.trim()
        if (!t.startsWith("#")) t = "#" + t
        if (/^#([0-9a-fA-F]{3}|[0-9a-fA-F]{6})$/.test(t)) onPick(t)
        else field.value = value || ""
    })
    const v = (value || "").toLowerCase()
    return h("div", { className: "sg-colors" },
        h("div", { className: "sg-swatches" }, swatches.map(c =>
            h("button", { className: "sg-swatch" + (c === v ? " sg-on" : ""), title: c, style: { background: c },
                          onclick: () => onPick(c) }))),
        field)
}

function row(label, control) {
    return h("div", { className: "sg-row" }, h("span", { className: "sg-row-label", title: label }, label), control)
}

export class Panel {
    constructor(app, parent) {
        this.app = app
        this.ctl = app.ctl
        this.el = h("aside", { className: "sg-panel" })
        this.body = h("div", { className: "sg-panel-body" })
        this.layersEl = h("div", { className: "sg-layers" })
        this.el.append(this.body, this.layersEl)
        parent.appendChild(this.el)
        this.selJson = ""
        this.layersJson = ""
        this.pending = false
        // A rebuild put off while a field had focus happens when it loses it.
        this.body.addEventListener("focusout", () => setTimeout(() => {
            if (this.pending && !this.body.contains(document.activeElement)) this.render()
        }))
        this.layersEl.addEventListener("focusout", () => setTimeout(() => {
            if (this.layersPending) this.renderLayers()
        }))
    }

    set(key, value) { this.ctl.setProp(key, String(value)); this.app.changed() }
    // A property of every selected item of one kind ("device", "note", "link").
    setFor(kind, key, value) { this.ctl.setProp(kind + ":" + key, String(value)); this.app.changed() }
    setFrame(key, value) { this.ctl.setFrame(key, String(value)); this.app.changed() }

    update(selJson, layersJson) {
        const layersChanged = layersJson !== this.layersJson
        if (layersChanged) {
            this.layersJson = layersJson
            this.layers = JSON.parse(layersJson)
            // Only a layer being renamed holds the list back.
            const a = document.activeElement
            if (this.layersEl.contains(a) && a.tagName === "INPUT" && a.type === "text") this.layersPending = true
            else this.renderLayers()
        }
        // (The layer list of the selection shows the layers too.)
        if (selJson === this.selJson && !layersChanged) return
        const sel = JSON.parse(selJson)
        const same = this.sel && sel.type === this.sel.type && sel.id === this.sel.id && sel.device === this.sel.device
        this.selJson = selJson
        this.sel = sel
        // Typing in a field changes the selection's values; keep the field
        // (and its focus) and catch up when it is left.
        if (same && this.typing()) {
            this.pending = true
            return
        }
        this.render()
    }

    // A text field of the panel has focus.
    typing() {
        const a = document.activeElement
        return this.body.contains(a) && (a.tagName === "TEXTAREA" ||
            (a.tagName === "INPUT" && ["text", "number", ""].includes(a.type)))
    }

    render() {
        this.pending = false
        // Lists and buttons give their focus back to the canvas after use.
        const keep = this.body.contains(document.activeElement)
        const sel = this.sel
        const scroll = this.body.scrollTop
        this.body.replaceChildren(...this.content(sel))
        this.body.scrollTop = scroll
        if (keep) this.app.view.focus()
    }

    content(sel) {
        const type = sel.type
        const multi = type === "multi"
        const dev = type === "device" ? sel : (multi ? sel.dev : null)
        const note = type === "note" ? sel : (multi ? sel.note : null)
        const link = type === "link" ? sel : (multi ? sel.link : null)
        const out = []
        const title = multi
            ? [sel.devices ? plural(sel.devices, "device", "devices") : "", sel.links ? plural(sel.links, "link", "links") : "",
               sel.notes ? plural(sel.notes, "text", "texts") : "", sel.points ? plural(sel.points, "point", "points") : ""]
                .filter(t => t).join(", ")
            : ({ none: "Nothing selected", device: "Device", note: "Text", port: "Port", link: "Link", point: "Connection point" })[type]
        out.push(h("h2", {}, title))

        if (type === "none") {
            out.push(...this.frameSection(sel.frame || null))
            out.push(h("div", { className: "sg-help", innerHTML: HELP }))
            return out
        }
        if (multi) out.push(h("p", { className: "sg-hint" }, "Changes apply to all selected items of each kind."))

        // device
        if (multi && dev) out.push(h("h3", {}, plural(sel.devices, "Device", "Devices")))
        if (type === "device") out.push(h("label", {}, "Name"), text(sel.name, v => this.set("name", v)))
        if (multi && dev) {
            const pattern = h("input", { type: "text", placeholder: "e.g. sw-## gives sw-01, sw-02…" })
            const rename = () => { if (pattern.value.trim() !== "") { this.set("name_pattern", pattern.value); pattern.value = "" } }
            pattern.addEventListener("keydown", (e) => { if (e.key === "Enter") rename() })
            out.push(h("label", {}, "Rename in reading order (# is the number)"),
                     h("div", { className: "sg-inline" }, pattern, h("button", { onclick: rename }, "Rename")))
        }
        if (dev) {
            const kind = mixed(dev.kind) ? null : dev.kind
            const known = KIND_GROUPS.some(([, list]) => list.includes(kind))
            const options = [["", "none"]]
            if (kind && !known) options.push([kind, kind])
            options.push(...KIND_GROUPS.map(([g, list]) => [g, list.map(k => [k, k])]), ["\u0000other", "other…"])
            out.push(h("label", {}, "Type" + (kind === null ? " (mixed)" : "")),
                     select(options, kind, (v) => {
                         if (v === "\u0000other") {
                             const typed = prompt("Device type", kind || "")
                             if (typed !== null) this.setFor("device", "kind", typed.trim())
                             else this.render()
                         } else this.setFor("device", "kind", v)
                     }))
            if (type === "device" && kind === "patch panel") {
                // Its sockets; the panel widens to fit them.
                out.push(h("div", { className: "sg-grid2" },
                    h("label", {}, "Ports"), h("label", {}, "Connector"),
                    number(sel.panel_ports, 1, 288, 1, v => this.set("panel_ports", Math.round(v)), 24),
                    select([["rj45", "RJ45"], ["lc", "LC (duplex)"], ["sc", "SC"]], sel.connector, v => this.set("connector", v))))
            }
            if (type === "device" && sel.takes_ports) {
                // A switch, router, firewall or server with ports (0: a box):
                // its front, odd ports on top, even ones below, the uplinks
                // (SFP) apart at the right.
                out.push(h("div", { className: "sg-grid2" },
                    h("label", {}, "Ports (0: a box)"), h("label", {}, "Connector"),
                    number(sel.panel_ports, 0, 288, 1, v => this.set("panel_ports", Math.round(v)), 0),
                    select([["rj45", "RJ45"], ["sfp", "SFP"]], sel.connector, v => this.set("connector", v))))
                if (sel.panel_ports > 0) {
                    out.push(h("label", {}, "Uplinks (SFP)"),
                             number(sel.uplinks, 0, 48, 1, v => this.set("uplinks", Math.round(v)), 0))
                }
            }
            out.push(h("label", {}, "Fill color" + (mixed(dev.color) ? " (mixed)" : "")),
                     colorPicker(mixed(dev.color) ? "" : dev.color, DEVICE_COLORS, c => this.setFor("device", "color", c)))
            out.push(h("div", { className: "sg-grid2" },
                h("label", {}, "Width" + (mixed(dev.w) ? " (mixed)" : "")), h("label", {}, "Height" + (mixed(dev.h) ? " (mixed)" : "")),
                number(dev.w, 40, 2000, 10, v => this.setFor("device", "w", v), 120),
                number(dev.h, 30, 2000, 10, v => this.setFor("device", "h", v), 60)))
        }
        if (type === "device" || type === "port") {
            out.push(h("label", {}, type === "device" ? `Add port (${sel.ports} so far)` : "Add another port"),
                     h("div", { className: "sg-buttons4" }, [["top", "↑ Top"], ["right", "Right →"], ["bottom", "↓ Bottom"], ["left", "← Left"]]
                         .map(([side, label]) => h("button", { onclick: () => { this.ctl.addPort(side); this.app.changed() } }, label))))
        }

        // note
        if (multi && note) out.push(h("h3", {}, plural(sel.notes, "Text", "Texts")))
        if (note) {
            if (type === "note") out.push(h("label", {}, "Text"), textarea(sel.text, v => this.set("text", v), { wrap: "off" }))
            out.push(h("div", { className: "sg-inline" },
                h("span", {}, "Size" + (mixed(note.size) ? " (mixed)" : "")),
                number(note.size, 6, 96, 1, v => this.setFor("note", "size", v), 14),
                checkbox("Bold" + (mixed(note.bold) ? " (mixed)" : ""), note.bold === true, c => this.setFor("note", "bold", c))))
            if (type === "note") {
                out.push(h("div", { className: "sg-inline" },
                    h("p", { className: "sg-hint" }, sel.boxed ? "Fixed box: the text wraps to it. Drag the corner handle to resize."
                                                              : "Drag the corner handle to give it a fixed box."),
                    sel.boxed ? h("button", { onclick: () => this.setFor("note", "box", "fit") }, "Fit to text") : null))
            }
            out.push(h("label", {}, "Color" + (mixed(note.color) ? " (mixed)" : "")),
                     colorPicker(mixed(note.color) ? "" : note.color, LINK_COLORS, c => this.setFor("note", "color", c)))
        }

        // point
        if (type === "point") {
            out.push(h("p", { className: "sg-hint" },
                "Lines end here as if attached to nothing; the point itself is not exported (its label is). " +
                plural(sel.links, "line", "lines") + " attached. Drag to move it; drop it on a device border to attach " +
                "its lines there. Deleting a bend point joins its two lines back into one."))
            out.push(h("label", {}, "Label"), text(sel.label, v => this.set("label", v), { placeholder: "e.g. Internet, to branch office" }))
            out.push(h("div", { className: "sg-grid2" }, h("label", {}, "X"), h("label", {}, "Y"),
                number(Math.round(sel.x), -100000, 100000, 10, v => this.set("x", v), 0),
                number(Math.round(sel.y), -100000, 100000, 10, v => this.set("y", v), 0)))
        }

        // port
        if (type === "port") {
            out.push(h("p", { className: "sg-hint" }, `on ${sel.device}, ${sel.links} link(s)`))
            out.push(h("label", {}, "Name (e.g. Gi1/0/1)"), text(sel.name, v => this.set("name", v)))
            out.push(h("label", {}, "Side"), select(SIDES.map(s => [s, s]), sel.side, v => this.set("side", v)))
            out.push(h("label", {}, "Position along side (% from " + (sel.side === "left" || sel.side === "right" ? "top" : "left") + ")"),
                     number(sel.pos, 5, 95, 5, v => this.set("pos", v), 50))
            // A layer of its own, to show or hide it apart from its device.
            const itemLayers = (this.layers || []).filter(l => !l.frame)
            const own = itemLayers.find(l => l.id === sel.device_layer)
            out.push(h("label", { title: "Hide the layer to hide the port and its name (its lines stay)" }, "Layer"),
                     select([["", "as its device (" + (own ? own.name : "") + ")"],
                             ...itemLayers.map(l => [String(l.id), l.path + (l.shown ? "" : " (hidden)") + (l.held ? " (locked)" : "")])],
                            sel.port_layer ? String(sel.port_layer) : "", v => this.set("layer", v)))
        }

        // link
        if (multi && link) out.push(h("h3", {}, plural(sel.links, "Link", "Links")))
        if (link) {
            if (type === "link") out.push(h("p", { className: "sg-hint" }, sel.from + "  →  " + sel.to))
            out.push(h("label", {}, "Type"), select(LINK_KINDS, mixed(link.kind) ? null : link.kind, v => this.setFor("link", "kind", v)))
            if (type === "link") {
                // Encapsulation: what the line is carried over, innermost
                // first (VXLAN over IPsec over MPLS).
                const over = sel.over || []
                out.push(h("div", { className: "sg-inline" },
                    h("span", { innerHTML: over.length > 0 ? "Carried over: <b>" + escapeHtml(sel.tag) + "</b>" : "Carried over: nothing" }),
                    over.length > 0 ? h("button", { title: "Take away the outermost carrier",
                                                    onclick: () => this.set("over", over.slice(0, -1).join(",")) }, "Remove outer") : null))
                const carriers = LINK_KINDS.filter(([k]) => k !== "plain")
                out.push(select([["", "Add a carrier around it…"], ...carriers], "", (v) => {
                    if (v) this.set("over", over.concat([v]).join(","))
                }, { title: "Show the line encapsulated: e.g. a VXLAN line carried over IPsec" }))
            }
            out.push(h("div", { className: "sg-inline" },
                h("label", {}, "Label" + (mixed(link.label) ? " (mixed)" : "")),
                type === "link" && sel.label_placed ? h("button", { title: "Place the label automatically again",
                                                                    onclick: () => this.setFor("link", "label_at", "auto") }, "Auto position") : null))
            out.push(text(mixed(link.label) ? "" : link.label, v => this.setFor("link", "label", v),
                          { placeholder: multi ? "Same label for all" : "e.g. 10G, VLAN 20" }))
            out.push(checkbox("Show label" + (mixed(link.label_hidden) ? " (mixed)" : ""), link.label_hidden === false,
                              c => this.setFor("link", "label_hidden", !c),
                              { title: "The label and the type's tag, on screen and in exports (the text is kept)" }))
            const pick = (key, list) => select(list.map(x => Array.isArray(x) ? x : [x, x]), mixed(link[key]) ? null : link[key],
                                               v => this.setFor("link", key, v))
            out.push(h("div", { className: "sg-grid2" },
                h("label", {}, "Start"), h("label", {}, "End"), pick("head_start", HEADS), pick("head_end", HEADS),
                h("label", {}, "Line"), h("label", {}, "Routing"), pick("style", STYLES), pick("routing", ROUTINGS)))
            out.push(h("div", { className: "sg-grid2" },
                h("label", {}, "Width" + (mixed(link.width) ? " (mixed)" : "")),
                h("label", { title: "Free line auto layout keeps on each side of the label" },
                  "Label margin" + (mixed(link.label_margin) ? " (mixed)" : "")),
                number(link.width, 1, 10, 1, v => this.setFor("link", "width", v), 2),
                number(link.label_margin, 0, 200, 4, v => this.setFor("link", "label_margin", v), 24)))
            out.push(h("label", {}, "Color" + (mixed(link.color) ? " (mixed)" : "")),
                     colorPicker(mixed(link.color) ? "" : link.color, LINK_COLORS, c => this.setFor("link", "color", c)))
            if (type === "link") out.push(h("button", { className: "sg-wide", onclick: () => { this.ctl.reverseLink(); this.app.changed() } },
                                            "⇄ Reverse direction"))
            out.push(h("p", { className: "sg-hint" }, "New links use the style you set last."))
        }

        // layer and stacking
        if (["device", "note", "link", "point", "multi"].includes(type)) {
            const itemLayers = (this.layers || []).filter(l => !l.frame)
            const layerOptions = itemLayers.map(l => [String(l.id), l.path + (l.shown ? "" : " (hidden)") + (l.held ? " (locked)" : "")])
            out.push(h("h3", {}, "Layer"),
                     select(layerOptions, sel.layer >= 0 ? String(sel.layer) : null, v => this.set("layer", v)))
            out.push(h("label", {}, "Arrange (front to back, across all layers)"),
                     h("div", { className: "sg-grid2" }, [["front", "⤒ To front", "Home"], ["forward", "↑ Forward", "Page Up"],
                                                         ["back", "⤓ To back", "End"], ["backward", "↓ Backward", "Page Down"]]
                         .map(([op, label, key]) => h("button", { title: key, onclick: () => { this.ctl.arrange(op); this.app.changed() } }, label))))
        }
        const dup = ["device", "note"].includes(type) || (multi && sel.devices + sel.notes > 0)
        out.push(h("div", { className: "sg-grid2 sg-actions" },
            dup ? h("button", { onclick: () => { this.ctl.duplicate(); this.app.changed() } }, "Duplicate") : null,
            h("button", { onclick: () => { this.ctl.deleteSelected(); this.app.changed() } }, "Delete")))
        return out
    }

    // frame

    frameSection(frame) {
        const t = frame ? frame.template : "none"
        const uses = (...ts) => ts.includes(t)
        const out = [h("h3", {}, "Frame"),
                     select([["none", "none"], ...FRAME_GROUPS], t, v => this.setFrame("template", v),
                            { title: "A border with header and footer around the diagram, on its own bottom layer" })]
        if (!frame) return out
        const by = (labels) => labels[t]
        const fields = [
            ["title", "Title", ""],
            ["subtitle", "Subtitle", "e.g. Site Berlin"],
            ["author", t === "signoff" ? "Prepared by" : "Author", ""],
            ["note", "Note", "e.g. Confidential, v1.2"],
            ["customer", by({ proposal: "Prepared for" }), "Customer"],
            ["company", by({ corporate: "Company", proposal: "Company", presentation: "Company" }), ""],
            ["address", by({ site: "Address" }), ""],
            ["location", by({ location: "Location path" }), "EU > Berlin > HQ > Room 2.14"],
            ["doc_id", by({ iso: "Drawing no.", change: "Change ID", site: "Site code", controlled: "Document ID",
                            incident: "Incident ID", location: "Location code" }),
             by({ iso: "e.g. NET-0042", change: "e.g. CR-1042", site: "e.g. BER-01", controlled: "e.g. NET-DOC-007",
                  incident: "e.g. INC-2291", location: "e.g. BER-HQ-R2.14-A07" }) || ""],
            ["version", by({ controlled: "Version", solution: "Version" }), "e.g. 1.2"],
            ["revision", by({ iso: "Revision" }), ""],
            ["owner", by({ controlled: "Owner", solution: "Owner" }), ""],
            ["review_date", by({ controlled: "Next review" }), "e.g. 2027-04-01"],
            ["reviewer", by({ signoff: "Reviewed by", solution: "Reviewers" }), ""],
            ["approver", by({ iso: "Checked by", change: "Approved by", signoff: "Approved by", asbuilt: "Accepted by" }), ""],
            ["installer", by({ asbuilt: "Installed by" }), ""],
            ["commissioned", by({ asbuilt: "Commissioned" }), "e.g. 2026-09-30"],
            ["start", by({ incident: "Start" }), "e.g. 2026-10-18 09:12"],
            ["end", by({ incident: "End" }), "ongoing"],
            ["services", by({ incident: "Affected" }), "e.g. VPN, Email"],
            ["page", t === "iso" ? "Sheet" : "Page", "e.g. 1/3"],
        ]
        for (const [key, label, hint] of fields) {
            if (label) out.push(row(label, text(frame[key], v => this.setFrame(key, v), { placeholder: hint })))
        }
        // Status choices for the templates with a status badge.
        const statuses = { change: ["draft", "approved", "implemented", "rejected"], solution: ["draft", "in review", "final"],
                           noc: ["operational", "degraded", "outage", "maintenance"] }[t]
        if (statuses) out.push(row("Status", suggest(frame.status, statuses, v => this.setFrame("status", v))))
        if (uses("classification", "controlled")) {
            const levels = ["public", "internal", "confidential", "restricted"]
            out.push(row("Classification", select(levels.map(l => [l, l]), frame.level || "internal", v => this.setFrame("level", v))))
        }
        if (uses("incident")) {
            out.push(row("Severity", select([["P1", "P1 · critical"], ["P2", "P2 · high"], ["P3", "P3 · medium"], ["P4", "P4 · low"]],
                                            (frame.severity || "P3").toUpperCase(), v => this.setFrame("severity", v))))
        }
        if (uses("executive")) {
            out.push(h("label", {}, "Summary"),
                     textarea(frame.summary, v => this.setFrame("summary", v), { placeholder: "A few sentences on what the diagram shows" }))
        }
        out.push(row("Watermark", suggest(frame.watermark, ["DRAFT", "CONFIDENTIAL", "SAMPLE", "FOR REVIEW", "SUPERSEDED"],
                                          v => this.setFrame("watermark", v))))
        out.push(row("Margin", number(frame.margin, 0, 400, 10, v => this.setFrame("margin", v), 40)))
        if (uses("revision")) {
            out.push(h("label", {}, "Revisions, one per line: rev | date | change | by"),
                     textarea(frame.revisions, v => this.setFrame("revisions", v), { wrap: "off", placeholder: "A | 2026-10-01 | First issue | Shin" }))
        }
        out.push(checkbox("Show date and time of last change", frame.show_date, c => this.setFrame("show_date", c)))
        out.push(checkbox("Show the file name", frame.show_file, c => this.setFrame("show_file", c)))
        for (const [key, label] of [["logo", "Logo"], ["logo2", uses("proposal") ? "Customer logo" : "Second logo"]]) {
            const input = h("input", { type: "file", accept: "image/png,image/jpeg", hidden: true })
            input.addEventListener("change", () => {
                const file = input.files[0]
                if (!file) return
                const reader = new FileReader()
                reader.onload = () => this.setFrame(key, reader.result)
                reader.readAsDataURL(file)
            })
            out.push(h("div", { className: "sg-inline" }, h("span", { className: "sg-grow" }, label), input,
                       h("button", { onclick: () => input.click() }, frame[key] ? "Change…" : "Choose…"),
                       frame[key] ? h("button", { onclick: () => this.setFrame(key, "") }, "Remove") : null))
        }
        out.push(h("label", {}, "Color"), colorPicker(frame.color, FRAME_COLORS, c => this.setFrame("color", c)))
        out.push(h("hr"))
        return out
    }

    // layers

    renderLayers() {
        this.layersPending = false
        // A layer button used gives the focus back to the canvas.
        const keep = this.layersEl.contains(document.activeElement)
        // The list stays where it was scrolled to.
        const old = this.layersEl.querySelector(".sg-layer-list")
        const scrolled = old ? old.scrollTop : 0
        const layers = this.layers || []
        const ctl = this.ctl
        const act = (f) => () => { f(); this.app.changed() }
        const active = layers.find(l => l.active)
        // A layer's items with those of the layers under it.
        const total = (l) => layers.filter(x => x === l || x.path.startsWith(l.path + " / ")).reduce((n, x) => n + x.count, 0)
        const head = h("div", { className: "sg-inline" },
            h("h3", { className: "sg-grow" }, "Layers"),
            h("button", { className: "sg-flat", title: "Add a layer; new items go into it", onclick: act(() => ctl.addLayer()) }, "+ Layer"),
            h("button", { className: "sg-flat", title: "Add a layer under the highlighted one; new items go into it",
                          disabled: !active || active.frame, onclick: act(() => ctl.addSublayer()) }, "+ Sub-layer"),
            h("button", { className: "sg-flat", title: "Remove the active layer and everything on it (and the layers under it)",
                          disabled: layers.length <= 1 || layers.some(l => l.active && l.held),
                          onclick: act(() => {
                              if (!active) return
                              // Asked first only when there is something to lose.
                              const ask = removeQuestion(active, layers)
                              if (!ask || confirm(ask + "\nYou can undo this with Ctrl+Z.")) ctl.removeLayer(active.id)
                          }) }, "Remove"))
        // Dragging a layer: before or after another (under the same layer),
        // or into it, which then lights up as the new parent.
        const parentOf = new Map(layers.map(l => [l.id, l.parent]))
        const under = (id, of) => {
            const seen = new Set()
            for (let at = parentOf.get(id); at && !seen.has(at); at = parentOf.get(at)) {
                if (at === of) return true
                seen.add(at)
            }
            return false
        }
        const rowOf = new Map()
        const unmark = () => list.querySelectorAll(".sg-drop-into, .sg-drop-before, .sg-drop-after, .sg-drop-parent")
            .forEach(r => r.classList.remove("sg-drop-into", "sg-drop-before", "sg-drop-after", "sg-drop-parent"))
        let drop = null
        const dragOver = (e, l, row) => {
            const from = this.dragLayer
            if (!from || from === l.id || under(l.id, from)) { unmark(); drop = null; return }
            const r = row.getBoundingClientRect(), y = (e.clientY - r.top) / r.height
            let place = y < 0.25 ? "before" : y > 0.75 ? "after" : "into"
            if (l.frame && place === "into") place = y < 0.5 ? "before" : "after"
            // Below an unfolded layer the list shows its sub-layers: in there.
            if (place === "after" && l.children > 0 && l.expanded) place = "into"
            e.preventDefault()
            e.dataTransfer.dropEffect = "move"
            if (drop && drop.target === l.id && drop.place === place) return
            unmark()
            drop = { target: l.id, place }
            const parent = place === "into" ? l.id : l.parent
            if (place !== "into") row.classList.add("sg-drop-" + place)
            if (parent && rowOf.has(parent)) rowOf.get(parent).classList.add(place === "into" ? "sg-drop-into" : "sg-drop-parent")
        }
        const list = h("div", { className: "sg-layer-list" }, layers.filter(l => !l.folded).map(l => {
            const name = h("span", { className: "sg-grow sg-layer-name", title: (l.depth ? l.path + "\n" : "") + "Double-click to rename" }, l.name)
            name.style.fontWeight = l.active ? "600" : ""
            name.style.fontStyle = l.held ? "italic" : ""
            // Hidden, by itself or with a layer it is under.
            name.style.color = l.shown ? "" : "var(--sg-faint)"
            name.addEventListener("dblclick", (e) => {
                e.stopPropagation()
                const field = h("input", { type: "text", value: l.name, className: "sg-grow" })
                let done = false
                const finish = (save) => {
                    if (done) return
                    done = true
                    if (save && field.value !== l.name) ctl.renameLayer(l.id, field.value)
                    this.layersJson = ""   // redraw the list either way
                    this.app.changed()
                }
                field.addEventListener("keydown", (e) => {
                    e.stopPropagation()
                    if (e.key === "Enter") field.blur()
                    else if (e.key === "Escape") { finish(false); field.blur() }
                })
                field.addEventListener("blur", () => finish(true))
                name.replaceWith(field)
                field.focus()
                field.select()
            })
            const vis = h("input", { type: "checkbox", checked: l.visible,
                                     title: !l.visible ? "Hidden (click to show)" : l.shown ? "Shown (click to hide)" : "Hidden with the layer it is under" })
            vis.addEventListener("click", (e) => e.stopPropagation())
            vis.addEventListener("change", act(() => ctl.setLayerVisible(l.id, vis.checked)))
            const icon = (cls, title, on, f) => h("button", { className: "sg-icon " + cls + (on ? " sg-on" : ""), title,
                                                               onclick: (e) => { e.stopPropagation(); f() } })
            const bg = l.background
            const bgIcon = l.frame ? null : icon("sg-bg", bg ? "Layer background (click to change)" : "Click to add a background behind this layer's items",
                                                 !!bg, () => this.backgroundPopup(l))
            if (bgIcon && bg) {
                bgIcon.style.setProperty("--fill", bg.fill)
                bgIcon.style.setProperty("--border", bg.border === "none" ? "var(--sg-help)" : bg.border_color)
                bgIcon.style.setProperty("--radius", bg.rounded ? "3px" : "0")
            }
            // Sub-layers fold away under their layer.
            const fold = l.children > 0
                ? h("button", { className: "sg-fold", title: l.expanded ? "Fold the layers under it away" : `Show the ${l.children} layer(s) under it`,
                                onclick: (e) => { e.stopPropagation(); ctl.setLayerExpanded(l.id, !l.expanded); this.app.changed() } },
                    l.expanded ? "\u25be" : "\u25b8")
                : h("span", { className: "sg-fold" })
            const row = h("div", { className: "sg-layer" + (l.active ? " sg-active" : ""),
                              onclick: () => { if (!l.frame) { ctl.setActiveLayer(l.id); this.app.changed() } } },
                fold, vis, name,
                bgIcon,
                l.frame ? null : icon("sg-tag", l.show_name ? "Layer name shown on its devices (click to hide)" : "Click to show the layer name on its devices",
                                      l.show_name, act(() => ctl.setLayerShowName(l.id, !l.show_name))),
                l.frame ? null : icon("sg-lock", l.locked ? "Locked: shown but read only (click to unlock)" : "Click to lock: shown but read only",
                                      l.locked, act(() => ctl.setLayerLocked(l.id, !l.locked))),
                h("span", { className: "sg-count", title: l.frame ? "Holds only the frame; set it up with nothing selected"
                                                          : l.children ? "Items on it (and on the layers under it)" : "" },
                  l.frame ? "frame" : String(l.children ? total(l) : l.count)))
            // Deep down, the steps get smaller (the path is in the tooltip).
            row.style.paddingLeft = (2 + (l.depth <= 8 ? 14 * l.depth : Math.min(112 + 3 * (l.depth - 8), 180))) + "px"
            rowOf.set(l.id, row)
            if (!l.frame) {
                row.draggable = true
                row.addEventListener("dragstart", (e) => {
                    if (e.target !== row) return
                    this.dragLayer = l.id
                    e.dataTransfer.effectAllowed = "move"
                    e.dataTransfer.setData("text/plain", l.name)
                    row.classList.add("sg-dragging")
                })
                row.addEventListener("dragend", () => { this.dragLayer = null; row.classList.remove("sg-dragging"); unmark(); drop = null })
            }
            row.addEventListener("dragover", (e) => { e.stopPropagation(); dragOver(e, l, row) })
            row.addEventListener("drop", (e) => {
                e.preventDefault()
                e.stopPropagation()
                const from = this.dragLayer
                const d = drop
                unmark()
                drop = null
                this.dragLayer = null
                if (from && d) { ctl.moveLayer(from, d.target, d.place); this.app.changed() }
            })
            return row
        }))
        // Below the last row: to the end, as a top layer.
        const tops = layers.filter(l => !l.parent && !l.frame)
        list.addEventListener("dragover", (e) => {
            const last = tops[tops.length - 1]
            if (!this.dragLayer || !last || last.id === this.dragLayer) return
            e.preventDefault()
            unmark()
            drop = { target: last.id, place: "after" }
            const lastRow = list.lastElementChild
            if (lastRow) lastRow.classList.add("sg-drop-after")
        })
        list.addEventListener("dragleave", (e) => { if (!list.contains(e.relatedTarget)) { unmark(); drop = null } })
        list.addEventListener("drop", (e) => {
            e.preventDefault()
            const from = this.dragLayer, d = drop
            unmark()
            drop = null
            this.dragLayer = null
            if (from && d) { ctl.moveLayer(from, d.target, d.place); this.app.changed() }
        })
        this.layersEl.replaceChildren(head, list,
            h("p", { className: "sg-hint" }, "New items go into the highlighted layer. Click to choose, double-click to rename, drag to reorder or onto another layer to put it under that one; \u25b8 shows the layers under one."))
        list.scrollTop = scrolled
        if (keep) this.app.view.focus()
    }

    // Settings of one layer's background, in a popup beside the panel.
    backgroundPopup(layer) {
        const ctl = this.ctl
        const set = (key, value) => { ctl.setLayerBackground(layer.id, key, String(value)); this.app.changed(); build() }
        const pop = h("div", { className: "sg-popup sg-bg-popup" })
        const build = () => {
            const l = (this.layers || []).find(x => x.id === layer.id) || layer
            const bg = l.background
            const kids = [h("h3", {}, "Background of " + l.name),
                          checkbox("Rectangle behind this layer's items", !!bg, c => set("enabled", c))]
            if (bg) {
                kids.push(row("Margin", number(bg.margin, 0, 400, 5, v => set("margin", v), 30)),
                          row("Opacity %", number(bg.opacity, 0, 100, 5, v => set("opacity", v), 50)),
                          row("Corners", select([["true", "rounded"], ["false", "square"]], String(!!bg.rounded), v => set("rounded", v))))
                if (bg.rounded) kids.push(row("Radius", number(bg.radius, 0, 100, 2, v => set("radius", v), 14)))
                kids.push(row("Border", select(["none", "solid", "dashed", "dotted"].map(s => [s, s]), bg.border, v => set("border", v))))
                if (bg.border !== "none") kids.push(row("Border width", number(Math.round(bg.border_width), 1, 10, 1, v => set("border_width", v), 2)))
                kids.push(h("label", {}, "Fill"), colorPicker(bg.fill, BG_FILLS, c => set("fill", c)))
                if (bg.border !== "none") kids.push(h("label", {}, "Border color"), colorPicker(bg.border_color, BG_BORDERS, c => set("border_color", c)))
            }
            kids.push(h("button", { className: "sg-wide", onclick: () => close() }, "Close"))
            pop.replaceChildren(...kids)
        }
        const close = () => { pop.remove(); document.removeEventListener("pointerdown", outside, true) }
        const outside = (e) => { if (!pop.contains(e.target)) close() }
        pop.addEventListener("keydown", (e) => { if (e.key === "Escape") close() })
        build()
        this.app.el.appendChild(pop)
        const r = this.el.getBoundingClientRect(), app = this.app.el.getBoundingClientRect()
        pop.style.right = (app.right - r.left + 8) + "px"
        pop.style.bottom = "40px"
        setTimeout(() => document.addEventListener("pointerdown", outside, true))
    }
}

// A text field with suggestions (an editable list).
function suggest(value, choices, onSet) {
    const id = "sg-list-" + Math.random().toString(36).slice(2)
    const i = h("input", { type: "text", value: value ?? "" })
    i.setAttribute("list", id)
    i.addEventListener("change", () => onSet(i.value))
    return h("span", { className: "sg-suggest" }, i, h("datalist", { id }, choices.map(c => h("option", { value: c }))))
}

function escapeHtml(s) {
    return String(s).replace(/[&<>"]/g, c => ({ "&": "&amp;", "<": "&lt;", ">": "&gt;", '"': "&quot;" })[c])
}
