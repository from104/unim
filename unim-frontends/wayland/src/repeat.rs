//! 키 반복 상태 관리
//!
//! Wayland의 input-method-v2 프로토콜에서는 키 반복을 클라이언트가 직접 처리합니다.
//! 컴포지터가 RepeatInfo(rate, delay)를 전달하면, timerfd를 사용하여
//! delay 후 1/rate 간격으로 키 이벤트를 반복 생성합니다.

use std::os::fd::{AsFd, AsRawFd};
use std::time::Duration;

use nix::sys::time::TimeSpec;
use nix::sys::timerfd::{ClockId, Expiration, TimerFd, TimerFlags, TimerSetTimeFlags};

/// 키 반복 정보 (컴포지터에서 수신)
#[derive(Debug, Clone, Copy)]
pub struct RepeatInfo {
    /// 초당 반복 횟수
    pub rate: i32,
    /// 최초 반복까지의 지연 (밀리초)
    pub delay: i32,
}

/// 키 반복 상태 머신
#[derive(Debug, Clone, Copy)]
pub enum PressState {
    /// 키가 눌리지 않은 상태
    NotPressing,
    /// 키가 눌린 상태 (반복 대기 또는 반복 중)
    Pressing {
        /// 눌린 키의 evdev keycode
        key: u32,
        /// Wayland 이벤트 시간
        wayland_time: u32,
        /// delay → interval 전환 여부
        is_repeating: bool,
    },
}

impl PressState {
    /// 지정된 키가 현재 눌린 상태인지 확인
    pub fn is_pressing(&self, key: u32) -> bool {
        matches!(self, PressState::Pressing { key: k, .. } if *k == key)
    }
}

/// timerfd 래퍼 (mio::event::Source 구현)
pub struct RepeatTimer {
    fd: TimerFd,
}

impl RepeatTimer {
    /// 새 RepeatTimer 생성
    pub fn new() -> Self {
        let fd = TimerFd::new(ClockId::CLOCK_MONOTONIC, TimerFlags::TFD_NONBLOCK)
            .expect("timerfd 생성 실패");
        Self { fd }
    }

    /// 최초 반복 대기 타이머 설정 (oneshot)
    pub fn set_delay(&self, delay_ms: i32) {
        let duration = Duration::from_millis(delay_ms as u64);
        let timespec = TimeSpec::from_duration(duration);
        let _ = self
            .fd
            .set(Expiration::OneShot(timespec), TimerSetTimeFlags::empty());
    }

    /// 반복 간격 타이머 설정 (interval)
    pub fn set_interval(&self, rate: i32) {
        if rate <= 0 {
            self.cancel();
            return;
        }
        let interval_ns = 1_000_000_000u64 / rate as u64;
        let duration = Duration::from_nanos(interval_ns);
        let timespec = TimeSpec::from_duration(duration);
        let _ = self
            .fd
            .set(Expiration::Interval(timespec), TimerSetTimeFlags::empty());
    }

    /// 타이머 취소
    pub fn cancel(&self) {
        let zero = TimeSpec::from_duration(Duration::ZERO);
        let _ = self
            .fd
            .set(Expiration::OneShot(zero), TimerSetTimeFlags::empty());
    }

    /// 타이머 만료 횟수 읽기 (non-blocking)
    pub fn read(&self) -> std::io::Result<u64> {
        match self.fd.wait() {
            Ok(()) => Ok(1),
            Err(nix::Error::EAGAIN) => Ok(0),
            Err(e) => Err(std::io::Error::from_raw_os_error(e as i32)),
        }
    }
}

impl mio::event::Source for RepeatTimer {
    fn register(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        mio::unix::SourceFd(&self.fd.as_fd().as_raw_fd()).register(registry, token, interests)
    }

    fn reregister(
        &mut self,
        registry: &mio::Registry,
        token: mio::Token,
        interests: mio::Interest,
    ) -> std::io::Result<()> {
        mio::unix::SourceFd(&self.fd.as_fd().as_raw_fd()).reregister(registry, token, interests)
    }

    fn deregister(&mut self, registry: &mio::Registry) -> std::io::Result<()> {
        mio::unix::SourceFd(&self.fd.as_fd().as_raw_fd()).deregister(registry)
    }
}
