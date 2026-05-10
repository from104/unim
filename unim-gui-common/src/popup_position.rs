//! toolkit-free 팝업 포지셔닝 타입 및 환경 감지
//!
//! `DisplayServer` enum + 감지 함수는 GTK/Qt 양쪽에서 공유.
//! GTK Window를 직접 받는 `position_popup`, `x11_install_outside_click_handler` 등은
//! `unim-gui-gtk::popup_positioning`에 남는다.
//!
//! `compute_popup_xy`: 커서 위치·윈도우 크기·화면 경계가 주어졌을 때 popup 좌상단
//! (x, y)를 계산하는 순수 함수. 현재 X11은 항상 화면 정중앙, Wayland는 layer_shell
//! margin 직접 설정 방식이므로 이 함수는 미래 Qt·다른 포지셔닝 정책을 위한 API로
//! 준비만 한다. 좌표 산식 변경 금지 (셀 아래 폴백 + 화면 경계 클램프).

/// 디스플레이 서버 환경
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum DisplayServer {
    X11,
    WaylandLayerShell,
    GnomeWayland,
}

/// GNOME Wayland 환경인지 감지 (환경변수 기반, toolkit 무관)
pub fn is_gnome_wayland() -> bool {
    let is_wayland = std::env::var("XDG_SESSION_TYPE")
        .map(|v| v == "wayland")
        .unwrap_or(false);

    if !is_wayland {
        return false;
    }

    if std::env::var("GNOME_SHELL_SESSION_MODE").is_ok() {
        return true;
    }
    if let Ok(desktop) = std::env::var("XDG_CURRENT_DESKTOP") {
        if desktop.to_uppercase().contains("GNOME") {
            return true;
        }
    }
    if let Ok(session) = std::env::var("DESKTOP_SESSION") {
        if session.to_lowercase().contains("gnome") {
            return true;
        }
    }

    false
}

/// 커서 위치/높이와 윈도우 크기, 화면 경계가 주어졌을 때 popup의
/// 최종 (x, y) 좌상단 좌표를 반환한다. toolkit 무관 순수 계산.
///
/// 기본 전략: cursor 아래(cursor_y + cursor_h + 4)에 배치.
/// 화면 하단을 벗어나면 cursor 위로 폴백(cursor_y - win_h - 4).
/// 수평은 오른쪽 경계를 넘지 않도록 클램프.
/// 결과는 `(screen_x, screen_y)` 기준 절대 좌표.
pub fn compute_popup_xy(
    cursor_x: i32,
    cursor_y: i32,
    cursor_h: i32,
    win_w: i32,
    win_h: i32,
    screen_x: i32,
    screen_y: i32,
    screen_w: i32,
    screen_h: i32,
) -> (i32, i32) {
    let gap = 4;

    // Y: 커서 아래 배치, 화면 하단 초과 시 위로 폴백
    let y_below = cursor_y + cursor_h + gap;
    let y_above = cursor_y - win_h - gap;
    let y = if y_below + win_h <= screen_y + screen_h {
        y_below
    } else {
        y_above.max(screen_y)
    };

    // X: 오른쪽 경계 클램프
    let x = cursor_x.max(screen_x).min(screen_x + screen_w - win_w);

    (x, y)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn below_cursor_normal() {
        // 충분한 공간 → cursor 아래
        let (x, y) = compute_popup_xy(100, 200, 20, 300, 200, 0, 0, 1920, 1080);
        assert_eq!(y, 200 + 20 + 4); // 224
        assert_eq!(x, 100);
    }

    #[test]
    fn fallback_above_when_no_room_below() {
        // cursor_y=900, cursor_h=20, win_h=200, screen_h=1080
        // y_below = 924, 924+200=1124 > 1080 → 위로 폴백
        let (_, y) = compute_popup_xy(100, 900, 20, 300, 200, 0, 0, 1920, 1080);
        assert_eq!(y, 900 - 200 - 4); // 696
    }

    #[test]
    fn clamp_right_edge() {
        // cursor_x=1700, win_w=300, screen_w=1920 → 1700+300=2000 > 1920 → 클램프
        let (x, _) = compute_popup_xy(1700, 200, 20, 300, 200, 0, 0, 1920, 1080);
        assert_eq!(x, 1920 - 300); // 1620
    }
}
