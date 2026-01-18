cmake_minimum_required(VERSION 3.16)
project(unim-gtk4 C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# GTK4 찾기
find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK4 REQUIRED gtk4)

# unim-capi 라이브러리 (로컬 빌드 우선, 시스템 설치 fallback)
set(UNIM_LOCAL_ROOT "${CMAKE_SOURCE_DIR}/../..")
set(UNIM_CAPI_LOCAL_INCLUDE "${UNIM_LOCAL_ROOT}/unim-capi/include")

if(EXISTS ${UNIM_CAPI_LOCAL_INCLUDE}/unim.h)
    set(UNIM_CAPI_INCLUDE ${UNIM_CAPI_LOCAL_INCLUDE})
else()
    find_path(UNIM_CAPI_INCLUDE unim.h PATHS /usr/local/include /usr/include)
endif()

if(NOT DEFINED UNIM_CAPI_LIB)
    if(EXISTS "${UNIM_LOCAL_ROOT}/target/release/libunim_capi.so")
        set(UNIM_CAPI_LIB "${UNIM_LOCAL_ROOT}/target/release/libunim_capi.so")
    elseif(EXISTS "${UNIM_LOCAL_ROOT}/target/debug/libunim_capi.so")
        set(UNIM_CAPI_LIB "${UNIM_LOCAL_ROOT}/target/debug/libunim_capi.so")
    else()
        find_library(UNIM_CAPI_LIB unim_capi PATHS /usr/local/lib /usr/lib)
    endif()
endif()

message(STATUS "UNIM C API include: ${UNIM_CAPI_INCLUDE}")
message(STATUS "UNIM C API lib: ${UNIM_CAPI_LIB}")

# IM 모듈 빌드
add_library(im-unim-gtk4 MODULE
    src/immodule.c
)

target_include_directories(im-unim-gtk4 PRIVATE
    ${GTK4_INCLUDE_DIRS}
    ${UNIM_CAPI_INCLUDE}
)

target_link_libraries(im-unim-gtk4
    ${GTK4_LIBRARIES}
    ${UNIM_CAPI_LIB}
)

target_compile_options(im-unim-gtk4 PRIVATE ${GTK4_CFLAGS_OTHER})

# 설치 경로 설정
pkg_get_variable(GTK4_LIBDIR gtk4 libdir)
set(GTK4_IM_MODULEDIR "${GTK4_LIBDIR}/gtk-4.0/4.0.0/immodules")

install(TARGETS im-unim-gtk4
    LIBRARY DESTINATION ${GTK4_IM_MODULEDIR}
)

# 개발용 로컬 설치
add_custom_target(install-local
    COMMAND ${CMAKE_COMMAND} -E copy $<TARGET_FILE:im-unim-gtk4> ${CMAKE_SOURCE_DIR}/im-unim-gtk4.so
    DEPENDS im-unim-gtk4
)
