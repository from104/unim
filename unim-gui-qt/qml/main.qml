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

        onShow_hanja_popup: function(target, candidatesJson, cx, cy, cw, ch) {
            hanjaPopup.showPopup(target, candidatesJson, cx, cy, cw, ch)
        }

        onShow_special_popup: function(target, charsJson, topRow, cx, cy, cw, ch) {
            specialPopup.showPopup(target, charsJson, topRow, cx, cy, cw, ch)
        }

        onHide_popup: function() {
            hanjaPopup.hidePopup()
            specialPopup.hidePopup()
        }

        onMode_changed: function(isKorean) {
            // 모드 변경 시 (로깅 등)
        }
    }

    HanjaPopup {
        id: hanjaPopup
        bridge: bridge
    }

    SpecialPopup {
        id: specialPopup
        bridge: bridge
    }
}
