#pragma once
// screen_query.h — 화면 크기 동적 조회 헬퍼
// QGuiApplication::screenAt(QPoint(x, y)) → availableGeometry() 반환.
// null 시 primaryScreen() fallback.
//
// extern "C" 링키지: Rust FFI 에서 직접 호출.

#ifdef __cplusplus
extern "C" {
#endif

// cursor_x/cursor_y 기준 화면의 availableGeometry 반환.
// out_* 포인터에 결과 기록. 실패 시 1920×1080 폴백.
void unim_screen_geom_for(
    int  cursor_x,
    int  cursor_y,
    int *out_x,
    int *out_y,
    int *out_w,
    int *out_h
);

#ifdef __cplusplus
}
#endif
