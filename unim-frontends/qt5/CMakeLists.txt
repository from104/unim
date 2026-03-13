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
    ${QT_COMMON_DIR}/src/unim_hanja_popup.cpp
    ${QT_COMMON_DIR}/src/unim_special_popup.cpp
    ${QT_COMMON_DIR}/include/unim_hanja_popup.hpp
    ${QT_COMMON_DIR}/include/unim_special_popup.hpp
)

# UNIM C-API (PopupState)
set(UNIM_ROOT "${CMAKE_SOURCE_DIR}/../..")
set(UNIM_CAPI_INCLUDE "${UNIM_ROOT}/unim-capi/include")
set(UNIM_CAPI_LIB_DIR "${UNIM_ROOT}/target/release")

target_include_directories(unim PRIVATE
    ${QT_COMMON_DIR}/include
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${Qt5Gui_PRIVATE_INCLUDE_DIRS}
    ${UNIM_CAPI_INCLUDE}
)

target_link_directories(unim PRIVATE ${UNIM_CAPI_LIB_DIR})

target_link_libraries(unim
    Qt5::Core
    Qt5::Gui
    Qt5::GuiPrivate
    Qt5::DBus
    Qt5::Widgets
    unim_capi
)

# 설치 경로 — qmake 감지 후 폴백
get_target_property(QT5_QMAKE_EXECUTABLE Qt5::qmake LOCATION)
if(QT5_QMAKE_EXECUTABLE)
    execute_process(
        COMMAND ${QT5_QMAKE_EXECUTABLE} -query QT_INSTALL_PLUGINS
        OUTPUT_VARIABLE QT5_PLUGINS_DIR
        OUTPUT_STRIP_TRAILING_WHITESPACE
        RESULT_VARIABLE QT5_QMAKE_RESULT
    )
endif()
if(NOT QT5_PLUGINS_DIR OR NOT QT5_QMAKE_RESULT EQUAL 0)
    set(QT5_PLUGINS_DIR "${CMAKE_INSTALL_PREFIX}/lib/${CMAKE_LIBRARY_ARCHITECTURE}/qt5/plugins")
    message(STATUS "Qt5 qmake unavailable, using fallback: ${QT5_PLUGINS_DIR}")
endif()
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
