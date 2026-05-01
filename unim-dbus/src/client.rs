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

    /// 이모지 검색
    fn search_emoji(&self, keyword: &str) -> Result<Vec<String>>;

    /// 설정값 조회
    fn get_config(&self, key: &str) -> Result<String>;

    /// 설정값 변경
    fn set_config(&self, key: &str, value: &str) -> Result<()>;

    /// 전체 설정을 YAML 문자열로 조회
    fn get_config_yaml(&self) -> Result<String>;

    /// 전체 설정을 JSON 문자열로 조회 (ConfigChangedJson signal과 동일 직렬화)
    fn get_config_json(&self) -> Result<String>;

    /// 전체 설정을 YAML 문자열로 수신하여 저장·반영·브로드캐스트
    fn set_config_yaml(&self, yaml: &str) -> Result<()>;

    /// 외부 단축키 우회 트리거 — InputContext 비보유 클라이언트(CLI, 외부 단축키 도구)용.
    ///
    /// `InputContextProxy::trigger_action`은 InputContext를 가진 클라이언트(GNOME extension)용,
    /// 본 메서드는 어느 InputContext인지 알 수 없는 환경(KDE/Hyprland/Sway, AHK 등)에서
    /// 동일한 동작을 가능하게 한다. cursor 좌표는 마지막으로 보고된 전역 캐시 사용.
    /// 지원 액션: `"emoji_popup"`. 알 수 없는 액션은 데몬이 무시한다.
    ///
    /// 데몬은 본 호출에 대해 `org.atit.unim.InputContext` 인터페이스의
    /// `ShowEmojiPopup` 시그널을 InputMethod path에서 발행한다 — 기존 GUI 구독
    /// (`path_namespace=/org/atit/unim`, `interface=org.atit.unim.InputContext`)이
    /// 그대로 수신한다. 별도의 InputMethod-level 시그널은 추가하지 않는다 (중복 방지).
    fn trigger_action(&self, action: &str) -> Result<()>;

    /// 글로벌 이모지 커밋 — InputContext 비보유 경로에서 마지막 활성 입력 컨텍스트로 redirect.
    ///
    /// GNOME extension처럼 자체 InputContext를 가진 클라이언트라도 GTK4_IM_MODULE=unim 환경에서는
    /// extension의 컨텍스트로 commit하면 GTK4_IM 모듈을 우회하여 사용자 앱에 도달하지 못한다.
    /// 본 메서드는 데몬이 캐시한 `last_active_input_context_path`(GTK3/4·Qt5/6·XIM·Wayland 중
    /// 마지막 포커스된 컨텍스트)를 향해 `CommitText` + `HidePopup` 시그널을 발행하여
    /// IM 모듈이 이를 받아 사용자 앱에 그대로 commit되게 한다. emoji MRU 즐겨찾기도 갱신.
    ///
    /// 캐시가 비어있으면 데몬이 경고 로그만 남기고 no-op (호환성 유지).
    fn commit_emoji(&self, emoji: &str) -> Result<()>;

    /// 전역 모드 변경 시그널
    #[zbus(signal)]
    fn global_mode_changed(&self, is_korean: bool) -> Result<()>;

    /// 레거시 단일 키 설정 변경 시그널
    #[zbus(signal)]
    fn config_changed(&self, key: String, value: String) -> Result<()>;

    /// 전체 Config JSON payload 설정 변경 시그널
    #[zbus(signal)]
    fn config_changed_json(&self, json: String) -> Result<()>;
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

    /// 현재 한자 후보 목록의 즐겨찾기 상태 조회 (GetHanjaCandidates 순서와 동일)
    fn get_hanja_bookmark_states(&self) -> Result<Vec<bool>>;

    /// 한자 후보 즐겨찾기 토글 - 반환: (index, bookmarked)
    fn toggle_hanja_bookmark(&self, index: u32) -> Result<(u32, bool)>;

    /// 한자 모드 취소 - 반환: 커밋된 트리거 문자
    fn cancel_hanja(&self) -> Result<String>;

    /// 특수문자 후보 조회 - 반환: (target, characters, top_row)
    fn get_special_char_candidates(&self) -> Result<(String, Vec<String>, String)>;

    /// 특수문자 선택 - 반환: 선택된 문자열
    fn select_special_char(&self, index: u32) -> Result<String>;

    /// 특수문자 모드 취소 - 반환: 커밋된 트리거 문자
    fn cancel_special_char(&self) -> Result<String>;

    /// 커서 위치 보고 (팝업 포지셔닝용)
    fn report_cursor_rect(&self, x: i32, y: i32, width: i32, height: i32) -> Result<()>;

    /// 입력 필드 목적 설정 (비밀번호/PIN 감지)
    fn set_content_type(&self, purpose: u32) -> Result<()>;

    /// Surrounding text 설정
    fn set_surrounding_text(&self, text: &str, cursor_pos: u32, anchor_pos: u32) -> Result<()>;

    /// Smart Backspace (자모 단위 삭제)
    /// 반환: (삭제 문자 수, 대체 텍스트)
    fn smart_backspace(&self) -> Result<(u32, String)>;

    /// 외부 단축키 우회 트리거
    ///
    /// GNOME Wayland 등 컴포지터가 시스템 조합 키를 가로채는 환경에서, extension
    /// 이 단축키를 직접 캡처해 데몬에 동작을 요청할 때 사용한다.
    /// 지원 액션: `"emoji_popup"`. 알 수 없는 액션은 데몬이 무시한다.
    fn trigger_action(&self, action: &str) -> Result<()>;

    /// delete_surrounding_text 시그널
    #[zbus(signal)]
    fn delete_surrounding_text(&self, offset: i32, n_chars: u32) -> Result<()>;

    /// Preedit 텍스트 업데이트 시그널
    #[zbus(signal)]
    fn update_preedit_text(&self, text: String, cursor_pos: u32, visible: bool) -> Result<()>;

    /// 텍스트 커밋 시그널
    #[zbus(signal)]
    fn commit_text(&self, text: String) -> Result<()>;

    /// 한자 팝업 표시 시그널
    ///
    /// `top_row`는 활성 영문 키맵의 가장 위 9문자 (특수문자와 동일 source).
    /// expanded(9x9) 컬럼 헤더와 Letter 키 점프에 사용한다.
    #[zbus(signal)]
    fn show_hanja_popup(
        &self,
        target: String,
        candidates: Vec<(String, String)>,
        top_row: String,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    ) -> Result<()>;

    /// 특수문자 팝업 표시 시그널
    #[zbus(signal)]
    fn show_special_popup(
        &self,
        target: String,
        characters: Vec<String>,
        top_row: String,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    ) -> Result<()>;

    /// 이모지 팝업 표시 시그널 (단축키 우회 트리거)
    ///
    /// GNOME extension 등이 [`InputContextProxy::trigger_action`]("emoji_popup")
    /// 호출 시 데몬이 발행한다. cursor 좌표는 가장 최근 보고된 위치.
    #[zbus(signal)]
    fn show_emoji_popup(
        &self,
        cursor_x: i32,
        cursor_y: i32,
        cursor_width: i32,
        cursor_height: i32,
    ) -> Result<()>;

    /// AutoTypeFix 교정 시그널 (엔진 → 프론트엔드)
    /// delete_chars 삭제 → commit_text 커밋 → preedit_text를 preedit으로
    #[zbus(signal)]
    fn auto_typefix_apply(
        &self,
        delete_chars: u32,
        commit_text: String,
        preedit_text: String,
    ) -> Result<()>;

    /// 팝업 숨김 시그널
    #[zbus(signal)]
    fn hide_popup(&self) -> Result<()>;

    /// 한자 즐겨찾기 변경 시그널 (엔진 → 프론트엔드)
    #[zbus(signal)]
    fn hanja_bookmark_changed(&self, index: u32, bookmarked: bool) -> Result<()>;

    /// 한자 후보 재정렬 시그널 (즐겨찾기 토글 후 즐겨찾기 우선 정렬)
    #[zbus(signal)]
    fn hanja_candidates_reordered(
        &self,
        target: String,
        hanjas: Vec<String>,
        meanings: Vec<String>,
        bookmarks: Vec<bool>,
        new_cursor: u32,
        page: i32,
        sel_row: i32,
        sel_col: i32,
        bookmarked: bool,
    ) -> Result<()>;

    /// 팝업 네비게이션 시그널 (페이지/선택 변경)
    #[zbus(signal)]
    fn popup_navigate(
        &self,
        page: i32,
        total_pages: i32,
        selected: i32,
        rows: i32,
        cols: i32,
        sel_row: i32,
        sel_col: i32,
    ) -> Result<()>;
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
