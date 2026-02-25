use cxx_qt_build::{CxxQtBuilder, QmlModule};

fn main() {
    CxxQtBuilder::new_qml_module(
        QmlModule::new("io.github.from104.unim.qt")
            .qml_file("qml/main.qml")
            .qml_file("qml/HanjaPopup.qml")
            .qml_file("qml/SpecialPopup.qml"),
    )
    .files(["src/bridge.rs"])
    .build();
}
