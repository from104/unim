cmake_minimum_required(VERSION 3.16)
project(unim-gtk3 C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# GTK3 찾기
find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK3 REQUIRED gtk+-3.0)

# unim-capi 라이브러리 (로컬 빌드 우선, 시스템 설치 fallback)
set(UNIM_LOCAL_ROOT "${CMAKE_SOURCE_DIR}/../..")
set(UNIM_CAPI_LOCAL_INCLUDE "${UNIM_LOCAL_ROOT}/unim-capi/include")
set(UNIM_CAPI_LOCAL_LIB "${UNIM_LOCAL_ROOT}/target/debug/libunim_capi.so")

if(EXISTS ${UNIM_CAPI_LOCAL_INCLUDE}/unim.h)
    set(UNIM_CAPI_INCLUDE ${UNIM_CAPI_LOCAL_INCLUDE})
else()
    find_path(UNIM_CAPI_INCLUDE unim.h PATHS /usr/local/include /usr/include)
endif()

if(EXISTS ${UNIM_CAPI_LOCAL_LIB})
    set(UNIM_CAPI_LIB ${UNIM_CAPI_LOCAL_LIB})
else()
    find_library(UNIM_CAPI_LIB unim_capi PATHS /usr/local/lib /usr/lib)
endif()

message(STATUS "UNIM C API include: ${UNIM_CAPI_INCLUDE}")
message(STATUS "UNIM C API lib: ${UNIM_CAPI_LIB}")

# IM 모듈 빌드
add_library(im-unim MODULE
    src/immodule.c
)

target_include_directories(im-unim PRIVATE
    ${GTK3_INCLUDE_DIRS}
    ${UNIM_CAPI_INCLUDE}
)

target_link_libraries(im-unim
    ${GTK3_LIBRARIES}
    ${UNIM_CAPI_LIB}
)

target_compile_options(im-unim PRIVATE ${GTK3_CFLAGS_OTHER})

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
