import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.github.from104.unim.qt 1.0

// 사용자 사전 페이지
// TODO Phase 3b: UserDictionary CRUD invokable 추가 후 완전 구현
// 현재: 안내 메시지 제공, 편집은 GTK UI 또는 CLI

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
                    text: tr("page_userdict_title")
                    font.pointSize: 12; font.bold: true; color: pal.windowText
                }
                Label {
                    text: tr("userdict_group_desc")
                    font.pointSize: 9; color: pal.windowText
                    wrapMode: Text.WordWrap; Layout.fillWidth: true
                }
                Label {
                    text: {
                        var ko = "\nPhase 3b에서 Qt 전용 CRUD UI가 추가될 예정입니다.\n" +
                                 "현재는 GTK 설정 UI(unim-gui-gtk --settings) 또는 CLI를 사용하세요."
                        var en = "\nPhase 3b will add Qt-native CRUD UI.\n" +
                                 "For now, use GTK settings (unim-gui-gtk --settings) or CLI."
                        return bridge ? ko : en
                    }
                    font.pointSize: 9; color: pal.mid
                    wrapMode: Text.WordWrap; Layout.fillWidth: true
                }
            }
        }

        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("userdict_group_title")

            Label {
                text: tr("userdict_empty")
                font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
                width: parent.width
            }
        }

        // TODO Phase 3b 표시
        Label {
            Layout.fillWidth: true
            Layout.margins: 12
            text: "TODO Phase 3b: UserDictionary invokable 추가 후 CRUD UI 구현"
            font.pointSize: 8; color: pal.mid; font.italic: true
            wrapMode: Text.WordWrap
        }

        Item { Layout.fillWidth: true; height: 16 }
    }
}
