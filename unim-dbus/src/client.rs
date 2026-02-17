//! DBus 클라이언트 구현 (프론트엔드 측)
//!
//! XIM, Wayland 프론트엔드 등에서 사용할 DBus 클라이언트 프록시를 제공합니다.

use zbus::{proxy, Connection, Result};

/// InputMethod 서비스 프록시
#[proxy(
    interface = "org.atit.unim.InputMethod",
    default_service = "org.atit.unim.InputMethod",
    default_path = "/org/atit/unim/InputMethod"
)]
trait InputMethod {
    /// 새 입력 컨텍스트 생성 (window_id: 창 식별자, 빈 문자열이면 client_name 사용)
    fn create_input_context(&self, client_name: &str, window_id: &str) -> Result<String>;

    /// 전역 입력 모드 설정
    fn set_global_mode(&self, is_korean: bool) -> Result<()>;

    /// 전역 입력 모드 조회
    fn get_global_mode(&self) -> Result<bool>;

    /// 전역 모드 변경 시그널
    #[zbus(signal)]
    fn global_mode_changed(&self, is_korean: bool) -> Result<()>;
}

/// InputContext 프록시 생성을 위한 trait
#[proxy(
    interface = "org.atit.unim.InputContext",
    default_service = "org.atit.unim.InputMethod"
)]
trait InputContext {
    /// 키 이벤트 처리 - 반환: (consumed, preedit, commit)
    fn process_key_event(
        &self,
        keyval: u32,
        keycode: u32,
        state: u32,
    ) -> Result<(bool, String, String)>;

    /// 포커스 획득 (window_id: 창 식별자)
    fn focus_in(&self, window_id: &str) -> Result<()>;

    /// 포커스 상실 - 반환: 커밋된 텍스트
    fn focus_out(&self) -> Result<String>;

    /// 입력 상태 초기화
    fn reset(&self) -> Result<()>;

    /// 컨텍스트 파괴
    fn destroy(&self) -> Result<()>;

    /// 한자 후보 목록 조회 - 반환: (target, Vec<(hanja, meaning)>)
    fn get_hanja_candidates(&self) -> Result<(String, Vec<(String, String)>)>;

    /// 한자 선택 - 반환: 선택된 한자 문자열
    fn select_hanja(&self, index: u32) -> Result<String>;

    /// 한자 모드 취소
    fn cancel_hanja(&self) -> Result<()>;

    /// Preedit 텍스트 업데이트 시그널
    #[zbus(signal)]
    fn update_preedit_text(&self, text: String, cursor_pos: u32, visible: bool) -> Result<()>;

    /// 텍스트 커밋 시그널
    #[zbus(signal)]
    fn commit_text(&self, text: String) -> Result<()>;
}

/// DBus 클라이언트 연결 관리자
pub struct UnimClient {
    connection: Connection,
}

impl UnimClient {
    /// 세션 버스에 연결
    pub async fn connect() -> Result<Self> {
        let connection = Connection::session().await?;
        Ok(Self { connection })
    }

    /// InputMethod 서비스 프록시 획득
    pub async fn input_method(&self) -> Result<InputMethodProxy<'_>> {
        InputMethodProxy::new(&self.connection).await
    }

    /// 특정 경로의 InputContext 프록시 획득
    pub async fn input_context<'a>(&'a self, path: &'a str) -> Result<InputContextProxy<'a>> {
        InputContextProxy::builder(&self.connection)
            .path(path)?
            .build()
            .await
    }

    /// 새 입력 컨텍스트 생성 후 경로 반환
    pub async fn create_context(&self, client_name: &str, window_id: &str) -> Result<String> {
        let im = self.input_method().await?;
        im.create_input_context(client_name, window_id).await
    }
}
