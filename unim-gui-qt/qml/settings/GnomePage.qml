import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.github.from104.unim.qt 1.0

// GNOME Shell 전용 페이지
// is_gnome_session() == true 일 때만 SettingsWindow에서 페이지 목록에 추가됨
// GSettings(gschema) 직접 읽기/쓰기는 Rust bridge에서 처리할 예정 (Phase 3b)
// 현재: 안내 메시지 + 상태 표시

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
                    text: tr("page_gnome_title")
                    font.pointSize: 12; font.bold: true; color: pal.windowText
                }
                Label {
                    text: {
                        var ko = "GNOME Shell 확장 전용 설정입니다.\n" +
                                 "이 페이지는 GNOME 세션에서만 표시됩니다.\n\n" +
                                 "Phase 3b에서 GSettings 연동 UI가 추가될 예정입니다.\n" +
                                 "현재는 GTK 설정 UI(unim-gui-gtk --settings)를 사용하세요."
                        var en = "GNOME Shell extension settings.\n" +
                                 "This page is only shown in GNOME sessions.\n\n" +
                                 "Phase 3b will add GSettings integration.\n" +
                                 "For now, use the GTK settings UI (unim-gui-gtk --settings)."
                        return bridge ? ko : en
                    }
                    font.pointSize: 9; color: pal.windowText
                    wrapMode: Text.WordWrap; Layout.fillWidth: true
                }
            }
        }

        // 표시 그룹 (향후 GSettings 바인딩 대상)
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("group_display")

            ColumnLayout {
                width: parent.width
                spacing: 8

                Label {
                    text: tr("row_panel_indicator") + " — " + tr("row_panel_indicator_subtitle")
                    font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
                    width: parent.width
                }
                Label {
                    text: tr("row_show_notification") + " — " + tr("row_show_notification_subtitle")
                    font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
                    width: parent.width
                }
            }
        }

        // IME 그룹
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("group_ime")

            ColumnLayout {
                width: parent.width
                spacing: 4

                Label {
                    text: tr("group_ime_subtitle")
                    font.pointSize: 8; color: pal.mid; wrapMode: Text.WordWrap
                    width: parent.width
                }
                Label {
                    text: tr("row_ime_enable") + " — " + tr("row_ime_enable_subtitle")
                    font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
                    width: parent.width
                }
                Label {
                    text: bridge && bridge.is_wayland_session() ?
                          "(Wayland 세션 감지됨 — IME 설정 활성)" :
                          "(X11 세션 — IME 옵션 비활성)"
                    font.pointSize: 8; color: bridge && bridge.is_wayland_session() ? pal.highlight : pal.mid
                    wrapMode: Text.WordWrap; width: parent.width
                }
            }
        }

        // TODO Phase 3b 표시
        Label {
            Layout.fillWidth: true; Layout.margins: 12
            text: "TODO Phase 3b: GSettings(org.gnome.shell.extensions.unim) 바인딩 구현"
            font.pointSize: 8; color: pal.mid; font.italic: true; wrapMode: Text.WordWrap
        }

        Item { Layout.fillWidth: true; height: 16 }
    }
}
