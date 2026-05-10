use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("io.github.from104.unim.qt")
            .qml_file("qml/main.qml")
            .qml_file("qml/HanjaPopup.qml")
            .qml_file("qml/SpecialPopup.qml")
            .qml_file("qml/EmojiPopup.qml")
            // Phase 3: Settings 다이얼로그
            .qml_file("qml/SettingsWindow.qml")
            .qml_file("qml/settings/GeneralPage.qml")
            .qml_file("qml/settings/AutoTypeFixPage.qml")
            .qml_file("qml/settings/BlacklistPage.qml")
            .qml_file("qml/settings/UserDictPage.qml")
            .qml_file("qml/settings/GnomePage.qml"),
    )
    .files(["src/bridge.rs"])
    // 5-B: 화면 크기 동적 조회 C++ 헬퍼
    .cpp_file("cpp/screen_query.cpp")
    .include_dir("cpp")
    .qt_module("Gui")
    .build();
}
