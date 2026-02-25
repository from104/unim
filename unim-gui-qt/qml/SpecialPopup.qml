import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: popup
    visible: false
    width: 440
    height: contentCol.implicitHeight + 16
    flags: Qt.ToolTip | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "#1e1e2e"

    property var bridge: null
    property string target: ""
    property var characters: []       // ["★", "☆", ...]
    property string topRowText: ""    // 상단 행 텍스트
    property int currentPage: 0
    property int pageSize: 9
    property int totalPages: Math.ceil(characters.length / pageSize)

    function showPopup(t, charsJson, topRow, cx, cy, cw, ch) {
        target = t
        topRowText = topRow
        try {
            characters = JSON.parse(charsJson)
        } catch(e) {
            characters = []
        }
        currentPage = 0
        if (characters.length === 0) { hidePopup(); return }

        x = cx
        y = cy + ch + 4
        visible = true
        requestActivate()
    }

    function hidePopup() {
        visible = false
        characters = []
    }

    function currentPageItems() {
        var start = currentPage * pageSize
        return characters.slice(start, start + pageSize)
    }

    function selectItem(idx) {
        var globalIdx = currentPage * pageSize + idx
        if (globalIdx < characters.length && bridge) {
            bridge.select_special_char(globalIdx)
        }
        hidePopup()
    }

    Column {
        id: contentCol
        anchors.fill: parent
        anchors.margins: 8
        spacing: 2

        // 헤더
        Rectangle {
            width: parent.width
            height: 28
            color: "#313244"
            radius: 4
            RowLayout {
                anchors.fill: parent
                anchors.leftMargin: 8
                anchors.rightMargin: 8
                Text {
                    text: "특수문자: " + target
                    color: "#cdd6f4"
                    font.pixelSize: 14
                    font.bold: true
                }
                Item { Layout.fillWidth: true }
                Text {
                    text: (currentPage + 1) + "/" + Math.max(totalPages, 1)
                    color: "#6c7086"
                    font.pixelSize: 12
                }
            }
        }

        // 상단 행 (분류 표시)
        Text {
            visible: topRowText.length > 0
            text: topRowText
            color: "#a6adc8"
            font.pixelSize: 12
            topPadding: 2
            bottomPadding: 2
        }

        // 특수문자 그리드 (3열)
        Grid {
            columns: 3
            spacing: 4
            width: parent.width

            Repeater {
                model: popup.currentPageItems()
                delegate: Rectangle {
                    width: (contentCol.width - 8) / 3
                    height: 32
                    color: charArea.containsMouse ? "#45475a" : "#313244"
                    radius: 4

                    RowLayout {
                        anchors.fill: parent
                        anchors.leftMargin: 6
                        spacing: 4
                        Text {
                            text: (index + 1) + "."
                            color: "#f38ba8"
                            font.pixelSize: 12
                            font.bold: true
                            Layout.preferredWidth: 18
                        }
                        Text {
                            text: modelData
                            color: "#cdd6f4"
                            font.pixelSize: 16
                            Layout.fillWidth: true
                            horizontalAlignment: Text.AlignHCenter
                        }
                    }

                    MouseArea {
                        id: charArea
                        anchors.fill: parent
                        hoverEnabled: true
                        onClicked: popup.selectItem(index)
                    }
                }
            }
        }

        // 안내
        Text {
            text: "← → 페이지 이동 | 1~9 선택 | ESC 취소"
            color: "#585b70"
            font.pixelSize: 10
            topPadding: 4
        }
    }

    Keys.onPressed: function(event) {
        if (event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
            var idx = event.key - Qt.Key_1
            if (idx < currentPageItems().length) {
                selectItem(idx)
            }
            event.accepted = true
        } else if (event.key === Qt.Key_Right) {
            if (currentPage < totalPages - 1) currentPage++
            event.accepted = true
        } else if (event.key === Qt.Key_Left) {
            if (currentPage > 0) currentPage--
            event.accepted = true
        } else if (event.key === Qt.Key_Escape) {
            if (bridge) bridge.cancel_special_char()
            hidePopup()
            event.accepted = true
        }
    }
}
