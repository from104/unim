import QtQuick
import QtQuick.Controls
import io.github.from104.unim.qt 1.0

Window {
    id: root
    // i18n: title은 Rust bridge의 window_title()로 채움 (LANG 기반)
    title: bridge.window_title()
    visible: false
    width: 1
    height: 1
    flags: Qt.Tool

    UnimBridge {
        id: bridge

        onMode_changed: function(isKorean) {
            // 모드 변경 시 (로깅 등)
            // qsTr 마크업: 향후 Qt linguist 도입 시 사용
            // qsTr("Typing Korean"), qsTr("Typing English")
        }
    }
}
