// EmojiPopup.qml — 이모지 팝업
//
// 좌측 9탭 사이드바 + 우측 81칸(9×9) 그리드.
// GTK SoT (emoji_popup.rs) 동작 재현.

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: emojiRoot
    title: ""
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"

    property var bridge: null

    SystemPalette { id: palette; colorGroup: SystemPalette.Active }

    Connections {
        target: bridge
        function onPopup_render_changed() { repaintTrigger.repaintCount++ }
        function onPopup_show()           { repaintTrigger.repaintCount++ }
    }

    QtObject {
        id: repaintTrigger
        property int repaintCount: 0
    }

    Rectangle {
        anchors.fill: parent
        color: palette.window
        border.color: palette.mid
        border.width: 1
        radius: 4

        RowLayout {
            anchors.fill: parent
            anchors.margins: 4
            spacing: 4

            // ─── 좌측 탭 사이드바 (9탭) ───
            Column {
                spacing: 1
                Layout.alignment: Qt.AlignTop

                Repeater {
                    model: bridge ? bridge.popup_tab_count() : 9
                    delegate: Rectangle {
                        width: 36; height: 36
                        property int tabIdx: index
                        property bool isActive: bridge
                                                ? (bridge.popup_active_tab() === tabIdx)
                                                : false
                        property int _rep: repaintTrigger.repaintCount

                        color: isActive ? palette.highlight
                                        : (tabMouse.containsMouse ? palette.alternateBase : "transparent")
                        radius: 4
                        border.color: isActive ? palette.highlight : "transparent"
                        border.width: 1

                        Text {
                            anchors.centerIn: parent
                            text: bridge ? bridge.popup_tab_label(tabIdx) : ""
                            font.pixelSize: 18
                            color: isActive ? palette.highlightedText : palette.windowText
                        }

                        MouseArea {
                            id: tabMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: Qt.PointingHandCursor
                            onClicked: if (bridge) bridge.popup_set_emoji_category(tabIdx)
                        }
                    }
                }
            }

            // ─── 우측: 헤더 + 9×9 그리드 + 푸터 ───
            ColumnLayout {
                Layout.fillWidth: true
                Layout.fillHeight: true
                spacing: 2

                // 헤더 (카테고리명)
                Text {
                    Layout.fillWidth: true
                    text: bridge ? bridge.popup_header_text() : ""
                    color: palette.windowText
                    font.pixelSize: 12
                    font.bold: true
                    elide: Text.ElideRight
                }

                // 9×9 그리드
                GridLayout {
                    id: emojiGrid
                    Layout.fillWidth: true
                    columns: 9
                    rowSpacing: 2
                    columnSpacing: 2

                    property int _rep: repaintTrigger.repaintCount

                    // column-major: cell(row, col)
                    // 화면 row-major 순서: index → row=index%9, col=index/9
                    Repeater {
                        model: 81
                        delegate: Rectangle {
                            property int ri: index % 9
                            property int ci: Math.floor(index / 9)
                            property int _rep: repaintTrigger.repaintCount

                            width: 36; height: 36

                            property int cellFlags: bridge ? bridge.popup_cell_flags(ri, ci) : 0
                            property bool hasData:    (cellFlags & 0x01) !== 0
                            property bool isSelected: (cellFlags & 0x02) !== 0

                            color: isSelected
                                   ? palette.highlight
                                   : (em.containsMouse && hasData ? palette.alternateBase : palette.base)
                            radius: 3
                            border.color: isSelected ? palette.highlightedText : "transparent"
                            border.width: isSelected ? 1 : 0

                            Text {
                                anchors.centerIn: parent
                                text: (hasData && bridge)
                                      ? bridge.popup_cell_text(ri, ci) : ""
                                font.pixelSize: 22
                                color: isSelected ? palette.highlightedText : palette.windowText
                            }

                            MouseArea {
                                id: em
                                anchors.fill: parent
                                hoverEnabled: true
                                cursorShape: hasData ? Qt.PointingHandCursor : Qt.ArrowCursor
                                onClicked: {
                                    if (bridge && hasData) {
                                        const emoji = bridge.popup_cell_text(ri, ci)
                                        if (emoji !== "") bridge.popup_commit_emoji(emoji)
                                    }
                                }
                            }
                        }
                    }
                }

                // 푸터
                RowLayout {
                    Layout.fillWidth: true
                    visible: bridge ? bridge.popup_show_footer() : false
                    spacing: 2

                    Button {
                        text: "◀"; flat: true; font.pixelSize: 12
                        onClicked: if (bridge) bridge.popup_change_page(-1)
                    }
                    Text {
                        Layout.fillWidth: true
                        horizontalAlignment: Text.AlignHCenter
                        text: bridge ? bridge.popup_footer_text() : ""
                        color: palette.windowText
                        font.pixelSize: 11
                    }
                    Button {
                        text: "▶"; flat: true; font.pixelSize: 12
                        onClicked: if (bridge) bridge.popup_change_page(1)
                    }
                }
            }
        }

        // ─── 키 핸들링 ───
        Keys.onPressed: function(event) {
            if (!bridge) return
            if (event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
                bridge.popup_set_emoji_category(event.key - Qt.Key_1)
                event.accepted = true
            } else if (event.key === Qt.Key_Return || event.key === Qt.Key_Enter) {
                const row = bridge.popup_sel_row()
                const col = bridge.popup_sel_col()
                const emoji = bridge.popup_cell_text(row, col)
                if (emoji !== "") bridge.popup_commit_emoji(emoji)
                event.accepted = true
            } else if (event.key === Qt.Key_Tab) {
                bridge.popup_change_page(1); event.accepted = true
            } else if (event.key === Qt.Key_Backtab) {
                bridge.popup_change_page(-1); event.accepted = true
            } else if (event.key === Qt.Key_Escape) {
                event.accepted = true
            }
        }
    }
}
