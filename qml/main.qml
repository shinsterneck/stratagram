// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

import QtCore
import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import QtQuick.Dialogs
import com.stratagram

ApplicationWindow {
    readonly property var tc: uiColors
    id: win
    width: 1280
    height: 800
    visible: true
    title: ctl.title

    DiagramController { id: ctl }

    Settings {
        id: prefs
        location: StandardPaths.writableLocation(StandardPaths.ConfigLocation) + "/stratagram.conf"
        category: "ui"
        property bool panelVisible: true
        property int layoutGap: 60
        property bool layoutTidyPorts: true
        property bool layoutAvoid: true
        property bool showGrid: true
        // "light", "deep" (Dark) or "neon": the canvas, exports and the panels.
        property string theme: "light"
        onThemeChanged: ctl.setTheme(theme)
    }

    // Interface colors for the theme (handed to the panels as `tc`).
    readonly property bool darkUi: prefs.theme !== "light"
    readonly property var uiColors: darkUi ? darkColors : lightColors
    QtObject {
        id: lightColors
        property color text: "#0f172a"; property color muted: "#475569"; property color muted2: "#64748b"
        property color faint: "#94a3b8"; property color line: "#e2e8f0"; property color alt: "#f1f5f9"
        property color base: "#ffffff"; property color accent: "#2563eb"; property color border: "#cbd5e1"
        property color drop: "#bfdbfe"; property color dropParent: "#e0ecff"; property color active: "#dbeafe"
    }
    QtObject {
        id: darkColors
        property color text: "#e2e8f0"; property color muted: "#a5b4c8"; property color muted2: "#94a3b8"
        property color faint: "#64748b"; property color line: "#1e293b"; property color alt: "#0b1220"
        property color base: "#111a2b"; property color accent: "#60a5fa"; property color border: "#334155"
        property color drop: "#1e40af"; property color dropParent: "#1e3a8a"; property color active: "#1e3a5f"
    }
    // The controls (Fusion) in dark colors too.
    Palette {
        id: darkPalette
        window: "#111a2b"; windowText: "#e2e8f0"; base: "#0b1220"; alternateBase: "#131c2e"
        text: "#e2e8f0"; button: "#1a2438"; buttonText: "#e2e8f0"; brightText: "#ffffff"
        highlight: "#2563eb"; highlightedText: "#ffffff"; placeholderText: "#64748b"
        light: "#2a3650"; midlight: "#222d44"; mid: "#1a2438"; dark: "#0b1220"; shadow: "#000000"
        toolTipBase: "#1e293b"; toolTipText: "#e2e8f0"; link: "#60a5fa"
        disabled.windowText: "#475569"; disabled.text: "#475569"; disabled.buttonText: "#475569"
        // The same while the window is not focused.
        inactive.window: "#111a2b"; inactive.windowText: "#e2e8f0"; inactive.base: "#0b1220"
        inactive.alternateBase: "#131c2e"; inactive.text: "#e2e8f0"; inactive.button: "#1a2438"
        inactive.buttonText: "#e2e8f0"; inactive.highlight: "#1e40af"; inactive.highlightedText: "#ffffff"
        inactive.placeholderText: "#64748b"; inactive.light: "#2a3650"; inactive.midlight: "#222d44"
        inactive.mid: "#1a2438"; inactive.dark: "#0b1220"
    }
    // The system's own palette, read from a window that is never shown, for
    // the light theme.
    Window { id: sysProbe; visible: false; width: 1; height: 1 }
    Palette {
        id: lightPalette
        window: sysProbe.palette.window
        windowText: sysProbe.palette.windowText
        base: sysProbe.palette.base
        alternateBase: sysProbe.palette.alternateBase
        text: sysProbe.palette.text
        button: sysProbe.palette.button
        buttonText: sysProbe.palette.buttonText
        brightText: sysProbe.palette.brightText
        highlight: sysProbe.palette.highlight
        highlightedText: sysProbe.palette.highlightedText
        placeholderText: sysProbe.palette.placeholderText
        light: sysProbe.palette.light
        midlight: sysProbe.palette.midlight
        mid: sysProbe.palette.mid
        dark: sysProbe.palette.dark
        shadow: sysProbe.palette.shadow
        toolTipBase: sysProbe.palette.toolTipBase
        toolTipText: sysProbe.palette.toolTipText
        link: sysProbe.palette.link
        disabled.windowText: sysProbe.palette.disabled.windowText
        disabled.text: sysProbe.palette.disabled.text
        disabled.buttonText: sysProbe.palette.disabled.buttonText
        disabled.base: sysProbe.palette.disabled.base
        disabled.button: sysProbe.palette.disabled.button
        inactive.window: sysProbe.palette.inactive.window
        inactive.windowText: sysProbe.palette.inactive.windowText
        inactive.base: sysProbe.palette.inactive.base
        inactive.text: sysProbe.palette.inactive.text
        inactive.button: sysProbe.palette.inactive.button
        inactive.buttonText: sysProbe.palette.inactive.buttonText
        inactive.highlight: sysProbe.palette.inactive.highlight
        inactive.highlightedText: sysProbe.palette.inactive.highlightedText
    }
    palette: darkUi ? darkPalette : lightPalette

    // Action to run once unsaved changes are saved or discarded.
    property var pending: null
    property bool forceQuit: false

    function guard(fn) {
        if (ctl.dirty) { pending = fn; confirmDlg.open() } else fn()
    }
    function runPending() {
        const f = pending
        pending = null
        if (f) f()
    }
    function save() {
        if (ctl.hasFile) { if (ctl.save()) runPending() }
        else saveAs()
    }
    function saveAs() {
        // Suggest the current name as a .stratagram file (older ones were
        // .json). Native dialogs read it on opening; Qt's own may reset it
        // when opening, so set it again after.
        const name = ctl.fileFolder() + "/" + ctl.fileBaseName() + ".stratagram"
        saveDlg.currentFolder = ctl.fileFolder()
        saveDlg.selectedFile = name
        saveDlg.open()
        saveDlg.selectedFile = name
    }
    function openFile() {
        guard(() => { openDlg.currentFolder = ctl.fileFolder(); openDlg.open() })
    }

    onClosing: (close) => {
        if (ctl.dirty && !forceQuit) {
            close.accepted = false
            pending = () => { forceQuit = true; win.close() }
            confirmDlg.open()
        }
    }

    Component.onCompleted: {
        ctl.setTheme(prefs.theme)
        ctl.startup()
        Qt.callLater(canvas.fit)
    }

    // actions
    Action { id: actNew; text: "New"; shortcut: StandardKey.New; onTriggered: guard(() => { ctl.newDiagram(); forgetSheetViews(); canvas.fit() }) }
    Action { id: actOpen; text: "Open…"; shortcut: StandardKey.Open; onTriggered: openFile() }
    Action { id: actSave; text: "Save"; shortcut: StandardKey.Save; onTriggered: save() }
    Action { id: actSaveAs; text: "Save as…"; shortcut: "Ctrl+Shift+S"; onTriggered: saveAs() }
    Action {
        id: actExport; text: "Export SVG…"; shortcut: "Ctrl+E"
        onTriggered: { exportDlg.currentFolder = ctl.fileFolder(); exportDlg.open() }
    }
    Action {
        id: actExportPng; text: "Export PNG\u2026"; shortcut: "Ctrl+Alt+E"
        onTriggered: { pngDlg.currentFolder = ctl.fileFolder(); pngDlg.open() }
    }
    Action {
        id: actExportDrawio; text: "Export draw.io\u2026"; shortcut: "Ctrl+Shift+E"
        onTriggered: { drawioDlg.currentFolder = ctl.fileFolder(); drawioDlg.open() }
    }
    Action {
        id: actExportCml; text: "Export CML lab\u2026"
        onTriggered: { cmlDlg.currentFolder = ctl.fileFolder(); cmlDlg.open() }
    }
    // Ctrl+C on the canvas; text fields keep their own copy while focused.
    Action {
        id: actCopy; text: "Copy for draw.io"; shortcut: StandardKey.Copy
        onTriggered: ctl.copyDrawio()
    }
    Action { id: actCut; text: "Cut"; shortcut: StandardKey.Cut; onTriggered: ctl.cut() }
    // The whole sheet, layers and frame included, to paste into Stratagram.
    Action { id: actCopySheet; text: "Copy sheet with layers and frame"; onTriggered: ctl.copySheet() }
    Action {
        id: actPaste; text: "Paste"; shortcut: StandardKey.Paste
        onTriggered: {
            const a = canvas.toWorld(0, 0), b = canvas.toWorld(canvas.width, canvas.height)
            ctl.paste(a.x, a.y, b.x - a.x, b.y - a.y)
        }
    }
    Action { id: actUndo; text: "Undo"; shortcut: StandardKey.Undo; enabled: ctl.canUndo; onTriggered: ctl.undo() }
    Action { id: actRedo; text: "Redo"; shortcut: StandardKey.Redo; enabled: ctl.canRedo; onTriggered: ctl.redo() }
    Action {
        id: actAdd; text: "+ Device"; shortcut: "Ctrl+Shift+N"
        onTriggered: { const c = canvas.viewCenter(); ctl.addDevice(c.x, c.y) }
    }
    Action {
        id: actNote; text: "+ Text"; shortcut: "Ctrl+Shift+T"
        onTriggered: { const c = canvas.viewCenter(); ctl.addNote(c.x - 20, c.y - 10) }
    }
    Action {
        id: actPoint; text: "+ Point"; shortcut: "Ctrl+Shift+P"
        onTriggered: { const c = canvas.viewCenter(); ctl.addPoint(c.x, c.y) }
    }
    Action { id: actDup; text: "Duplicate"; shortcut: "Ctrl+D"; onTriggered: ctl.duplicate() }
    Action { id: actDel; text: "Delete"; onTriggered: ctl.deleteSelected() }
    Action {
        id: actObjectZoom; text: "Zoom into"; shortcut: "Ctrl+I"
        checkable: true; checked: canvas.deepZoom
        onTriggered: {
            canvas.deepZoom = checked
            ctl.setDeepZoom(checked)
        }
    }
    Action { id: actFit; text: "Fit"; shortcut: "Ctrl+0"; onTriggered: canvas.fit() }
    Action {
        id: actGrid; text: "Grid"; shortcut: "Ctrl+'"
        checkable: true; checked: prefs.showGrid
        onTriggered: prefs.showGrid = checked
    }
    Action {
        id: actPanel; text: "Panel"; shortcut: "Ctrl+B"
        checkable: true; checked: prefs.panelVisible
        onTriggered: prefs.panelVisible = checked
    }
    Shortcut { sequence: "Ctrl+K"; onActivated: quickAdd.forceActiveFocus() }
    Shortcut { sequence: "Ctrl+L"; onActivated: layoutPopup.open() }
    // Stacking order one step at a time; Home/End/PageUp/PageDown on the
    // canvas do the rest (Ctrl+Shift+bracket would clash with these).
    Shortcut { sequence: "Ctrl+]"; onActivated: ctl.arrange("forward") }
    Shortcut { sequence: "Ctrl+["; onActivated: ctl.arrange("backward") }
    Shortcut { sequence: "Ctrl+Y"; onActivated: ctl.redo() }
    Shortcut { sequence: StandardKey.SelectAll; onActivated: ctl.selectAll() }
    Shortcut { sequences: [StandardKey.ZoomIn, "Ctrl+="]; onActivated: canvas.zoomBy(1.2) }
    Shortcut { sequences: [StandardKey.ZoomOut]; onActivated: canvas.zoomBy(1 / 1.2) }
    Shortcut { sequences: [StandardKey.Quit]; onActivated: win.close() }

    // toolbar
    header: ToolBar {
        RowLayout {
            anchors.fill: parent
            spacing: 2
            ToolButton { action: actNew }
            ToolButton { action: actOpen }
            ToolButton { action: actSave }
            ToolButton { action: actSaveAs }
            ToolButton {
                id: exportButton
                text: "Export ▾"
                onClicked: exportMenu.open()
                Menu {
                    id: exportMenu
                    y: exportButton.height
                    MenuItem { action: actExport }
                    MenuItem { action: actExportPng }
                    MenuItem { action: actExportDrawio }
                    MenuItem { action: actExportCml }
                    MenuSeparator {}
                    MenuItem { action: actCopy }
                    MenuItem { action: actCopySheet }
                    MenuItem { action: actPaste; text: "Paste (from stratagram or draw.io)" }
                }
            }
            ToolSeparator {}
            // "+ Device" adds a plain device; the arrow beside it one of a type.
            ToolButton { action: actAdd; font.bold: true; rightPadding: 2 }
            ToolButton {
                id: addKindButton
                text: "▾"
                leftPadding: 2
                rightPadding: 6
                checkable: true
                checked: kindPopup.visible
                onClicked: kindPopup.visible ? kindPopup.close() : kindPopup.open()
                ToolTip.visible: hovered && !kindPopup.visible
                ToolTip.delay: 500
                ToolTip.text: "Add a device of a type"
            }
            ToolButton { action: actNote; font.bold: true }
            ToolButton {
                action: actPoint; font.bold: true
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "Free connection point for lines that point at nothing (Ctrl+Shift+P)"
            }
            ToolButton { action: actUndo }
            ToolButton { action: actRedo }
            ToolButton { action: actDup }
            ToolButton { action: actDel }
            ToolSeparator {}
            ToolButton {
                id: layoutButton
                text: "Layout ▾"
                checkable: true
                checked: layoutPopup.visible
                onClicked: layoutPopup.visible ? layoutPopup.close() : layoutPopup.open()
                ToolTip.visible: hovered && !layoutPopup.visible
                ToolTip.delay: 500
                ToolTip.text: "Auto layout (Ctrl+L)"
            }
            ToolSeparator {}
            CheckBox {
                text: "Snap"
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "Snap to grid"
                checked: ctl.snap
                onToggled: ctl.snap = checked
            }
            CheckBox {
                action: actGrid
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "Show the grid (Ctrl+')"
            }
            ComboBox {
                id: themeBox
                model: [{ value: "light", text: "Light" }, { value: "deep", text: "Dark" }, { value: "neon", text: "Neon" }]
                textRole: "text"
                valueRole: "value"
                Layout.preferredWidth: 92
                currentIndex: Math.max(0, ["light", "deep", "neon"].indexOf(prefs.theme))
                onActivated: prefs.theme = currentValue
                ToolTip.visible: hovered && !popup.visible
                ToolTip.delay: 500
                ToolTip.text: "Theme: the colors of the diagram, its exports (SVG, PNG) and the panels.\nThe file keeps its own colors."
            }
            ToolSeparator {}
            TextField {
                id: quickAdd
                Layout.fillWidth: true
                Layout.maximumWidth: 420
                Layout.minimumWidth: 100
                placeholderText: "Quick add (Ctrl+K): Firewall, Router, switch|my-box"
                selectByMouse: true
                ToolTip.visible: activeFocus && text === ""
                ToolTip.delay: 300
                ToolTip.text: "Comma-separated devices, then Enter.\n" +
                              "\"Router\" adds a router labeled Router;\n" +
                              "\"router|edge-01\" (type, then name) adds a router labeled edge-01."
                onAccepted: {
                    const c = canvas.viewCenter()
                    if (ctl.quickAdd(text, c.x, c.y) > 0) {
                        text = ""
                        canvas.revealSelection()
                        canvas.forceActiveFocus()
                    }
                }
                Keys.onEscapePressed: { text = ""; canvas.forceActiveFocus() }
            }
            Item { Layout.fillWidth: true }
            ToolButton {
                action: actObjectZoom
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "Deep zoom (Ctrl+I): zoom into a device without limit and add things inside it;\n" +
                              "they fade out when you zoom back out and reappear when you zoom in"
            }
            ToolButton { text: "−"; onClicked: canvas.zoomBy(1 / 1.2) }
            Label {
                text: Math.round(canvas.zoom * 100) + "%"
                horizontalAlignment: Text.AlignHCenter
                Layout.preferredWidth: 44
            }
            ToolButton { text: "+"; onClicked: canvas.zoomBy(1.2) }
            ToolButton { action: actFit }
            ToolSeparator {}
            ToolButton { action: actPanel }
        }
    }

    footer: ToolBar {
        height: 26
        Label {
            anchors.left: parent.left
            anchors.right: versionLabel.left
            anchors.top: parent.top
            anchors.bottom: parent.bottom
            anchors.leftMargin: 10
            anchors.rightMargin: 12
            verticalAlignment: Text.AlignVCenter
            elide: Text.ElideRight
            text: ctl.status
            color: tc.muted
        }
        // Version, in the lower right corner.
        Label {
            id: versionLabel
            anchors.right: parent.right
            anchors.rightMargin: 10
            anchors.verticalCenter: parent.verticalCenter
            text: "Stratagram " + ctl.version()
            color: tc.faint
            font.pixelSize: 11
        }
    }

    // content
    RowLayout {
        anchors.fill: parent
        spacing: 0

        // The canvas with the sheet tabs under it.
        ColumnLayout {
            Layout.fillWidth: true
            Layout.fillHeight: true
            spacing: 0
            DiagramCanvas {
                id: canvas
                ctl: ctl
                showGrid: prefs.showGrid
                Layout.fillWidth: true
                Layout.fillHeight: true
            }
            SheetBar {
                ctl: ctl
                tc: win.uiColors
                Layout.fillWidth: true
                onRemoveRequested: (index, name) => { removeSheetDlg.index = index; removeSheetDlg.name = name; removeSheetDlg.open() }
            }
        }

        // Thin strip that collapses the panel into the right edge.
        Rectangle {
            Layout.preferredWidth: 14
            Layout.fillHeight: true
            color: handleArea.containsMouse ? tc.line : tc.alt
            Rectangle { width: 1; height: parent.height; color: tc.line }
            Label {
                anchors.centerIn: parent
                text: prefs.panelVisible ? "\u25b8" : "\u25c2"
                color: tc.muted2
            }
            MouseArea {
                id: handleArea
                anchors.fill: parent
                hoverEnabled: true
                cursorShape: Qt.PointingHandCursor
                onClicked: prefs.panelVisible = !prefs.panelVisible
            }
            ToolTip.visible: handleArea.containsMouse
            ToolTip.delay: 600
            ToolTip.text: (prefs.panelVisible ? "Hide" : "Show") + " panel (Ctrl+B)"
        }

        PropertiesPanel {
            id: panel
            ctl: ctl
            tc: win.uiColors
            visible: prefs.panelVisible
            Layout.preferredWidth: 290
            Layout.fillHeight: true
        }
    }

    // dialogs
    // Diagrams are saved as .stratagram; older ones (.json) still open.
    // Files from before the rename (.netdiag) still open.
    readonly property var openFilters: ["Network diagrams (*.stratagram *.netdiag *.json)", "All files (*)"]
    readonly property var saveFilters: ["Network diagrams (*.stratagram)", "All files (*)"]

    FileDialog {
        id: openDlg
        title: "Open diagram"
        fileMode: FileDialog.OpenFile
        nameFilters: win.openFilters
        onAccepted: { if (ctl.open(selectedFile)) { forgetSheetViews(); canvas.fit() } canvas.forceActiveFocus() }
    }

    FileDialog {
        id: saveDlg
        title: "Save diagram"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "stratagram"
        nameFilters: win.saveFilters
        onAccepted: { if (ctl.saveAs(selectedFile)) runPending(); else pending = null }
        onRejected: pending = null
    }

    FileDialog {
        id: exportDlg
        title: "Export SVG"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "svg"
        nameFilters: ["SVG images (*.svg)"]
        onAccepted: ctl.exportSvg(selectedFile)
    }

    FileDialog {
        id: pngDlg
        title: "Export PNG"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "png"
        nameFilters: ["PNG images (*.png)"]
        onAccepted: ctl.exportPng(selectedFile)
    }

    FileDialog {
        id: drawioDlg
        title: "Export draw.io"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "drawio"
        nameFilters: ["draw.io diagrams (*.drawio)", "XML files (*.xml)"]
        onAccepted: ctl.exportDrawio(selectedFile)
    }

    // Cisco Modeling Labs: the sheet in view as a lab to import.
    FileDialog {
        id: cmlDlg
        title: "Export CML lab"
        fileMode: FileDialog.SaveFile
        defaultSuffix: "yaml"
        nameFilters: ["CML lab topologies (*.yaml *.yml)"]
        onAccepted: ctl.exportCml(selectedFile)
    }

    // Device types to add, by category (the panel's list).
    Popup {
        id: kindPopup
        parent: addKindButton
        y: addKindButton.height + 2
        padding: 4
        width: 220
        height: Math.min(kindList.contentHeight + 2 * padding, win.height - 120)
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent
        onClosed: canvas.forceActiveFocus()

        ListView {
            id: kindList
            anchors.fill: parent
            clip: true
            boundsBehavior: Flickable.StopAtBounds
            ScrollBar.vertical: ScrollBar {}
            model: panel.kindItems
            delegate: ItemDelegate {
                required property var modelData
                width: ListView.view.width
                enabled: !modelData.header
                leftPadding: modelData.header ? 8 : 22
                topPadding: modelData.header ? 8 : 5
                bottomPadding: modelData.header ? 2 : 5
                contentItem: Label {
                    text: modelData.header ? modelData.name.toUpperCase() : (modelData.key === "" ? "plain device" : modelData.name)
                    font.bold: modelData.header
                    font.pixelSize: modelData.header ? 10 : 13
                    font.italic: !modelData.header && modelData.key === ""
                    color: modelData.header ? tc.muted2 : tc.text
                }
                onClicked: {
                    const c = canvas.viewCenter()
                    ctl.addDeviceOf(modelData.key, c.x, c.y)
                    kindPopup.close()
                }
            }
        }
    }

    // Auto layout: pick an arrangement; it applies at once (Ctrl+Z undoes),
    // and the popup stays open to try another.
    Popup {
        id: layoutPopup
        parent: layoutButton
        y: layoutButton.height + 2
        padding: 12
        closePolicy: Popup.CloseOnEscape | Popup.CloseOnPressOutsideParent
        onClosed: canvas.forceActiveFocus()

        function apply(kind) {
            if (ctl.autoLayout(kind, prefs.layoutGap, prefs.layoutTidyPorts, prefs.layoutAvoid)) canvas.fit()
        }

        ColumnLayout {
            spacing: 4
            Label { text: "Auto layout"; font.bold: true }
            Label {
                Layout.maximumWidth: 260
                wrapMode: Text.WordWrap
                color: tc.muted2
                font.pixelSize: 11
                text: "Arranges the selected devices, or all shown and unlocked ones when fewer than " +
                      "two are selected. Ctrl+Z undoes."
            }
            Repeater {
                model: [["hierarchy", "Hierarchy ↓", "Layers from the internet and firewalls down to servers;\nredundant pairs side by side"],
                        ["hierarchy-lr", "Hierarchy →", "The same, left to right"],
                        ["radial", "Radial", "Rings around the most upstream device"],
                        ["circle", "Circle", "All devices on one circle, neighbors together"],
                        ["grid", "Grid", "Rows of equal cells, neighbors together"],
                        ["force", "Force-directed", "Organic: linked devices pull together (run again to refine)"],
                        ["tidy", "Tidy", "Straighten what is there: align rows and columns, center, straight lines; devices barely move"]]
                ItemDelegate {
                    required property var modelData
                    Layout.fillWidth: true
                    padding: 6
                    contentItem: ColumnLayout {
                        spacing: 0
                        Label { text: modelData[1]; font.bold: true }
                        Label { text: modelData[2]; color: tc.muted2; font.pixelSize: 11 }
                    }
                    onClicked: layoutPopup.apply(modelData[0])
                }
            }
            RowLayout {
                Layout.topMargin: 4
                Label { text: "Spacing" }
                SpinBox {
                    from: 10; to: 400; stepSize: 10; editable: true
                    value: prefs.layoutGap
                    onValueModified: prefs.layoutGap = value
                }
            }
            CheckBox {
                text: "Move ports to face their links"
                checked: prefs.layoutTidyPorts
                onToggled: prefs.layoutTidyPorts = checked
            }
            CheckBox {
                text: "Route lines around objects"
                checked: prefs.layoutAvoid
                onToggled: prefs.layoutAvoid = checked
                ToolTip.visible: hovered
                ToolTip.delay: 500
                ToolTip.text: "Lines that would cross a device or text get a routing that does not\n" +
                              "(straight or orthogonal if that is enough, else around objects)"
            }
        }
    }

    // Each sheet keeps its own view: back on a sheet, it is where you left
    // it; a sheet not seen yet is fitted.
    property var sheetViews: ({})
    property int shownSheet: 0
    // Another document: none of the views kept apply.
    function forgetSheetViews() { sheetViews = ({}); shownSheet = 0 }
    Connections {
        target: ctl
        function onSheetsChanged() {
            const sheets = JSON.parse(ctl.sheets)
            const now = sheets.findIndex(s => s.active)
            if (now === win.shownSheet) return
            win.sheetViews[win.shownSheet] = { zoom: canvas.zoom, panX: canvas.panX, panY: canvas.panY }
            win.shownSheet = now
            const v = win.sheetViews[now]
            if (v) { canvas.zoom = v.zoom; canvas.panX = v.panX; canvas.panY = v.panY; canvas.autoFit = false }
            else Qt.callLater(canvas.fit)
        }
    }

    Dialog {
        id: removeSheetDlg
        property int index: -1
        property string name: ""
        title: "Remove sheet"
        modal: true
        anchors.centerIn: parent
        standardButtons: Dialog.Ok | Dialog.Cancel
        onOpened: sheetDlgText.forceActiveFocus()
        Label {
            id: sheetDlgText
            // Enter confirms, Escape (handled by the dialog) cancels.
            Keys.onReturnPressed: removeSheetDlg.accept()
            Keys.onEnterPressed: removeSheetDlg.accept()
            text: "Remove \"" + removeSheetDlg.name + "\" and everything on it?\nYou can undo this with Ctrl+Z."
        }
        onAccepted: ctl.removeSheet(index)
        onClosed: canvas.forceActiveFocus()
    }

    Dialog {
        id: confirmDlg
        title: "Unsaved changes"
        modal: true
        anchors.centerIn: parent
        standardButtons: Dialog.Save | Dialog.Discard | Dialog.Cancel
        Label { text: "Save the changes to the current diagram first?" }
        onAccepted: win.save()
        onDiscarded: { confirmDlg.close(); win.runPending() }
        onRejected: win.pending = null
    }
}
