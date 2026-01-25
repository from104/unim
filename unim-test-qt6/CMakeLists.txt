cmake_minimum_required(VERSION 3.16)
project(unim-test-qt6 VERSION 0.1.0 LANGUAGES CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_CXX_STANDARD_REQUIRED ON)
set(CMAKE_AUTOMOC ON)

# Qt6 찾기
find_package(Qt6 REQUIRED COMPONENTS Widgets Core DBus)

# 실행 파일
add_executable(unim-test-qt6
    src/main.cpp
)

target_compile_options(unim-test-qt6 PRIVATE
    -Wall
    -Wextra
)

target_link_libraries(unim-test-qt6 PRIVATE
    Qt6::Widgets
    Qt6::Core
    Qt6::DBus
)

# 설치
install(TARGETS unim-test-qt6
    RUNTIME DESTINATION bin
)
