import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: popup
    visible: false
    width: 420
    height: contentCol.implicitHeight + 16
    flags: Qt.ToolTip | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "#1e1e2e"

    property var bridge: null
    property string target: ""
    property var candidates: []       // [[hanja, meaning], ...]
    property int currentPage: 0
    property int pageSize: 9
    property int totalPages: Math.ceil(candidates.length / pageSize)

    function showPopup(t, candidatesJson, cx, cy, cw, ch) {
        target = t
        try {
            candidates = JSON.parse(candidatesJson)
        } catch(e) {
            candidates = []
        }
        currentPage = 0
        if (candidates.length === 0) { hidePopup(); return }

        // 위치 계산 (커서 아래)
        x = cx
        y = cy + ch + 4
        visible = true
        requestActivate()
    }

    function hidePopup() {
        visible = false
        candidates = []
    }

    function currentPageItems() {
        var start = currentPage * pageSize
        return candidates.slice(start, start + pageSize)
    }

    function selectItem(idx) {
        var globalIdx = currentPage * pageSize + idx
        if (globalIdx < candidates.length && bridge) {
            bridge.select_hanja(globalIdx)
        }
        hidePopup()
    }

    Column {
        id: contentCol
        anchors.fill: parent
        anchors.margins: 8
        spacing: 2

        // 헤더: 대상 글자 + 페이지
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
                    text: "한자: " + target
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

        // 후보 목록
        Repeater {
            model: popup.currentPageItems()
            delegate: Rectangle {
                width: contentCol.width
                height: 28
                color: candidateArea.containsMouse ? "#45475a" : "transparent"
                radius: 4

                RowLayout {
                    anchors.fill: parent
                    anchors.leftMargin: 8
                    anchors.rightMargin: 8
                    spacing: 8
                    Text {
                        text: (index + 1) + "."
                        color: "#f38ba8"
                        font.pixelSize: 13
                        font.bold: true
                        Layout.preferredWidth: 20
                    }
                    Text {
                        text: modelData[0]
                        color: "#cdd6f4"
                        font.pixelSize: 16
                    }
                    Text {
                        text: modelData[1] || ""
                        color: "#6c7086"
                        font.pixelSize: 12
                        Layout.fillWidth: true
                        elide: Text.ElideRight
                    }
                }

                MouseArea {
                    id: candidateArea
                    anchors.fill: parent
                    hoverEnabled: true
                    onClicked: popup.selectItem(index)
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

    // 키보드 처리
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
            if (bridge) bridge.cancel_hanja()
            hidePopup()
            event.accepted = true
        }
    }
}
