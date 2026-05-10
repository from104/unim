import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.github.from104.unim.qt 1.0

// 일반 페이지: 자판·키맵, 입력 모드, 자동 영문 전환, 모아치기
// 변경 즉시 저장 (Apply 버튼 없음)

ScrollView {
    id: root
    property UnimBridge bridge: null

    clip: true
    contentWidth: availableWidth

    SystemPalette { id: pal; colorGroup: SystemPalette.Active }

    // 한국어 자판 선택지 (JSON 파싱)
    property var korLayouts: []
    property var engLayouts: []

    Component.onCompleted: {
        if (bridge) {
            korLayouts = JSON.parse(bridge.available_korean_layouts())
            engLayouts = JSON.parse(bridge.available_english_layouts())
        }
    }

    function tr(key) { return bridge ? bridge.tr_key(key) : key }

    ColumnLayout {
        width: root.availableWidth
        spacing: 0

        // ─── 그룹: 자판 및 키맵 ───
        GroupBox {
            Layout.fillWidth: true
            Layout.margins: 12
            title: tr("group_layouts_keymap")

            ColumnLayout {
                width: parent.width
                spacing: 8

                // 한국어 자판 선택
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label {
                            text: tr("row_korean_layout")
                            font.pointSize: 10
                        }
                        Label {
                            text: tr("row_korean_layout_subtitle")
                            font.pointSize: 8
                            color: pal.mid
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                    }

                    ComboBox {
                        id: korLayoutCombo
                        Layout.preferredWidth: 200
                        model: {
                            var names = []
                            for (var i = 0; i < root.korLayouts.length; i++) {
                                names.push(root.korLayouts[i].display)
                            }
                            return names
                        }
                        currentIndex: {
                            if (!bridge) return 0
                            var current = bridge.korean_layout
                            for (var i = 0; i < root.korLayouts.length; i++) {
                                if (root.korLayouts[i].display === current ||
                                    root.korLayouts[i].name === current) return i
                            }
                            return 0
                        }
                        ToolTip.visible: hovered
                        ToolTip.text: tr("row_korean_layout_subtitle")

                        onActivated: function(idx) {
                            if (!bridge || idx < 0 || idx >= root.korLayouts.length) return
                            bridge.apply_korean_layout(root.korLayouts[idx].name)
                        }
                    }
                }

                // 영어 자판 선택
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label {
                            text: tr("row_english_layout")
                            font.pointSize: 10
                        }
                        Label {
                            text: tr("row_english_layout_subtitle")
                            font.pointSize: 8
                            color: pal.mid
                            wrapMode: Text.WordWrap
                            Layout.fillWidth: true
                        }
                    }

                    ComboBox {
                        id: engLayoutCombo
                        Layout.preferredWidth: 200
                        model: {
                            var names = []
                            for (var i = 0; i < root.engLayouts.length; i++) {
                                names.push(root.engLayouts[i].display)
                            }
                            return names
                        }
                        currentIndex: {
                            if (!bridge) return 0
                            var current = bridge.english_layout
                            for (var i = 0; i < root.engLayouts.length; i++) {
                                if (root.engLayouts[i].name === current ||
                                    root.engLayouts[i].display === current) return i
                            }
                            return 0
                        }
                        onActivated: function(idx) {
                            if (!bridge || idx < 0 || idx >= root.engLayouts.length) return
                            bridge.apply_english_layout(root.engLayouts[idx].name)
                        }
                    }
                }

                // 한/영 전환 키
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_toggle_keys"); font.pointSize: 10 }
                        Label {
                            text: tr("row_toggle_keys_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }

                    TextField {
                        id: toggleKeysField
                        Layout.preferredWidth: 200
                        text: bridge ? bridge.toggle_keys : ""
                        onEditingFinished: {
                            if (bridge) bridge.apply_toggle_keys(text)
                        }
                        ToolTip.visible: hovered
                        ToolTip.text: tr("row_toggle_keys_subtitle")
                    }
                }

                // 한자 키
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8

                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_hanja_keys"); font.pointSize: 10 }
                        Label {
                            text: tr("row_hanja_keys_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }

                    TextField {
                        id: hanjaKeysField
                        Layout.preferredWidth: 200
                        text: bridge ? bridge.hanja_keys : ""
                        onEditingFinished: {
                            if (bridge) bridge.apply_hanja_keys(text)
                        }
                    }
                }
            }
        }

        // ─── 그룹: 입력 모드 ───
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("group_input_mode")

            ColumnLayout {
                width: parent.width
                spacing: 8

                // 초기 입력 모드
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_initial_mode"); font.pointSize: 10 }
                        Label {
                            text: tr("row_initial_mode_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    ComboBox {
                        Layout.preferredWidth: 160
                        model: [tr("mode_initial_korean"), tr("mode_initial_english")]
                        currentIndex: bridge ? bridge.initial_mode : 1
                        onActivated: function(idx) {
                            if (bridge) bridge.apply_initial_mode(idx)
                        }
                    }
                }

                // 모드 공유 방식
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_mode_sharing"); font.pointSize: 10 }
                        Label {
                            text: tr("row_mode_sharing_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    ComboBox {
                        Layout.preferredWidth: 200
                        model: [tr("mode_share_global"), tr("mode_share_perapp")]
                        currentIndex: bridge ? bridge.mode_sharing : 0
                        onActivated: function(idx) {
                            if (bridge) bridge.apply_mode_sharing(idx)
                        }
                    }
                }
            }
        }

        // ─── 그룹: 자동 영문 전환 ───
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("group_auto_english")

            ColumnLayout {
                width: parent.width
                spacing: 8

                // 사용 여부
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_auto_english_enable"); font.pointSize: 10 }
                        Label {
                            text: tr("row_auto_english_enable_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.auto_english_enabled : false
                        onToggled: { if (bridge) bridge.apply_auto_english_enabled(checked) }
                    }
                }

                // 트리거 키
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_auto_english_triggers"); font.pointSize: 10 }
                        Label {
                            text: tr("row_auto_english_triggers_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    TextField {
                        Layout.preferredWidth: 240
                        text: bridge ? bridge.auto_english_triggers : ""
                        onEditingFinished: {
                            if (bridge) bridge.apply_auto_english_triggers(text)
                        }
                    }
                }
            }
        }

        // ─── 그룹: 모아치기 (chord_window_ms > 0인 자판에서만 의미 있음) ───
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("group_moachigi_title")

            ColumnLayout {
                width: parent.width
                spacing: 12

                // 양방향 자모 결합
                RowLayout {
                    Layout.fillWidth: true
                    spacing: 8
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_moachigi_bidirectional_label"); font.pointSize: 10 }
                        Label {
                            text: tr("row_moachigi_bidirectional_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.moachigi_bidir : false
                        onToggled: { if (bridge) bridge.apply_moachigi_bidir(checked) }
                    }
                }

                // 동시 입력 시간 슬라이더 (0, 50, 100, 150, 200 ms)
                ColumnLayout {
                    Layout.fillWidth: true
                    spacing: 4

                    RowLayout {
                        Layout.fillWidth: true
                        Label { text: tr("row_moachigi_chord_label"); font.pointSize: 10 }
                        Item { Layout.fillWidth: true }
                        Label {
                            text: bridge ? bridge.moachigi_chord_ms + " ms" : "0 ms"
                            font.pointSize: 10; font.bold: true
                        }
                    }
                    Label {
                        text: tr("row_moachigi_chord_subtitle")
                        font.pointSize: 8; color: pal.mid
                        wrapMode: Text.WordWrap; Layout.fillWidth: true
                    }

                    // 슬라이더: 0, 50, 100, 150, 200 스냅
                    Slider {
                        id: chordSlider
                        Layout.fillWidth: true
                        from: 0
                        to: 200
                        stepSize: 50
                        snapMode: Slider.SnapAlways
                        value: bridge ? bridge.moachigi_chord_ms : 0

                        onMoved: {
                            if (bridge) bridge.apply_moachigi_chord_ms(Math.round(value / 50) * 50)
                        }

                        // tick 마크 (0, 50, 100, 150, 200)
                        background: Rectangle {
                            x: chordSlider.leftPadding
                            y: chordSlider.topPadding + chordSlider.availableHeight / 2 - height / 2
                            implicitWidth: 200; implicitHeight: 4
                            width: chordSlider.availableWidth; height: implicitHeight
                            radius: 2; color: pal.midlight

                            Rectangle {
                                width: chordSlider.visualPosition * parent.width
                                height: parent.height; radius: 2; color: pal.highlight
                            }

                            // tick 마크
                            Repeater {
                                model: [0, 50, 100, 150, 200]
                                Rectangle {
                                    x: modelData / 200 * (parent.width - 2)
                                    y: parent.height + 4
                                    width: 1; height: 6; color: pal.mid
                                    Label {
                                        anchors.horizontalCenter: parent.horizontalCenter
                                        anchors.top: parent.bottom
                                        anchors.topMargin: 2
                                        text: modelData === 0 ? "OFF" : modelData
                                        font.pointSize: 7; color: pal.mid
                                    }
                                }
                            }
                        }
                    }

                    // tick 라벨 여백
                    Item { Layout.fillWidth: true; height: 20 }
                }
            }
        }

        // 하단 여백
        Item { Layout.fillWidth: true; height: 16 }
    }
}
