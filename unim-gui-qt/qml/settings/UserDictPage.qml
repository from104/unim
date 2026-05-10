import QtQuick
import QtQuick.Controls
import QtQuick.Layouts
import io.github.from104.unim.qt 1.0

// 사용자 사전 페이지 — Phase 5-A 완전 구현
// 단일 ListView + 추가/편집 다이얼로그

ScrollView {
    id: root
    property UnimBridge bridge: null

    clip: true
    contentWidth: availableWidth

    SystemPalette { id: pal; colorGroup: SystemPalette.Active }
    function tr(key) { return bridge ? bridge.tr_key(key) : key }

    // 단어 목록 (idx 배열로 관리)
    property var wordIndices: []

    function refreshList() {
        if (!bridge) return
        var cnt = bridge.userdict_count()
        var arr = []
        for (var i = 0; i < cnt; i++) arr.push(i)
        wordIndices = arr
    }

    Component.onCompleted: refreshList()

    Connections {
        target: bridge
        function onUserdictChanged() { refreshList() }
    }

    // ── 편집 다이얼로그 ──────────────────────────────────────────────
    Dialog {
        id: editDialog
        title: editMode ? tr("userdict_edit_dialog_title_edit") : tr("userdict_edit_dialog_title_add")
        modal: true
        anchors.centerIn: Overlay.overlay
        width: 360

        property bool editMode: false
        property int  editIdx:  -1
        property string errorText: ""

        standardButtons: Dialog.Ok | Dialog.Cancel

        onOpened: {
            wordField.forceActiveFocus()
            errorLabel.visible = false
            errorText = ""
        }

        onAccepted: {
            var w = wordField.text.trim()
            var n = noteField.text.trim()
            var err = ""
            if (editMode) {
                err = bridge ? bridge.userdict_update(editIdx, w, n) : ""
            } else {
                err = bridge ? bridge.userdict_add(w, n) : ""
            }
            if (err !== "") {
                errorText = err
                errorLabel.visible = true
                // 다이얼로그 닫히지 않도록 — accept 이미 실행됨, 재열기
                Qt.callLater(function() { editDialog.open() })
            }
        }

        ColumnLayout {
            anchors.fill: parent
            spacing: 8

            Label { text: tr("userdict_edit_dialog_word_label"); font.bold: true }
            TextField {
                id: wordField
                Layout.fillWidth: true
                placeholderText: "git, kubectl, vim …"
                onTextChanged: {
                    if (bridge) {
                        var err = bridge.userdict_validate(text)
                        // 빈 입력은 오류 표시 안 함
                        if (text.trim() === "") {
                            errorLabel.visible = false
                        } else if (err !== "") {
                            errorLabel.text = err
                            errorLabel.visible = true
                        } else {
                            errorLabel.visible = false
                        }
                    }
                }
            }

            Label { text: tr("userdict_edit_dialog_note_label") }
            TextField {
                id: noteField
                Layout.fillWidth: true
                placeholderText: "설명 (선택)"
            }

            Label {
                id: errorLabel
                Layout.fillWidth: true
                visible: false
                color: "red"
                font.pointSize: 9
                wrapMode: Text.WordWrap
            }
        }

        function openAdd() {
            editMode = false
            editIdx  = -1
            wordField.text = ""
            noteField.text = ""
            errorLabel.visible = false
            open()
        }

        function openEdit(idx) {
            editMode = true
            editIdx  = idx
            wordField.text = bridge ? bridge.userdict_word(idx) : ""
            noteField.text = bridge ? bridge.userdict_note(idx) : ""
            errorLabel.visible = false
            open()
        }
    }

    // ── 메인 콘텐츠 ─────────────────────────────────────────────────
    ColumnLayout {
        width: root.availableWidth
        spacing: 8

        // 제목 + 추가 버튼 행
        RowLayout {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12
            Layout.topMargin: 12

            Label {
                text: tr("page_userdict_title")
                font.pointSize: 12; font.bold: true
                color: pal.windowText
                Layout.fillWidth: true
            }

            Button {
                text: tr("userdict_add_button")
                onClicked: { editDialog.openAdd() }
            }

            Button {
                text: tr("userdict_reload_button")
                onClicked: { if (bridge) bridge.userdict_reload() }
            }
        }

        // 설명 한 줄
        Label {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12
            text: tr("userdict_group_desc")
            font.pointSize: 9; color: pal.mid; wrapMode: Text.WordWrap
        }

        // 단어 목록
        GroupBox {
            Layout.fillWidth: true
            Layout.leftMargin: 12; Layout.rightMargin: 12
            title: tr("userdict_group_title") + " (" + wordIndices.length + ")"

            ColumnLayout {
                anchors.left: parent.left
                anchors.right: parent.right
                spacing: 0

                Repeater {
                    model: wordIndices
                    delegate: UserDictRow {
                        Layout.fillWidth: true
                        theBridge: root.bridge
                        wordIdx: modelData
                        onEditClicked: { editDialog.openEdit(wordIdx) }
                        onDeleteClicked: { if (theBridge) theBridge.userdict_remove(wordIdx) }
                    }
                }

                Label {
                    visible: wordIndices.length === 0
                    text: tr("userdict_empty")
                    font.pointSize: 9; color: pal.mid
                    leftPadding: 4; topPadding: 2; bottomPadding: 2
                }
            }
        }

        Item { Layout.fillWidth: true; height: 16 }
    }

    // ── 인라인 컴포넌트: 단어 행 ─────────────────────────────────────
    component UserDictRow: RowLayout {
        id: dictRow
        required property var theBridge
        required property int wordIdx

        signal editClicked
        signal deleteClicked

        spacing: 4
        height: 34

        // 단어 + 메모
        ColumnLayout {
            Layout.fillWidth: true
            spacing: 1

            Label {
                text: theBridge ? theBridge.userdict_word(wordIdx) : ""
                font.bold: true
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
            Label {
                visible: theBridge ? theBridge.userdict_note(wordIdx) !== "" : false
                text: theBridge ? theBridge.userdict_note(wordIdx) : ""
                font.pointSize: 8; color: pal.mid
                elide: Text.ElideRight
                Layout.fillWidth: true
            }
        }

        // 편집 버튼
        Button {
            text: "✎"
            flat: true; implicitWidth: 32; implicitHeight: 28
            ToolTip.text: theBridge ? theBridge.tr_key("userdict_btn_edit") : ""
            ToolTip.visible: hovered
            onClicked: dictRow.editClicked()
        }

        // 삭제 버튼
        Button {
            text: "✕"
            flat: true; implicitWidth: 32; implicitHeight: 28
            ToolTip.text: theBridge ? theBridge.tr_key("blacklist_btn_delete") : ""
            ToolTip.visible: hovered
            onClicked: dictRow.deleteClicked()
        }
    }
}
