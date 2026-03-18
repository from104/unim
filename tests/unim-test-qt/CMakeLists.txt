# UNIM Qt 입력기 테스트 — Qt5/Qt6 통합 빌드
#
# 상위 CMakeLists.txt에서:
#   add_subdirectory(tests/unim-test-qt)
#
# 또는 단독 빌드:
#   cmake -B build -DQT_VERSION=5  (또는 6)
#   cmake --build build

cmake_minimum_required(VERSION 3.16)
project(unim-test-qt LANGUAGES C CXX)

set(CMAKE_CXX_STANDARD 17)
set(CMAKE_AUTOMOC ON)

# 공통 테스트 라이브러리
add_subdirectory(${CMAKE_CURRENT_SOURCE_DIR}/../common ${CMAKE_CURRENT_BINARY_DIR}/common)

# ─── Qt5 ──────────────────────────────────────────────────────────
find_package(Qt5 QUIET COMPONENTS Widgets DBus)
if(Qt5_FOUND)
    add_executable(unim-test-qt5 src/main.cpp)
    target_link_libraries(unim-test-qt5 PRIVATE
        Qt5::Widgets Qt5::DBus
        unim-test-common
    )
    target_include_directories(unim-test-qt5 PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../common
    )
    install(TARGETS unim-test-qt5 DESTINATION bin)
    message(STATUS "Qt5 test app: enabled")
else()
    message(STATUS "Qt5 test app: disabled (Qt5 not found)")
endif()

# ─── Qt6 ──────────────────────────────────────────────────────────
find_package(Qt6 QUIET COMPONENTS Widgets DBus)
if(Qt6_FOUND)
    add_executable(unim-test-qt6 src/main.cpp)
    target_link_libraries(unim-test-qt6 PRIVATE
        Qt6::Widgets Qt6::DBus
        unim-test-common
    )
    target_include_directories(unim-test-qt6 PRIVATE
        ${CMAKE_CURRENT_SOURCE_DIR}/../common
    )
    install(TARGETS unim-test-qt6 DESTINATION bin)
    message(STATUS "Qt6 test app: enabled")
else()
    message(STATUS "Qt6 test app: disabled (Qt6 not found)")
endif()
