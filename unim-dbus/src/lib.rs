//! UNIM DBus IPC 라이브러리
//!
//! DBus를 통한 모듈 간 통신을 위한 인터페이스 및 서비스 구현체를 제공합니다.
//!
//! ## 주요 컴포넌트
//!
//! - `interfaces`: DBus 인터페이스 정의 (`org.atit.unim.InputMethod`, `org.atit.unim.InputContext`)
//! - `service`: DBus 서비스 구현체 (서버 측)
//! - `client`: DBus 클라이언트 구현체 (프론트엔드 측)

pub mod client;
pub mod engine_worker;
pub mod ibus_compat;
pub mod interfaces;
pub mod service;

/// DBus 버스 이름
pub const BUS_NAME: &str = "org.atit.unim.InputMethod";

/// DBus 객체 경로 (InputMethod 서비스)
pub const INPUT_METHOD_PATH: &str = "/org/atit/unim/InputMethod";

/// DBus 객체 경로 접두사 (InputContext)
pub const INPUT_CONTEXT_PATH_PREFIX: &str = "/org/atit/unim/InputContext_";
