import QtQuick
import QtQuick.Controls
import io.github.from104.unim.qt 1.0

Window {
    id: root
    visible: false
    width: 1
    height: 1
    flags: Qt.Tool

    UnimBridge {
        id: bridge

        onMode_changed: function(isKorean) {
            // 모드 변경 시 (로깅 등)
        }
    }
}
