// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: BSD-2-Clause

import QtQuick
import "components"

Item {
    id: root

    required property var browserState

    width: 1100
    height: 760

    Rectangle {
        anchors.fill: parent
        color: "#f4f4f2"
    }

    NavigationBar {
        id: navigationBar
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: parent.top
        browserState: root.browserState
    }

    BrowserSurface {
        objectName: "browserSurface"
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.top: navigationBar.bottom
        anchors.bottom: parent.bottom
    }
}
