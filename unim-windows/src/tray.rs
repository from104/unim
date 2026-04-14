//! 시스템 트레이 아이콘 (Windows 전용)
//!
//! tray-icon 크레이트를 사용하여 Windows 알림 영역에
//! 한/영 모드 상태 아이콘을 표시합니다.
//!
//! 현재 Linux에서는 스텁으로 동작합니다.
//! Windows 빌드에서만 실제 트레이 아이콘이 활성화됩니다.

/// 트레이 아이콘 상태 (플랫폼 독립 인터페이스)
#[allow(dead_code)]
pub struct TrayState {
    pub is_korean: bool,
}

#[allow(dead_code)]
impl TrayState {
    pub fn new() -> Self {
        Self { is_korean: true }
    }

    /// 모드 변경 시 호출 (향후 트레이 아이콘 업데이트)
    pub fn set_mode(&mut self, is_korean: bool) {
        self.is_korean = is_korean;
        // Windows에서 tray-icon 크레이트를 사용하여 아이콘 업데이트
        // 현재는 상태만 저장
    }
}
