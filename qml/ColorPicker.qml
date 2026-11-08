// This file is part of Stratagram: https://github.com/shinsterneck/stratagram
// Copyright 2026 Shin Sterneck. Licensed under the Apache License, Version 2.0 (see LICENSE).
// SPDX-License-Identifier: Apache-2.0

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

// Swatches plus a hex field.
ColumnLayout {
    // The interface colors of the theme (given by main.qml).
    property var tc: ({ text: "#0f172a", muted: "#475569", muted2: "#64748b", faint: "#94a3b8", line: "#e2e8f0", alt: "#f1f5f9", base: "#ffffff", accent: "#2563eb", border: "#cbd5e1", drop: "#bfdbfe", dropParent: "#e0ecff", active: "#dbeafe" })
    id: root
    property string color: "#ffffff"
    property var swatches: []
    signal picked(string color)
    spacing: 6

    Flow {
        Layout.fillWidth: true
        spacing: 4
        Repeater {
            model: root.swatches
            Rectangle {
                required property string modelData
                width: 22; height: 22; radius: 4
                color: modelData
                border.width: root.color.toLowerCase() === modelData ? 2 : 1
                border.color: root.color.toLowerCase() === modelData ? root.tc.accent : root.tc.faint
                MouseArea {
                    anchors.fill: parent
                    cursorShape: Qt.PointingHandCursor
                    onClicked: root.picked(parent.modelData)
                }
            }
        }
    }

    TextField {
        Layout.fillWidth: true
        text: root.color
        placeholderText: "#rrggbb"
        validator: RegularExpressionValidator { regularExpression: /#?[0-9a-fA-F]{0,6}/ }
        onEditingFinished: {
            let t = text.trim()
            if (!t.startsWith("#")) t = "#" + t
            if (t.length === 4 || t.length === 7) root.picked(t)
            else text = root.color
        }
    }
}
