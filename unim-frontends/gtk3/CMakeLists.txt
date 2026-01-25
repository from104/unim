cmake_minimum_required(VERSION 3.16)
project(unim-gtk3 C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# GTK3 및 GIO 찾기 (GDBus 포함)
find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK3 REQUIRED gtk+-3.0)
pkg_check_modules(GIO REQUIRED gio-2.0)

# 공통 DBus 클라이언트 경로
set(GTK_COMMON_DIR "${CMAKE_SOURCE_DIR}/../gtk-common")

# IM 모듈 빌드
add_library(im-unim MODULE
    src/immodule.c
    ${GTK_COMMON_DIR}/src/unim_dbus_client.c
)

target_include_directories(im-unim PRIVATE
    ${GTK3_INCLUDE_DIRS}
    ${GIO_INCLUDE_DIRS}
    ${GTK_COMMON_DIR}/include
)

target_link_libraries(im-unim
    ${GTK3_LIBRARIES}
    ${GIO_LIBRARIES}
)

target_compile_options(im-unim PRIVATE 
    ${GTK3_CFLAGS_OTHER}
    ${GIO_CFLAGS_OTHER}
)

# 설치 경로 설정
pkg_get_variable(GTK3_LIBDIR gtk+-3.0 libdir)
set(GTK3_IM_MODULEDIR "${GTK3_LIBDIR}/gtk-3.0/3.0.0/immodules")

install(TARGETS im-unim
    LIBRARY DESTINATION ${GTK3_IM_MODULEDIR}
)

# 개발용 로컬 설치
add_custom_target(install-local
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:im-unim> ${CMAKE_SOURCE_DIR}/im-unim.so
    DEPENDS im-unim
)

message(STATUS "GTK3 IM Module: Using DBus client")
message(STATUS "GTK Common Dir: ${GTK_COMMON_DIR}")
