// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import com.stratagram

// The sheets of the document as tabs under the canvas: click to switch,
// double-click to rename in place, drag sideways to reorder, × to remove,
// + to add.
Rectangle {
    // The interface colors of the theme (given by main.qml).
    property var tc: ({ text: "#0f172a", muted: "#475569", muted2: "#64748b", faint: "#94a3b8", line: "#e2e8f0", alt: "#f1f5f9", base: "#ffffff", accent: "#2563eb", border: "#cbd5e1", drop: "#bfdbfe", dropParent: "#e0ecff", active: "#dbeafe" })
    id: root
    required property DiagramController ctl
    readonly property var sheets: JSON.parse(ctl.sheets)
    // Asks before removing a sheet with something on it.
    signal removeRequested(int index, string name)
    // A tab being dragged: its index and the tab it would take the place of.
    property int dragFrom: -1
    property int dropAt: -1

    implicitHeight: 30
    color: root.tc.alt
    Rectangle { width: parent.width; height: 1; color: root.tc.line }

    ListView {
        id: tabs
        anchors.fill: parent
        anchors.leftMargin: 4
        orientation: ListView.Horizontal
        spacing: 2
        clip: true
        boundsBehavior: Flickable.StopAtBounds
        model: root.sheets
        ScrollBar.horizontal: ScrollBar { height: 4 }

        delegate: Rectangle {
            id: tab
            required property var modelData
            required property int index
            property bool renaming: false
            width: row.implicitWidth + 20
            height: tabs.height - 3
            y: 0
            radius: 0
            color: modelData.active ? root.tc.base : (tabMouse.containsMouse ? root.tc.line : "transparent")
            border.width: modelData.active ? 1 : 0
            border.color: root.tc.line
            // The active tab joins the canvas above it.
            Rectangle { visible: tab.modelData.active; width: parent.width; height: 2; color: root.tc.accent; anchors.bottom: parent.bottom }

            // Where the dragged tab would go: a bar on this tab's left (moving
            // left) or right (moving right) edge.
            Rectangle {
                visible: root.dropAt === tab.index && root.dragFrom !== tab.index
                width: 3
                height: parent.height
                color: root.tc.accent
                x: root.dragFrom > tab.index ? 0 : parent.width - width
                z: 2
            }

            MouseArea {
                id: tabMouse
                anchors.fill: parent
                hoverEnabled: true
                acceptedButtons: Qt.LeftButton
                property real pressX: 0
                onPressed: (m) => { pressX = m.x }
                // Dragged sideways past a few pixels: reordering.
                onPositionChanged: (m) => {
                    if (!pressed) return
                    if (root.dragFrom < 0 && Math.abs(m.x - pressX) > 6) root.dragFrom = tab.index
                    if (root.dragFrom < 0) return
                    const p = mapToItem(tabs.contentItem, m.x, height / 2)
                    const at = tabs.indexAt(p.x, p.y)
                    if (at >= 0) root.dropAt = at
                }
                property bool wasDrag: false
                onReleased: {
                    wasDrag = root.dragFrom >= 0
                    const from = root.dragFrom, to = root.dropAt
                    root.dragFrom = -1
                    root.dropAt = -1
                    if (wasDrag && to >= 0 && to !== from) root.ctl.moveSheet(from, to)
                }
                onCanceled: { root.dragFrom = -1; root.dropAt = -1 }
                // (A drag released on its own tab ends in a click too: it only
                // reorders.)
                onClicked: if (!wasDrag) root.ctl.switchSheet(tab.index)
                onDoubleClicked: {
                    root.ctl.switchSheet(tab.index)
                    tab.renaming = true
                    nameField.text = tab.modelData.name
                    nameField.forceActiveFocus()
                    nameField.selectAll()
                }
            }
            RowLayout {
                id: row
                anchors.verticalCenter: parent.verticalCenter
                anchors.left: parent.left
                anchors.leftMargin: 10
                spacing: 4
                Label {
                    visible: !tab.renaming
                    text: tab.modelData.name
                    font.bold: tab.modelData.active
                    color: tab.modelData.active ? root.tc.text : root.tc.muted
                }
                TextField {
                    id: nameField
                    visible: tab.renaming
                    Layout.preferredWidth: Math.max(80, contentWidth + 24)
                    padding: 2
                    selectByMouse: true
                    function finish(save) {
                        if (!tab.renaming) return
                        tab.renaming = false
                        if (save) root.ctl.renameSheet(tab.index, text)
                    }
                    onAccepted: finish(true)
                    onActiveFocusChanged: if (!activeFocus) finish(true)
                    Keys.onEscapePressed: finish(false)
                }
                // Remove: on every tab (always there, so tabs keep their width).
                Label {
                    text: "×"
                    visible: root.sheets.length > 1 && !tab.renaming
                    color: closeMouse.containsMouse ? "#dc2626" : root.tc.faint
                    font.pixelSize: 15
                    MouseArea {
                        id: closeMouse
                        anchors.fill: parent
                        anchors.margins: -3
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: {
                            if (tab.modelData.empty) root.ctl.removeSheet(tab.index)
                            else root.removeRequested(tab.index, tab.modelData.name)
                        }
                    }
                    ToolTip.visible: closeMouse.containsMouse
                    ToolTip.delay: 500
                    ToolTip.text: "Remove this sheet"
                }
            }
        }

        footer: ToolButton {
            text: "+"
            height: tabs.height - 3
            font.bold: true
            onClicked: root.ctl.addSheet()
            ToolTip.visible: hovered
            ToolTip.delay: 500
            ToolTip.text: "Add a sheet"
        }
    }
}
