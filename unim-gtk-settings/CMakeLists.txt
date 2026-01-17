cmake_minimum_required(VERSION 3.16)
project(unim-gtk-settings VERSION 0.1.0 LANGUAGES C)

# GTK4 찾기
find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK4 REQUIRED gtk4)

# unim-capi 라이브러리 경로
set(UNIM_ROOT "${CMAKE_SOURCE_DIR}/..")
set(UNIM_CAPI_LIB "${UNIM_ROOT}/target/release/libunim_capi.a")
set(UNIM_CAPI_INCLUDE "${UNIM_ROOT}/unim-capi")

# unim-capi 헤더 생성 (cbindgen 사용)
add_custom_command(
    OUTPUT "${UNIM_CAPI_INCLUDE}/unim.h"
    COMMAND cbindgen --config "${UNIM_ROOT}/unim-capi/cbindgen.toml" 
            --crate unim-capi 
            --output "${UNIM_CAPI_INCLUDE}/unim.h"
    WORKING_DIRECTORY "${UNIM_ROOT}"
    COMMENT "Generating unim-capi C header"
)

# 실행 파일
add_executable(unim-gtk-settings
    src/main.c
    src/settings_dialog.c
)

target_include_directories(unim-gtk-settings PRIVATE
    ${GTK4_INCLUDE_DIRS}
    ${UNIM_CAPI_INCLUDE}
)

target_compile_options(unim-gtk-settings PRIVATE
    ${GTK4_CFLAGS_OTHER}
    -Wall
    -Wextra
)

target_link_directories(unim-gtk-settings PRIVATE
    ${GTK4_LIBRARY_DIRS}
    "${UNIM_ROOT}/target/release"
)

target_link_libraries(unim-gtk-settings
    ${GTK4_LIBRARIES}
    unim_capi
    pthread
    m
    dl
)

# 설치
install(TARGETS unim-gtk-settings
    RUNTIME DESTINATION bin
)
