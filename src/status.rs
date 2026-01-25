//! 상태 파일 관리 모듈
//!
//! 입력 모드 상태를 파일로 공유하여 인디케이터 등 외부 프로그램과 연동합니다.
//! 상태 파일 경로: `~/.cache/unim/status`

use std::fs;
use std::io::{self, Read, Write};
use std::path::PathBuf;

/// 입력 카테고리
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputCategory {
    Korean,
    English,
}

impl InputCategory {
    /// 문자열로 변환
    pub fn as_str(&self) -> &'static str {
        match self {
            InputCategory::Korean => "korean",
            InputCategory::English => "english",
        }
    }

    /// 문자열에서 파싱
    pub fn from_str(s: &str) -> Option<Self> {
        match s.trim() {
            "korean" => Some(InputCategory::Korean),
            "english" => Some(InputCategory::English),
            _ => None,
        }
    }
}

/// 상태 파일 경로 반환
pub fn status_file_path() -> PathBuf {
    dirs::cache_dir()
        .unwrap_or_else(|| PathBuf::from("/tmp"))
        .join("unim")
        .join("status")
}

/// 현재 상태 읽기
pub fn get_status() -> io::Result<InputCategory> {
    let path = status_file_path();
    let mut file = fs::File::open(&path)?;
    let mut content = String::new();
    file.read_to_string(&mut content)?;

    InputCategory::from_str(&content)
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidData, "Invalid status value"))
}

/// 상태 설정 (파일에 쓰기)
pub fn set_status(category: InputCategory) -> io::Result<()> {
    let path = status_file_path();

    // 디렉토리 생성
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut file = fs::File::create(&path)?;
    file.write_all(category.as_str().as_bytes())?;
    file.write_all(b"\n")?;

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_category_conversion() {
        assert_eq!(InputCategory::Korean.as_str(), "korean");
        assert_eq!(InputCategory::English.as_str(), "english");

        assert_eq!(
            InputCategory::from_str("korean"),
            Some(InputCategory::Korean)
        );
        assert_eq!(
            InputCategory::from_str("english"),
            Some(InputCategory::English)
        );
        assert_eq!(InputCategory::from_str("invalid"), None);
        assert_eq!(
            InputCategory::from_str("  korean  "),
            Some(InputCategory::Korean)
        );
    }

    #[test]
    fn test_status_file_path() {
        let path = status_file_path();
        assert!(path.ends_with("unim/status"));
    }
}
