// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: BSD-2-Clause

import QtQuick
import QtQuick.Controls

ToolButton {
    id: root

    required property string accessibleName
    required property string glyph

    Accessible.name: accessibleName
    implicitHeight: 34
    implicitWidth: 34
    opacity: enabled ? 1 : 0.35

    contentItem: Text {
        color: "#30302d"
        font.pixelSize: 19
        horizontalAlignment: Text.AlignHCenter
        text: root.glyph
        verticalAlignment: Text.AlignVCenter
    }

    background: Rectangle {
        color: root.down ? "#d5d5d0" : root.hovered ? "#e2e2de" : "transparent"
        radius: 6
    }

    ToolTip {
        parent: root
        popupType: Popup.Window
        text: root.accessibleName
        visible: root.hovered
        x: (root.width - implicitWidth) / 2
        y: root.height + 3
    }
}
