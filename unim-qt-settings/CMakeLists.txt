cmake_minimum_required(VERSION 3.16)
project(unim-qt-settings VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)
set(CMAKE_AUTORCC ON)
set(CMAKE_AUTOUIC ON)

# Qt6 찾기 (LinguistTools 생략)
find_package(Qt6 REQUIRED COMPONENTS Widgets Core)

# unim-capi 라이브러리 경로
set(UNIM_ROOT "${CMAKE_SOURCE_DIR}/..")
set(UNIM_CAPI_INCLUDE "${UNIM_ROOT}/unim-capi/include")

# 실행 파일
add_executable(unim-qt-settings
    src/main.cpp
    src/SettingsDialog.cpp
    src/SettingsDialog.h
    i18n.qrc
)

target_include_directories(unim-qt-settings PRIVATE
    ${UNIM_CAPI_INCLUDE}
)

target_compile_options(unim-qt-settings PRIVATE
    -Wall
    -Wextra
)

target_link_directories(unim-qt-settings PRIVATE
    "${UNIM_ROOT}/target/release"
)

target_link_libraries(unim-qt-settings PRIVATE
    Qt6::Widgets
    Qt6::Core
    unim_capi
    pthread
    m
    dl
)

# 설치
install(TARGETS unim-qt-settings
    RUNTIME DESTINATION bin
)

