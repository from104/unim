cmake_minimum_required(VERSION 3.16)
project(unim-test-xim C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# ─── Dependencies ────────────────────────────────────────────────────────
find_package(X11 REQUIRED)
find_package(PkgConfig REQUIRED)

pkg_check_modules(XFT REQUIRED xft)
pkg_check_modules(FONTCONFIG REQUIRED fontconfig)
pkg_check_modules(GIO REQUIRED gio-2.0)

# ─── Sources ─────────────────────────────────────────────────────────────
set(SOURCES
    src/main.c
    src/ui.c
    src/xim.c
    src/dbus.c
    src/fields.c
    src/log.c
)

# ─── Executable ──────────────────────────────────────────────────────────
add_executable(unim-test-xim ${SOURCES})

target_include_directories(unim-test-xim PRIVATE
    ${CMAKE_CURRENT_SOURCE_DIR}/src
    ${X11_INCLUDE_DIR}
    ${XFT_INCLUDE_DIRS}
    ${FONTCONFIG_INCLUDE_DIRS}
    ${GIO_INCLUDE_DIRS}
)

target_link_libraries(unim-test-xim
    ${X11_LIBRARIES}
    ${XFT_LIBRARIES}
    ${FONTCONFIG_LIBRARIES}
    ${GIO_LIBRARIES}
)

# ─── Install ─────────────────────────────────────────────────────────────
install(TARGETS unim-test-xim DESTINATION bin)
