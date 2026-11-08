// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import com.stratagram

// Context-sensitive editor for the current selection, with the layer list
// pinned underneath.
Pane {
    // The interface colors of the theme (given by main.qml).
    property var tc: ({ text: "#0f172a", muted: "#475569", muted2: "#64748b", faint: "#94a3b8", line: "#e2e8f0", alt: "#f1f5f9", base: "#ffffff", accent: "#2563eb", border: "#cbd5e1", drop: "#bfdbfe", dropParent: "#e0ecff", active: "#dbeafe" })
    id: root
    required property DiagramController ctl
    readonly property var sel: JSON.parse(ctl.selection)
    readonly property string type: sel.type
    readonly property var layers: JSON.parse(ctl.layers)
    // The rows of the layer list: sub-layers of folded layers left out.
    readonly property var layerRows: layers.filter(l => !l.folded)
    // A layer's items with those of the layers under it.
    function layerTotal(l) {
        return layers.filter(x => x.id === l.id || x.path.startsWith(l.path + " / ")).reduce((n, x) => n + x.count, 0)
    }
    // What removing layer `l` asks, naming what goes with it: the layers
    // under it and the items on them, those there are; "" with nothing.
    function removeQuestion(l) {
        const under = layers.filter(x => x.path.startsWith(l.path + " / "))
        const items = l.count + under.reduce((n, x) => n + x.count, 0)
        const n = (k, one) => k + " " + one + (k === 1 ? "" : "s")
        const subs = under.length ? n(under.length, "layer") + " under it" : ""
        const on = items ? n(items, "item") + (under.length ? " on them" : " on it") : ""
        if (!subs && !on) return ""
        return "Remove \"" + l.name + "\" and " + (subs && on ? "the " + subs + " and the " + on : "the " + (subs || on)) + "?"
    }

    // Where the layer list was scrolled to, kept when its rows change.
    property real layerScroll: -1
    function keepLayerScroll() { layerScroll = layerList.contentY }

    // Dragging a layer: before or after another (under the same layer), or
    // into it, which then lights up as the new parent.
    property int dragLayer: 0
    property int dropTarget: 0
    property string dropPlace: ""
    // Where the pointer is in the list (for scrolling at its edges).
    property real dragViewY: -1
    readonly property int dropParent: dropTarget === 0 ? 0 : dropPlace === "into" ? dropTarget
                                      : (layers.find(l => l.id === dropTarget) || { parent: 0 }).parent
    function layerUnder(id, of) {
        const parentOf = {}
        for (const l of layers) parentOf[l.id] = l.parent
        const seen = {}
        for (let at = parentOf[id]; at && !seen[at]; at = parentOf[at]) {
            if (at === of) return true
            seen[at] = true
        }
        return false
    }
    // The pointer at `y` in the list's content.
    function dragOver(y) {
        dragViewY = y - layerList.contentY
        const i = layerList.indexAt(10, y)
        if (i < 0) {
            // Below the last row: to the end, as a top layer.
            const tops = layers.filter(l => !l.parent && !l.frame)
            const last = tops[tops.length - 1]
            if (y > 0 && last && last.id !== dragLayer && !layerUnder(last.id, dragLayer)) { dropTarget = last.id; dropPlace = "after" }
            else { dropTarget = 0; dropPlace = "" }
            return
        }
        const l = layerRows[i]
        if (!l || l.id === dragLayer || layerUnder(l.id, dragLayer)) { dropTarget = 0; dropPlace = ""; return }
        const item = layerList.itemAtIndex(i)
        const f = item ? (y - item.y) / item.height : 0.5
        let place = f < 0.25 ? "before" : f > 0.75 ? "after" : "into"
        if (l.frame && place === "into") place = f < 0.5 ? "before" : "after"
        // Below an unfolded layer the list shows its sub-layers: in there.
        if (place === "after" && l.children > 0 && l.expanded) place = "into"
        dropTarget = l.id
        dropPlace = place
    }
    function dropDragged() {
        if (dragLayer && dropTarget) root.ctl.moveLayer(dragLayer, dropTarget, dropPlace)
        dragLayer = 0
        dropTarget = 0
        dropPlace = ""
        dragViewY = -1
    }
    // Layers that hold items (the frame's layer holds only the frame).
    readonly property var itemLayers: layers.filter(l => !l.frame)
    readonly property bool arrangeable: ["device", "note", "link", "point", "multi"].includes(type)
    // Values per kind of item, for one item or (with "multi") shared by all
    // selected items of that kind; a null value means they differ.
    readonly property bool multi: type === "multi"
    readonly property var dev: type === "device" ? sel : (multi ? sel.dev : null)
    readonly property var note: type === "note" ? sel : (multi ? sel.note : null)
    readonly property var link: type === "link" ? sel : (multi ? sel.link : null)

    // Device types by category, as in the frame list; headers are not
    // selectable. "" is a plain device.
    readonly property var kindGroups: [
        ["", [""]],
        ["Routing & switching", ["router", "ospf router", "bgp reflector", "switch", "load balancer"]],
        ["Security", ["firewall", "cloud proxy", "zscaler"]],
        ["Wireless", ["access point", "standalone ap", "managed ap", "wlan controller"]],
        ["Servers & clients", ["server", "client"]],
        ["Virtualization", ["virtual switch", "distributed virtual switch"]],
        ["Data center", ["aci l3out"]],
        ["Cabling", ["patch panel"]],
        ["Internet & cloud", ["internet", "cloud", "aws", "aws transit gateway", "aws direct connect gateway", "azure", "google cloud", "alibaba cloud", "telekom cloud",
                              "stackit"]]
    ]
    readonly property var kindItems: {
        const items = []
        for (const [group, list] of kindGroups) {
            if (group !== "") items.push({ header: true, key: "", name: group, group: group })
            for (const key of list) items.push({ header: false, key: key, name: key === "" ? "none" : key, group: group })
        }
        return items
    }
    readonly property var deviceColors: ["#dbeafe", "#bfdbfe", "#bbf7d0", "#fde68a", "#fecaca", "#ddd6fe",
                                         "#e0f2fe", "#ccfbf1", "#fef3c7", "#e5e7eb", "#1e3a8a", "#334155"]
    readonly property var linkColors: ["#334155", "#0f172a", "#2563eb", "#16a34a", "#dc2626", "#ea580c",
                                       "#9333ea", "#0891b2", "#94a3b8"]
    readonly property var heads: ["none", "arrow", "open", "diamond", "circle"]
    readonly property var styles: ["solid", "dashed", "dotted"]
    readonly property var routings: ["straight", "orthogonal", "around"]
    readonly property var routingNames: ["straight", "orthogonal", "around objects"]
    readonly property var sides: ["top", "right", "bottom", "left"]
    // Frame templates by category; headers are not selectable.
    readonly property var frameGroups: [
        ["", [["none", "none"]]],
        ["Basic", [["lines", "lines"], ["banner", "banner"], ["minimal", "minimal"], ["card", "card"]]],
        ["Business", [["corporate", "corporate"], ["report", "report page"], ["presentation", "presentation (16:9)"],
                      ["executive", "executive summary"], ["proposal", "customer proposal"]]],
        ["Technical drawing", [["titleblock", "title block"], ["iso", "ISO title block"], ["revision", "revision block"],
                               ["blueprint", "blueprint"], ["asbuilt", "as-built"]]],
        ["Documentation & governance", [["controlled", "controlled document"], ["signoff", "sign-off"],
                                        ["solution", "solution design"], ["change", "change request"],
                                        ["classification", "classification banner"]]],
        ["Operations", [["site", "site sheet"], ["location", "data center location"], ["incident", "incident report"],
                        ["noc", "NOC display"]]]
    ]
    readonly property var frameItems: {
        const items = []
        for (const [group, list] of frameGroups) {
            if (group !== "") items.push({ header: true, name: group, key: "", group: group })
            for (const [key, name] of list) items.push({ header: false, name: name, key: key, group: group })
        }
        return items
    }
    readonly property var frame: sel ? sel.frame : null
    readonly property string frameTemplate: frame ? frame.template : "none"
    function frameUses(...templates) { return templates.includes(frameTemplate) }
    // The text fields the current template shows: key, label, placeholder.
    readonly property var frameFields: {
        const t = frameTemplate
        const by = (labels) => labels[t]
        const all = [
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
            ["page", t === "iso" ? "Sheet" : "Page", "e.g. 1/3"]
        ]
        return all.filter(f => f[1]).map(f => ({ key: f[0], label: f[1], hint: f[2] }))
    }
    // Status choices for the templates with a status badge.
    readonly property var statusChoices: ({
        change: ["draft", "approved", "implemented", "rejected"],
        solution: ["draft", "in review", "final"],
        noc: ["operational", "degraded", "outage", "maintenance"]
    })[frameTemplate] || null
    function setFrame(key, value) { ctl.setFrame(key, String(value)) }
    readonly property var linkKinds: ["plain", "trunk", "ipsec", "vxlan", "sdwan", "mpls", "l2", "l3", "l3sub", "capwap"]
    readonly property var linkKindNames: ["plain", "trunk (802.1Q)", "IPsec tunnel", "VXLAN tunnel", "SD-WAN", "MPLS",
                                         "Layer 2 (switched)", "Layer 3 (routed)", "L3 subinterface (802.1Q)",
                                         "CAPWAP tunnel (AP to WLC)"]

    function set(key, value) { ctl.setProp(key, String(value)) }
    // Set a property on every selected item of one kind ("device", "note", "link").
    function setFor(kind, key, value) { ctl.setProp(kind + ":" + key, String(value)) }
    function mixed(v) { return v === null || v === undefined }
    function plural(n, one, many) { return n + " " + (n === 1 ? one : many) }

    padding: 12
    background: Rectangle {
        color: root.tc.base
        Rectangle { width: 1; height: parent.height; color: root.tc.line }
    }

    ColumnLayout {
        anchors.fill: parent
        spacing: 0

        ScrollView {
            Layout.fillWidth: true
            Layout.fillHeight: true
            // Reach into the panel's right padding: the scroll bar sits
            // there, at the panel's edge, beside the content instead of on
            // top of it (where clicks meant for the fields hit the bar).
            Layout.rightMargin: -root.rightPadding + 1
            rightPadding: root.rightPadding - 1
            contentWidth: availableWidth

            ColumnLayout {
                width: parent.width
                spacing: 8

                Label {
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    text: root.multi
                          ? [root.sel.devices ? root.plural(root.sel.devices, "device", "devices") : "",
                             root.sel.links ? root.plural(root.sel.links, "link", "links") : "",
                             root.sel.notes ? root.plural(root.sel.notes, "text", "texts") : "",
                             root.sel.points ? root.plural(root.sel.points, "point", "points") : ""].filter(t => t).join(", ")
                          : ({ none: "Nothing selected", device: "Device", note: "Text", port: "Port", link: "Link",
                               point: "Connection point" })[root.type]
                    font.bold: true
                    font.pixelSize: 15
                }

                // frame
                ColumnLayout {
                    visible: root.type === "none"
                    Layout.fillWidth: true
                    spacing: 6
                    Label { text: "Frame"; font.bold: true }
                    ComboBox {
                        id: templateBox
                        Layout.fillWidth: true
                        model: root.frameItems
                        currentIndex: root.frameItems.findIndex(i => !i.header && i.key === root.frameTemplate)
                        displayText: currentIndex < 0 ? "" : (root.frameItems[currentIndex].group
                                     ? root.frameItems[currentIndex].group + "  ›  " : "") + root.frameItems[currentIndex].name
                        onActivated: (i) => { if (!root.frameItems[i].header) root.setFrame("template", root.frameItems[i].key) }
                        delegate: ItemDelegate {
                            required property var modelData
                            required property int index
                            width: ListView.view ? ListView.view.width : templateBox.width
                            enabled: !modelData.header
                            highlighted: templateBox.highlightedIndex === index
                            leftPadding: modelData.header ? 8 : 22
                            contentItem: Label {
                                text: modelData.header ? modelData.name.toUpperCase() : modelData.name
                                font.bold: modelData.header || modelData.key === root.frameTemplate
                                font.pixelSize: modelData.header ? 10 : 13
                                color: modelData.header ? root.tc.muted2 : root.tc.text
                            }
                        }
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: "A border with header and footer around the diagram, on its own bottom layer"
                    }
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WrapAnywhere
                        color: root.tc.faint
                        font.pixelSize: 11
                        text: "Defaults (author, logo, …): [frame] or [frame-" +
                              (root.frameTemplate === "none" ? "<template>" : root.frameTemplate) + "] in " +
                              root.ctl.configPath()
                    }
                    Repeater {
                        model: root.frame ? root.frameFields : []
                        RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            Label { text: modelData.label; Layout.preferredWidth: 88; elide: Text.ElideRight }
                            TextField {
                                Layout.fillWidth: true
                                text: root.frame ? (root.frame[modelData.key] || "") : ""
                                placeholderText: modelData.hint
                                onTextEdited: root.setFrame(modelData.key, text)
                            }
                        }
                    }
                    RowLayout {
                        visible: root.statusChoices !== null
                        Layout.fillWidth: true
                        Label { text: "Status"; Layout.preferredWidth: 88 }
                        ComboBox {
                            Layout.fillWidth: true
                            editable: true
                            model: root.statusChoices || []
                            editText: root.frame ? root.frame.status : ""
                            onActivated: root.setFrame("status", currentText)
                            onAccepted: root.setFrame("status", editText)
                        }
                    }
                    RowLayout {
                        visible: root.frameUses("classification", "controlled")
                        Layout.fillWidth: true
                        Label { text: "Classification"; Layout.preferredWidth: 88; elide: Text.ElideRight }
                        ComboBox {
                            Layout.fillWidth: true
                            readonly property var levels: ["public", "internal", "confidential", "restricted"]
                            model: levels
                            currentIndex: Math.max(0, levels.indexOf(root.frame && root.frame.level ? root.frame.level : "internal"))
                            onActivated: (i) => root.setFrame("level", levels[i])
                        }
                    }
                    RowLayout {
                        visible: root.frameUses("incident")
                        Layout.fillWidth: true
                        Label { text: "Severity"; Layout.preferredWidth: 88 }
                        ComboBox {
                            Layout.fillWidth: true
                            readonly property var levels: ["P1", "P2", "P3", "P4"]
                            model: ["P1 · critical", "P2 · high", "P3 · medium", "P4 · low"]
                            currentIndex: Math.max(0, levels.indexOf(root.frame && root.frame.severity ? root.frame.severity.toUpperCase() : "P3"))
                            onActivated: (i) => root.setFrame("severity", levels[i])
                        }
                    }
                    ColumnLayout {
                        visible: root.frameUses("executive")
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: "Summary" }
                        TextArea {
                            Layout.fillWidth: true
                            Layout.minimumHeight: 70
                            wrapMode: TextEdit.Wrap
                            placeholderText: "A few sentences on what the diagram shows"
                            text: root.frame ? root.frame.summary : ""
                            onTextChanged: if (activeFocus) root.setFrame("summary", text)
                            background: Rectangle { border.color: root.tc.border; radius: 3 }
                        }
                    }
                    RowLayout {
                        visible: root.frame !== null
                        Layout.fillWidth: true
                        Label { text: "Watermark"; Layout.preferredWidth: 88 }
                        ComboBox {
                            Layout.fillWidth: true
                            editable: true
                            model: ["", "DRAFT", "CONFIDENTIAL", "SAMPLE", "FOR REVIEW", "SUPERSEDED"]
                            editText: root.frame ? root.frame.watermark : ""
                            onActivated: root.setFrame("watermark", currentText)
                            onAccepted: root.setFrame("watermark", editText)
                        }
                    }
                    RowLayout {
                        visible: root.frame !== null
                        Layout.fillWidth: true
                        Label { text: "Margin"; Layout.preferredWidth: 88 }
                        SpinBox {
                            Layout.fillWidth: true
                            from: 0; to: 400; stepSize: 10
                            editable: true
                            value: root.frame ? root.frame.margin : 40
                            onValueModified: root.setFrame("margin", value)
                        }
                    }
                    ColumnLayout {
                        visible: root.frameUses("revision")
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: "Revisions, one per line: rev | date | change | by" }
                        TextArea {
                            Layout.fillWidth: true
                            Layout.minimumHeight: 60
                            wrapMode: TextEdit.NoWrap
                            placeholderText: "A | 2026-10-01 | First issue | Shin"
                            text: root.frame ? root.frame.revisions : ""
                            onTextChanged: if (activeFocus) root.setFrame("revisions", text)
                            background: Rectangle { border.color: root.tc.border; radius: 3 }
                        }
                    }
                    CheckBox {
                        visible: root.frame !== null
                        text: "Show date and time of last change"
                        checked: root.frame ? root.frame.show_date : true
                        onToggled: root.setFrame("show_date", checked)
                    }
                    CheckBox {
                        visible: root.frame !== null
                        text: "Show the file name"
                        checked: root.frame ? root.frame.show_file : false
                        onToggled: root.setFrame("show_file", checked)
                    }
                    Repeater {
                        model: root.frame ? [["logo", "Logo"], ["logo2", root.frameUses("proposal") ? "Customer logo" : "Second logo"]] : []
                        RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            Label { text: modelData[1]; Layout.fillWidth: true }
                            Button {
                                text: root.frame && root.frame[modelData[0]] ? "Change…" : "Choose…"
                                onClicked: { logoDialog.key = modelData[0]; logoDialog.open() }
                            }
                            Button {
                                visible: root.frame !== null && root.frame[modelData[0]]
                                text: "Remove"
                                onClicked: root.setFrame(modelData[0], "")
                            }
                        }
                    }
                    ColumnLayout {
                        visible: root.frame !== null
                        Layout.fillWidth: true
                        spacing: 2
                        Label {
                            text: "Save as default for new frames"
                            ToolTip.visible: saveHover.hovered
                            ToolTip.delay: 500
                            ToolTip.text: "Author, company, note, color, margin, options, level, owner, reviewer, " +
                                          "approver, installer and logos go into " + root.ctl.configPath() +
                                          "; title, IDs and dates stay with this diagram"
                            HoverHandler { id: saveHover }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Button {
                                Layout.fillWidth: true
                                text: "All templates"
                                onClicked: root.ctl.saveFrameDefaults(true)
                            }
                            Button {
                                Layout.fillWidth: true
                                text: "Only " + ((root.frameItems.find(i => i.key === root.frameTemplate) || {}).name || "")
                                onClicked: root.ctl.saveFrameDefaults(false)
                            }
                        }
                    }
                    FileDialog {
                        id: logoDialog
                        property string key: "logo"
                        title: "Choose a logo"
                        nameFilters: ["Images (*.png *.jpg *.jpeg)"]
                        onAccepted: root.setFrame(key, decodeURIComponent(selectedFile.toString().replace(/^file:\/\//, "")))
                    }
                    ColorPicker {
                        tc: root.tc
                        visible: root.frame !== null
                        Layout.fillWidth: true
                        color: root.frame ? root.frame.color : "#1e293b"
                        swatches: ["#1e293b", "#000000", "#1e3a8a", "#2563eb", "#0f766e", "#16a34a", "#b91c1c",
                                   "#ea580c", "#7c3aed", "#e20074"]
                        onPicked: (c) => root.setFrame("color", c)
                    }
                    Rectangle { Layout.fillWidth: true; Layout.topMargin: 4; height: 1; color: root.tc.line }
                }

                // help
                Label {
                    visible: root.type === "none"
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: root.tc.muted
                    textFormat: Text.StyledText
                    text: "<b>Double-click</b> empty space to add a device.<br><br>" +
                          "<b>Drag from a device border</b> (or a port) onto another device to connect them. " +
                          "Ports are created where you start and end.<br><br>" +
                          "<b>Double-click a border</b> to add a port.<br><br>" +
                          "<b>Drag a port along the border</b> to move it (Shift+drag moves it from anywhere); " +
                          "pull a port with lines off the device to turn it into a free point (the lines stay), " +
                          "or pull an unused port away to draw a link.<br><br>" +
                          "<b>Double-click</b> a device, port, link or text to edit it in place, " +
                          "or select it and press F2, Enter or just start typing.<br><br>" +
                          "<b>Quick add</b> (Ctrl+K): type devices into the toolbar field, comma-separated, " +
                          "and press Enter. <i>Firewall, Router</i> adds a firewall and a router; " +
                          "<i>firewall|my-box</i> adds a firewall labeled my-box.<br><br>" +
                          "<b>+ Text</b> (Ctrl+Shift+T) adds a free text annotation; Shift+Enter starts a new line.<br><br>" +
                          "<b>Drop a device on a line</b> (it lights up green) to put it in between; deleting " +
                          "the device joins the line again when both halves have the same type and label.<br><br>" +
                          "<b>Drag a line</b> to bend it: a connection point appears where you grab it and " +
                          "the line runs through it. Delete the point to straighten the line again.<br><br>" +
                          "<b>Lines to nowhere</b>: drop a line on empty space and it ends at a free connection point " +
                          "(a small ring, not exported), e.g. an arrow pointing out of the drawing. Drag the ring to " +
                          "move the end; drop it on a device border (or port) to attach the lines there. Shift+drag " +
                          "from it to draw on, drop other lines onto it. <b>+ Point</b> " +
                          "(Ctrl+Shift+P) adds one on its own.<br><br>" +
                          "<b>Drag on empty space</b> to select several items (links too), <b>Ctrl+click</b> to add " +
                          "devices, texts or links to the selection. With several selected, this panel edits them " +
                          "all at once: type, color, size, line style, arrows, labels, layer, and numbered renaming " +
                          "(<i>sw-##</i> gives sw-01, sw-02…).<br><br>" +
                          "<b>Hold Ctrl while dragging</b> a box or port to line it up with nearby boxes and ports " +
                          "(edges, centers, straight links); a red guide shows the match. It also snaps to equal " +
                          "spacing: midway between two boxes (also diagonally), or repeating the gap of the next " +
                          "pair in a row or column. Pink markers show the equal distances.<br><br>" +
                          "<b>Layers</b> (bottom of this panel): new items go into the highlighted layer. " +
                          "Tick a layer to show or hide it; hidden layers cannot be selected and are left out of " +
                          "SVG export. The padlock locks a layer: its items stay visible but are read only (not " +
                          "selectable, movable or connectable) and nothing new can be added to it; clicks pass " +
                          "through to what is underneath. The tag shows the layer's name in the lower right " +
                          "corner of its devices. " +
                          "Move a selection to another layer with its Layer box.<br><br>" +
                          "<b>Arrange</b> stacks items in front of or behind others, across all layers: " +
                          "Home to front, Page Up (Ctrl+]) forward, Page Down (Ctrl+[) backward, End to back.<br><br>" +
                          "<b>Zoom into</b> (Ctrl+I): zoom into a device as far as you like and add things " +
                          "inside it; they are made small to match the zoom, fade out when you zoom back out and " +
                          "reappear when you zoom in again. Moving a device moves what is inside it.<br><br>" +
                          "<b>Layout ▾</b> (Ctrl+L) arranges devices automatically: hierarchy (top down or left " +
                          "to right), radial, circle, grid or force-directed. It works on the selected devices, " +
                          "or on all of them; lines that would cross a device or text are rerouted. Ctrl+Z " +
                          "undoes.<br><br>" +
                          "<b>Pan</b> with middle/right drag or Space+drag, <b>zoom</b> with the wheel.<br><br>" +
                          "<b>Ctrl+C</b> copies the selection (or everything) for draw.io: paste it there with " +
                          "Ctrl+V. <b>Ctrl+V</b> here pastes stratagram copies, shapes copied in draw.io, or text " +
                          "(as a note); Ctrl+X cuts.<br><br>" +
                          "Arrow keys nudge, Del deletes, Ctrl+D duplicates."
                }

                // device
                Label {
                    visible: root.multi
                    text: "Changes apply to all selected items of each kind."
                    Layout.fillWidth: true
                    wrapMode: Text.WordWrap
                    color: root.tc.muted2
                }
                Label {
                    visible: root.multi && root.dev !== null
                    text: root.multi ? root.plural(root.sel.devices, "Device", "Devices") : ""
                    font.bold: true
                    Layout.topMargin: 4
                }
                ColumnLayout {
                    visible: root.type === "device"
                    Layout.fillWidth: true
                    spacing: 6

                    Label { text: "Name" }
                    TextField {
                        id: deviceName
                        Layout.fillWidth: true
                        text: root.type === "device" ? root.sel.name : ""
                        onTextEdited: root.set("name", text)
                    }
                }

                ColumnLayout {
                    visible: root.multi && root.dev !== null
                    Layout.fillWidth: true
                    spacing: 6
                    Label { text: "Rename in reading order (# is the number)" }
                    RowLayout {
                        Layout.fillWidth: true
                        TextField {
                            id: renamePattern
                            Layout.fillWidth: true
                            placeholderText: "e.g. sw-## gives sw-01, sw-02…"
                            onAccepted: rename()
                            function rename() { if (text.trim() !== "") { root.set("name_pattern", text); text = "" } }
                        }
                        Button { text: "Rename"; enabled: renamePattern.text.trim() !== ""; onClicked: renamePattern.rename() }
                    }
                }

                ColumnLayout {
                    visible: root.dev !== null
                    Layout.fillWidth: true
                    spacing: 6

                    Label { text: "Type" + (root.dev && root.mixed(root.dev.kind) ? " (mixed)" : "") }
                    ComboBox {
                        id: kindBox
                        Layout.fillWidth: true
                        // Any type can be typed; the list offers the known
                        // ones by category.
                        editable: true
                        model: root.kindItems
                        textRole: "key"
                        property string current: root.dev && !root.mixed(root.dev.kind) ? root.dev.kind : ""
                        function indexOfKind(k) { return root.kindItems.findIndex(i => !i.header && i.key === k) }
                        onCurrentChanged: { currentIndex = indexOfKind(current); editText = current }
                        Component.onCompleted: { currentIndex = indexOfKind(current); editText = current }
                        onActivated: (i) => {
                            if (root.kindItems[i].header) { currentIndex = indexOfKind(current); editText = current; return }
                            root.setFor("device", "kind", root.kindItems[i].key)
                        }
                        onAccepted: root.setFor("device", "kind", editText.trim())
                        delegate: ItemDelegate {
                            required property var modelData
                            required property int index
                            width: ListView.view ? ListView.view.width : kindBox.width
                            enabled: !modelData.header
                            highlighted: kindBox.highlightedIndex === index
                            leftPadding: modelData.header ? 8 : 22
                            contentItem: Label {
                                text: modelData.header ? modelData.name.toUpperCase() : modelData.name
                                font.bold: modelData.header || (modelData.key === kindBox.current && kindBox.current !== "")
                                font.pixelSize: modelData.header ? 10 : 13
                                font.italic: !modelData.header && modelData.key === ""
                                color: modelData.header ? root.tc.muted2 : root.tc.text
                            }
                        }
                    }

                    Label { text: "Fill color" + (root.dev && root.mixed(root.dev.color) ? " (mixed)" : "") }
                    ColorPicker {
                        tc: root.tc
                        Layout.fillWidth: true
                        color: root.dev && !root.mixed(root.dev.color) ? root.dev.color : ""
                        swatches: root.deviceColors
                        onPicked: (c) => root.setFor("device", "color", c)
                    }
                }

                // A patch panel's sockets; it widens to fit them.
                GridLayout {
                    visible: root.type === "device" && root.sel.kind === "patch panel"
                    columns: 2
                    Layout.fillWidth: true
                    Label { text: "Ports" }
                    Label { text: "Connector" }
                    SpinBox {
                        Layout.fillWidth: true
                        from: 1; to: 288; editable: true
                        value: root.type === "device" && root.sel.panel_ports ? root.sel.panel_ports : 24
                        onValueModified: root.set("panel_ports", value)
                    }
                    ComboBox {
                        Layout.fillWidth: true
                        readonly property var keys: ["rj45", "lc", "sc"]
                        model: ["RJ45", "LC (duplex)", "SC"]
                        currentIndex: Math.max(0, keys.indexOf(root.type === "device" ? root.sel.connector : "rj45"))
                        onActivated: (i) => root.set("connector", keys[i])
                    }
                }

                // The ports of a switch, router, firewall or server (0: a
                // box): odd ones on top, even ones below, the uplinks (SFP)
                // apart at the right.
                GridLayout {
                    visible: root.type === "device" && !!root.sel.takes_ports
                    columns: 2
                    Layout.fillWidth: true
                    Label { text: "Ports (0: a box)" }
                    Label { text: "Connector" }
                    SpinBox {
                        Layout.fillWidth: true
                        from: 0; to: 288; editable: true
                        value: root.type === "device" && root.sel.panel_ports ? root.sel.panel_ports : 0
                        onValueModified: root.set("panel_ports", value)
                    }
                    ComboBox {
                        Layout.fillWidth: true
                        readonly property var keys: ["rj45", "sfp"]
                        model: ["RJ45", "SFP"]
                        currentIndex: Math.max(0, keys.indexOf(root.type === "device" ? root.sel.connector : "rj45"))
                        onActivated: (i) => root.set("connector", keys[i])
                    }
                    Label {
                        visible: root.type === "device" && root.sel.panel_ports > 0
                        text: "Uplinks (SFP)"
                        Layout.columnSpan: 2
                    }
                    SpinBox {
                        visible: root.type === "device" && root.sel.panel_ports > 0
                        Layout.fillWidth: true
                        Layout.columnSpan: 2
                        from: 0; to: 48; editable: true
                        value: root.type === "device" && root.sel.uplinks ? root.sel.uplinks : 0
                        onValueModified: root.set("uplinks", value)
                    }
                }

                GridLayout {
                    visible: root.dev !== null
                    columns: 2
                    Layout.fillWidth: true
                    Label { text: "Width" + (root.dev && root.mixed(root.dev.w) ? " (mixed)" : "") }
                    Label { text: "Height" + (root.dev && root.mixed(root.dev.h) ? " (mixed)" : "") }
                    SpinBox {
                        Layout.fillWidth: true
                        from: 40; to: 2000; stepSize: 10; editable: true
                        value: root.dev && !root.mixed(root.dev.w) ? root.dev.w : 120
                        onValueModified: root.setFor("device", "w", value)
                    }
                    SpinBox {
                        Layout.fillWidth: true
                        from: 30; to: 2000; stepSize: 10; editable: true
                        value: root.dev && !root.mixed(root.dev.h) ? root.dev.h : 60
                        onValueModified: root.setFor("device", "h", value)
                    }
                }

                ColumnLayout {
                    visible: root.type === "device" || root.type === "port"
                    Layout.fillWidth: true
                    spacing: 6
                    Label { text: root.type === "device" ? "Add port (" + root.sel.ports + " so far)" : "Add another port" }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 4
                        Repeater {
                            model: [["top", "↑ Top"], ["right", "Right →"], ["bottom", "↓ Bottom"], ["left", "← Left"]]
                            Button {
                                required property var modelData
                                Layout.fillWidth: true
                                text: modelData[1]
                                onClicked: root.ctl.addPort(modelData[0])
                            }
                        }
                    }
                }

                // note
                Label {
                    visible: root.multi && root.note !== null
                    text: root.multi ? root.plural(root.sel.notes, "Text", "Texts") : ""
                    font.bold: true
                    Layout.topMargin: 4
                }
                ColumnLayout {
                    visible: root.note !== null
                    Layout.fillWidth: true
                    spacing: 6

                    Label { visible: root.type === "note"; text: "Text" }
                    TextArea {
                        visible: root.type === "note"
                        Layout.fillWidth: true
                        Layout.minimumHeight: 70
                        wrapMode: TextEdit.NoWrap
                        text: root.type === "note" ? root.sel.text : ""
                        onTextChanged: if (activeFocus) root.set("text", text)
                    }
                    RowLayout {
                        Label { text: "Size" + (root.note && root.mixed(root.note.size) ? " (mixed)" : "") }
                        SpinBox {
                            from: 6; to: 96; editable: true
                            value: root.note && !root.mixed(root.note.size) ? root.note.size : 14
                            onValueModified: root.setFor("note", "size", value)
                        }
                        CheckBox {
                            text: "Bold" + (root.note && root.mixed(root.note.bold) ? " (mixed)" : "")
                            checked: root.note !== null && root.note.bold === true
                            onToggled: root.setFor("note", "bold", checked)
                        }
                    }
                    RowLayout {
                        visible: root.type === "note"
                        Layout.fillWidth: true
                        Label {
                            Layout.fillWidth: true
                            wrapMode: Text.WordWrap
                            color: root.tc.muted2
                            text: root.type === "note" && root.sel.boxed
                                  ? "Fixed box: the text wraps to it. Drag the corner handle to resize."
                                  : "Drag the corner handle to give it a fixed box."
                        }
                        Button {
                            visible: root.type === "note" && root.sel.boxed === true
                            text: "Fit to text"
                            onClicked: root.setFor("note", "box", "fit")
                        }
                    }
                    Label { text: "Color" + (root.note && root.mixed(root.note.color) ? " (mixed)" : "") }
                    ColorPicker {
                        tc: root.tc
                        Layout.fillWidth: true
                        color: root.note && !root.mixed(root.note.color) ? root.note.color : ""
                        swatches: root.linkColors
                        onPicked: (c) => root.setFor("note", "color", c)
                    }
                }

                // point
                ColumnLayout {
                    visible: root.type === "point"
                    Layout.fillWidth: true
                    spacing: 6
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: root.tc.muted2
                        text: root.type === "point"
                              ? "Lines end here as if attached to nothing; the point itself is not exported (its label is). " +
                                root.plural(root.sel.links, "line", "lines") + " attached.\n" +
                                "Drag to move it; drop it on a device border to attach its lines there. " +
                                "Shift+drag from it to draw another line, or drop a line onto it. " +
                                "Deleting a bend point joins its two lines back into one."
                              : ""
                    }
                    Label { text: "Label" }
                    TextField {
                        Layout.fillWidth: true
                        placeholderText: "e.g. Internet, to branch office"
                        text: root.type === "point" ? root.sel.label : ""
                        onTextEdited: root.set("label", text)
                    }
                    GridLayout {
                        columns: 2
                        Layout.fillWidth: true
                        Label { text: "X" }
                        Label { text: "Y" }
                        SpinBox {
                            Layout.fillWidth: true
                            from: -100000; to: 100000; stepSize: 10; editable: true
                            value: root.type === "point" ? Math.round(root.sel.x) : 0
                            onValueModified: root.set("x", value)
                        }
                        SpinBox {
                            Layout.fillWidth: true
                            from: -100000; to: 100000; stepSize: 10; editable: true
                            value: root.type === "point" ? Math.round(root.sel.y) : 0
                            onValueModified: root.set("y", value)
                        }
                    }
                }

                // port
                ColumnLayout {
                    visible: root.type === "port"
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        text: root.type === "port" ? "on " + root.sel.device + ", " + root.sel.links + " link(s)" : ""
                        color: root.tc.muted2
                    }
                    Label { text: "Name (e.g. Gi1/0/1)" }
                    TextField {
                        id: portName
                        Layout.fillWidth: true
                        text: root.type === "port" ? root.sel.name : ""
                        onTextEdited: root.set("name", text)
                    }
                    Label { text: "Side" }
                    ComboBox {
                        Layout.fillWidth: true
                        model: root.sides
                        currentIndex: root.type === "port" ? root.sides.indexOf(root.sel.side) : 0
                        onActivated: (i) => root.set("side", root.sides[i])
                    }
                    Label { text: "Position along side (% from " + (root.sel.side === "left" || root.sel.side === "right" ? "top" : "left") + ")" }
                    SpinBox {
                        Layout.fillWidth: true
                        from: 5; to: 95; stepSize: 5; editable: true
                        value: root.type === "port" ? root.sel.pos : 50
                        onValueModified: root.set("pos", value)
                    }
                    // A layer of its own, to show or hide it apart from its device.
                    Label { text: "Layer" }
                    ComboBox {
                        Layout.fillWidth: true
                        readonly property var deviceLayer: root.type === "port" ? root.itemLayers.find(l => l.id === root.sel.device_layer) : null
                        model: ["as its device (" + (deviceLayer ? deviceLayer.name : "") + ")"]
                               .concat(root.itemLayers.map(l => l.path + (l.shown ? "" : " (hidden)") + (l.held ? " (locked)" : "")))
                        currentIndex: root.type !== "port" || !root.sel.port_layer ? 0
                                      : 1 + root.itemLayers.findIndex(l => l.id === root.sel.port_layer)
                        onActivated: (i) => root.set("layer", i === 0 ? "" : root.itemLayers[i - 1].id)
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: "Hide the layer to hide the port and its name (its lines stay)"
                    }
                }

                // link
                Label {
                    visible: root.multi && root.link !== null
                    text: root.multi ? root.plural(root.sel.links, "Link", "Links") : ""
                    font.bold: true
                    Layout.topMargin: 4
                }
                ColumnLayout {
                    visible: root.link !== null
                    Layout.fillWidth: true
                    spacing: 6

                    Label {
                        visible: root.type === "link"
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: root.tc.muted2
                        text: root.type === "link" ? root.sel.from + "  →  " + root.sel.to : ""
                    }
                    Label { text: "Type" }
                    ComboBox {
                        Layout.fillWidth: true
                        model: root.linkKindNames
                        currentIndex: root.link ? root.linkKinds.indexOf(root.link.kind) : 0
                        displayText: currentIndex < 0 ? "(mixed)" : currentText
                        onActivated: (i) => root.setFor("link", "kind", root.linkKinds[i])
                    }
                    // Encapsulation: what the line is carried over, innermost
                    // first (VXLAN over IPsec over MPLS).
                    ColumnLayout {
                        visible: root.type === "link"
                        Layout.fillWidth: true
                        spacing: 4
                        readonly property var over: root.type === "link" && root.sel.over ? root.sel.over : []
                        RowLayout {
                            Layout.fillWidth: true
                            Label {
                                Layout.fillWidth: true
                                wrapMode: Text.WordWrap
                                text: parent.parent.over.length > 0 ? "Carried over: <b>" + root.sel.tag + "</b>"
                                                                   : "Carried over: nothing"
                            }
                            Button {
                                visible: parent.parent.over.length > 0
                                text: "Remove outer"
                                onClicked: root.set("over", parent.parent.over.slice(0, -1).join(","))
                                ToolTip.visible: hovered
                                ToolTip.delay: 500
                                ToolTip.text: "Take away the outermost carrier"
                            }
                        }
                        ComboBox {
                            Layout.fillWidth: true
                            readonly property var carriers: root.linkKinds.filter(k => k !== "plain")
                            model: ["Add a carrier around it…"].concat(carriers.map(k => root.linkKindNames[root.linkKinds.indexOf(k)]))
                            currentIndex: 0
                            onActivated: (i) => {
                                if (i > 0) root.set("over", parent.over.concat([carriers[i - 1]]).join(","))
                                currentIndex = 0
                            }
                            ToolTip.visible: hovered
                            ToolTip.delay: 500
                            ToolTip.text: "Show the line encapsulated: e.g. a VXLAN line carried over IPsec. Each carrier is a band around what it carries."
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Label { text: "Label" + (root.link && root.mixed(root.link.label) ? " (mixed)" : ""); Layout.fillWidth: true }
                        Button {
                            visible: root.type === "link" && root.sel.label_placed === true
                            text: "Auto position"
                            onClicked: root.setFor("link", "label_at", "auto")
                            ToolTip.visible: hovered
                            ToolTip.delay: 500
                            ToolTip.text: "Let the label be placed automatically again (it was dragged along the line)"
                        }
                    }
                    TextField {
                        id: linkLabel
                        Layout.fillWidth: true
                        placeholderText: root.multi ? "Same label for all" : "e.g. 10G, VLAN 20"
                        text: root.link && !root.mixed(root.link.label) ? root.link.label : ""
                        onTextEdited: root.setFor("link", "label", text)
                    }
                    CheckBox {
                        text: "Show label" + (root.link && root.mixed(root.link.label_hidden) ? " (mixed)" : "")
                        checked: root.link !== null && root.link.label_hidden === false
                        onToggled: root.setFor("link", "label_hidden", !checked)
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: "The label and the type's tag, on screen and in exports (the text is kept)"
                    }

                    GridLayout {
                        columns: 2
                        Layout.fillWidth: true
                        columnSpacing: 6
                        Label { text: "Start" }
                        Label { text: "End" }
                        ComboBox {
                            Layout.fillWidth: true
                            model: root.heads
                            currentIndex: root.link ? root.heads.indexOf(root.link.head_start) : 0
                            displayText: currentIndex < 0 ? "(mixed)" : currentText
                            onActivated: (i) => root.setFor("link", "head_start", root.heads[i])
                        }
                        ComboBox {
                            Layout.fillWidth: true
                            model: root.heads
                            currentIndex: root.link ? root.heads.indexOf(root.link.head_end) : 0
                            displayText: currentIndex < 0 ? "(mixed)" : currentText
                            onActivated: (i) => root.setFor("link", "head_end", root.heads[i])
                        }
                        Label { text: "Line" }
                        Label { text: "Routing" }
                        ComboBox {
                            Layout.fillWidth: true
                            model: root.styles
                            currentIndex: root.link ? root.styles.indexOf(root.link.style) : 0
                            displayText: currentIndex < 0 ? "(mixed)" : currentText
                            onActivated: (i) => root.setFor("link", "style", root.styles[i])
                        }
                        ComboBox {
                            Layout.fillWidth: true
                            model: root.routingNames
                            currentIndex: root.link ? root.routings.indexOf(root.link.routing) : 0
                            displayText: currentIndex < 0 ? "(mixed)" : currentText
                            onActivated: (i) => root.setFor("link", "routing", root.routings[i])
                        }
                    }

                    RowLayout {
                        Label { text: "Width" + (root.link && root.mixed(root.link.width) ? " (mixed)" : "") }
                        SpinBox {
                            from: 1; to: 10
                            value: root.link && !root.mixed(root.link.width) ? root.link.width : 2
                            onValueModified: root.setFor("link", "width", value)
                        }
                    }
                    RowLayout {
                        Label {
                            text: "Label margin" + (root.link && root.mixed(root.link.label_margin) ? " (mixed)" : "")
                            ToolTip.visible: marginHover.hovered
                            ToolTip.delay: 500
                            ToolTip.text: "Free line auto layout keeps on each side of the label, so it does not crowd the devices"
                            HoverHandler { id: marginHover }
                        }
                        SpinBox {
                            from: 0; to: 200; stepSize: 4
                            editable: true
                            value: root.link && !root.mixed(root.link.label_margin) ? root.link.label_margin : 24
                            onValueModified: root.setFor("link", "label_margin", value)
                        }
                    }

                    Label { text: "Color" + (root.link && root.mixed(root.link.color) ? " (mixed)" : "") }
                    ColorPicker {
                        tc: root.tc
                        Layout.fillWidth: true
                        color: root.link && !root.mixed(root.link.color) ? root.link.color : ""
                        swatches: root.linkColors
                        onPicked: (c) => root.setFor("link", "color", c)
                    }

                    Button {
                        visible: root.type === "link"
                        Layout.fillWidth: true
                        text: "⇄ Reverse direction"
                        onClicked: root.ctl.reverseLink()
                    }
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: root.tc.faint
                        font.pixelSize: 11
                        text: "New links use the style you set last."
                    }
                }

                // layer and stacking
                ColumnLayout {
                    visible: root.arrangeable
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    spacing: 6

                    Label { text: "Layer" }
                    ComboBox {
                        Layout.fillWidth: true
                        model: root.itemLayers.map(l => l.path + (l.shown ? "" : " (hidden)") + (l.held ? " (locked)" : ""))
                        currentIndex: root.itemLayers.findIndex(l => l.id === root.sel.layer)
                        displayText: currentIndex < 0 ? "(several)" : currentText
                        onActivated: (i) => root.set("layer", root.itemLayers[i].id)
                    }
                    Label { text: "Arrange (front to back, across all layers)"; wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    GridLayout {
                        columns: 2
                        Layout.fillWidth: true
                        columnSpacing: 4
                        rowSpacing: 4
                        Repeater {
                            model: [["front", "⤒ To front", "Home"], ["forward", "↑ Forward", "Page Up or Ctrl+]"],
                                    ["back", "⤓ To back", "End"], ["backward", "↓ Backward", "Page Down or Ctrl+["]]
                            Button {
                                required property var modelData
                                Layout.fillWidth: true
                                text: modelData[1]
                                ToolTip.visible: hovered
                                ToolTip.delay: 500
                                ToolTip.text: modelData[2]
                                onClicked: root.ctl.arrange(modelData[0])
                            }
                        }
                    }
                }

                // actions
                RowLayout {
                    visible: root.type !== "none"
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    Button {
                        visible: ["device", "note"].includes(root.type) || (root.multi && root.sel.devices + root.sel.notes > 0)
                        Layout.fillWidth: true
                        text: "Duplicate"
                        onClicked: root.ctl.duplicate()
                    }
                    Button {
                        Layout.fillWidth: true
                        text: "Delete"
                        onClicked: root.ctl.deleteSelected()
                    }
                }
            }
        }

        // layers
        Rectangle { Layout.fillWidth: true; Layout.topMargin: 8; height: 1; color: root.tc.line }
        RowLayout {
            Layout.fillWidth: true
            Layout.topMargin: 6
            Label { text: "Layers"; font.bold: true; Layout.fillWidth: true }
            ToolButton {
                text: "+ Layer"
                onClicked: { root.keepLayerScroll(); root.ctl.addLayer() }
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "Add a layer; new items go into it"
            }
            ToolButton {
                text: "+ Sub-layer"
                enabled: root.layers.some(l => l.active && !l.frame)
                onClicked: { root.keepLayerScroll(); root.ctl.addSublayer() }
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "Add a layer under the highlighted one; new items go into it"
            }
            ToolButton {
                text: "Remove"
                enabled: root.layers.length > 1 && !root.layers.some(l => l.active && l.held)
                onClicked: {
                    const l = root.layers.find(l => l.active)
                    if (!l) return
                    root.keepLayerScroll()
                    // Asked first only when there is something to lose.
                    if (root.removeQuestion(l) === "") root.ctl.removeLayer(l.id)
                    else { removeDlg.layer = l; removeDlg.open() }
                }
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "Remove the active layer and everything on it (and the layers under it)"
            }
        }
        ListView {
            id: layerList
            Layout.fillWidth: true
            Layout.preferredHeight: Math.min(contentHeight, 6 * 30)
            clip: true
            // By count: changes to the layers (shown, locked, active...)
            // update the rows in place, so the list stays where it is
            // scrolled to; when rows come or go, it is put back there.
            model: root.layerRows.length
            onCountChanged: if (root.layerScroll >= 0) {
                const y = root.layerScroll
                root.layerScroll = -1
                Qt.callLater(() => layerList.contentY = Math.max(0, Math.min(y, layerList.contentHeight - layerList.height)))
            }
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            // Dragging a layer near the list's top or bottom scrolls it.
            Timer {
                interval: 40
                repeat: true
                running: root.dragLayer !== 0 && root.dragViewY >= 0
                         && (root.dragViewY < 20 || root.dragViewY > layerList.height - 20)
                onTriggered: {
                    const step = root.dragViewY < 20 ? -8 : 8
                    layerList.contentY = Math.max(0, Math.min(layerList.contentY + step, layerList.contentHeight - layerList.height))
                    root.dragOver(layerList.contentY + root.dragViewY)
                }
            }
            delegate: Rectangle {
                id: row
                required property int index
                readonly property var modelData: root.layerRows[index] || ({})
                property bool renaming: false
                width: ListView.view.width
                height: 30
                radius: 4
                // Where a dragged layer goes: into this one (its new parent,
                // lit up), or beside it (a line, the parent lit up lightly).
                readonly property bool dropInto: root.dragLayer !== 0 && root.dropParent === modelData.id && root.dropPlace === "into"
                readonly property bool dropParent: root.dragLayer !== 0 && root.dropParent === modelData.id && root.dropPlace !== "into"
                color: dropInto ? root.tc.drop : dropParent ? root.tc.dropParent : modelData.active ? root.tc.active : (rowMouse.containsMouse ? root.tc.alt : "transparent")
                border.color: root.tc.accent
                border.width: dropInto ? 2 : 0
                opacity: root.dragLayer === modelData.id ? 0.45 : 1
                Rectangle {
                    visible: root.dragLayer !== 0 && root.dropTarget === row.modelData.id && root.dropPlace !== "into"
                    width: parent.width
                    height: 2
                    color: root.tc.accent
                    y: root.dropPlace === "before" ? 0 : parent.height - 2
                    z: 2
                }

                MouseArea {
                    id: rowMouse
                    anchors.fill: parent
                    hoverEnabled: true
                    // Dragged up or down, the layer moves (the list does not scroll).
                    preventStealing: true
                    property real pressY: 0
                    property bool dragging: false
                    property bool dragged: false
                    onPressed: (m) => { pressY = m.y; dragging = false; dragged = false }
                    onPositionChanged: (m) => {
                        if (!pressed || row.modelData.frame) return
                        if (!dragging && Math.abs(m.y - pressY) > 6) {
                            dragging = true
                            root.dragLayer = row.modelData.id
                        }
                        if (dragging) root.dragOver(mapToItem(layerList.contentItem, m.x, m.y).y)
                    }
                    onReleased: {
                        dragged = dragging
                        if (dragging) root.dropDragged()
                        dragging = false
                    }
                    onCanceled: { dragging = false; root.dragLayer = 0; root.dropTarget = 0; root.dragViewY = -1 }
                    // The frame's layer takes no items, so it cannot be the active one.
                    onClicked: if (!dragged && !row.modelData.frame) root.ctl.setActiveLayer(row.modelData.id)
                    onDoubleClicked: {
                        nameField.text = row.modelData.name
                        row.renaming = true
                        nameField.forceActiveFocus()
                        nameField.selectAll()
                    }
                }
                RowLayout {
                    anchors.fill: parent
                    // Deep down, the steps get smaller.
                    anchors.leftMargin: (row.modelData.depth || 0) <= 8 ? 14 * (row.modelData.depth || 0)
                                        : Math.min(112 + 3 * (row.modelData.depth - 8), 180)
                    anchors.rightMargin: 8
                    spacing: 2
                    // Sub-layers fold away under their layer.
                    ToolButton {
                        Layout.preferredWidth: 18
                        padding: 0
                        enabled: row.modelData.children > 0
                        opacity: enabled ? 1 : 0
                        text: row.modelData.expanded ? "\u25be" : "\u25b8"
                        onClicked: { root.keepLayerScroll(); root.ctl.setLayerExpanded(row.modelData.id, !row.modelData.expanded) }
                        ToolTip.visible: hovered && enabled
                        ToolTip.delay: 500
                        ToolTip.text: row.modelData.expanded ? "Fold the layers under it away"
                                                             : "Show the " + row.modelData.children + " layer(s) under it"
                    }
                    CheckBox {
                        checked: !!row.modelData.visible
                        // (The row stays, so the tick follows the layer again, after undo too.)
                        onToggled: {
                            root.ctl.setLayerVisible(row.modelData.id, checked)
                            checked = Qt.binding(() => !!row.modelData.visible)
                        }
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: !checked ? "Hidden (click to show)"
                                     : row.modelData.shown ? "Shown (click to hide)" : "Hidden with the layer it is under"
                    }
                    Label {
                        visible: !row.renaming
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                        text: row.modelData.name || ""
                        font.bold: !!row.modelData.active
                        font.italic: !!row.modelData.held
                        // Hidden, by itself or with a layer it is under.
                        color: row.modelData.shown ? root.tc.text : root.tc.faint
                    }
                    TextField {
                        id: nameField
                        visible: row.renaming
                        Layout.fillWidth: true
                        text: row.modelData.name
                        function finish(save) {
                            if (!row.renaming) return
                            row.renaming = false
                            if (save) root.ctl.renameLayer(row.modelData.id, text)
                        }
                        onAccepted: finish(true)
                        onActiveFocusChanged: if (!activeFocus) finish(true)
                        Keys.onEscapePressed: finish(false)
                    }
                    // Background: a filled square in its color when the layer
                    // has one, an outline when not. Opens its settings.
                    Item {
                        visible: !row.modelData.frame
                        implicitWidth: 22
                        implicitHeight: 24
                        Rectangle {
                            anchors.centerIn: parent
                            width: 14; height: 12
                            radius: row.modelData.background && !row.modelData.background.rounded ? 0 : 3
                            color: row.modelData.background ? row.modelData.background.fill : "transparent"
                            border.width: 1.5
                            border.color: row.modelData.background ? (row.modelData.background.border === "none"
                                          ? root.tc.muted : row.modelData.background.border_color)
                                          : (bgMouse.containsMouse ? root.tc.muted : root.tc.faint)
                        }
                        MouseArea {
                            id: bgMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: { bgPopup.layerId = row.modelData.id; bgPopup.open() }
                        }
                        ToolTip.visible: bgMouse.containsMouse
                        ToolTip.delay: 500
                        ToolTip.text: row.modelData.background ? "Layer background (click to change)"
                                                               : "Click to add a background behind this layer's items"
                    }
                    // Name tag: dark when the layer's name shows on its devices.
                    Item {
                        visible: !row.modelData.frame
                        implicitWidth: 22
                        implicitHeight: 24
                        Canvas {
                            anchors.centerIn: parent
                            width: 16; height: 12
                            property bool on: row.modelData.show_name
                            property bool hot: tagMouse.containsMouse
                            onOnChanged: requestPaint()
                            onHotChanged: requestPaint()
                            onPaint: {
                                const ctx = getContext("2d")
                                ctx.reset()
                                const c = on ? root.tc.text : (hot ? root.tc.muted : root.tc.faint)
                                // A luggage tag pointing left, with its hole.
                                ctx.beginPath()
                                ctx.moveTo(1, 6); ctx.lineTo(5, 1); ctx.lineTo(15, 1)
                                ctx.lineTo(15, 11); ctx.lineTo(5, 11); ctx.closePath()
                                ctx.fillStyle = c
                                if (on) ctx.fill()
                                ctx.lineWidth = 1.5
                                ctx.strokeStyle = c
                                ctx.stroke()
                                ctx.beginPath()
                                ctx.arc(5.5, 6, 1.5, 0, 2 * Math.PI)
                                ctx.fillStyle = on ? root.tc.base : c
                                ctx.fill()
                            }
                        }
                        MouseArea {
                            id: tagMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.ctl.setLayerShowName(row.modelData.id, !row.modelData.show_name)
                        }
                        ToolTip.visible: tagMouse.containsMouse
                        ToolTip.delay: 500
                        ToolTip.text: row.modelData.show_name ? "Layer name shown on its devices (click to hide)"
                                                              : "Click to show the layer name on its devices"
                    }
                    // Padlock: closed and dark when locked, open and faint when not.
                    Item {
                        visible: !row.modelData.frame
                        implicitWidth: 24
                        implicitHeight: 24
                        Canvas {
                            id: lockIcon
                            anchors.centerIn: parent
                            width: 14; height: 16
                            property bool locked: row.modelData.locked
                            property bool hot: lockMouse.containsMouse
                            onLockedChanged: requestPaint()
                            onHotChanged: requestPaint()
                            onPaint: {
                                const ctx = getContext("2d")
                                ctx.reset()
                                const c = locked ? root.tc.text : (hot ? root.tc.muted : root.tc.faint)
                                ctx.strokeStyle = c
                                ctx.fillStyle = c
                                ctx.lineWidth = 1.8
                                ctx.beginPath()
                                // Shackle; lifted to the left when open.
                                const lift = locked ? 0 : 3
                                ctx.moveTo(3.5, 8 - lift)
                                ctx.lineTo(3.5, 5 - lift)
                                ctx.arc(7, 5 - lift, 3.5, Math.PI, 0)
                                ctx.lineTo(10.5, locked ? 8 : 5 - lift)
                                ctx.stroke()
                                ctx.fillRect(1, 8, 12, 8)
                            }
                        }
                        MouseArea {
                            id: lockMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: root.ctl.setLayerLocked(row.modelData.id, !row.modelData.locked)
                        }
                        ToolTip.visible: lockMouse.containsMouse
                        ToolTip.delay: 500
                        ToolTip.text: row.modelData.locked ? "Locked: shown but read only (click to unlock)"
                                                           : "Click to lock: shown but read only"
                    }
                    Label {
                        text: row.modelData.frame ? "frame" : row.modelData.children ? root.layerTotal(row.modelData) : row.modelData.count
                        color: root.tc.faint
                        font.pixelSize: 11
                        ToolTip.visible: row.modelData.frame && frameHint.containsMouse
                        ToolTip.delay: 500
                        ToolTip.text: "Holds only the frame; set it up with nothing selected"
                        MouseArea { id: frameHint; anchors.fill: parent; hoverEnabled: true }
                    }
                }
            }
        }
        Label {
            Layout.fillWidth: true
            wrapMode: Text.WordWrap
            color: root.tc.faint
            font.pixelSize: 11
            text: "New items go into the highlighted layer. Click to choose, double-click to rename, drag to reorder or onto another layer to put it under that one; \u25b8 shows the layers under one."
        }
    }

    // Settings of one layer's background.
    Popup {
        id: bgPopup
        property int layerId: -1
        readonly property var layer: root.layers.find(l => l.id === layerId) || null
        readonly property var bg: layer ? layer.background : null
        function set(key, value) { root.ctl.setLayerBackground(layerId, key, String(value)) }
        parent: Overlay.overlay
        x: Math.max(8, root.mapToItem(Overlay.overlay, 0, 0).x - width - 8)
        y: Math.max(8, (Overlay.overlay ? Overlay.overlay.height : 600) - height - 40)
        width: 260
        padding: 12
        modal: false
        focus: true
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutside

        ColumnLayout {
            width: parent.width
            spacing: 6
            Label { text: "Background of " + (bgPopup.layer ? bgPopup.layer.name : ""); font.bold: true }
            CheckBox {
                text: "Rectangle behind this layer's items"
                checked: bgPopup.bg !== null
                onToggled: bgPopup.set("enabled", checked)
            }
            GridLayout {
                visible: bgPopup.bg !== null
                columns: 2
                columnSpacing: 6
                Layout.fillWidth: true
                Label { text: "Margin" }
                SpinBox {
                    Layout.fillWidth: true
                    from: 0; to: 400; stepSize: 5
                    editable: true
                    value: bgPopup.bg ? bgPopup.bg.margin : 30
                    onValueModified: bgPopup.set("margin", value)
                }
                Label { text: "Opacity %" }
                SpinBox {
                    Layout.fillWidth: true
                    from: 0; to: 100; stepSize: 5
                    editable: true
                    value: bgPopup.bg ? bgPopup.bg.opacity : 50
                    onValueModified: bgPopup.set("opacity", value)
                }
                Label { text: "Corners" }
                ComboBox {
                    Layout.fillWidth: true
                    model: ["rounded", "square"]
                    currentIndex: bgPopup.bg && !bgPopup.bg.rounded ? 1 : 0
                    onActivated: (i) => bgPopup.set("rounded", i === 0)
                }
                Label { text: "Radius"; visible: bgPopup.bg !== null && bgPopup.bg.rounded }
                SpinBox {
                    visible: bgPopup.bg !== null && bgPopup.bg.rounded
                    Layout.fillWidth: true
                    from: 0; to: 100; stepSize: 2
                    editable: true
                    value: bgPopup.bg ? bgPopup.bg.radius : 14
                    onValueModified: bgPopup.set("radius", value)
                }
                Label { text: "Border" }
                ComboBox {
                    Layout.fillWidth: true
                    readonly property var styles: ["none", "solid", "dashed", "dotted"]
                    model: styles
                    currentIndex: bgPopup.bg ? Math.max(0, styles.indexOf(bgPopup.bg.border)) : 1
                    onActivated: (i) => bgPopup.set("border", styles[i])
                }
                Label { text: "Border width"; visible: bgPopup.bg !== null && bgPopup.bg.border !== "none" }
                SpinBox {
                    visible: bgPopup.bg !== null && bgPopup.bg.border !== "none"
                    Layout.fillWidth: true
                    from: 1; to: 10
                    value: bgPopup.bg ? Math.round(bgPopup.bg.border_width) : 2
                    onValueModified: bgPopup.set("border_width", value)
                }
            }
            Label { visible: bgPopup.bg !== null; text: "Fill" }
            ColorPicker {
                tc: root.tc
                visible: bgPopup.bg !== null
                Layout.fillWidth: true
                color: bgPopup.bg ? bgPopup.bg.fill : "#e0f2fe"
                swatches: ["#e0f2fe", "#dbeafe", "#dcfce7", "#fef9c3", "#ffedd5", "#fee2e2", "#f3e8ff", "#f1f5f9",
                           "#e5e7eb", "#ffffff"]
                onPicked: (c) => bgPopup.set("fill", c)
            }
            Label { visible: bgPopup.bg !== null && bgPopup.bg.border !== "none"; text: "Border color" }
            ColorPicker {
                tc: root.tc
                visible: bgPopup.bg !== null && bgPopup.bg.border !== "none"
                Layout.fillWidth: true
                color: bgPopup.bg ? bgPopup.bg.border_color : "#7dd3fc"
                swatches: ["#7dd3fc", "#2563eb", "#16a34a", "#ca8a04", "#ea580c", "#dc2626", "#9333ea", "#64748b",
                           "#334155", "#000000"]
                onPicked: (c) => bgPopup.set("border_color", c)
            }
        }
    }

    Dialog {
        id: removeDlg
        property var layer: null
        anchors.centerIn: Overlay.overlay
        modal: true
        title: "Remove layer"
        standardButtons: Dialog.Ok | Dialog.Cancel
        onOpened: dlgText.forceActiveFocus()
        Label {
            id: dlgText
            // Enter confirms, Escape (handled by the dialog) cancels.
            Keys.onReturnPressed: removeDlg.accept()
            Keys.onEnterPressed: removeDlg.accept()
            text: removeDlg.layer ? root.removeQuestion(removeDlg.layer) + "\nYou can undo this with Ctrl+Z." : ""
        }
        onAccepted: root.ctl.removeLayer(layer.id)
    }
}
