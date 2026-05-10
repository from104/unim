import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.github.from104.unim.qt 1.0

// 오타 교정 페이지
// AutoTypeFix 마스터, 순방향(EN→KO), 역방향(KO→EN) 설정
// 수치는 슬라이더 + tick 마크 (SpinBox 금지)

ScrollView {
    id: root
    property UnimBridge bridge: null

    clip: true
    contentWidth: availableWidth

    SystemPalette { id: pal; colorGroup: SystemPalette.Active }

    function tr(key) { return bridge ? bridge.tr_key(key) : key }

    // 정수 슬라이더 컴포넌트 (min~max, step=1)
    component IntSlider: ColumnLayout {
        id: intSliderRoot
        property string label: ""
        property string subtitle: ""
        property int sliderMin: 1
        property int sliderMax: 10
        property int sliderValue: sliderMin
        signal sliderMoved(int val)

        width: parent ? parent.width : 300
        spacing: 4

        RowLayout {
            Layout.fillWidth: true
            Label { text: intSliderRoot.label; font.pointSize: 10 }
            Item { Layout.fillWidth: true }
            Label {
                text: intSliderRoot.sliderValue.toString()
                font.pointSize: 10; font.bold: true
            }
        }
        Label {
            text: intSliderRoot.subtitle
            font.pointSize: 8; color: pal.mid
            wrapMode: Text.WordWrap; Layout.fillWidth: true
            visible: intSliderRoot.subtitle !== ""
        }
        Slider {
            Layout.fillWidth: true
            from: intSliderRoot.sliderMin
            to: intSliderRoot.sliderMax
            stepSize: 1
            snapMode: Slider.SnapAlways
            value: intSliderRoot.sliderValue
            onMoved: intSliderRoot.sliderMoved(Math.round(value))

            background: Rectangle {
                x: parent.leftPadding
                y: parent.topPadding + parent.availableHeight / 2 - height / 2
                width: parent.availableWidth; height: 4; radius: 2; color: pal.midlight
                Rectangle {
                    width: parent.parent.visualPosition * parent.width
                    height: parent.height; radius: 2; color: pal.highlight
                }
                // tick 마크: min과 max
                Rectangle {
                    x: 0; y: parent.height + 4; width: 1; height: 6; color: pal.mid
                    Label {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.top: parent.bottom; anchors.topMargin: 2
                        text: intSliderRoot.sliderMin.toString()
                        font.pointSize: 7; color: pal.mid
                    }
                }
                Rectangle {
                    x: parent.width - 1; y: parent.height + 4; width: 1; height: 6; color: pal.mid
                    Label {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.top: parent.bottom; anchors.topMargin: 2
                        text: intSliderRoot.sliderMax.toString()
                        font.pointSize: 7; color: pal.mid
                    }
                }
            }
        }
        Item { height: 18 } // tick 라벨 여백
    }

    // 시간 윈도우 슬라이더 컴포넌트 (ms 단위, 0.5초 step, 표시는 초)
    component TimeSlider: ColumnLayout {
        id: timeSliderRoot
        property string label: ""
        property string subtitle: ""
        // ms 단위 — AUTO_TYPEFIX_TIME_WINDOW_MIN=500, MAX=5000
        property int timeMin: 500
        property int timeMax: 5000
        property int timeValue: 5000
        signal timeMoved(int ms)

        width: parent ? parent.width : 300
        spacing: 4

        RowLayout {
            Layout.fillWidth: true
            Label { text: timeSliderRoot.label; font.pointSize: 10 }
            Item { Layout.fillWidth: true }
            Label {
                text: (timeSliderRoot.timeValue / 1000.0).toFixed(1) + " s"
                font.pointSize: 10; font.bold: true
            }
        }
        Label {
            text: timeSliderRoot.subtitle
            font.pointSize: 8; color: pal.mid
            wrapMode: Text.WordWrap; Layout.fillWidth: true
            visible: timeSliderRoot.subtitle !== ""
        }
        Slider {
            Layout.fillWidth: true
            from: timeSliderRoot.timeMin / 1000.0
            to: timeSliderRoot.timeMax / 1000.0
            stepSize: 0.5
            snapMode: Slider.SnapAlways
            value: timeSliderRoot.timeValue / 1000.0
            onMoved: {
                var snapped = Math.round(value * 2) / 2  // 0.5 단위
                timeSliderRoot.timeMoved(Math.round(snapped * 1000))
            }

            background: Rectangle {
                x: parent.leftPadding
                y: parent.topPadding + parent.availableHeight / 2 - height / 2
                width: parent.availableWidth; height: 4; radius: 2; color: pal.midlight
                Rectangle {
                    width: parent.parent.visualPosition * parent.width
                    height: parent.height; radius: 2; color: pal.highlight
                }
                Rectangle {
                    x: 0; y: parent.height + 4; width: 1; height: 6; color: pal.mid
                    Label {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.top: parent.bottom; anchors.topMargin: 2
                        text: (timeSliderRoot.timeMin / 1000.0).toFixed(1) + "s"
                        font.pointSize: 7; color: pal.mid
                    }
                }
                Rectangle {
                    x: parent.width - 1; y: parent.height + 4; width: 1; height: 6; color: pal.mid
                    Label {
                        anchors.horizontalCenter: parent.horizontalCenter
                        anchors.top: parent.bottom; anchors.topMargin: 2
                        text: (timeSliderRoot.timeMax / 1000.0).toFixed(1) + "s"
                        font.pointSize: 7; color: pal.mid
                    }
                }
            }
        }
        Item { height: 18 }
    }

    ColumnLayout {
        width: root.availableWidth
        spacing: 0

        // ─── 그룹: 마스터 ───
        GroupBox {
            Layout.fillWidth: true
            Layout.margins: 12
            title: tr("group_typefix_master")

            ColumnLayout {
                width: parent.width
                spacing: 12

                // 마스터 ON/OFF
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_typefix_master"); font.pointSize: 10; font.bold: true }
                        Label {
                            text: tr("row_typefix_master_row_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.autotypefix_enabled : true
                        onToggled: { if (bridge) bridge.apply_autotypefix_enabled(checked) }
                    }
                }

                // 재트리거 자동 감지
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("group_typefix_retrigger"); font.pointSize: 10 }
                        Label {
                            text: tr("row_typefix_retrigger_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.autotypefix_rollback : true
                        onToggled: { if (bridge) bridge.apply_autotypefix_rollback(checked) }
                    }
                }

                // 관찰 창 (초) — 5~15
                IntSlider {
                    Layout.fillWidth: true
                    label: tr("row_typefix_observe_window")
                    subtitle: tr("row_typefix_observe_window_subtitle")
                    sliderMin: 5; sliderMax: 15
                    sliderValue: bridge ? bridge.autotypefix_obs_secs : 10
                    onSliderMoved: function(v) { if (bridge) bridge.apply_autotypefix_obs_secs(v) }
                }

                // 임시 억제 만료 (시간) — 1~12
                IntSlider {
                    Layout.fillWidth: true
                    label: tr("row_typefix_tentative_expiry")
                    subtitle: tr("row_typefix_tentative_expiry_subtitle")
                    sliderMin: 1; sliderMax: 12
                    sliderValue: bridge ? bridge.autotypefix_expiry_hours : 4
                    onSliderMoved: function(v) { if (bridge) bridge.apply_autotypefix_expiry_hours(v) }
                }
            }
        }

        // ─── 그룹: 순방향 교정 (EN→KO) ───
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("group_typefix_forward")

            ColumnLayout {
                width: parent.width
                spacing: 12

                // 사용
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_typefix_enable"); font.pointSize: 10 }
                        Label {
                            text: tr("row_typefix_forward_enable_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.fwd_enabled : true
                        onToggled: { if (bridge) bridge.apply_fwd_enabled(checked) }
                    }
                }

                // 임계 음절 수 — 2~6
                IntSlider {
                    Layout.fillWidth: true
                    label: tr("row_typefix_kor_threshold")
                    subtitle: tr("row_typefix_kor_threshold_subtitle")
                    sliderMin: 2; sliderMax: 6
                    sliderValue: bridge ? bridge.fwd_kor_threshold : 2
                    onSliderMoved: function(v) { if (bridge) bridge.apply_fwd_kor_threshold(v) }
                }

                // 트리거 윈도우 (초) — 0.5~5.0
                TimeSlider {
                    Layout.fillWidth: true
                    label: tr("row_typefix_forward_window")
                    subtitle: tr("row_typefix_forward_window_subtitle")
                    timeMin: 500; timeMax: 5000
                    timeValue: bridge ? bridge.fwd_window_ms : 5000
                    onTimeMoved: function(ms) { if (bridge) bridge.apply_fwd_window_ms(ms) }
                }

                // 영단어 매칭 시 억제
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_typefix_skip_engword"); font.pointSize: 10 }
                        Label {
                            text: tr("row_typefix_skip_engword_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.fwd_skip_engword : true
                        onToggled: { if (bridge) bridge.apply_fwd_skip_engword(checked) }
                    }
                }
            }
        }

        // ─── 그룹: 역방향 교정 (KO→EN) ───
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12; Layout.bottomMargin: 12
            title: tr("group_typefix_reverse")

            ColumnLayout {
                width: parent.width
                spacing: 12

                // 사용
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_typefix_enable"); font.pointSize: 10 }
                        Label {
                            text: tr("row_typefix_reverse_enable_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.rev_enabled : true
                        onToggled: { if (bridge) bridge.apply_rev_enabled(checked) }
                    }
                }

                // 임계 글자 수 — 3~8
                IntSlider {
                    Layout.fillWidth: true
                    label: tr("row_typefix_eng_threshold")
                    subtitle: tr("row_typefix_eng_threshold_subtitle")
                    sliderMin: 3; sliderMax: 8
                    sliderValue: bridge ? bridge.rev_eng_threshold : 5
                    onSliderMoved: function(v) { if (bridge) bridge.apply_rev_eng_threshold(v) }
                }

                // 트리거 윈도우 (초) — 0.5~5.0
                TimeSlider {
                    Layout.fillWidth: true
                    label: tr("row_typefix_forward_window")
                    subtitle: tr("row_typefix_reverse_window_subtitle")
                    timeMin: 500; timeMax: 5000
                    timeValue: bridge ? bridge.rev_window_ms : 5000
                    onTimeMoved: function(ms) { if (bridge) bridge.apply_rev_window_ms(ms) }
                }

                // 온전한 음절 매칭 시 억제
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_typefix_skip_complete"); font.pointSize: 10 }
                        Label {
                            text: tr("row_typefix_skip_complete_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.rev_skip_complete : true
                        onToggled: { if (bridge) bridge.apply_rev_skip_complete(checked) }
                    }
                }

                // 사용자 사전 사용
                RowLayout {
                    Layout.fillWidth: true
                    ColumnLayout {
                        Layout.fillWidth: true
                        spacing: 2
                        Label { text: tr("row_typefix_userdict"); font.pointSize: 10 }
                        Label {
                            text: tr("row_typefix_userdict_subtitle")
                            font.pointSize: 8; color: pal.mid
                            wrapMode: Text.WordWrap; Layout.fillWidth: true
                        }
                    }
                    Switch {
                        checked: bridge ? bridge.rev_userdict_enabled : true
                        onToggled: { if (bridge) bridge.apply_rev_userdict_enabled(checked) }
                    }
                }
            }
        }

        Item { Layout.fillWidth: true; height: 16 }
    }
}
