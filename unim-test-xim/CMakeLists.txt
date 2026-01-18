cmake_minimum_required(VERSION 3.16)
project(unim-test-xim C)

set(CMAKE_C_STANDARD 11)
set(CMAKE_C_STANDARD_REQUIRED ON)

# X11 찾기
find_package(X11 REQUIRED)

# 실행 파일 생성
add_executable(unim-test-xim src/main.c)

# X11 플래그 및 라이브러리
target_include_directories(unim-test-xim PRIVATE ${X11_INCLUDE_DIR})
target_link_libraries(unim-test-xim ${X11_LIBRARIES})

# 설치
install(TARGETS unim-test-xim DESTINATION bin)
