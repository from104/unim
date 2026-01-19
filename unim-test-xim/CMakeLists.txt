cmake_minimum_required(VERSION 3.16)
project(unim-test-xim C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# X11 및 Xft 찾기
find_package(X11 REQUIRED)
find_package(PkgConfig REQUIRED)

pkg_check_modules(XFT REQUIRED xft)
pkg_check_modules(FONTCONFIG REQUIRED fontconfig)

# 실행 파일 생성
add_executable(unim-test-xim src/main.c)

# X11 플래그 및 라이브러리
target_include_directories(unim-test-xim PRIVATE 
    ${X11_INCLUDE_DIR}
    ${XFT_INCLUDE_DIRS}
    ${FONTCONFIG_INCLUDE_DIRS}
)

target_link_libraries(unim-test-xim 
    ${X11_LIBRARIES}
    ${XFT_LIBRARIES}
    ${FONTCONFIG_LIBRARIES}
)

# 설치
install(TARGETS unim-test-xim DESTINATION bin)
