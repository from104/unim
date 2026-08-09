//! 키보드 위젯 공용 헬퍼 — 셀 통계(`KeyStat`)와 라벨 조회(`cell_label_at`).
//!
//! 실제 키보드 위젯은 [`crate::keyboard_view::KeyboardView`] 가 그린다(studio·typing 공용).
//! 이 모듈은 그 위젯과 typing-practice 통계 코어가 함께 쓰는 작은 값 타입·헬퍼만 둔다.

use unim::keystroke::profile::LayoutRows;

/// 한 셀의 시도/오타 통계 — 히트맵 입력.
#[derive(Debug, Default, Clone, Copy)]
pub struct KeyStat {
    pub attempts: u32,
    pub errors: u32,
}

impl KeyStat {
    pub fn ratio(&self) -> f64 {
        if self.attempts == 0 {
            0.0
        } else {
            self.errors as f64 / self.attempts as f64
        }
    }
}

/// LayoutRows의 한 행에서 col 인덱스의 라벨을 가져온다 (없으면 빈 문자열).
pub fn cell_label_at(rows: &LayoutRows, row: u8, col: u8) -> &str {
    let row_slice: &Vec<String> = match row {
        0 => &rows.row1,
        1 => &rows.row2,
        2 => &rows.row3,
        3 => &rows.row4,
        _ => return "",
    };
    row_slice.get(col as usize).map(|s| s.as_str()).unwrap_or("")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keystat_ratio_zero_attempts() {
        let s = KeyStat::default();
        assert_eq!(s.ratio(), 0.0);
    }

    #[test]
    fn keystat_ratio_half() {
        let s = KeyStat {
            attempts: 4,
            errors: 2,
        };
        assert!((s.ratio() - 0.5).abs() < 1e-9);
    }

    #[test]
    fn cell_label_at_returns_empty_for_oob() {
        let rows = LayoutRows {
            row1: vec!["a".into(), "b".into()],
            row2: vec![],
            row3: vec![],
            row4: vec![],
        };
        assert_eq!(cell_label_at(&rows, 0, 0), "a");
        assert_eq!(cell_label_at(&rows, 0, 1), "b");
        assert_eq!(cell_label_at(&rows, 0, 5), "");
        assert_eq!(cell_label_at(&rows, 7, 0), "");
    }
}
