find_package(PkgConfig REQUIRED)
pkg_check_modules(GIO REQUIRED gio-2.0)

add_library(unim-test-common STATIC unim_test.c)
set_target_properties(unim-test-common PROPERTIES LINKER_LANGUAGE C)
target_include_directories(unim-test-common PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${GIO_INCLUDE_DIRS}
)
target_link_libraries(unim-test-common PUBLIC ${GIO_LIBRARIES})
