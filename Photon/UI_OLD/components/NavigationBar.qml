// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: GPL-3.0-only

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Rectangle {
    id: root

    required property var browserState

    height: 50
    color: "#f4f4f2"

    RowLayout {
        anchors.fill: parent
        anchors.leftMargin: 10
        anchors.rightMargin: 10
        anchors.topMargin: 7
        anchors.bottomMargin: 8
        spacing: 4

        NavigationButton {
            accessibleName: qsTr("Back")
            glyph: "←"
            enabled: root.browserState.canGoBack
            onClicked: root.browserState.go_back()
        }

        NavigationButton {
            accessibleName: qsTr("Forward")
            glyph: "→"
            enabled: root.browserState.canGoForward
            onClicked: root.browserState.go_forward()
        }

        NavigationButton {
            accessibleName: qsTr("Reload")
            glyph: "↻"
            onClicked: root.browserState.reload()
        }

        AddressBar {
            id: addressBar
            Layout.fillWidth: true
            Layout.fillHeight: true
            Layout.leftMargin: 4
            currentUrl: root.browserState.url
            onNavigateRequested: input => {
                if (root.browserState.navigate(input)) {
                    finishEditing();
                    root.browserState.focus_web_content();
                }
            }
        }

        BusyIndicator {
            Layout.preferredHeight: 24
            Layout.preferredWidth: 24
            running: root.browserState.loading
            visible: running
        }
    }

    Rectangle {
        anchors.left: parent.left
        anchors.right: parent.right
        anchors.bottom: parent.bottom
        color: "#d7d7d2"
        height: 1
    }

    Shortcut {
        context: Qt.ApplicationShortcut
        sequence: Qt.platform.os === "osx" ? "Meta+L" : "Ctrl+L"
        onActivated: addressBar.focusAndSelect()
    }

    Shortcut {
        context: Qt.ApplicationShortcut
        sequences: [StandardKey.Refresh]
        onActivated: root.browserState.reload()
    }

    Shortcut {
        context: Qt.ApplicationShortcut
        sequences: [StandardKey.Back]
        onActivated: root.browserState.go_back()
    }

    Shortcut {
        context: Qt.ApplicationShortcut
        sequences: [StandardKey.Forward]
        onActivated: root.browserState.go_forward()
    }
}
