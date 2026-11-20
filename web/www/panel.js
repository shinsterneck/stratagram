// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

// The properties panel: an editor for the current selection, with the layer
// list pinned underneath (a port of qml/PropertiesPanel.qml).

import { tr } from "./i18n.js"

// Device types by category, as in the desktop app. "" is a plain device.
export const KIND_GROUPS = [
    ["deviceGroup.routing", ["router", "ospf router", "bgp reflector", "switch", "load balancer"]],
    ["deviceGroup.security", ["firewall", "cloud proxy", "zscaler"]],
    ["deviceGroup.wireless", ["access point", "standalone ap", "managed ap", "wlan controller"]],
    ["deviceGroup.servers", ["server", "client"]],
    ["deviceGroup.virtualization", ["virtual switch", "distributed virtual switch"]],
    ["deviceGroup.datacenter", ["aci l3out"]],
    ["deviceGroup.cabling", ["patch panel"]],
    ["deviceGroup.cloud", ["internet", "cloud", "aws", "aws transit gateway", "aws direct connect gateway", "azure", "google cloud", "alibaba cloud", "telekom cloud", "stackit"]],
]

const FRAME_GROUPS = [
    ["frameGroup.basic", [["lines", "frameType.lines"], ["banner", "frameType.banner"], ["minimal", "frameType.minimal"], ["card", "frameType.card"]]],
    ["frameGroup.business", [["corporate", "frameType.corporate"], ["report", "frameType.report"], ["presentation", "frameType.presentation"],
                  ["executive", "frameType.executive"], ["proposal", "frameType.proposal"]]],
    ["frameGroup.technical", [["titleblock", "frameType.titleblock"], ["iso", "frameType.iso"], ["revision", "frameType.revision"],
                           ["blueprint", "frameType.blueprint"], ["asbuilt", "frameType.asbuilt"]]],
    ["frameGroup.governance", [["controlled", "frameType.controlled"], ["signoff", "frameType.signoff"],
                                    ["solution", "frameType.solution"], ["change", "frameType.change"],
                                    ["classification", "frameType.classification"]]],
    ["frameGroup.operations", [["site", "frameType.site"], ["location", "frameType.location"], ["incident", "frameType.incident"],
                    ["noc", "frameType.noc"]]],
]

const DEVICE_COLORS = ["#dbeafe", "#bfdbfe", "#bbf7d0", "#fde68a", "#fecaca", "#ddd6fe",
                       "#e0f2fe", "#ccfbf1", "#fef3c7", "#e5e7eb", "#1e3a8a", "#334155"]
const LINK_COLORS = ["#334155", "#0f172a", "#2563eb", "#16a34a", "#dc2626", "#ea580c", "#9333ea", "#0891b2", "#94a3b8"]
const FRAME_COLORS = ["#1e293b", "#000000", "#1e3a8a", "#2563eb", "#0f766e", "#16a34a", "#b91c1c", "#ea580c", "#7c3aed", "#e20074"]
const BG_FILLS = ["#e0f2fe", "#dbeafe", "#dcfce7", "#fef9c3", "#ffedd5", "#fee2e2", "#f3e8ff", "#f1f5f9", "#e5e7eb", "#ffffff"]
const BG_BORDERS = ["#7dd3fc", "#2563eb", "#16a34a", "#ca8a04", "#ea580c", "#dc2626", "#9333ea", "#64748b", "#334155", "#000000"]
const HEADS = ["none", "arrow", "open", "diamond", "circle"]
const STYLES = ["solid", "dashed", "dotted"]
const ROUTINGS = [["straight", "style.straight"], ["orthogonal", "style.orthogonal"], ["around", "style.around"]]
const SIDES = ["top", "right", "bottom", "left"]
const LINK_KINDS = [["plain", "linkKind.plain"], ["trunk", "linkKind.trunk"], ["ipsec", "linkKind.ipsec"], ["vxlan", "linkKind.vxlan"],
                    ["sdwan", "term.sdwan"], ["mpls", "term.mpls"], ["l2", "linkKind.l2"], ["l3", "linkKind.l3"],
                    ["l3sub", "linkKind.l3sub"], ["capwap", "linkKind.capwap"]]

const HELP = [
    "help.addDevice", "help.connect", "help.addPort", "help.movePort", "help.edit",
    "help.quickAdd", "help.addText", "help.insertDevice", "help.bend", "help.freePoint",
    "help.select", "help.align", "help.layers", "help.arrange", "help.deepZoom",
    "help.layout", "help.contextMenu", "help.pan", "help.clipboard", "help.keys",
]

// A DOM element: h("div", {className: "x", onclick: f}, child, "text", ...).
// What removing layer `l` asks, naming what goes with it: the layers under
// it and the items on them, those there are; "" with nothing on or under it.
export function removeQuestion(l, layers) {
    const under = layers.filter(x => x.path.startsWith(l.path + " / "))
    const items = l.count + under.reduce((n, x) => n + x.count, 0)
    const n = (k, one) => plural(k, one, one + "s")
    if (!under.length && !items) return ""
    const subs = under.length ? tr("confirm.subLayers", { layers: n(under.length, "layer") }) : ""
    const on = items ? tr(under.length ? "confirm.treeItems" : "confirm.layerItems", { items: n(items, "item") }) : ""
    const contents = subs && on ? tr("confirm.layersAndItems", { layers: subs, items: on }) : subs || on
    return tr("confirm.removeLayer", { name: l.name, contents })
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
const plural = (n, one, many) => tr("count." + (n === 1 ? one : many), { count: n })

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
    if (value === null) s.appendChild(h("option", { value: "", disabled: true, hidden: true }, tr("value.mixedOption")))
    s.value = value === null ? "" : value
    s.addEventListener("change", () => onPick(s.value))
    return s
}

function number(value, min, max, step, onSet, fallback) {
    const i = h("input", { type: "number", min, max, step, value: mixed(value) ? "" : Math.round(value * 100) / 100,
                           placeholder: mixed(value) ? tr("value.mixed") : "" })
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
            : ({ none: tr("selection.empty"), device: tr("selection.device"), note: tr("selection.text"), port: tr("selection.port"), link: tr("selection.link"), point: tr("selection.point") })[type]
        out.push(h("h2", {}, title))

        if (type === "none") {
            out.push(...this.frameSection(sel.frame || null))
            out.push(h("div", { className: "sg-help", innerHTML: HELP.map(key => tr(key)).join("<br><br>") }))
            return out
        }
        if (multi) out.push(h("p", { className: "sg-hint" }, tr("selection.multipleHint")))

        // device
        if (multi && dev) out.push(h("h3", {}, plural(sel.devices, "Device", "Devices")))
        if (type === "device") out.push(h("label", {}, tr("property.name")), text(sel.name, v => this.set("name", v)))
        if (multi && dev) {
            const pattern = h("input", { type: "text", placeholder: tr("property.renameExample") })
            const rename = () => { if (pattern.value.trim() !== "") { this.set("name_pattern", pattern.value); pattern.value = "" } }
            pattern.addEventListener("keydown", (e) => { if (e.key === "Enter") rename() })
            out.push(h("label", {}, tr("property.renamePattern")),
                     h("div", { className: "sg-inline" }, pattern, h("button", { onclick: rename }, tr("property.rename"))))
        }
        if (dev) {
            const kind = mixed(dev.kind) ? null : dev.kind
            const known = KIND_GROUPS.some(([, list]) => list.includes(kind))
            const options = [["", tr("property.none")]]
            if (kind && !known) options.push([kind, kind])
            options.push(...KIND_GROUPS.map(([g, list]) => [tr(g), list.map(k => [k, tr("device." + k.replace(/ /g, "_"))])]), ["\u0000other", tr("property.other")])
            out.push(h("label", {}, tr("property.type") + (kind === null ? tr("value.mixedSuffix") : "")),
                     select(options, kind, (v) => {
                         if (v === "\u0000other") {
                             const typed = prompt(tr("property.deviceType"), kind || "")
                             if (typed !== null) this.setFor("device", "kind", typed.trim())
                             else this.render()
                         } else this.setFor("device", "kind", v)
                     }))
            if (type === "device" && kind === "patch panel") {
                // Its sockets; the panel widens to fit them.
                out.push(h("div", { className: "sg-grid2" },
                    h("label", {}, tr("property.ports")), h("label", {}, tr("property.connector")),
                    number(sel.panel_ports, 1, 288, 1, v => this.set("panel_ports", Math.round(v)), 24),
                    select([["rj45", tr("term.rj45")], ["lc", tr("property.lcDuplex")], ["sc", tr("term.sc")]], sel.connector, v => this.set("connector", v))))
            }
            if (type === "device" && sel.takes_ports) {
                // A switch, router, firewall or server with ports (0: a box):
                // its front, odd ports on top, even ones below, the uplinks
                // (SFP) apart at the right.
                out.push(h("div", { className: "sg-grid2" },
                    h("label", {}, tr("property.portsOrBox")), h("label", {}, tr("property.connector")),
                    number(sel.panel_ports, 0, 288, 1, v => this.set("panel_ports", Math.round(v)), 0),
                    select([["rj45", tr("term.rj45")], ["sfp", tr("term.sfp")]], sel.connector, v => this.set("connector", v))))
                if (sel.panel_ports > 0) {
                    out.push(h("label", {}, tr("property.uplinks")),
                             number(sel.uplinks, 0, 48, 1, v => this.set("uplinks", Math.round(v)), 0))
                }
            }
            out.push(h("label", {}, tr("property.fillColor") + (mixed(dev.color) ? tr("value.mixedSuffix") : "")),
                     colorPicker(mixed(dev.color) ? "" : dev.color, DEVICE_COLORS, c => this.setFor("device", "color", c)))
            out.push(h("div", { className: "sg-grid2" },
                h("label", {}, tr("property.width") + (mixed(dev.w) ? tr("value.mixedSuffix") : "")), h("label", {}, tr("property.height") + (mixed(dev.h) ? tr("value.mixedSuffix") : "")),
                number(dev.w, 40, 2000, 10, v => this.setFor("device", "w", v), 120),
                number(dev.h, 30, 2000, 10, v => this.setFor("device", "h", v), 60)))
        }
        if (type === "device" || type === "port") {
            out.push(h("label", {}, type === "device" ? tr("property.addPortCount", { count: sel.ports }) : tr("property.addPort")),
                     h("div", { className: "sg-buttons4" }, [["top", tr("property.topButton")], ["right", tr("property.rightButton")], ["bottom", tr("property.bottomButton")], ["left", tr("property.leftButton")]]
                         .map(([side, label]) => h("button", { onclick: () => { this.ctl.addPort(side); this.app.changed() } }, label))))
        }

        // note
        if (multi && note) out.push(h("h3", {}, plural(sel.notes, "Text", "Texts")))
        if (note) {
            if (type === "note") out.push(h("label", {}, tr("selection.text")), textarea(sel.text, v => this.set("text", v), { wrap: "off" }))
            out.push(h("div", { className: "sg-inline" },
                h("span", {}, tr("property.size") + (mixed(note.size) ? tr("value.mixedSuffix") : "")),
                number(note.size, 6, 96, 1, v => this.setFor("note", "size", v), 14),
                checkbox(tr("property.bold") + (mixed(note.bold) ? tr("value.mixedSuffix") : ""), note.bold === true, c => this.setFor("note", "bold", c))))
            if (type === "note") {
                out.push(h("div", { className: "sg-inline" },
                    h("p", { className: "sg-hint" }, sel.boxed ? tr("property.fixedBoxHint")
                                                              : tr("property.resizeBoxHint")),
                    sel.boxed ? h("button", { onclick: () => this.setFor("note", "box", "fit") }, tr("property.fitText")) : null))
            }
            out.push(h("label", {}, tr("property.color") + (mixed(note.color) ? tr("value.mixedSuffix") : "")),
                     colorPicker(mixed(note.color) ? "" : note.color, LINK_COLORS, c => this.setFor("note", "color", c)))
        }

        // point
        if (type === "point") {
            out.push(h("p", { className: "sg-hint" },
                tr("point.intro") +
                plural(sel.links, "line", "lines") + tr("point.moveHint")))
            out.push(h("label", {}, tr("property.label")), text(sel.label, v => this.set("label", v), { placeholder: tr("value.endpointExample") }))
            out.push(h("div", { className: "sg-grid2" }, h("label", {}, "X"), h("label", {}, "Y"),
                number(Math.round(sel.x), -100000, 100000, 10, v => this.set("x", v), 0),
                number(Math.round(sel.y), -100000, 100000, 10, v => this.set("y", v), 0)))
        }

        // port
        if (type === "port") {
            out.push(h("p", { className: "sg-hint" }, tr("property.portConnections", { device: sel.device, count: sel.links })))
            out.push(h("label", {}, tr("property.portName")), text(sel.name, v => this.set("name", v)))
            out.push(h("label", {}, tr("property.side")), select(SIDES.map(s => [s, tr("side." + s)]), sel.side, v => this.set("side", v)))
            out.push(h("label", {}, tr("property.sidePosition", { side: tr(sel.side === "left" || sel.side === "right" ? "side.top" : "side.left") })),
                     number(sel.pos, 5, 95, 5, v => this.set("pos", v), 50))
            // A layer of its own, to show or hide it apart from its device.
            const itemLayers = (this.layers || []).filter(l => !l.frame)
            const own = itemLayers.find(l => l.id === sel.device_layer)
            out.push(h("label", { title: tr("layer.portVisibilityHint") }, tr("layer.label")),
                     select([["", tr("property.deviceLayer", { name: own ? own.name : "" })],
                             ...itemLayers.map(l => [String(l.id), l.path + (l.shown ? "" : tr("value.hiddenSuffix")) + (l.held ? tr("value.lockedSuffix") : "")])],
                            sel.port_layer ? String(sel.port_layer) : "", v => this.set("layer", v)))
        }

        // link
        if (multi && link) out.push(h("h3", {}, plural(sel.links, "Link", "Links")))
        if (link) {
            if (type === "link") out.push(h("p", { className: "sg-hint" }, sel.from + "  →  " + sel.to))
            out.push(h("label", {}, tr("property.type")), select(LINK_KINDS.map(([k, name]) => [k, tr(name)]), mixed(link.kind) ? null : link.kind, v => this.setFor("link", "kind", v)))
            if (type === "link") {
                // Encapsulation: what the line is carried over, innermost
                // first (VXLAN over IPsec over MPLS).
                const over = sel.over || []
                out.push(h("div", { className: "sg-inline" },
                    h("span", { innerHTML: over.length > 0 ? tr("link.carrierPrefix") + "<b>" + escapeHtml(sel.tag) + "</b>" : tr("link.noCarrier") }),
                    over.length > 0 ? h("button", { title: tr("link.removeOuterHint"),
                                                    onclick: () => this.set("over", over.slice(0, -1).join(",")) }, tr("link.removeOuter")) : null))
                const carriers = LINK_KINDS.filter(([k]) => k !== "plain")
                out.push(select([["", tr("link.addCarrier")], ...carriers.map(([k, name]) => [k, tr(name)])], "", (v) => {
                    if (v) this.set("over", over.concat([v]).join(","))
                }, { title: tr("link.carrierHint") }))
            }
            out.push(h("div", { className: "sg-inline" },
                h("label", {}, tr("property.label") + (mixed(link.label) ? tr("value.mixedSuffix") : "")),
                type === "link" && sel.label_placed ? h("button", { title: tr("link.autoPositionHint"),
                                                                    onclick: () => this.setFor("link", "label_at", "auto") }, tr("link.autoPosition")) : null))
            out.push(text(mixed(link.label) ? "" : link.label, v => this.setFor("link", "label", v),
                          { placeholder: multi ? tr("link.commonLabel") : tr("link.labelExample") }))
            out.push(checkbox(tr("link.showLabel") + (mixed(link.label_hidden) ? tr("value.mixedSuffix") : ""), link.label_hidden === false,
                              c => this.setFor("link", "label_hidden", !c),
                              { title: tr("link.showLabelHint") }))
            const pick = (key, list) => select(list.map(x => Array.isArray(x) ? [x[0], tr(x[1])] : [x, tr(x === "none" ? "property.none" : "style." + x)]), mixed(link[key]) ? null : link[key],
                                               v => this.setFor("link", key, v))
            out.push(h("div", { className: "sg-grid2" },
                h("label", {}, tr("link.start")), h("label", {}, tr("link.end")), pick("head_start", HEADS), pick("head_end", HEADS),
                h("label", {}, tr("link.line")), h("label", {}, tr("link.routing")), pick("style", STYLES), pick("routing", ROUTINGS)))
            out.push(h("div", { className: "sg-grid2" },
                h("label", {}, tr("property.width") + (mixed(link.width) ? tr("value.mixedSuffix") : "")),
                h("label", { title: tr("link.labelMarginHint") },
                  tr("link.labelMargin") + (mixed(link.label_margin) ? tr("value.mixedSuffix") : "")),
                number(link.width, 1, 10, 1, v => this.setFor("link", "width", v), 2),
                number(link.label_margin, 0, 200, 4, v => this.setFor("link", "label_margin", v), 24)))
            out.push(h("label", {}, tr("property.color") + (mixed(link.color) ? tr("value.mixedSuffix") : "")),
                     colorPicker(mixed(link.color) ? "" : link.color, LINK_COLORS, c => this.setFor("link", "color", c)))
            if (type === "link") out.push(h("button", { className: "sg-wide", onclick: () => { this.ctl.reverseLink(); this.app.changed() } },
                                            tr("link.reverse")))
            out.push(h("p", { className: "sg-hint" }, tr("link.styleHint")))
        }

        // layer and stacking
        if (["device", "note", "link", "point", "multi"].includes(type)) {
            const itemLayers = (this.layers || []).filter(l => !l.frame)
            const layerOptions = itemLayers.map(l => [String(l.id), l.path + (l.shown ? "" : tr("value.hiddenSuffix")) + (l.held ? tr("value.lockedSuffix") : "")])
            out.push(h("h3", {}, tr("layer.label")),
                     select(layerOptions, sel.layer >= 0 ? String(sel.layer) : null, v => this.set("layer", v)))
            out.push(h("label", {}, tr("arrange.label")),
                     h("div", { className: "sg-grid2" }, [["front", tr("arrange.front"), "Home"], ["forward", tr("arrange.forward"), "Page Up"],
                                                         ["back", tr("arrange.back"), "End"], ["backward", tr("arrange.backward"), "Page Down"]]
                         .map(([op, label, key]) => h("button", { title: key, onclick: () => { this.ctl.arrange(op); this.app.changed() } }, label))))
        }
        const dup = ["device", "note"].includes(type) || (multi && sel.devices + sel.notes > 0)
        out.push(h("div", { className: "sg-grid2 sg-actions" },
            dup ? h("button", { onclick: () => { this.ctl.duplicate(); this.app.changed() } }, tr("edit.duplicate")) : null,
            h("button", { onclick: () => { this.ctl.deleteSelected(); this.app.changed() } }, tr("edit.delete"))))
        return out
    }

    // frame

    frameSection(frame) {
        const t = frame ? frame.template : "none"
        const uses = (...ts) => ts.includes(t)
        const out = [h("h3", {}, tr("frame.label")),
                     select([["none", tr("property.none")], ...FRAME_GROUPS.map(([g, items]) => [tr(g), items.map(([id, name]) => [id, tr(name)])])], t, v => this.setFrame("template", v),
                            { title: tr("frame.hint") })]
        if (!frame) return out
        const by = (labels) => labels[t]
        const fields = [
            ["title", tr("frame.title"), ""],
            ["subtitle", tr("frame.subtitle"), tr("example.subtitle")],
            ["author", t === "signoff" ? tr("frame.preparedBy") : tr("frame.author"), ""],
            ["note", tr("frame.note"), tr("example.note")],
            ["customer", by({ proposal: tr("frame.preparedFor") }), tr("frame.customer")],
            ["company", by({ corporate: tr("frame.company"), proposal: tr("frame.company"), presentation: tr("frame.company") }), ""],
            ["address", by({ site: tr("frame.address") }), ""],
            ["location", by({ location: tr("frame.location") }), tr("example.locationPath")],
            ["doc_id", by({ iso: tr("frame.drawingNo"), change: tr("frame.changeId"), site: tr("frame.siteCode"), controlled: tr("frame.documentId"),
                            incident: tr("frame.incidentId"), location: tr("frame.locationCode") }),
             by({ iso: tr("example.drawingNo"), change: tr("example.changeId"), site: tr("example.siteCode"), controlled: tr("example.documentId"),
                  incident: tr("example.incidentId"), location: tr("example.locationCode") }) || ""],
            ["version", by({ controlled: tr("frame.version"), solution: tr("frame.version") }), tr("example.version")],
            ["revision", by({ iso: tr("frame.revision") }), ""],
            ["owner", by({ controlled: tr("frame.owner"), solution: tr("frame.owner") }), ""],
            ["review_date", by({ controlled: tr("frame.nextReview") }), tr("example.reviewDate")],
            ["reviewer", by({ signoff: tr("frame.reviewedBy"), solution: tr("frame.reviewers") }), ""],
            ["approver", by({ iso: tr("frame.checkedBy"), change: tr("frame.approvedBy"), signoff: tr("frame.approvedBy"), asbuilt: tr("frame.acceptedBy") }), ""],
            ["installer", by({ asbuilt: tr("frame.installedBy") }), ""],
            ["commissioned", by({ asbuilt: tr("frame.commissioned") }), tr("example.commissioned")],
            ["start", by({ incident: tr("frame.startTime") }), tr("example.start")],
            ["end", by({ incident: tr("frame.endTime") }), tr("example.ongoing")],
            ["services", by({ incident: tr("frame.affected") }), tr("example.services")],
            ["page", t === "iso" ? tr("frame.sheet") : tr("frame.page"), tr("example.page")],
        ]
        for (const [key, label, hint] of fields) {
            if (label) out.push(row(label, text(frame[key], v => this.setFrame(key, v), { placeholder: hint })))
        }
        // Status choices for the templates with a status badge.
        const statuses = { change: ["draft", "approved", "implemented", "rejected"], solution: ["draft", "in review", "final"],
                           noc: ["operational", "degraded", "outage", "maintenance"] }[t]
        if (statuses) {
            const labels = statuses.map(code => tr("badge." + code.replace(" ", "_")))
            const shown = labels[statuses.indexOf(frame.status)] ?? frame.status
            out.push(row(tr("frame.status"), suggest(shown, labels, v => this.setFrame("status", statuses[labels.indexOf(v)] ?? v))))
        }
        if (uses("classification", "controlled")) {
            const levels = ["public", "internal", "confidential", "restricted"]
            out.push(row(tr("frame.classification"), select(levels.map(l => [l, tr("classification." + l)]), frame.level || "internal", v => this.setFrame("level", v))))
        }
        if (uses("incident")) {
            out.push(row(tr("frame.severity"), select([["P1", tr("severity.p1")], ["P2", tr("severity.p2")], ["P3", tr("severity.p3")], ["P4", tr("severity.p4")]],
                                            (frame.severity || "P3").toUpperCase(), v => this.setFrame("severity", v))))
        }
        if (uses("executive")) {
            out.push(h("label", {}, tr("frame.summary")),
                     textarea(frame.summary, v => this.setFrame("summary", v), { placeholder: tr("frame.summaryHint") }))
        }
        out.push(row(tr("frame.watermark"), suggest(frame.watermark, ["draft", "confidential", "sample", "for_review", "superseded"].map(key => tr("watermark." + key)),
                                          v => this.setFrame("watermark", v))))
        out.push(row(tr("frame.margin"), number(frame.margin, 0, 400, 10, v => this.setFrame("margin", v), 40)))
        if (uses("revision")) {
            out.push(h("label", {}, tr("frame.revisionsFormat")),
                     textarea(frame.revisions, v => this.setFrame("revisions", v), { wrap: "off", placeholder: tr("example.revision") }))
        }
        out.push(checkbox(tr("frame.showDate"), frame.show_date, c => this.setFrame("show_date", c)))
        out.push(checkbox(tr("frame.showFilename"), frame.show_file, c => this.setFrame("show_file", c)))
        for (const [key, label] of [["logo", tr("frame.logo")], ["logo2", uses("proposal") ? tr("frame.customerLogo") : tr("frame.secondLogo")]]) {
            const input = h("input", { type: "file", accept: "image/png,image/jpeg", hidden: true })
            input.addEventListener("change", () => {
                const file = input.files[0]
                if (!file) return
                const reader = new FileReader()
                reader.onload = () => this.setFrame(key, reader.result)
                reader.readAsDataURL(file)
            })
            out.push(h("div", { className: "sg-inline" }, h("span", { className: "sg-grow" }, label), input,
                       h("button", { onclick: () => input.click() }, frame[key] ? tr("frame.changeLogo") : tr("frame.chooseLogo")),
                       frame[key] ? h("button", { onclick: () => this.setFrame(key, "") }, tr("action.remove")) : null))
        }
        out.push(h("label", {}, tr("property.color")), colorPicker(frame.color, FRAME_COLORS, c => this.setFrame("color", c)))
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
            h("h3", { className: "sg-grow" }, tr("layer.title")),
            h("button", { className: "sg-flat", title: tr("layer.addHint"), onclick: act(() => ctl.addLayer()) }, tr("layer.add")),
            h("button", { className: "sg-flat", title: tr("layer.addChildHint"),
                          disabled: !active || active.frame, onclick: act(() => ctl.addSublayer()) }, tr("layer.addChild")),
            h("button", { className: "sg-flat", title: tr("layer.removeHint"),
                          disabled: layers.length <= 1 || layers.some(l => l.active && l.held),
                          onclick: act(() => {
                              if (!active) return
                              // Asked first only when there is something to lose.
                              const ask = removeQuestion(active, layers)
                              if (!ask || confirm(ask + "\n" + tr("confirm.undoHint"))) ctl.removeLayer(active.id)
                          }) }, tr("action.remove")))
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
            const name = h("span", { className: "sg-grow sg-layer-name", title: (l.depth ? l.path + "\n" : "") + tr("sheet.renameHint") }, l.name)
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
                                     title: !l.visible ? tr("layer.hiddenHint") : l.shown ? tr("layer.shownHint") : tr("layer.parentHidden") })
            vis.addEventListener("click", (e) => e.stopPropagation())
            vis.addEventListener("change", act(() => ctl.setLayerVisible(l.id, vis.checked)))
            const icon = (cls, title, on, f) => h("button", { className: "sg-icon " + cls + (on ? " sg-on" : ""), title,
                                                               onclick: (e) => { e.stopPropagation(); f() } })
            const bg = l.background
            const bgIcon = l.frame ? null : icon("sg-bg", bg ? tr("layer.backgroundHint") : tr("layer.addBackgroundHint"),
                                                 !!bg, () => this.backgroundPopup(l))
            if (bgIcon && bg) {
                bgIcon.style.setProperty("--fill", bg.fill)
                bgIcon.style.setProperty("--border", bg.border === "none" ? "var(--sg-help)" : bg.border_color)
                bgIcon.style.setProperty("--radius", bg.rounded ? "3px" : "0")
            }
            // Sub-layers fold away under their layer.
            const fold = l.children > 0
                ? h("button", { className: "sg-fold", title: l.expanded ? tr("layer.foldHint") : tr("property.expandLayers", { count: l.children }),
                                onclick: (e) => { e.stopPropagation(); ctl.setLayerExpanded(l.id, !l.expanded); this.app.changed() } },
                    l.expanded ? "\u25be" : "\u25b8")
                : h("span", { className: "sg-fold" })
            const row = h("div", { className: "sg-layer" + (l.active ? " sg-active" : ""),
                              onclick: () => { if (!l.frame) { ctl.setActiveLayer(l.id); this.app.changed() } } },
                fold, vis, name,
                bgIcon,
                l.frame ? null : icon("sg-tag", l.show_name ? tr("layer.shownTagHint") : tr("layer.showTagHint"),
                                      l.show_name, act(() => ctl.setLayerShowName(l.id, !l.show_name))),
                l.frame ? null : icon("sg-lock", l.locked ? tr("layer.lockedHint") : tr("layer.lockHint"),
                                      l.locked, act(() => ctl.setLayerLocked(l.id, !l.locked))),
                h("span", { className: "sg-count", title: l.frame ? tr("layer.frameHint")
                                                          : l.children ? tr("layer.countHint") : "" },
                  l.frame ? tr("layer.frameBadge") : String(l.children ? total(l) : l.count)))
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
            h("p", { className: "sg-hint" }, tr("layer.instructions")))
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
            const kids = [h("h3", {}, tr("property.layerBackground", { name: l.name })),
                          checkbox(tr("background.enabled"), !!bg, c => set("enabled", c))]
            if (bg) {
                kids.push(row(tr("frame.margin"), number(bg.margin, 0, 400, 5, v => set("margin", v), 30)),
                          row(tr("background.opacity"), number(bg.opacity, 0, 100, 5, v => set("opacity", v), 50)),
                          row(tr("background.corners"), select([["true", tr("background.rounded")], ["false", tr("background.square")]], String(!!bg.rounded), v => set("rounded", v))))
                if (bg.rounded) kids.push(row(tr("background.radius"), number(bg.radius, 0, 100, 2, v => set("radius", v), 14)))
                kids.push(row(tr("background.border"), select(["none", "solid", "dashed", "dotted"].map(s => [s, tr(s === "none" ? "property.none" : "style." + s)]), bg.border, v => set("border", v))))
                if (bg.border !== "none") kids.push(row(tr("background.borderWidth"), number(Math.round(bg.border_width), 1, 10, 1, v => set("border_width", v), 2)))
                kids.push(h("label", {}, tr("background.fill")), colorPicker(bg.fill, BG_FILLS, c => set("fill", c)))
                if (bg.border !== "none") kids.push(h("label", {}, tr("background.borderColor")), colorPicker(bg.border_color, BG_BORDERS, c => set("border_color", c)))
            }
            kids.push(h("button", { className: "sg-wide", onclick: () => close() }, tr("background.close")))
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
