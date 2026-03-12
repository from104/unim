cmake_minimum_required(VERSION 3.16)
project(unim-test-gtk3 C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# GTK3, GIO 찾기
find_package(PkgConfig REQUIRED)
pkg_check_modules(GTK3 REQUIRED gtk+-3.0)
pkg_check_modules(GIO REQUIRED gio-2.0)

# 실행 파일 생성
add_executable(unim-test-gtk3 src/main.c)

# GTK3/GIO 플래그 및 라이브러리
target_include_directories(unim-test-gtk3 PRIVATE ${GTK3_INCLUDE_DIRS} ${GIO_INCLUDE_DIRS})
target_compile_options(unim-test-gtk3 PRIVATE ${GTK3_CFLAGS_OTHER} ${GIO_CFLAGS_OTHER})
target_link_libraries(unim-test-gtk3 ${GTK3_LIBRARIES} ${GIO_LIBRARIES})

# 설치
install(TARGETS unim-test-gtk3 DESTINATION bin)
