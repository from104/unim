import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.github.from104.unim.qt 1.0

// 교정 억제 단어 페이지 — Phase 5-A 완전 구현
// 세 그룹(임시/확정/비활성) 각각 CRUD 버튼 포함

ScrollView {
    id: root
    property UnimBridge bridge: null

    clip: true
    contentWidth: availableWidth

    SystemPalette { id: pal; colorGroup: SystemPalette.Active }
    function tr(key) { return bridge ? bridge.tr_key(key) : key }

    // 세 그룹 모델 (status별 전역 idx 목록)
    property var tentativeIndices: []
    property var confirmedIndices: []
    property var inactiveIndices:  []

    function refreshAll() {
        if (!bridge) return
        tentativeIndices = JSON.parse(bridge.blacklist_indices_by_status(0))
        confirmedIndices = JSON.parse(bridge.blacklist_indices_by_status(1))
        inactiveIndices  = JSON.parse(bridge.blacklist_indices_by_status(2))
    }

    Component.onCompleted: refreshAll()

    // bridge 시그널 연결 — 변경 시 자동 갱신
    Connections {
        target: bridge
        function onBlacklistChanged() { refreshAll() }
    }

    ColumnLayout {
        width: root.availableWidth
        spacing: 8

        // 제목
        Label {
            Layout.fillWidth: true
            Layout.margins: 12
            text: tr("page_blacklist_title")
            font.pointSize: 12; font.bold: true
            color: pal.windowText
        }

        // 설명 한 줄
        Label {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12
            text: "AutoTypeFix가 자동 감지한 교정 억제 단어 목록. 확정하면 영구 억제됩니다."
            font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
        }

        // ── 임시 억제 그룹 ──────────────────────────────────────────
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12
            title: tr("blacklist_group_tentative") + " (" + tentativeIndices.length + ")"

            ColumnLayout {
                anchors.left: parent.left
                anchors.right: parent.right
                spacing: 0

                Repeater {
                    model: tentativeIndices
                    delegate: BlacklistEntryRow {
                        Layout.fillWidth: true
                        theBridge: root.bridge
                        globalIdx: modelData
                        showPromote:    true
                        showDeactivate: false
                        showReactivate: false
                    }
                }

                Label {
                    visible: tentativeIndices.length === 0
                    text: tr("blacklist_empty")
                    font.pointSize: 9; color: pal.mid
                    leftPadding: 4; topPadding: 2; bottomPadding: 2
                }
            }
        }

        // ── 확정 억제 그룹 ──────────────────────────────────────────
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12
            title: tr("blacklist_group_confirmed") + " (" + confirmedIndices.length + ")"

            ColumnLayout {
                anchors.left: parent.left
                anchors.right: parent.right
                spacing: 0

                Repeater {
                    model: confirmedIndices
                    delegate: BlacklistEntryRow {
                        Layout.fillWidth: true
                        theBridge: root.bridge
                        globalIdx: modelData
                        showPromote:    false
                        showDeactivate: true
                        showReactivate: false
                    }
                }

                Label {
                    visible: confirmedIndices.length === 0
                    text: tr("blacklist_empty")
                    font.pointSize: 9; color: pal.mid
                    leftPadding: 4; topPadding: 2; bottomPadding: 2
                }
            }
        }

        // ── 비활성 그룹 ─────────────────────────────────────────────
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12
            title: tr("blacklist_group_inactive") + " (" + inactiveIndices.length + ")"

            ColumnLayout {
                anchors.left: parent.left
                anchors.right: parent.right
                spacing: 0

                Repeater {
                    model: inactiveIndices
                    delegate: BlacklistEntryRow {
                        Layout.fillWidth: true
                        theBridge: root.bridge
                        globalIdx: modelData
                        showPromote:    false
                        showDeactivate: false
                        showReactivate: true
                    }
                }

                Label {
                    visible: inactiveIndices.length === 0
                    text: tr("blacklist_empty")
                    font.pointSize: 9; color: pal.mid
                    leftPadding: 4; topPadding: 2; bottomPadding: 2
                }
            }
        }

        // 새로고침 버튼
        Button {
            Layout.leftMargin: 12
            text: tr("blacklist_reload_button")
            onClicked: { if (bridge) bridge.blacklist_reload() }
        }

        Item { Layout.fillWidth: true; height: 16 }
    }

    // ── 인라인 컴포넌트: 행 하나 ───────────────────────────────────
    component BlacklistEntryRow: RowLayout {
        id: entryRow
        required property var theBridge
        required property int globalIdx
        property bool showPromote:    false
        property bool showDeactivate: false
        property bool showReactivate: false

        spacing: 4
        height: 34

        // ASCII 텍스트 + 방향 + hits
        Label {
            Layout.fillWidth: true
            text: {
                if (!theBridge) return ""
                var ascii = theBridge.blacklist_entry_ascii(globalIdx)
                var dir   = theBridge.blacklist_entry_direction(globalIdx)
                var hits  = theBridge.blacklist_entry_hits(globalIdx)
                var dirTxt = dir === 0 ? "→한" : "→영"
                return ascii + "  [" + dirTxt + " × " + hits + "]"
            }
            font.family: "monospace"
            elide: Text.ElideRight
            verticalAlignment: Text.AlignVCenter
        }

        // 승격 버튼 (Tentative → Confirmed)
        Button {
            visible: showPromote
            text: "✓"
            flat: true; implicitWidth: 32; implicitHeight: 28
            ToolTip.text: theBridge ? theBridge.tr_key("blacklist_btn_confirm") : ""
            ToolTip.visible: hovered
            onClicked: { if (theBridge) theBridge.blacklist_promote(globalIdx) }
        }

        // 비활성화 버튼 (Confirmed → Inactive)
        Button {
            visible: showDeactivate
            text: "⏸"
            flat: true; implicitWidth: 32; implicitHeight: 28
            ToolTip.text: theBridge ? theBridge.tr_key("blacklist_btn_disable") : ""
            ToolTip.visible: hovered
            onClicked: { if (theBridge) theBridge.blacklist_deactivate(globalIdx) }
        }

        // 재활성화 버튼 (Inactive → Confirmed)
        Button {
            visible: showReactivate
            text: "▶"
            flat: true; implicitWidth: 32; implicitHeight: 28
            ToolTip.text: theBridge ? theBridge.tr_key("blacklist_btn_reactivate") : ""
            ToolTip.visible: hovered
            onClicked: { if (theBridge) theBridge.blacklist_reactivate(globalIdx) }
        }

        // 삭제 버튼 (항상)
        Button {
            text: "✕"
            flat: true; implicitWidth: 32; implicitHeight: 28
            ToolTip.text: theBridge ? theBridge.tr_key("blacklist_btn_delete") : ""
            ToolTip.visible: hovered
            onClicked: { if (theBridge) theBridge.blacklist_remove(globalIdx) }
        }
    }
}
