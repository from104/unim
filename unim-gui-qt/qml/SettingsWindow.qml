import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.github.from104.unim.qt 1.0

// UNIM 설정 창 — Phase 3
// 좌측: 페이지 선택 ListView (NavigationDrawer 스타일)
// 우측: StackLayout (페이지 콘텐츠)
// KDE Breeze/시스템 테마 자동 추종 (SystemPalette 사용)
// Apply 버튼 없이 변경 즉시 저장

ApplicationWindow {
    id: settingsWindow

    property UnimBridge bridge: null

    title: bridge ? bridge.tr_key("settings_window_title") : "UNIM Settings"
    width: 760
    height: 580
    minimumWidth: 660
    minimumHeight: 480
    visible: false

    // 시스템 팔레트 (KDE Breeze / 기타 테마 자동 추종)
    SystemPalette { id: sysPalette; colorGroup: SystemPalette.Active }
    SystemPalette { id: sysPaletteDisabled; colorGroup: SystemPalette.Disabled }

    // 저장 토스트 알림 (하단 SnackBar)
    Popup {
        id: toastPopup
        x: (parent.width - width) / 2
        y: parent.height - height - 16
        width: toastLabel.implicitWidth + 32
        height: 40
        closePolicy: Popup.NoAutoClose

        background: Rectangle {
            color: sysPalette.highlight
            radius: 6
        }
        Label {
            id: toastLabel
            anchors.centerIn: parent
            color: sysPalette.highlightedText
            font.pointSize: 10
        }

        Timer {
            id: toastTimer
            interval: 2000
            onTriggered: toastPopup.close()
        }
    }

    function showToast(msg) {
        toastLabel.text = msg
        toastPopup.open()
        toastTimer.restart()
    }

    // bridge 연결: toast_message 시그널 수신
    Connections {
        target: settingsWindow.bridge
        function onToast_message(text) {
            settingsWindow.showToast(text)
        }
        function onConfig_reloaded() {
            // 페이지들이 bridge 프로퍼티를 바인딩하므로 자동 갱신됨
        }
    }

    // ─── 페이지 정의 ───
    // 각 항목: { title: string, component: Component }
    // GnomePage는 is_gnome_session() 조건부

    property var pages: []

    Component.onCompleted: {
        var p = [
            { title: bridge ? bridge.tr_key("page_general_title") : "General",
              source: "settings/GeneralPage.qml" },
            { title: bridge ? bridge.tr_key("page_typefix_title") : "Auto TypeFix",
              source: "settings/AutoTypeFixPage.qml" },
            { title: bridge ? bridge.tr_key("page_blacklist_title") : "Suppression List",
              source: "settings/BlacklistPage.qml" },
            { title: bridge ? bridge.tr_key("page_userdict_title") : "User Dictionary",
              source: "settings/UserDictPage.qml" },
        ]
        // GNOME 세션에서만 GNOME 페이지 표시
        if (bridge && bridge.is_gnome_session()) {
            p.push({
                title: bridge.tr_key("page_gnome_title"),
                source: "settings/GnomePage.qml"
            })
        }
        pages = p
        // 페이지 동적 로드
        for (var i = 0; i < p.length; i++) {
            var comp = Qt.createComponent(p[i].source)
            if (comp.status === Component.Ready) {
                var obj = comp.createObject(pageStack, { bridge: settingsWindow.bridge })
                pageItems.push(obj)
            }
        }
        pageList.currentIndex = 0
    }

    property var pageItems: []

    // ─── 레이아웃 ───
    RowLayout {
        anchors.fill: parent
        spacing: 0

        // 좌측 사이드바
        Rectangle {
            id: sidebar
            Layout.preferredWidth: 180
            Layout.fillHeight: true
            color: sysPalette.window

            // 구분선
            Rectangle {
                anchors.right: parent.right
                anchors.top: parent.top
                anchors.bottom: parent.bottom
                width: 1
                color: sysPalette.mid
            }

            ListView {
                id: pageList
                anchors.fill: parent
                anchors.topMargin: 8
                anchors.bottomMargin: 8
                model: settingsWindow.pages
                currentIndex: 0
                clip: true

                delegate: ItemDelegate {
                    width: pageList.width
                    height: 42
                    highlighted: pageList.currentIndex === index

                    background: Rectangle {
                        color: highlighted ? sysPalette.highlight : (hovered ? sysPalette.midlight : "transparent")
                        radius: 4
                        anchors.margins: 3
                    }

                    contentItem: Label {
                        text: modelData.title
                        color: highlighted ? sysPalette.highlightedText : sysPalette.windowText
                        font.pointSize: 10
                        leftPadding: 12
                        verticalAlignment: Text.AlignVCenter
                    }

                    onClicked: {
                        pageList.currentIndex = index
                        pageStack.currentIndex = index
                    }
                }

                onCurrentIndexChanged: {
                    pageStack.currentIndex = currentIndex
                }
            }
        }

        // 우측 페이지 스택
        StackLayout {
            id: pageStack
            Layout.fillWidth: true
            Layout.fillHeight: true
            currentIndex: pageList.currentIndex
            // 페이지 항목들은 Component.onCompleted에서 동적으로 추가됨
        }
    }

    // 닫기 시 설정 재로드 (외부에서 변경됐을 수 있으므로)
    onVisibilityChanged: {
        if (visible && bridge) {
            bridge.reload_config()
        }
    }
}
