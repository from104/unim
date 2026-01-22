cmake_minimum_required(VERSION 3.16)
project(unim-test-qt5 VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Qt5 찾기
find_package(Qt5 REQUIRED COMPONENTS Widgets Core)

# 실행 파일
add_executable(unim-test-qt5
    src/main.cpp
)

target_compile_options(unim-test-qt5 PRIVATE
    -Wall
    -Wextra
)

target_link_libraries(unim-test-qt5 PRIVATE
    Qt5::Widgets
    Qt5::Core
)

# 설치
install(TARGETS unim-test-qt5
    RUNTIME DESTINATION bin
)
