/// 테스트 시나리오 정의
///
/// 레이아웃별 evdev keycode 매핑:
///
/// === 2벌식 (Dubeolsik) ===
///   G(34)=ㅎ K(37)=ㅏ S(31)=ㄴ R(19)=ㄱ M(50)=ㅡ F(33)=ㄹ
///   Shift+R(19)=ㄲ
///
/// === 3벌식 390 (Sebeolsik390) ===
///   초성: M(50)=ㅎ, K(37)=ㄱ, H(35)=ㄴ, Y(21)=ㄹ
///   중성: F(33)=ㅏ, G(34)=ㅡ, D(32)=ㅣ
///   종성: S(31)=ㄴ, W(17)=ㄹ, X(45)=ㄱ
///
/// Backspace=14, Space=57, Enter=28, Hangul=122
/// 키 입력 한 단위
pub struct KeyPress {
    pub keycode: u32,
    pub state: u32,
}

impl KeyPress {
    pub const fn new(keycode: u32) -> Self {
        Self { keycode, state: 0 }
    }

    pub const fn shift(keycode: u32) -> Self {
        Self { keycode, state: 1 }
    }
}

/// 테스트 시나리오
pub struct TestScenario {
    pub name: &'static str,
    pub keys: Vec<KeyPress>,
    /// 모든 키 입력 후 마지막 preedit
    pub expected_final_preedit: &'static str,
    /// 모든 키 입력 중 누적된 commit 문자열
    pub expected_total_commit: &'static str,
    /// 각 키 입력 후의 (preedit, commit) 기대값
    pub steps: Option<Vec<(&'static str, &'static str)>>,
    /// 테스트 전 한글 모드 설정
    pub initial_korean_mode: bool,
}

/// 지원하는 레이아웃
pub enum Layout {
    Dubeolsik,
    Sebeolsik390,
}

impl Layout {
    pub fn from_name(name: &str) -> Self {
        match name {
            "3bul390" | "Sebeolsik390" => Layout::Sebeolsik390,
            _ => Layout::Dubeolsik,
        }
    }

    pub fn display_name(&self) -> &'static str {
        match self {
            Layout::Dubeolsik => "2벌식",
            Layout::Sebeolsik390 => "3벌식 390",
        }
    }
}

/// 레이아웃에 맞는 시나리오 반환
pub fn scenarios_for_layout(layout: &Layout) -> Vec<TestScenario> {
    match layout {
        Layout::Dubeolsik => dubeolsik_scenarios(),
        Layout::Sebeolsik390 => sebeolsik390_scenarios(),
    }
}

/// 레이아웃 공통 테스트 (영문 모드, 모드 전환 등)
pub fn common_scenarios() -> Vec<TestScenario> {
    vec![TestScenario {
        name: "영문 모드: 키 패스스루",
        keys: vec![KeyPress::new(34)], // G in English mode
        expected_final_preedit: "",
        expected_total_commit: "",
        steps: None,
        initial_korean_mode: false,
    }]
}

// ─── 2벌식 시나리오 ────────────────────────────────────────────────

fn dubeolsik_scenarios() -> Vec<TestScenario> {
    vec![
        TestScenario {
            name: "[2벌] 초성 단독: ㅎ",
            keys: vec![KeyPress::new(34)], // G
            expected_final_preedit: "ㅎ",
            expected_total_commit: "",
            steps: Some(vec![("ㅎ", "")]),
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[2벌] 초성+중성: 하",
            keys: vec![KeyPress::new(34), KeyPress::new(37)], // G, K
            expected_final_preedit: "하",
            expected_total_commit: "",
            steps: Some(vec![("ㅎ", ""), ("하", "")]),
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[2벌] 완성형: 한",
            keys: vec![
                KeyPress::new(34), // G → ㅎ
                KeyPress::new(37), // K → 하
                KeyPress::new(31), // S → 한
            ],
            expected_final_preedit: "한",
            expected_total_commit: "",
            steps: Some(vec![("ㅎ", ""), ("하", ""), ("한", "")]),
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[2벌] 두 글자: 한글",
            keys: vec![
                KeyPress::new(34), // G → ㅎ
                KeyPress::new(37), // K → 하
                KeyPress::new(31), // S → 한
                KeyPress::new(19), // R → 한ㄱ → commit 한, preedit ㄱ
                KeyPress::new(50), // M → 그
                KeyPress::new(33), // F → 글
            ],
            expected_final_preedit: "글",
            expected_total_commit: "한",
            steps: None,
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[2벌] 쌍자음: ㄲ",
            keys: vec![KeyPress::shift(19)], // Shift+R
            expected_final_preedit: "ㄲ",
            expected_total_commit: "",
            steps: None,
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[2벌] Backspace 조합 중 삭제",
            keys: vec![
                KeyPress::new(34), // G → ㅎ
                KeyPress::new(37), // K → 하
                KeyPress::new(14), // BS → ㅎ
            ],
            expected_final_preedit: "ㅎ",
            expected_total_commit: "",
            steps: Some(vec![("ㅎ", ""), ("하", ""), ("ㅎ", "")]),
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[2벌] 스페이스 조합 확정",
            keys: vec![
                KeyPress::new(34), // G → ㅎ
                KeyPress::new(37), // K → 하
                KeyPress::new(57), // Space
            ],
            expected_final_preedit: "",
            expected_total_commit: "하 ",
            steps: None,
            initial_korean_mode: true,
        },
    ]
}

// ─── 3벌식 390 시나리오 ────────────────────────────────────────────

fn sebeolsik390_scenarios() -> Vec<TestScenario> {
    // 3벌식 390 evdev keycodes:
    // 초성: M(50)=ㅎ, K(37)=ㄱ, H(35)=ㄴ, Y(21)=ㄹ, U(22)=ㄷ,
    //       I(23)=ㅁ, J(36)=ㅇ, N(49)=ㅅ, L(38)=ㅈ, ;(39)=ㅂ,
    //       O(24)=ㅊ, P(25)=ㅍ, '(40)=ㅌ
    // 중성: F(33)=ㅏ, G(34)=ㅡ, D(32)=ㅣ, E(18)=ㅕ, R(19)=ㅐ,
    //       T(20)=ㅓ, V(47)=ㅗ, B(48)=ㅜ, C(46)=ㅔ
    // 종성: S(31)=ᆫ(ㄴ), W(17)=ᆯ(ㄹ), X(45)=ᆨ(ㄱ), A(30)=ᆼ(ㅇ),
    //       Z(44)=ᆷ(ㅁ), Q(16)=ᆺ(ㅆ)
    // Shift 중성: R(19)=ㅒ
    vec![
        TestScenario {
            name: "[3벌390] 초성 단독: ㅎ",
            keys: vec![KeyPress::new(50)], // M → ㅎ
            expected_final_preedit: "ㅎ",
            expected_total_commit: "",
            steps: Some(vec![("ㅎ", "")]),
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[3벌390] 초성+중성: 하",
            keys: vec![
                KeyPress::new(50), // M → ㅎ
                KeyPress::new(33), // F → 하
            ],
            expected_final_preedit: "하",
            expected_total_commit: "",
            steps: Some(vec![("ㅎ", ""), ("하", "")]),
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[3벌390] 완성형: 한",
            keys: vec![
                KeyPress::new(50), // M → ㅎ (초성)
                KeyPress::new(33), // F → 하 (중성)
                KeyPress::new(31), // S → 한 (종성 ㄴ)
            ],
            expected_final_preedit: "한",
            expected_total_commit: "",
            steps: Some(vec![("ㅎ", ""), ("하", ""), ("한", "")]),
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[3벌390] 두 글자: 한글",
            keys: vec![
                KeyPress::new(50), // M → ㅎ (초성)
                KeyPress::new(33), // F → 하 (중성)
                KeyPress::new(31), // S → 한 (종성 ㄴ)
                KeyPress::new(37), // K → ㄱ (다음 초성, 한 commit)
                KeyPress::new(34), // G → 그 (중성 ㅡ)
                KeyPress::new(17), // W → 글 (종성 ㄹ)
            ],
            expected_final_preedit: "글",
            expected_total_commit: "한",
            steps: None,
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[3벌390] 중성 단독: ㅏ",
            keys: vec![KeyPress::new(33)], // F → ㅏ
            expected_final_preedit: "ㅏ",
            expected_total_commit: "",
            steps: None,
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[3벌390] Backspace 조합 중 삭제",
            keys: vec![
                KeyPress::new(50), // M → ㅎ
                KeyPress::new(33), // F → 하
                KeyPress::new(14), // BS → ㅎ
            ],
            expected_final_preedit: "ㅎ",
            expected_total_commit: "",
            steps: Some(vec![("ㅎ", ""), ("하", ""), ("ㅎ", "")]),
            initial_korean_mode: true,
        },
        TestScenario {
            name: "[3벌390] 스페이스 조합 확정",
            keys: vec![
                KeyPress::new(50), // M → ㅎ
                KeyPress::new(33), // F → 하
                KeyPress::new(57), // Space
            ],
            expected_final_preedit: "",
            expected_total_commit: "하 ",
            steps: None,
            initial_korean_mode: true,
        },
    ]
}
