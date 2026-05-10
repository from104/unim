// screen_query.cpp — 화면 크기 동적 조회 헬퍼 구현
// QGuiApplication::screenAt(QPoint(x, y)) → availableGeometry() 반환.

#include "screen_query.h"

#include <QGuiApplication>
#include <QPoint>
#include <QRect>
#include <QScreen>

extern "C"
void unim_screen_geom_for(
    int  cursor_x,
    int  cursor_y,
    int *out_x,
    int *out_y,
    int *out_w,
    int *out_h
)
{
    QScreen *screen = QGuiApplication::screenAt(QPoint(cursor_x, cursor_y));
    if (!screen) {
        screen = QGuiApplication::primaryScreen();
    }

    if (!screen) {
        // 최후 폴백: 1920×1080
        *out_x = 0;
        *out_y = 0;
        *out_w = 1920;
        *out_h = 1080;
        return;
    }

    QRect geom = screen->availableGeometry();
    *out_x = static_cast<int>(geom.x());
    *out_y = static_cast<int>(geom.y());
    *out_w = static_cast<int>(geom.width());
    *out_h = static_cast<int>(geom.height());
}
