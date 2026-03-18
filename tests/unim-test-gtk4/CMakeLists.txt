cmake_minimum_required(VERSION 3.16)
project(unim-test-gtk4 C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# GTK4, GIO 찾기
find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK4 REQUIRED gtk4)
pkg_check_modules(GIO REQUIRED gio-2.0)

# 공통 테스트 라이브러리
add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/../common ${CMAKE_CURRENT_BINARY_DIR}/common)

# 실행 파일 생성
add_executable(unim-test-gtk4 src/main.c)

# GTK4/GIO 플래그 및 라이브러리
target_include_directories(unim-test-gtk4 PRIVATE
    ${GTK4_INCLUDE_DIRS} ${GIO_INCLUDE_DIRS}
    ${CMAKE_CURRENT_SOURCE_DIR}/../common
)
target_compile_options(unim-test-gtk4 PRIVATE ${GTK4_CFLAGS_OTHER} ${GIO_CFLAGS_OTHER})
target_link_libraries(unim-test-gtk4 ${GTK4_LIBRARIES} ${GIO_LIBRARIES} unim-test-common)

# 설치
install(TARGETS unim-test-gtk4 DESTINATION bin)
