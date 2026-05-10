// HanjaPopup.qml — 한자 팝업
//
// PopupModel 데이터를 bridge invokable로 조회하여 렌더링.
// compact(cols==1) / expanded(cols>1, 9×9) 자동 전환.
// GTK SoT (hanja_popup.rs) 동작 재현.

import QtQuick
import QtQuick.Controls
import QtQuick.Layouts

Window {
    id: hanjaRoot
    title: ""
    flags: Qt.Tool | Qt.FramelessWindowHint | Qt.WindowStaysOnTopHint
    color: "transparent"

    // bridge는 부모 main.qml에서 주입
    property var bridge: null

    SystemPalette { id: palette; colorGroup: SystemPalette.Active }

    // bridge 데이터 갱신 시 강제 재렌더
    Connections {
        target: bridge
        function onPopup_render_changed() { repaintTrigger.repaintCount++ }
        function onPopup_show()           { repaintTrigger.repaintCount++ }
    }

    QtObject {
        id: repaintTrigger
        property int repaintCount: 0
    }

    // ─── 팝업 컨테이너 ───
    Rectangle {
        id: contentRect
        anchors.fill: parent
        color: palette.window
        border.color: palette.mid
        border.width: 1
        radius: 4

        ColumnLayout {
            anchors.fill: parent
            anchors.margins: 4
            spacing: 2

            // ─── 헤더: [⊞/⊟] [header_text] ───
            RowLayout {
                Layout.fillWidth: true
                spacing: 4

                // expand 토글 버튼
                Text {
                    id: expandBtn
                    visible: {
                        const _ = repaintTrigger.repaintCount
                        return bridge ? bridge.popup_expand_visible() : false
                    }
                    text: {
                        const _ = repaintTrigger.repaintCount
                        return bridge ? bridge.popup_expand_text() : "⊞"
                    }
                    color: expandMouse.containsMouse ? palette.highlight : palette.windowText
                    font.pixelSize: 14

                    MouseArea {
                        id: expandMouse
                        anchors.fill: parent
                        hoverEnabled: true
                        cursorShape: Qt.PointingHandCursor
                        onClicked: if (bridge) bridge.popup_toggle_expand()
                    }
                }

                Text {
                    Layout.fillWidth: true
                    text: {
                        const _ = repaintTrigger.repaintCount
                        return bridge ? bridge.popup_header_text() : ""
                    }
                    color: palette.windowText
                    font.pixelSize: 13
                    font.bold: true
                    elide: Text.ElideRight
                }
            }

            // ─── 본문: compact or expanded ───
            // compact: 세로 목록 (cols==1)
            Column {
                id: compactBody
                Layout.fillWidth: true
                spacing: 1
                visible: bridge ? (bridge.popup_cols() <= 1) : true
                property int _rep: repaintTrigger.repaintCount

                Repeater {
                    model: {
                        const _ = repaintTrigger.repaintCount
                        return (bridge && bridge.popup_cols() <= 1)
                               ? Math.min(bridge.popup_rows(), 9) : 0
                    }
                    delegate: Rectangle {
                        width: compactBody.width
                        height: 28
                        property int _rep: repaintTrigger.repaintCount
                        property int cellFlags: {
                            const _ = repaintTrigger.repaintCount
                            return bridge ? bridge.popup_cell_flags(index, 0) : 0
                        }
                        property bool hasData:    (cellFlags & 0x01) !== 0
                        property bool isSelected: (cellFlags & 0x02) !== 0
                        property bool isBookmarked: (cellFlags & 0x10) !== 0

                        color: isSelected
                               ? palette.highlight
                               : (cMouse.containsMouse && hasData ? palette.alternateBase : palette.base)
                        radius: 2
                        border.color: isSelected ? palette.highlight : palette.mid
                        border.width: isSelected ? 1 : 0

                        RowLayout {
                            anchors.fill: parent
                            anchors.margins: 4
                            spacing: 4
                            visible: hasData

                            Text {
                                Layout.fillWidth: true
                                text: {
                                    const _ = repaintTrigger.repaintCount
                                    return bridge ? bridge.popup_cell_text(index, 0) : ""
                                }
                                color: isSelected ? palette.highlightedText : palette.windowText
                                font.pixelSize: 13
                                elide: Text.ElideRight
                            }
                            Text {
                                text: {
                                    const _ = repaintTrigger.repaintCount
                                    return bridge ? bridge.popup_cell_meaning(index, 0) : ""
                                }
                                color: isSelected
                                       ? Qt.rgba(palette.highlightedText.r, palette.highlightedText.g,
                                                 palette.highlightedText.b, 0.8)
                                       : palette.placeholderText
                                font.pixelSize: 11
                                elide: Text.ElideRight
                            }
                            Text {
                                visible: isBookmarked
                                text: "★"
                                color: "#f5c518"
                                font.pixelSize: 11
                            }
                        }

                        MouseArea {
                            id: cMouse
                            anchors.fill: parent
                            hoverEnabled: true
                            cursorShape: hasData ? Qt.PointingHandCursor : Qt.ArrowCursor
                            acceptedButtons: Qt.LeftButton | Qt.RightButton
                            onClicked: function(mouse) {
                                if (!bridge || !hasData) return
                                if (mouse.button === Qt.RightButton) {
                                    const gi = bridge.popup_current_page() * bridge.popup_rows() + index
                                    bridge.popup_toggle_bookmark(gi)
                                } else {
                                    bridge.popup_select_hanja(index)
                                }
                            }
                        }
                    }
                }
            }

            // expanded: 9×9 그리드 (열 헤더 행 + 행번호 열 + 데이터)
            Item {
                id: expandedBody
                Layout.fillWidth: true
                Layout.fillHeight: true
                visible: bridge ? (bridge.popup_cols() > 1) : false
                property int _rep: repaintTrigger.repaintCount

                GridLayout {
                    anchors.fill: parent
                    // columns = 1(행번호) + N(data cols)
                    columns: (bridge && bridge.popup_cols() > 1) ? (bridge.popup_cols() + 1) : 10
                    rowSpacing: 1
                    columnSpacing: 1

                    // 헤더 행: (0,0) 코너 + 열 헤더
                    // (0,0) 코너
                    Item { width: 20; height: 20 }

                    // 열 헤더
                    Repeater {
                        model: {
                            const _ = repaintTrigger.repaintCount
                            return (bridge && bridge.popup_cols() > 1) ? bridge.popup_cols() : 9
                        }
                        delegate: Rectangle {
                            width: 28; height: 20
                            property int _rep: repaintTrigger.repaintCount
                            color: {
                                const _ = repaintTrigger.repaintCount
                                return bridge && bridge.popup_col_header_hl(index)
                                       ? palette.highlight : "transparent"
                            }
                            Text {
                                anchors.centerIn: parent
                                text: {
                                    const _ = repaintTrigger.repaintCount
                                    return bridge ? bridge.popup_col_header(index) : ""
                                }
                                color: {
                                    const _ = repaintTrigger.repaintCount
                                    return bridge && bridge.popup_col_header_hl(index)
                                           ? palette.highlightedText : palette.windowText
                                }
                                font.pixelSize: 11
                                font.bold: true
                            }
                        }
                    }

                    // 데이터 행: 행번호 + 셀 (rows × (cols+1))
                    Repeater {
                        model: {
                            const _ = repaintTrigger.repaintCount
                            if (!bridge || bridge.popup_cols() <= 1) return 0
                            return bridge.popup_rows() * (bridge.popup_cols() + 1)
                        }
                        delegate: Loader {
                            property int totalCols: (bridge && bridge.popup_cols() > 1)
                                                    ? (bridge.popup_cols() + 1) : 10
                            property int ri: Math.floor(index / totalCols)
                            property int ci: index % totalCols

                            sourceComponent: ci === 0 ? rowHeaderComp : hanjaCellComp

                            Component {
                                id: rowHeaderComp
                                Rectangle {
                                    width: 20; height: 28
                                    color: {
                                        const _ = repaintTrigger.repaintCount
                                        return bridge && bridge.popup_row_header_hl(ri)
                                               ? palette.highlight : "transparent"
                                    }
                                    Text {
                                        anchors.centerIn: parent
                                        text: {
                                            const _ = repaintTrigger.repaintCount
                                            return bridge ? bridge.popup_row_header(ri) : ""
                                        }
                                        color: {
                                            const _ = repaintTrigger.repaintCount
                                            return bridge && bridge.popup_row_header_hl(ri)
                                                   ? palette.highlightedText : palette.windowText
                                        }
                                        font.pixelSize: 11
                                    }
                                }
                            }

                            Component {
                                id: hanjaCellComp
                                Rectangle {
                                    property int colIdx: ci - 1
                                    property int rowIdx: ri
                                    width: 28; height: 28
                                    property int _rep: repaintTrigger.repaintCount
                                    property int cellFlags: {
                                        const _ = repaintTrigger.repaintCount
                                        return bridge
                                               ? bridge.popup_cell_flags(rowIdx, colIdx)
                                               : 0
                                    }
                                    property bool hasData:    (cellFlags & 0x01) !== 0
                                    property bool isSelected: (cellFlags & 0x02) !== 0
                                    property bool isColHl:    (cellFlags & 0x04) !== 0
                                    property bool isRowHl:    (cellFlags & 0x08) !== 0
                                    property bool isBookmarked: (cellFlags & 0x10) !== 0

                                    color: isSelected
                                           ? palette.highlight
                                           : (isColHl || isRowHl)
                                             ? Qt.rgba(palette.highlight.r, palette.highlight.g,
                                                       palette.highlight.b, 0.15)
                                             : (hm.containsMouse && hasData ? palette.alternateBase : palette.base)
                                    border.color: isSelected ? palette.highlightedText : palette.mid
                                    border.width: isSelected ? 1 : 0
                                    radius: 2

                                    Text {
                                        anchors.centerIn: parent
                                        text: {
                                            const _ = repaintTrigger.repaintCount
                                            return (hasData && bridge)
                                                   ? bridge.popup_cell_text(rowIdx, colIdx) : ""
                                        }
                                        color: isSelected ? palette.highlightedText : palette.windowText
                                        font.pixelSize: 14
                                    }

                                    Text {
                                        anchors.right: parent.right
                                        anchors.top: parent.top
                                        anchors.margins: 1
                                        visible: isBookmarked
                                        text: "★"
                                        color: "#f5c518"
                                        font.pixelSize: 8
                                    }

                                    MouseArea {
                                        id: hm
                                        anchors.fill: parent
                                        hoverEnabled: true
                                        cursorShape: hasData ? Qt.PointingHandCursor : Qt.ArrowCursor
                                        acceptedButtons: Qt.LeftButton | Qt.RightButton
                                        onClicked: function(mouse) {
                                            if (!bridge || !hasData) return
                                            if (mouse.button === Qt.RightButton) {
                                                const rows = bridge.popup_rows()
                                                const pageSize = rows * bridge.popup_cols()
                                                const gi = bridge.popup_current_page() * pageSize
                                                    + colIdx * rows + rowIdx
                                                bridge.popup_toggle_bookmark(gi)
                                            } else {
                                                const localIdx = colIdx * bridge.popup_rows() + rowIdx
                                                bridge.popup_select_hanja(localIdx)
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }

            // ─── 푸터: [◀] [page N/M] [▶] ───
            RowLayout {
                Layout.fillWidth: true
                visible: {
                    const _ = repaintTrigger.repaintCount
                    return bridge ? bridge.popup_show_footer() : false
                }
                spacing: 2

                Button {
                    text: "◀"; flat: true; font.pixelSize: 12
                    onClicked: if (bridge) bridge.popup_change_page(-1)
                }
                Text {
                    Layout.fillWidth: true
                    horizontalAlignment: Text.AlignHCenter
                    text: {
                        const _ = repaintTrigger.repaintCount
                        return bridge ? bridge.popup_footer_text() : ""
                    }
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
            const cols = bridge.popup_cols()
            const rows = bridge.popup_rows()
            const isExp = cols > 1

            if (event.key >= Qt.Key_1 && event.key <= Qt.Key_9) {
                if (isExp) {
                    // expanded: 숫자 → 행 선택 (현재 sel_col 기준)
                    const row = event.key - Qt.Key_1
                    const col = bridge.popup_sel_col()
                    bridge.popup_select_hanja(col * rows + row)
                } else {
                    bridge.popup_select_hanja(event.key - Qt.Key_1)
                }
                event.accepted = true
            } else if (event.key === Qt.Key_Tab) {
                bridge.popup_change_page(1); event.accepted = true
            } else if (event.key === Qt.Key_Backtab) {
                bridge.popup_change_page(-1); event.accepted = true
            } else if (event.key === Qt.Key_Left) {
                bridge.popup_change_page(-1); event.accepted = true
            } else if (event.key === Qt.Key_Right) {
                bridge.popup_change_page(1); event.accepted = true
            } else if (event.key === Qt.Key_Period) {
                bridge.popup_toggle_expand(); event.accepted = true
            } else if (event.key === Qt.Key_Space) {
                const pageSize = rows * (cols > 0 ? cols : 1)
                const gi = bridge.popup_current_page() * pageSize
                    + bridge.popup_sel_col() * rows + bridge.popup_sel_row()
                bridge.popup_toggle_bookmark(gi); event.accepted = true
            } else if (event.key === Qt.Key_Escape) {
                bridge.popup_cancel_hanja(); event.accepted = true
            }
        }
    }
}
