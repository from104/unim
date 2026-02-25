cmake_minimum_required(VERSION 3.16)
project(unim-qt6 CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Qt6 찾기 (Core, Gui, DBus, Widgets)
find_package(Qt6 REQUIRED COMPONENTS Core Gui DBus Widgets)

# 공통 DBus 클라이언트 경로
set(QT_COMMON_DIR "${CMAKE_SOURCE_DIR}/../qt-common")

message(STATUS "Qt6 IM Module: Using DBus client")
message(STATUS "Qt Common Dir: ${QT_COMMON_DIR}")

# IM 플러그인 빌드
qt_add_plugin(unim
    CLASS_NAME UnimPlatformInputContextPlugin
    PLUGIN_TYPE platforminputcontexts
)

target_sources(unim PRIVATE
    src/plugin.cpp
    src/input_context.cpp
    ${QT_COMMON_DIR}/src/unim_dbus_client.cpp
)

target_include_directories(unim PRIVATE
    ${QT_COMMON_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(unim PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::GuiPrivate
    Qt6::DBus
    Qt6::Widgets
)

# 설치 경로
get_target_property(QT6_QMAKE_EXECUTABLE Qt6::qmake LOCATION)
execute_process(
    COMMAND ${QT6_QMAKE_EXECUTABLE} -query QT_INSTALL_PLUGINS
    OUTPUT_VARIABLE QT6_PLUGINS_DIR
    OUTPUT_STRIP_TRAILING_WHITESPACE
)
set(QT6_IM_PLUGINS_DIR "${QT6_PLUGINS_DIR}/platforminputcontexts")

install(TARGETS unim
    LIBRARY DESTINATION ${QT6_IM_PLUGINS_DIR}
)

# 개발용 로컬 설치 (Qt 플러그인 디렉토리 구조 생성)
# QT_PLUGIN_PATH=$PWD/build 로 사용 가능
add_custom_command(TARGET unim POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_BINARY_DIR}/platforminputcontexts
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:unim> ${CMAKE_BINARY_DIR}/platforminputcontexts/
    COMMENT "Copying plugin to platforminputcontexts/ for local testing"
)
