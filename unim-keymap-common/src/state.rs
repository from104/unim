//! 두 바이너리가 공유하는 가벼운 상태 컨테이너.
//!
//! `ProfileRegistry`는 두 바이너리 모두 단일 인스턴스를 들고 다니면 충분하므로
//! `Rc<RefCell<_>>`로 감싼다. GTK4는 단일 스레드 UI라 `Send`/`Sync`가 필요 없다.

use std::cell::RefCell;
use std::rc::Rc;

use unim::keystroke::profile::ProfileRegistry;

/// 사이드바·탭 위젯이 공유하는 레지스트리 핸들.
pub type SharedRegistry = Rc<RefCell<ProfileRegistry>>;

/// 기본 사용자 디렉토리(`~/.config/unim/layouts/`)로 레지스트리를 생성해 감싸기.
pub fn new_shared_registry() -> SharedRegistry {
    Rc::new(RefCell::new(ProfileRegistry::new()))
}
