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
        }

        onPopup_show: function() {
            // popup_kind에 따라 해당 팝업 포커스 설정
        }

        onPopup_hide: function() {
            // 모든 팝업 숨김은 visible 바인딩이 처리
        }
    }

    // ─── 한자 팝업 ───
    HanjaPopup {
        id: hanjaPopup
        visible: bridge.popup_visible && bridge.popup_kind === 0
        x: bridge.popup_x
        y: bridge.popup_y
        width: 300
        height: bridge.popup_expand_visible() ? 200 : 380
        bridge: bridge

        onVisibleChanged: {
            if (!visible && bridge.popup_kind === 0) {
                // 외부 클릭 등 비정상 닫힘 → cancel
                bridge.popup_cancel_hanja()
            }
        }
    }

    // ─── 특수문자 팝업 ───
    SpecialPopup {
        id: specialPopup
        visible: bridge.popup_visible && bridge.popup_kind === 1
        x: bridge.popup_x
        y: bridge.popup_y
        width: 340
        height: 310
        bridge: bridge

        onVisibleChanged: {
            if (!visible && bridge.popup_kind === 1) {
                bridge.popup_cancel_special()
            }
        }
    }

    // ─── 이모지 팝업 ───
    EmojiPopup {
        id: emojiPopup
        visible: bridge.popup_visible && bridge.popup_kind === 2
        x: bridge.popup_x
        y: bridge.popup_y
        width: 380
        height: 360
        bridge: bridge
    }
}
