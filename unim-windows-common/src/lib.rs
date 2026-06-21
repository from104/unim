//! UNIM Windows 공용 저수준 Win32/COM glue.
//!
//! consumer: `unim-tsf`(TSF cdylib), `unim-imm32`(IMM32 .ime cdylib).
//! Linux/macOS 빌드에는 들어가지 않는다 (모든 모듈 cfg(windows)).
//!
//! 비포함(assessment 근거): popup 와이어타입(serde, 별 크레이트), synth_input(TSF 특화 dead),
//! DllMain hinst 저장(자료구조 비대칭), windows feature 재노출.

#![cfg(windows)]

pub mod registry;
pub mod modifier;
pub mod debug;
pub mod activation;

pub use activation::remove_substitute_and_assembly;
