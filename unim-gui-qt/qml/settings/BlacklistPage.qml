import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.github.from104.unim.qt 1.0

// 교정 억제 단어 페이지
// TODO Phase 3b: Blacklist CRUD (BlacklistEntry 직접 조회 invokable 필요)
// 현재: 안내 메시지 + 재로드 버튼만 제공. 항목 편집은 CLI 또는 GTK UI 사용.

ScrollView {
    id: root
    property UnimBridge bridge: null

    clip: true
    contentWidth: availableWidth

    SystemPalette { id: pal; colorGroup: SystemPalette.Active }
    function tr(key) { return bridge ? bridge.tr_key(key) : key }

    ColumnLayout {
        width: root.availableWidth
        spacing: 0

        // 안내 배너
        Rectangle {
            Layout.fillWidth: true
            Layout.margins: 12
            height: noteLayout.implicitHeight + 24
            color: Qt.rgba(pal.highlight.r, pal.highlight.g, pal.highlight.b, 0.12)
            radius: 6
            border.color: Qt.rgba(pal.highlight.r, pal.highlight.g, pal.highlight.b, 0.4)
            border.width: 1

            ColumnLayout {
                id: noteLayout
                anchors { left: parent.left; right: parent.right; top: parent.top; margins: 12 }
                spacing: 6

                Label {
                    text: tr("page_blacklist_title")
                    font.pointSize: 12; font.bold: true
                    color: pal.windowText
                }
                Label {
                    text: {
                        var ko = "AutoTypeFix가 자동으로 감지한 교정 억제 단어 목록입니다.\n" +
                                 "임시(Pending) → 확정(Confirmed) → 비활성(Inactive) 상태로 관리됩니다.\n\n" +
                                 "Phase 3b에서 Qt 전용 CRUD UI가 추가될 예정입니다.\n" +
                                 "현재는 GTK 설정 UI(unim-gui-gtk --settings) 또는 CLI를 사용하세요."
                        var en = "AutoTypeFix-detected suppression words.\n" +
                                 "States: Pending → Confirmed → Inactive.\n\n" +
                                 "Phase 3b will add Qt-native CRUD UI.\n" +
                                 "For now, use GTK settings (unim-gui-gtk --settings) or CLI."
                        return bridge ? ko : en
                    }
                    font.pointSize: 9
                    color: pal.windowText
                    wrapMode: Text.WordWrap
                    Layout.fillWidth: true
                }
            }
        }

        // 상태 그룹들 (읽기 전용 레이블)
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("blacklist_group_tentative")
            Label {
                text: tr("blacklist_tentative_desc").replace("%{count}", "?")
                font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
                width: parent.width
            }
        }

        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("blacklist_group_confirmed")
            Label {
                text: tr("blacklist_confirmed_desc").replace("%{count}", "?")
                font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
                width: parent.width
            }
        }

        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("blacklist_group_inactive")
            Label {
                text: tr("blacklist_inactive_desc").replace("%{count}", "?")
                font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
                width: parent.width
            }
        }

        // TODO Phase 3b 표시
        Label {
            Layout.fillWidth: true
            Layout.margins: 12
            text: "TODO Phase 3b: BlacklistEntry invokable 추가 후 CRUD UI 구현"
            font.pointSize: 8; color: pal.mid; font.italic: true
            wrapMode: Text.WordWrap
        }

        Item { Layout.fillWidth: true; height: 16 }
    }
}
