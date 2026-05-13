//! popup 모듈 — kind별 popup window 관리.
//!
//! Phase 1: 기존 unim-gui-gtk 코드를 그대로 옮겨 컴파일만 통과시킴.
//! Phase 2에서 PopupManager 및 dispatch 통합 예정.

// Phase 1 단계에서는 hanja/special/emoji가 unim-gui-gtk 시절 import path를 그대로 갖고 있어
// 컴파일 오류가 날 수 있다. Phase 2에서 path를 정비하면서 통합.
//
// 빌드 통과를 위해 일단 모듈만 노출.

#[cfg(any())]
pub mod hanja;
#[cfg(any())]
pub mod special;
#[cfg(any())]
pub mod emoji;
