cmake_minimum_required(VERSION 3.16)
project(unim-qt6 CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Qt6 찾기 (Core, Gui, DBus, Widgets)
find_package(Qt6 REQUIRED COMPONENTS Core Gui DBus Widgets)

# `Qt6::GuiPrivate` 는 아래에서 링크하는데, 이 타겟을 얻는 방법이 Qt 판마다 다르다.
#   · Qt 6.4 (Ubuntu 24.04): Gui 를 찾을 때 함께 정의된다. 컴포넌트로 요청하면
#     오히려 실패한다 — Qt6GuiPrivateConfig.cmake 가 없기 때문이다.
#   · Qt 6.8+ (Fedora 43): private 모듈이 별도 config 로 분리돼 명시해야 생긴다.
# 그래서 "없을 때만 따로 찾는" 형태로 둔다. 어느 쪽에서도 깨지지 않는다.
#
# 2026-08-10 v0.4.0 의 linux-rpm 잡이 이것으로 깨졌다("Target unim links to
# Qt6::GuiPrivate but the target was not found"). deb 는 Qt 6.4 라 무사했다.
# qt5 쪽은 처음부터 `find_package(Qt5Gui ... REQUIRED Private)` 로 명시해 왔다.
if(NOT TARGET Qt6::GuiPrivate)
    find_package(Qt6 REQUIRED COMPONENTS GuiPrivate)
endif()

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
    ${QT_COMMON_DIR}/include/unim_dbus_client.hpp
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

# 설치 경로 — qmake 감지 후 폴백
get_target_property(QT6_QMAKE_EXECUTABLE Qt6::qmake LOCATION)
if(QT6_QMAKE_EXECUTABLE)
    execute_process(
        COMMAND ${QT6_QMAKE_EXECUTABLE} -query QT_INSTALL_PLUGINS
        OUTPUT_VARIABLE QT6_PLUGINS_DIR
        OUTPUT_STRIP_TRAILING_WHITESPACE
        RESULT_VARIABLE QT6_QMAKE_RESULT
    )
endif()
if(NOT QT6_PLUGINS_DIR OR NOT QT6_QMAKE_RESULT EQUAL 0)
    set(QT6_PLUGINS_DIR "${CMAKE_INSTALL_PREFIX}/lib/${CMAKE_LIBRARY_ARCHITECTURE}/qt6/plugins")
    message(STATUS "Qt6 qmake unavailable, using fallback: ${QT6_PLUGINS_DIR}")
endif()
set(QT6_IM_PLUGINS_DIR "${QT6_PLUGINS_DIR}/platforminputcontexts")

set_target_properties(unim PROPERTIES
    INSTALL_RPATH ""
    INSTALL_RPATH_USE_LINK_PATH OFF
    BUILD_WITH_INSTALL_RPATH ON
    SKIP_BUILD_RPATH ON
)

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
