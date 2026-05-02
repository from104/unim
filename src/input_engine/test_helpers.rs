//! 입력 엔진 테스트 공용 헬퍼.
//!
//! 여러 `tests_*.rs` 모듈이 공유하는 함수만 둔다. 한 그룹에서만 쓰는 헬퍼는
//! 해당 그룹 파일 내부에 그대로 둔다.

use super::InputEngine;
use crate::config::Config;

pub(super) fn create_test_engine() -> InputEngine {
    let config = Config::default();
    InputEngine::new(&config)
}
