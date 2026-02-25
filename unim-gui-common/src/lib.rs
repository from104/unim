//! UNIM GUI 공통 모듈
//!
//! GTK나 Qt 같은 툴킷에 의존하지 않는 순수 Rust 구현체 모음입니다.
//! DBus 통신, 시스템 트레이(ksni), 공통 타입, 상태 관리 등을 제공합니다.

pub mod dbus_client;
pub mod tray;
pub mod types;
