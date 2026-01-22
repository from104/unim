cmake_minimum_required(VERSION 3.16)
set(CMAKE_EXPORT_COMPILE_COMMANDS ON)

project(unim-qt5 CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Qt5 찾기
find_package(Qt5 REQUIRED COMPONENTS Core Gui)
find_package(Qt5Gui ${Qt5_VERSION} CONFIG REQUIRED Private)


# Qt5 Private 헤더 경로 명시적으로 가져오기
get_target_property(Qt5Gui_PRIVATE_INCLUDE_DIRS Qt5::GuiPrivate INTERFACE_INCLUDE_DIRECTORIES)

# unim-capi 라이브러리 (시스템 설치 또는 로컬)
find_library(UNIM_CAPI_LIB unim_capi PATHS /usr/local/lib /usr/lib)
find_path(UNIM_CAPI_INCLUDE unim.h PATHS /usr/local/include /usr/include)

# IM 플러그인 빌드
add_library(unim MODULE
    src/plugin.cpp
    src/input_context.cpp
)

target_include_directories(unim PRIVATE
    ${UNIM_CAPI_INCLUDE}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${Qt5Gui_PRIVATE_INCLUDE_DIRS}
)


target_link_libraries(unim
    Qt5::Core
    Qt5::Gui
    Qt5::GuiPrivate
    ${UNIM_CAPI_LIB}
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

# 개발용 로컬 설치
add_custom_target(install-local
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:unim> ${CMAKE_SOURCE_DIR}/libunim.so
    DEPENDS unim
)
