cmake_minimum_required(VERSION 3.16)
project(unim-qt6 CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Qt6 찾기
find_package(Qt6 REQUIRED COMPONENTS Core Gui)

# unim-capi 라이브러리 (커맨드라인에서 전달받거나 시스템에서 검색)
if(NOT UNIM_CAPI_LIB)
    find_library(UNIM_CAPI_LIB unim_capi PATHS /usr/local/lib /usr/lib)
endif()
if(NOT UNIM_CAPI_INCLUDE)
    find_path(UNIM_CAPI_INCLUDE unim.h PATHS /usr/local/include /usr/include)
endif()

message(STATUS "UNIM_CAPI_LIB: ${UNIM_CAPI_LIB}")
message(STATUS "UNIM_CAPI_INCLUDE: ${UNIM_CAPI_INCLUDE}")

# IM 플러그인 빌드
qt_add_plugin(unim
    CLASS_NAME UnimPlatformInputContextPlugin
    PLUGIN_TYPE platforminputcontexts
)

target_sources(unim PRIVATE
    src/plugin.cpp
    src/input_context.cpp
)

target_include_directories(unim PRIVATE
    ${UNIM_CAPI_INCLUDE}
    ${CMAKE_CURRENT_SOURCE_DIR}/src
)

target_link_libraries(unim PRIVATE
    Qt6::Core
    Qt6::Gui
    Qt6::GuiPrivate
    ${UNIM_CAPI_LIB}
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

# 개발용 로컬 설치
add_custom_target(install-local
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:unim> ${CMAKE_SOURCE_DIR}/libunim.so
    DEPENDS unim
)
