// SpecialPopup.qml — 특수문자 팝업
//
// 9×9 그리드 (column-major). 열 헤더 + 행 번호.
// GTK SoT (special_popup.rs) 동작 재현.

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: specialRoot
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

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 4
            spacing: 2

            // ─── 헤더 ───
            Text {
                Layout.fillWidth: true
                text: bridge ? bridge.popup_header_text() : ""
                color: palette.windowText
                font.pixelSize: 13
                font.bold: true
                elide: Text.ElideRight
            }

            // ─── 9×9 그리드: 코너 + 열헤더 + (행번호+셀) × 9 ───
            GridLayout {
                id: specialGrid
                Layout.fillWidth: true
                columns: 10  // 1 행번호 열 + 9 데이터 열
                rowSpacing: 1
                columnSpacing: 1

                property int _rep: repaintTrigger.repaintCount

                // 헤더 행: 코너(0,0) + 열헤더(1-9)
                // 코너
                Item { width: 20; height: 20 }

                // 열 헤더 ×9
                Repeater {
                    model: 9
                    delegate: Rectangle {
                        width: 30; height: 20
                        color: bridge && bridge.popup_col_header_hl(index)
                               ? palette.highlight : "transparent"
                        Text {
                            anchors.centerIn: parent
                            text: bridge ? bridge.popup_col_header(index) : ""
                            color: bridge && bridge.popup_col_header_hl(index)
                                   ? palette.highlightedText : palette.windowText
                            font.pixelSize: 11
                            font.bold: true
                        }
                    }
                }

                // 데이터 행: 9rows × (1행번호+9셀) = 9×10 = 90개 아이템
                Repeater {
                    model: 90  // 9 rows × 10 cols (행번호포함)
                    delegate: Loader {
                        property int ri: Math.floor(index / 10)   // 0-8 (행)
                        property int ci: index % 10               // 0=행번호, 1-9=데이터열

                        sourceComponent: ci === 0 ? specialRowHeaderComp : specialCellComp

                        Component {
                            id: specialRowHeaderComp
                            Rectangle {
                                width: 20; height: 28
                                color: bridge && bridge.popup_row_header_hl(ri)
                                       ? palette.highlight : "transparent"
                                Text {
                                    anchors.centerIn: parent
                                    text: bridge ? bridge.popup_row_header(ri) : String(ri + 1)
                                    color: bridge && bridge.popup_row_header_hl(ri)
                                           ? palette.highlightedText : palette.windowText
                                    font.pixelSize: 11
                                }
                            }
                        }

                        Component {
                            id: specialCellComp
                            Rectangle {
                                property int colIdx: ci - 1  // 0-8
                                property int rowIdx: ri       // 0-8
                                width: 30; height: 28

                                property int cellFlags: bridge
                                                        ? bridge.popup_cell_flags(rowIdx, colIdx)
                                                        : 0
                                property bool hasData:    (cellFlags & 0x01) !== 0
                                property bool isSelected: (cellFlags & 0x02) !== 0
                                property bool isColHl:    (cellFlags & 0x04) !== 0
                                property bool isRowHl:    (cellFlags & 0x08) !== 0

                                color: isSelected
                                       ? palette.highlight
                                       : (isColHl || isRowHl)
                                         ? Qt.rgba(palette.highlight.r, palette.highlight.g,
                                                   palette.highlight.b, 0.15)
                                         : (sm.containsMouse && hasData ? palette.alternateBase : palette.base)
                                border.color: isSelected ? palette.highlightedText : palette.mid
                                border.width: isSelected ? 1 : 0
                                radius: 2

                                Text {
                                    anchors.centerIn: parent
                                    text: (hasData && bridge)
                                          ? bridge.popup_cell_text(rowIdx, colIdx) : ""
                                    color: isSelected ? palette.highlightedText : palette.windowText
                                    font.pixelSize: 14
                                }

                                MouseArea {
                                    id: sm
                                    anchors.fill: parent
                                    hoverEnabled: true
                                    cursorShape: hasData ? Qt.PointingHandCursor : Qt.ArrowCursor
                                    onClicked: {
                                        if (bridge && hasData) {
                                            bridge.popup_select_special(colIdx, rowIdx)
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ─── 푸터: [◀] [N/M] [▶] ───
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

        // ─── 키 핸들링 ───
        Keys.onPressed: function(event) {
            if (!bridge) return
            if (event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
                const col = event.key - Qt.Key_1
                const row = bridge.popup_sel_row()
                bridge.popup_select_special(col, row)
                event.accepted = true
            } else if (event.key === Qt.Key_Tab) {
                bridge.popup_change_page(1); event.accepted = true
            } else if (event.key === Qt.Key_Backtab) {
                bridge.popup_change_page(-1); event.accepted = true
            } else if (event.key === Qt.Key_Escape) {
                bridge.popup_cancel_special(); event.accepted = true
            }
        }
    }
}
