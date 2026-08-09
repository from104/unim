find_package(PkgConfig REQUIRED)
pkg_check_modules(GIO REQUIRED gio-2.0)

# unim-test-log / unim-test-field 는 C 표준 라이브러리만 쓴다 (glib 무관).
# unim_test.c 만 gio 를 필요로 한다.
add_library(unim-test-common STATIC
    unim_test.c
    unim_test_log.c
    unim_test_field.c
    unim_test_dbus.c
    unim_test_spec.c
)
# Qt 앱은 CMAKE_AUTOMOC 을 켜는데, 그게 이 순수 C 라이브러리까지 물들여
# "No valid Qt version found" 경고를 낸다. 여기엔 moc 할 것이 없다.
set_target_properties(unim-test-common PROPERTIES
    LINKER_LANGUAGE C
    AUTOMOC OFF
    AUTOUIC OFF
    AUTORCC OFF
)
target_compile_options(unim-test-common PRIVATE -Wall -Wextra)
target_include_directories(unim-test-common PUBLIC
    ${CMAKE_CURRENT_SOURCE_DIR}
    ${GIO_INCLUDE_DIRS}
)
target_link_libraries(unim-test-common PUBLIC ${GIO_LIBRARIES})
