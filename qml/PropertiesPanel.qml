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
    required property var translate
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
        const n = (k, one) => plural(k, one, one + "s")
        if (!under.length && !items) return ""
        const subs = under.length ? root.translate("confirm.subLayers", { layers: n(under.length, "layer") }) : ""
        const on = items ? root.translate(under.length ? "confirm.treeItems" : "confirm.layerItems", { items: n(items, "item") }) : ""
        const contents = subs && on ? root.translate("confirm.layersAndItems", { layers: subs, items: on }) : subs || on
        return root.translate("confirm.removeLayer", { name: l.name, contents })
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
        ["deviceGroup.routing", ["router", "ospf router", "bgp reflector", "switch", "load balancer"]],
        ["deviceGroup.security", ["firewall", "cloud proxy", "zscaler"]],
        ["deviceGroup.wireless", ["access point", "standalone ap", "managed ap", "wlan controller"]],
        ["deviceGroup.servers", ["server", "client"]],
        ["deviceGroup.virtualization", ["virtual switch", "distributed virtual switch"]],
        ["deviceGroup.datacenter", ["aci l3out"]],
        ["deviceGroup.cabling", ["patch panel"]],
        ["deviceGroup.cloud", ["internet", "cloud", "aws", "aws transit gateway", "aws direct connect gateway", "azure", "google cloud", "alibaba cloud", "telekom cloud",
                              "stackit"]]
    ]
    readonly property var kindItems: {
        const items = []
        for (const [group, list] of kindGroups) {
            if (group !== "") items.push({ header: true, key: "", name: root.translate(group), group: root.translate(group) })
            for (const key of list) items.push({ header: false, key: key, name: root.translate(key === "" ? "property.none" : "device." + key.replace(/ /g, "_")), group: root.translate(group) })
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
    readonly property var routingNames: ["style.straight", "style.orthogonal", "style.around"]
    readonly property var sides: ["top", "right", "bottom", "left"]
    // Frame templates by category; headers are not selectable.
    readonly property var frameGroups: [
        ["", [["none", "property.none"]]],
        ["frameGroup.basic", [["lines", "frameType.lines"], ["banner", "frameType.banner"], ["minimal", "frameType.minimal"], ["card", "frameType.card"]]],
        ["frameGroup.business", [["corporate", "frameType.corporate"], ["report", "frameType.report"], ["presentation", "frameType.presentation"],
                      ["executive", "frameType.executive"], ["proposal", "frameType.proposal"]]],
        ["frameGroup.technical", [["titleblock", "frameType.titleblock"], ["iso", "frameType.iso"], ["revision", "frameType.revision"],
                               ["blueprint", "frameType.blueprint"], ["asbuilt", "frameType.asbuilt"]]],
        ["frameGroup.governance", [["controlled", "frameType.controlled"], ["signoff", "frameType.signoff"],
                                        ["solution", "frameType.solution"], ["change", "frameType.change"],
                                        ["classification", "frameType.classification"]]],
        ["frameGroup.operations", [["site", "frameType.site"], ["location", "frameType.location"], ["incident", "frameType.incident"],
                        ["noc", "frameType.noc"]]]
    ]
    readonly property var frameItems: {
        const items = []
        for (const [group, list] of frameGroups) {
            if (group !== "") items.push({ header: true, name: root.translate(group), key: "", group: root.translate(group) })
            for (const [key, name] of list) items.push({ header: false, name: root.translate(name), key: key, group: root.translate(group) })
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
            ["title", "frame.title", ""],
            ["subtitle", "frame.subtitle", "example.subtitle"],
            ["author", t === "signoff" ? "frame.preparedBy" : "frame.author", ""],
            ["note", "frame.note", "example.note"],
            ["customer", by({ proposal: "frame.preparedFor" }), "frame.customer"],
            ["company", by({ corporate: "frame.company", proposal: "frame.company", presentation: "frame.company" }), ""],
            ["address", by({ site: "frame.address" }), ""],
            ["location", by({ location: "frame.location" }), "example.locationPath"],
            ["doc_id", by({ iso: "frame.drawingNo", change: "frame.changeId", site: "frame.siteCode", controlled: "frame.documentId",
                            incident: "frame.incidentId", location: "frame.locationCode" }),
             by({ iso: "example.drawingNo", change: "example.changeId", site: "example.siteCode", controlled: "example.documentId",
                  incident: "example.incidentId", location: "example.locationCode" }) || ""],
            ["version", by({ controlled: "frame.version", solution: "frame.version" }), "example.version"],
            ["revision", by({ iso: "frame.revision" }), ""],
            ["owner", by({ controlled: "frame.owner", solution: "frame.owner" }), ""],
            ["review_date", by({ controlled: "frame.nextReview" }), "example.reviewDate"],
            ["reviewer", by({ signoff: "frame.reviewedBy", solution: "frame.reviewers" }), ""],
            ["approver", by({ iso: "frame.checkedBy", change: "frame.approvedBy", signoff: "frame.approvedBy", asbuilt: "frame.acceptedBy" }), ""],
            ["installer", by({ asbuilt: "frame.installedBy" }), ""],
            ["commissioned", by({ asbuilt: "frame.commissioned" }), "example.commissioned"],
            ["start", by({ incident: "frame.startTime" }), "example.start"],
            ["end", by({ incident: "frame.endTime" }), "example.ongoing"],
            ["services", by({ incident: "frame.affected" }), "example.services"],
            ["page", t === "iso" ? "frame.sheet" : "frame.page", "example.page"]
        ]
        return all.filter(f => f[1]).map(f => ({ key: f[0], label: root.translate(f[1]), hint: root.translate(f[2]) }))
    }
    // Status choices for the templates with a status badge.
    readonly property var statusChoices: ({
        change: ["draft", "approved", "implemented", "rejected"],
        solution: ["draft", "in review", "final"],
        noc: ["operational", "degraded", "outage", "maintenance"]
    })[frameTemplate] || null
    function setFrame(key, value) { ctl.setFrame(key, String(value)) }
    readonly property var linkKinds: ["plain", "trunk", "ipsec", "vxlan", "sdwan", "mpls", "l2", "l3", "l3sub", "capwap"]
    readonly property var linkKindNames: ["linkKind.plain", "linkKind.trunk", "linkKind.ipsec", "linkKind.vxlan", "term.sdwan", "term.mpls",
                                         "linkKind.l2", "linkKind.l3", "linkKind.l3sub",
                                         "linkKind.capwap"]

    function set(key, value) { ctl.setProp(key, String(value)) }
    // Set a property on every selected item of one kind ("device", "note", "link").
    function setFor(kind, key, value) { ctl.setProp(kind + ":" + key, String(value)) }
    function mixed(v) { return v === null || v === undefined }
    function plural(n, one, many) { return root.translate("count." + (n === 1 ? one : many), { count: n }) }

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
                          : root.translate(({ none: "selection.empty", device: "selection.device", note: "selection.text", port: "selection.port", link: "selection.link",
                               point: "selection.point" })[root.type])
                    font.bold: true
                    font.pixelSize: 15
                }

                // frame
                ColumnLayout {
                    visible: root.type === "none"
                    Layout.fillWidth: true
                    spacing: 6
                    Label { text: root.translate("frame.label"); font.bold: true }
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
                        ToolTip.text: root.translate("frame.hint")
                    }
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WrapAnywhere
                        color: root.tc.faint
                        font.pixelSize: 11
                        text: root.translate("desktop.frameDefaults",
                                             { template: root.frameTemplate === "none" ? "<template>" : root.frameTemplate, path: root.ctl.configPath() })
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
                        Label { text: root.translate("frame.status"); Layout.preferredWidth: 88 }
                        ComboBox {
                            Layout.fillWidth: true
                            editable: true
                            readonly property var codes: root.statusChoices || []
                            model: codes.map(code => root.translate("badge." + code.replace(" ", "_")))
                            editText: root.frame ? (model[codes.indexOf(root.frame.status)] ?? root.frame.status) : ""
                            onActivated: (index) => root.setFrame("status", codes[index])
                            onAccepted: root.setFrame("status", codes[model.indexOf(editText)] ?? editText)
                        }
                    }
                    RowLayout {
                        visible: root.frameUses("classification", "controlled")
                        Layout.fillWidth: true
                        Label { text: root.translate("frame.classification"); Layout.preferredWidth: 88; elide: Text.ElideRight }
                        ComboBox {
                            Layout.fillWidth: true
                            readonly property var levels: ["public", "internal", "confidential", "restricted"]
                            model: levels.map(label => root.translate("classification." + label))
                            currentIndex: Math.max(0, levels.indexOf(root.frame && root.frame.level ? root.frame.level : "internal"))
                            onActivated: (i) => root.setFrame("level", levels[i])
                        }
                    }
                    RowLayout {
                        visible: root.frameUses("incident")
                        Layout.fillWidth: true
                        Label { text: root.translate("frame.severity"); Layout.preferredWidth: 88 }
                        ComboBox {
                            Layout.fillWidth: true
                            readonly property var levels: ["P1", "P2", "P3", "P4"]
                            model: [root.translate("severity.p1"), root.translate("severity.p2"), root.translate("severity.p3"), root.translate("severity.p4")]
                            currentIndex: Math.max(0, levels.indexOf(root.frame && root.frame.severity ? root.frame.severity.toUpperCase() : "P3"))
                            onActivated: (i) => root.setFrame("severity", levels[i])
                        }
                    }
                    ColumnLayout {
                        visible: root.frameUses("executive")
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: root.translate("frame.summary") }
                        TextArea {
                            Layout.fillWidth: true
                            Layout.minimumHeight: 70
                            wrapMode: TextEdit.Wrap
                            placeholderText: root.translate("frame.summaryHint")
                            text: root.frame ? root.frame.summary : ""
                            onTextChanged: if (activeFocus) root.setFrame("summary", text)
                            background: Rectangle { border.color: root.tc.border; radius: 3 }
                        }
                    }
                    RowLayout {
                        visible: root.frame !== null
                        Layout.fillWidth: true
                        Label { text: root.translate("frame.watermark"); Layout.preferredWidth: 88 }
                        ComboBox {
                            Layout.fillWidth: true
                            editable: true
                            model: [""].concat(["draft", "confidential", "sample", "for_review", "superseded"].map(key => root.translate("watermark." + key)))
                            editText: root.frame ? root.frame.watermark : ""
                            onActivated: root.setFrame("watermark", currentText)
                            onAccepted: root.setFrame("watermark", editText)
                        }
                    }
                    RowLayout {
                        visible: root.frame !== null
                        Layout.fillWidth: true
                        Label { text: root.translate("frame.margin"); Layout.preferredWidth: 88 }
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
                        Label { text: root.translate("frame.revisionsFormat") }
                        TextArea {
                            Layout.fillWidth: true
                            Layout.minimumHeight: 60
                            wrapMode: TextEdit.NoWrap
                            placeholderText: root.translate("example.revision")
                            text: root.frame ? root.frame.revisions : ""
                            onTextChanged: if (activeFocus) root.setFrame("revisions", text)
                            background: Rectangle { border.color: root.tc.border; radius: 3 }
                        }
                    }
                    CheckBox {
                        visible: root.frame !== null
                        text: root.translate("frame.showDate")
                        checked: root.frame ? root.frame.show_date : true
                        onToggled: root.setFrame("show_date", checked)
                    }
                    CheckBox {
                        visible: root.frame !== null
                        text: root.translate("frame.showFilename")
                        checked: root.frame ? root.frame.show_file : false
                        onToggled: root.setFrame("show_file", checked)
                    }
                    Repeater {
                        model: root.frame ? [["logo", root.translate("frame.logo")], ["logo2", root.frameUses("proposal") ? root.translate("frame.customerLogo") : root.translate("frame.secondLogo")]] : []
                        RowLayout {
                            required property var modelData
                            Layout.fillWidth: true
                            Label { text: modelData[1]; Layout.fillWidth: true }
                            Button {
                                text: root.frame && root.frame[modelData[0]] ? root.translate("frame.changeLogo") : root.translate("frame.chooseLogo")
                                onClicked: { logoDialog.key = modelData[0]; logoDialog.open() }
                            }
                            Button {
                                visible: root.frame !== null && root.frame[modelData[0]]
                                text: root.translate("action.remove")
                                onClicked: root.setFrame(modelData[0], "")
                            }
                        }
                    }
                    ColumnLayout {
                        visible: root.frame !== null
                        Layout.fillWidth: true
                        spacing: 2
                        Label {
                            text: root.translate("frame.saveDefaults")
                            ToolTip.visible: saveHover.hovered
                            ToolTip.delay: 500
                            ToolTip.text: root.translate("desktop.saveDefaultsHint", { path: root.ctl.configPath() })
                            HoverHandler { id: saveHover }
                        }
                        RowLayout {
                            Layout.fillWidth: true
                            Button {
                                Layout.fillWidth: true
                                text: root.translate("frame.allTemplates")
                                onClicked: root.ctl.saveFrameDefaults(true)
                            }
                            Button {
                                Layout.fillWidth: true
                                text: root.translate("desktop.templateOnly", { template: (root.frameItems.find(i => i.key === root.frameTemplate) || {}).name || "" })
                                onClicked: root.ctl.saveFrameDefaults(false)
                            }
                        }
                    }
                    FileDialog {
                        id: logoDialog
                        property string key: "logo"
                        title: root.translate("frame.chooseLogoDialog")
                        nameFilters: [root.translate("filter.image")]
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
                    text: [
                        "help.addDevice",
                        "help.connect",
                        "help.addPort",
                        "help.movePort",
                        "help.edit",
                        "help.quickAdd",
                        "desktopHelp.addText",
                        "help.insertDevice",
                        "help.bend",
                        "desktopHelp.freePoint",
                        "desktopHelp.select",
                        "desktopHelp.align",
                        "desktopHelp.layers",
                        "desktopHelp.arrange",
                        "desktopHelp.deepZoom",
                        "desktopHelp.layout",
                        "help.contextMenu",
                        "help.pan",
                        "desktopHelp.clipboard",
                        "help.keys"
                    ].map(paragraph => root.translate(paragraph)).join("<br><br>")
                }

                // device
                Label {
                    visible: root.multi
                    text: root.translate("selection.multipleHint")
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

                    Label { text: root.translate("property.name") }
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
                    Label { text: root.translate("property.renamePattern") }
                    RowLayout {
                        Layout.fillWidth: true
                        TextField {
                            id: renamePattern
                            Layout.fillWidth: true
                            placeholderText: root.translate("property.renameExample")
                            onAccepted: rename()
                            function rename() { if (text.trim() !== "") { root.set("name_pattern", text); text = "" } }
                        }
                        Button { text: root.translate("property.rename"); enabled: renamePattern.text.trim() !== ""; onClicked: renamePattern.rename() }
                    }
                }

                ColumnLayout {
                    visible: root.dev !== null
                    Layout.fillWidth: true
                    spacing: 6

                    Label { text: root.translate("property.type") + (root.dev && root.mixed(root.dev.kind) ? root.translate("value.mixedSuffix") : "") }
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

                    Label { text: root.translate("property.fillColor") + (root.dev && root.mixed(root.dev.color) ? root.translate("value.mixedSuffix") : "") }
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
                    Label { text: root.translate("property.ports") }
                    Label { text: root.translate("property.connector") }
                    SpinBox {
                        Layout.fillWidth: true
                        from: 1; to: 288; editable: true
                        value: root.type === "device" && root.sel.panel_ports ? root.sel.panel_ports : 24
                        onValueModified: root.set("panel_ports", value)
                    }
                    ComboBox {
                        Layout.fillWidth: true
                        readonly property var keys: ["rj45", "lc", "sc"]
                        model: [root.translate("term.rj45"), root.translate("property.lcDuplex"), root.translate("term.sc")]
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
                    Label { text: root.translate("property.portsOrBox") }
                    Label { text: root.translate("property.connector") }
                    SpinBox {
                        Layout.fillWidth: true
                        from: 0; to: 288; editable: true
                        value: root.type === "device" && root.sel.panel_ports ? root.sel.panel_ports : 0
                        onValueModified: root.set("panel_ports", value)
                    }
                    ComboBox {
                        Layout.fillWidth: true
                        readonly property var keys: ["rj45", "sfp"]
                        model: [root.translate("term.rj45"), root.translate("term.sfp")]
                        currentIndex: Math.max(0, keys.indexOf(root.type === "device" ? root.sel.connector : "rj45"))
                        onActivated: (i) => root.set("connector", keys[i])
                    }
                    Label {
                        visible: root.type === "device" && root.sel.panel_ports > 0
                        text: root.translate("property.uplinks")
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
                    Label { text: root.translate("property.width") + (root.dev && root.mixed(root.dev.w) ? root.translate("value.mixedSuffix") : "") }
                    Label { text: root.translate("property.height") + (root.dev && root.mixed(root.dev.h) ? root.translate("value.mixedSuffix") : "") }
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
                    Label { text: root.type === "device" ? root.translate("property.addPortCount", { count: root.sel.ports }) : root.translate("property.addPort") }
                    RowLayout {
                        Layout.fillWidth: true
                        spacing: 4
                        Repeater {
                            model: [["top", root.translate("property.topButton")], ["right", root.translate("property.rightButton")], ["bottom", root.translate("property.bottomButton")], ["left", root.translate("property.leftButton")]]
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

                    Label { visible: root.type === "note"; text: root.translate("selection.text") }
                    TextArea {
                        visible: root.type === "note"
                        Layout.fillWidth: true
                        Layout.minimumHeight: 70
                        wrapMode: TextEdit.NoWrap
                        text: root.type === "note" ? root.sel.text : ""
                        onTextChanged: if (activeFocus) root.set("text", text)
                    }
                    RowLayout {
                        Label { text: root.translate("property.size") + (root.note && root.mixed(root.note.size) ? root.translate("value.mixedSuffix") : "") }
                        SpinBox {
                            from: 6; to: 96; editable: true
                            value: root.note && !root.mixed(root.note.size) ? root.note.size : 14
                            onValueModified: root.setFor("note", "size", value)
                        }
                        CheckBox {
                            text: root.translate("property.bold") + (root.note && root.mixed(root.note.bold) ? root.translate("value.mixedSuffix") : "")
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
                                  ? root.translate("property.fixedBoxHint")
                                  : root.translate("property.resizeBoxHint")
                        }
                        Button {
                            visible: root.type === "note" && root.sel.boxed === true
                            text: root.translate("property.fitText")
                            onClicked: root.setFor("note", "box", "fit")
                        }
                    }
                    Label { text: root.translate("property.color") + (root.note && root.mixed(root.note.color) ? root.translate("value.mixedSuffix") : "") }
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
                              ? root.translate("desktop.pointHelp",
                                               { lines: root.plural(root.sel.links, "line", "lines") })
                              : ""
                    }
                    Label { text: root.translate("property.label") }
                    TextField {
                        Layout.fillWidth: true
                        placeholderText: root.translate("value.endpointExample")
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
                        text: root.type === "port" ? root.translate("property.portConnections", { device: root.sel.device, count: root.sel.links }) : ""
                        color: root.tc.muted2
                    }
                    Label { text: root.translate("property.portName") }
                    TextField {
                        id: portName
                        Layout.fillWidth: true
                        text: root.type === "port" ? root.sel.name : ""
                        onTextEdited: root.set("name", text)
                    }
                    Label { text: root.translate("property.side") }
                    ComboBox {
                        Layout.fillWidth: true
                        model: root.sides.map(label => root.translate("side." + label))
                        currentIndex: root.type === "port" ? root.sides.indexOf(root.sel.side) : 0
                        onActivated: (i) => root.set("side", root.sides[i])
                    }
                    Label { text: root.translate("property.sidePosition", { side: root.translate(root.sel.side === "left" || root.sel.side === "right" ? "side.top" : "side.left") }) }
                    SpinBox {
                        Layout.fillWidth: true
                        from: 5; to: 95; stepSize: 5; editable: true
                        value: root.type === "port" ? root.sel.pos : 50
                        onValueModified: root.set("pos", value)
                    }
                    // A layer of its own, to show or hide it apart from its device.
                    Label { text: root.translate("layer.label") }
                    ComboBox {
                        Layout.fillWidth: true
                        readonly property var deviceLayer: root.type === "port" ? root.itemLayers.find(l => l.id === root.sel.device_layer) : null
                        model: [root.translate("property.deviceLayer", { name: deviceLayer ? deviceLayer.name : "" })]
                               .concat(root.itemLayers.map(l => l.path + (l.shown ? "" : " (hidden)") + (l.held ? " (locked)" : "")))
                        currentIndex: root.type !== "port" || !root.sel.port_layer ? 0
                                      : 1 + root.itemLayers.findIndex(l => l.id === root.sel.port_layer)
                        onActivated: (i) => root.set("layer", i === 0 ? "" : root.itemLayers[i - 1].id)
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: root.translate("layer.portVisibilityHint")
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
                    Label { text: root.translate("property.type") }
                    ComboBox {
                        Layout.fillWidth: true
                        model: root.linkKindNames.map(label => root.translate(label))
                        currentIndex: root.link ? root.linkKinds.indexOf(root.link.kind) : 0
                        displayText: currentIndex < 0 ? root.translate("value.mixedOption") : currentText
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
                                text: parent.parent.over.length > 0 ? root.translate("link.carrierPrefix") + "<b>" + root.sel.tag + "</b>"
                                                                   : root.translate("link.noCarrier")
                            }
                            Button {
                                visible: parent.parent.over.length > 0
                                text: root.translate("link.removeOuter")
                                onClicked: root.set("over", parent.parent.over.slice(0, -1).join(","))
                                ToolTip.visible: hovered
                                ToolTip.delay: 500
                                ToolTip.text: root.translate("link.removeOuterHint")
                            }
                        }
                        ComboBox {
                            Layout.fillWidth: true
                            readonly property var carriers: root.linkKinds.filter(k => k !== "plain")
                            model: [root.translate("link.addCarrier")].concat(carriers.map(k => root.translate(root.linkKindNames[root.linkKinds.indexOf(k)])))
                            currentIndex: 0
                            onActivated: (i) => {
                                if (i > 0) root.set("over", parent.over.concat([carriers[i - 1]]).join(","))
                                currentIndex = 0
                            }
                            ToolTip.visible: hovered
                            ToolTip.delay: 500
                            ToolTip.text: root.translate("desktop.carrierHint")
                        }
                    }
                    RowLayout {
                        Layout.fillWidth: true
                        Label { text: root.translate("property.label") + (root.link && root.mixed(root.link.label) ? root.translate("value.mixedSuffix") : ""); Layout.fillWidth: true }
                        Button {
                            visible: root.type === "link" && root.sel.label_placed === true
                            text: root.translate("link.autoPosition")
                            onClicked: root.setFor("link", "label_at", "auto")
                            ToolTip.visible: hovered
                            ToolTip.delay: 500
                            ToolTip.text: root.translate("desktop.autoPositionHint")
                        }
                    }
                    TextField {
                        id: linkLabel
                        Layout.fillWidth: true
                        placeholderText: root.multi ? root.translate("link.commonLabel") : root.translate("link.labelExample")
                        text: root.link && !root.mixed(root.link.label) ? root.link.label : ""
                        onTextEdited: root.setFor("link", "label", text)
                    }
                    CheckBox {
                        text: root.translate("link.showLabel") + (root.link && root.mixed(root.link.label_hidden) ? root.translate("value.mixedSuffix") : "")
                        checked: root.link !== null && root.link.label_hidden === false
                        onToggled: root.setFor("link", "label_hidden", !checked)
                        ToolTip.visible: hovered
                        ToolTip.delay: 500
                        ToolTip.text: root.translate("link.showLabelHint")
                    }

                    GridLayout {
                        columns: 2
                        Layout.fillWidth: true
                        columnSpacing: 6
                        Label { text: root.translate("link.start") }
                        Label { text: root.translate("link.end") }
                        ComboBox {
                            Layout.fillWidth: true
                            model: root.heads.map(label => root.translate(label === "none" ? "property.none" : "style." + label))
                            currentIndex: root.link ? root.heads.indexOf(root.link.head_start) : 0
                            displayText: currentIndex < 0 ? root.translate("value.mixedOption") : currentText
                            onActivated: (i) => root.setFor("link", "head_start", root.heads[i])
                        }
                        ComboBox {
                            Layout.fillWidth: true
                            model: root.heads.map(label => root.translate(label === "none" ? "property.none" : "style." + label))
                            currentIndex: root.link ? root.heads.indexOf(root.link.head_end) : 0
                            displayText: currentIndex < 0 ? root.translate("value.mixedOption") : currentText
                            onActivated: (i) => root.setFor("link", "head_end", root.heads[i])
                        }
                        Label { text: root.translate("link.line") }
                        Label { text: root.translate("link.routing") }
                        ComboBox {
                            Layout.fillWidth: true
                            model: root.styles.map(label => root.translate(label === "none" ? "property.none" : "style." + label))
                            currentIndex: root.link ? root.styles.indexOf(root.link.style) : 0
                            displayText: currentIndex < 0 ? root.translate("value.mixedOption") : currentText
                            onActivated: (i) => root.setFor("link", "style", root.styles[i])
                        }
                        ComboBox {
                            Layout.fillWidth: true
                            model: root.routingNames.map(label => root.translate(label))
                            currentIndex: root.link ? root.routings.indexOf(root.link.routing) : 0
                            displayText: currentIndex < 0 ? root.translate("value.mixedOption") : currentText
                            onActivated: (i) => root.setFor("link", "routing", root.routings[i])
                        }
                    }

                    RowLayout {
                        Label { text: root.translate("property.width") + (root.link && root.mixed(root.link.width) ? root.translate("value.mixedSuffix") : "") }
                        SpinBox {
                            from: 1; to: 10
                            value: root.link && !root.mixed(root.link.width) ? root.link.width : 2
                            onValueModified: root.setFor("link", "width", value)
                        }
                    }
                    RowLayout {
                        Label {
                            text: root.translate("link.labelMargin") + (root.link && root.mixed(root.link.label_margin) ? root.translate("value.mixedSuffix") : "")
                            ToolTip.visible: marginHover.hovered
                            ToolTip.delay: 500
                            ToolTip.text: root.translate("desktop.labelMarginHint")
                            HoverHandler { id: marginHover }
                        }
                        SpinBox {
                            from: 0; to: 200; stepSize: 4
                            editable: true
                            value: root.link && !root.mixed(root.link.label_margin) ? root.link.label_margin : 24
                            onValueModified: root.setFor("link", "label_margin", value)
                        }
                    }

                    Label { text: root.translate("property.color") + (root.link && root.mixed(root.link.color) ? root.translate("value.mixedSuffix") : "") }
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
                        text: root.translate("link.reverse")
                        onClicked: root.ctl.reverseLink()
                    }
                    Label {
                        Layout.fillWidth: true
                        wrapMode: Text.WordWrap
                        color: root.tc.faint
                        font.pixelSize: 11
                        text: root.translate("link.styleHint")
                    }
                }

                // layer and stacking
                ColumnLayout {
                    visible: root.arrangeable
                    Layout.fillWidth: true
                    Layout.topMargin: 8
                    spacing: 6

                    Label { text: root.translate("layer.label") }
                    ComboBox {
                        Layout.fillWidth: true
                        model: root.itemLayers.map(l => l.path + (l.shown ? "" : root.translate("value.hiddenSuffix")) + (l.held ? root.translate("value.lockedSuffix") : ""))
                        currentIndex: root.itemLayers.findIndex(l => l.id === root.sel.layer)
                        displayText: currentIndex < 0 ? root.translate("filter.several") : currentText
                        onActivated: (i) => root.set("layer", root.itemLayers[i].id)
                    }
                    Label { text: root.translate("arrange.label"); wrapMode: Text.WordWrap; Layout.fillWidth: true }
                    GridLayout {
                        columns: 2
                        Layout.fillWidth: true
                        columnSpacing: 4
                        rowSpacing: 4
                        Repeater {
                            model: [["front", root.translate("arrange.front"), "Home"], ["forward", root.translate("arrange.forward"), "Page Up or Ctrl+]"],
                                    ["back", root.translate("arrange.back"), "End"], ["backward", root.translate("arrange.backward"), "Page Down or Ctrl+["]]
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
                        text: root.translate("edit.duplicate")
                        onClicked: root.ctl.duplicate()
                    }
                    Button {
                        Layout.fillWidth: true
                        text: root.translate("edit.delete")
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
            Label { text: root.translate("layer.title"); font.bold: true; Layout.fillWidth: true }
            ToolButton {
                text: root.translate("layer.add")
                onClicked: { root.keepLayerScroll(); root.ctl.addLayer() }
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: root.translate("layer.addHint")
            }
            ToolButton {
                text: root.translate("layer.addChild")
                enabled: root.layers.some(l => l.active && !l.frame)
                onClicked: { root.keepLayerScroll(); root.ctl.addSublayer() }
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: root.translate("layer.addChildHint")
            }
            ToolButton {
                text: root.translate("action.remove")
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
                ToolTip.text: root.translate("layer.removeHint")
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
                        ToolTip.text: row.modelData.expanded ? root.translate("layer.foldHint")
                                                             : root.translate("property.expandLayers", { count: row.modelData.children })
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
                        ToolTip.text: !checked ? root.translate("layer.hiddenHint")
                                     : row.modelData.shown ? root.translate("layer.shownHint") : root.translate("layer.parentHidden")
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
                        ToolTip.text: row.modelData.background ? root.translate("layer.backgroundHint")
                                                               : root.translate("layer.addBackgroundHint")
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
                        ToolTip.text: row.modelData.show_name ? root.translate("layer.shownTagHint")
                                                              : root.translate("layer.showTagHint")
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
                        ToolTip.text: row.modelData.locked ? root.translate("layer.lockedHint")
                                                           : root.translate("layer.lockHint")
                    }
                    Label {
                        text: row.modelData.frame ? root.translate("layer.frameBadge") : row.modelData.children ? root.layerTotal(row.modelData) : row.modelData.count
                        color: root.tc.faint
                        font.pixelSize: 11
                        ToolTip.visible: row.modelData.frame && frameHint.containsMouse
                        ToolTip.delay: 500
                        ToolTip.text: root.translate("layer.frameHint")
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
            text: root.translate("layer.instructions")
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
            Label { text: root.translate("property.layerBackground", { name: bgPopup.layer ? bgPopup.layer.name : "" }); font.bold: true }
            CheckBox {
                text: root.translate("background.enabled")
                checked: bgPopup.bg !== null
                onToggled: bgPopup.set("enabled", checked)
            }
            GridLayout {
                visible: bgPopup.bg !== null
                columns: 2
                columnSpacing: 6
                Layout.fillWidth: true
                Label { text: root.translate("frame.margin") }
                SpinBox {
                    Layout.fillWidth: true
                    from: 0; to: 400; stepSize: 5
                    editable: true
                    value: bgPopup.bg ? bgPopup.bg.margin : 30
                    onValueModified: bgPopup.set("margin", value)
                }
                Label { text: root.translate("background.opacity") }
                SpinBox {
                    Layout.fillWidth: true
                    from: 0; to: 100; stepSize: 5
                    editable: true
                    value: bgPopup.bg ? bgPopup.bg.opacity : 50
                    onValueModified: bgPopup.set("opacity", value)
                }
                Label { text: root.translate("background.corners") }
                ComboBox {
                    Layout.fillWidth: true
                    model: [root.translate("background.rounded"), root.translate("background.square")]
                    currentIndex: bgPopup.bg && !bgPopup.bg.rounded ? 1 : 0
                    onActivated: (i) => bgPopup.set("rounded", i === 0)
                }
                Label { text: root.translate("background.radius"); visible: bgPopup.bg !== null && bgPopup.bg.rounded }
                SpinBox {
                    visible: bgPopup.bg !== null && bgPopup.bg.rounded
                    Layout.fillWidth: true
                    from: 0; to: 100; stepSize: 2
                    editable: true
                    value: bgPopup.bg ? bgPopup.bg.radius : 14
                    onValueModified: bgPopup.set("radius", value)
                }
                Label { text: root.translate("background.border") }
                ComboBox {
                    Layout.fillWidth: true
                    readonly property var styles: ["none", "solid", "dashed", "dotted"]
                    model: styles.map(label => root.translate(label === "none" ? "property.none" : "style." + label))
                    currentIndex: bgPopup.bg ? Math.max(0, styles.indexOf(bgPopup.bg.border)) : 1
                    onActivated: (i) => bgPopup.set("border", styles[i])
                }
                Label { text: root.translate("background.borderWidth"); visible: bgPopup.bg !== null && bgPopup.bg.border !== "none" }
                SpinBox {
                    visible: bgPopup.bg !== null && bgPopup.bg.border !== "none"
                    Layout.fillWidth: true
                    from: 1; to: 10
                    value: bgPopup.bg ? Math.round(bgPopup.bg.border_width) : 2
                    onValueModified: bgPopup.set("border_width", value)
                }
            }
            Label { visible: bgPopup.bg !== null; text: root.translate("background.fill") }
            ColorPicker {
                tc: root.tc
                visible: bgPopup.bg !== null
                Layout.fillWidth: true
                color: bgPopup.bg ? bgPopup.bg.fill : "#e0f2fe"
                swatches: ["#e0f2fe", "#dbeafe", "#dcfce7", "#fef9c3", "#ffedd5", "#fee2e2", "#f3e8ff", "#f1f5f9",
                           "#e5e7eb", "#ffffff"]
                onPicked: (c) => bgPopup.set("fill", c)
            }
            Label { visible: bgPopup.bg !== null && bgPopup.bg.border !== "none"; text: root.translate("background.borderColor") }
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
        title: root.translate("dialog.removeLayer")
        standardButtons: Dialog.Ok | Dialog.Cancel
        onOpened: {
            standardButton(Dialog.Ok).text = root.translate("action.ok")
            standardButton(Dialog.Cancel).text = root.translate("action.cancel")
            dlgText.forceActiveFocus()
        }
        Label {
            id: dlgText
            // Enter confirms, Escape (handled by the dialog) cancels.
            Keys.onReturnPressed: removeDlg.accept()
            Keys.onEnterPressed: removeDlg.accept()
            text: removeDlg.layer ? root.removeQuestion(removeDlg.layer) + "\n" + root.translate("confirm.undoHint") : ""
        }
        onAccepted: root.ctl.removeLayer(layer.id)
    }
}
