// Copyright (c) 2026, the Photon developers.
//
// SPDX-License-Identifier: BSD-2-Clause

import QtQuick
import QtQuick.Controls

TextField {
    id: root

    required property string currentUrl

    signal navigateRequested(string input)

    Accessible.name: qsTr("Address")
    ContextMenu.menu.popupType: Popup.Window

    function focusAndSelect() {
        forceActiveFocus();
        selectAll();
    }

    function finishEditing() {
        focus = false;
        text = currentUrl;
    }

    color: "#282825"
    font.pixelSize: 14
    leftPadding: 12
    rightPadding: 12
    selectByMouse: true
    selectedTextColor: "#ffffff"
    selectionColor: "#466d9d"

    Component.onCompleted: text = currentUrl
    onAccepted: navigateRequested(text)
    onActiveFocusChanged: {
        if (!activeFocus)
            text = currentUrl;
    }
    onCurrentUrlChanged: {
        if (!activeFocus)
            text = currentUrl;
    }

    background: Rectangle {
        border.color: root.activeFocus ? "#6b86a5" : "#cdcdc7"
        border.width: 1
        color: "#ffffff"
        radius: 7
    }
}
