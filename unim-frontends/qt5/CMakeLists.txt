cmake_minimum_required(VERSION 3.16)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

project(unim-qt5 CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Qt5 찾기 (Core, Gui, DBus, Widgets)
find_package(Qt5 REQUIRED COMPONENTS Core Gui DBus Widgets)
find_package(Qt5Gui ${Qt5_VERSION} CONFIG REQUIRED Private)

# Qt5 Private 헤더 경로 명시적으로 가져오기
get_target_property(Qt5Gui_PRIVATE_INCLUDE_DIRS Qt5::GuiPrivate INTERFACE_INCLUDE_DIRECTORIES)

# 공통 DBus 클라이언트 경로
set(QT_COMMON_DIR "${CMAKE_SOURCE_DIR}/../qt-common")

message(STATUS "Qt5 IM Module: Using DBus client")
message(STATUS "Qt Common Dir: ${QT_COMMON_DIR}")

# IM 플러그인 빌드
add_library(unim MODULE
    src/plugin.cpp
    src/input_context.cpp
    ${QT_COMMON_DIR}/src/unim_dbus_client.cpp
)

target_include_directories(unim PRIVATE
    ${QT_COMMON_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${Qt5Gui_PRIVATE_INCLUDE_DIRS}
)

target_link_libraries(unim
    Qt5::Core
    Qt5::Gui
    Qt5::GuiPrivate
    Qt5::DBus
    Qt5::Widgets
)

# 설치 경로
get_target_property(QT5_QMAKE_EXECUTABLE Qt5::qmake LOCATION)
execute_process(
    COMMAND ${QT5_QMAKE_EXECUTABLE} -query QT_INSTALL_PLUGINS
    OUTPUT_VARIABLE QT5_PLUGINS_DIR
    OUTPUT_STRIP_TRAILING_WHITESPACE
)
set(QT5_IM_PLUGINS_DIR "${QT5_PLUGINS_DIR}/platforminputcontexts")

install(TARGETS unim
    LIBRARY DESTINATION ${QT5_IM_PLUGINS_DIR}
)

# 개발용 로컬 설치 (Qt 플러그인 디렉토리 구조 생성)
# QT_PLUGIN_PATH=$PWD/build 로 사용 가능
add_custom_command(TARGET unim POST_BUILD
    COMMAND ${CMAKE_COMMAND} -E make_directory ${CMAKE_BINARY_DIR}/platforminputcontexts
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:unim> ${CMAKE_BINARY_DIR}/platforminputcontexts/
    COMMENT "Copying plugin to platforminputcontexts/ for local testing"
)
