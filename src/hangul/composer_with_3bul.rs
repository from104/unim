use crate::hangul::char::HangulChar;
use crate::hangul::composer::BaseHangulComposer;
use crate::hangul::composer::HangulComposer;
use crate::hangul::jamo::*;
use std::collections::{HashMap, VecDeque};

/**
 * 3벌식 자판 레이아웃에 특화된 한글 조합기입니다.
 *
 * 이 조합기는 [`BaseHangulComposer`]를 기반으로 하며, 3벌식 입력 방식의 특수한 규칙을 적용하여
 * 한글 음절을 조합합니다. 3벌식 자판은 초성, 중성, 종성이 별도의 키에 할당되어 있어,
 * 2벌식과는 다른 조합 규칙이 필요합니다.
 *
 * 예를 들어, 3벌식에서는 초성이나 중성 없이 종성이 먼저 입력될 수 없으며,
 * 중성이나 종성 다음에 초성이 오는 경우, 또는 종성 다음에 중성이 오는 경우는
 * 일반적으로 새로운 음절의 시작으로 간주됩니다.
 *
 * # 주요 기능
 * - [`JamoEnum`] 자모를 입력받아 조합 (`add_jamo`)
 * - 마지막 입력 자모 제거 (`remove_jamo`)
 * - 현재 큐의 자모를 바탕으로 음절 조합 시도 (`compose_hangul`)
 * - 강제 음절 완성 및 상태 초기화 (`force_compose_hangul`)
 * - 3벌식 조합 규칙에 따른 자모 조합 테이블 초기화 (`initialize_combined_jamo`)
 *
 * # 관련 모듈
 * - [`crate::hangul::jamo`]: 한글 자모(초성, 중성, 종성) 정의
 * - [`crate::hangul::char`]: 한글 음절 구조체 및 관련 기능
 * - [`crate::hangul::composer`]: 한글 조합기 트레이트 및 기본 구현
 */
#[derive(Debug, Default)]
pub struct HangulComposer3Bul {
    /// 기본적인 한글 조합 로직을 처리하는 내부 조합기 인스턴스입니다.
    base_composer: BaseHangulComposer,
}

impl HangulComposer3Bul {
    /**
     * 새로운 `HangulComposer3Bul` 인스턴스를 생성합니다.
     *
     * 내부적으로 `BaseHangulComposer`를 생성하고, 3벌식에 필요한
     * 자모 조합 규칙 테이블을 초기화합니다.
     *
     * # 반환값
     *
     * 초기화된 `HangulComposer3Bul` 인스턴스.
     */
    pub fn new() -> Self {
        let mut composer = HangulComposer3Bul {
            base_composer: BaseHangulComposer::new(),
        };
        composer.initialize_combined_jamo();
        composer
    }

    /**
     * 3벌식 조합 규칙에 필요한 복합 자모 테이블을 초기화합니다.
     *
     * 이 메서드는 `BaseHangulComposer`가 가지고 있는 `combined_jamo` 해시맵에
     * 3벌식에서 사용되는 초성, 중성, 종성의 조합 규칙을 설정합니다.
     * 예를 들어, 'ㄱ' + 'ㄱ' -> 'ㄲ', 'ㅗ' + 'ㅏ' -> 'ㅘ', 'ㄴ' + 'ㅈ' -> 'ㄵ' 등의
     * 규칙을 정의합니다.
     *
     * `new` 함수 내부에서 호출되어 인스턴스 생성 시 초기화됩니다.
     */
    fn initialize_combined_jamo(&mut self) {
        let mut combined_jamo = HashMap::new();

        // --- 초성 조합 규칙 ---
        let mut g_map = HashMap::new();
        g_map.insert(JamoEnum::Cho(Cho::G), JamoEnum::Cho(Cho::GG));
        combined_jamo.insert(JamoEnum::Cho(Cho::G), g_map);

        let mut d_map = HashMap::new();
        d_map.insert(JamoEnum::Cho(Cho::D), JamoEnum::Cho(Cho::DD));
        combined_jamo.insert(JamoEnum::Cho(Cho::D), d_map);

        let mut b_map = HashMap::new();
        b_map.insert(JamoEnum::Cho(Cho::B), JamoEnum::Cho(Cho::BB));
        combined_jamo.insert(JamoEnum::Cho(Cho::B), b_map);

        let mut s_map = HashMap::new();
        s_map.insert(JamoEnum::Cho(Cho::S), JamoEnum::Cho(Cho::SS));
        combined_jamo.insert(JamoEnum::Cho(Cho::S), s_map);

        let mut j_map = HashMap::new();
        j_map.insert(JamoEnum::Cho(Cho::J), JamoEnum::Cho(Cho::JJ));
        combined_jamo.insert(JamoEnum::Cho(Cho::J), j_map);

        // --- 중성 조합 규칙 ---
        let mut o_map = HashMap::new();
        o_map.insert(JamoEnum::Jung(Jung::A), JamoEnum::Jung(Jung::WA));
        o_map.insert(JamoEnum::Jung(Jung::AE), JamoEnum::Jung(Jung::WAE));
        o_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::OE));
        combined_jamo.insert(JamoEnum::Jung(Jung::O), o_map);

        let mut u_map = HashMap::new();
        u_map.insert(JamoEnum::Jung(Jung::EO), JamoEnum::Jung(Jung::WEO));
        u_map.insert(JamoEnum::Jung(Jung::E), JamoEnum::Jung(Jung::WE));
        u_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::WI));
        combined_jamo.insert(JamoEnum::Jung(Jung::U), u_map);

        let mut eu_map = HashMap::new();
        eu_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::YI));
        combined_jamo.insert(JamoEnum::Jung(Jung::EU), eu_map);

        // --- 종성 조합 규칙 ---
        let mut jong_g_map = HashMap::new();
        jong_g_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::GG));
        jong_g_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::GS));
        combined_jamo.insert(JamoEnum::Jong(Jong::G), jong_g_map);

        let mut n_map = HashMap::new();
        n_map.insert(JamoEnum::Jong(Jong::J), JamoEnum::Jong(Jong::NJ));
        n_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::NH));
        combined_jamo.insert(JamoEnum::Jong(Jong::N), n_map);

        let mut l_map = HashMap::new();
        l_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::LG));
        l_map.insert(JamoEnum::Jong(Jong::M), JamoEnum::Jong(Jong::LM));
        l_map.insert(JamoEnum::Jong(Jong::B), JamoEnum::Jong(Jong::LB));
        l_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::LS));
        l_map.insert(JamoEnum::Jong(Jong::T), JamoEnum::Jong(Jong::LT));
        l_map.insert(JamoEnum::Jong(Jong::P), JamoEnum::Jong(Jong::LP));
        l_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::LH));
        combined_jamo.insert(JamoEnum::Jong(Jong::L), l_map);

        let mut jong_b_map = HashMap::new();
        jong_b_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::BS));
        combined_jamo.insert(JamoEnum::Jong(Jong::B), jong_b_map);

        let mut s_jong_map = HashMap::new();
        s_jong_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::SS));
        combined_jamo.insert(JamoEnum::Jong(Jong::S), s_jong_map);

        // 생성된 조합 테이블을 base_composer의 테이블에 설정
        *self.base_composer.combined_jamo() = combined_jamo;
    }
}

impl HangulComposer for HangulComposer3Bul {
    /**
     * 3벌식 규칙에 따라 한글 자모를 입력받아 현재 조합 상태에 추가합니다.
     *
     * 입력된 자모(`jamo`)를 현재 조합 중인 자모 시퀀스(`jamo_queue`)의 끝에 추가하고,
     * `compose_hangul`을 호출하여 조합을 시도합니다.
     *
     * 만약 `compose_hangul` 호출이 실패하면 (즉, 입력된 자모가 현재 조합 상태와
     * 3벌식 규칙상 결합될 수 없는 경우), 이는 이전 상태까지의 자모들이 하나의
     * 완성된 음절을 이룬다는 것을 의미합니다. 이 경우 다음 단계를 따릅니다:
     * 1. 입력된 `jamo`를 `jamo_queue`에서 제거합니다.
     * 2. 다시 `compose_hangul`을 호출하여 이전 상태(`jamo`가 추가되기 전)의
     *    `current_hangul` 상태를 복원합니다.
     * 3. 복원된 `current_hangul`로부터 완성된 음절 문자(`complete_hangul`)를 얻습니다.
     *    (`get_syllable`은 `Result<char, HangulError>`를 반환하므로 `.ok()`를 사용합니다.)
     * 4. 이전 큐 상태를 last_jamo_queue에 저장
     * 5. 현재 큐를 비우고 새 자모만 추가
     * 6. current_hangul 상태 초기화 (clear_jamo 호출)
     * 7. 다시 `compose_hangul`을 호출하여 `jamo` 하나만 있는 상태로 `current_hangul`을 설정합니다.
     * 8. 3단계에서 얻은 `complete_hangul`을 `Some(char)`로 감싸 반환합니다.
     *
     * `compose_hangul` 호출이 성공하면 (즉, 입력된 자모가 현재 조합에 유효하게
     * 추가될 수 있는 경우), 조합이 계속 진행 중임을 의미하며 `None`을 반환합니다.
     *
     * # 매개변수
     *
     * * `jamo`: 입력할 한글 자모 (`JamoEnum`).
     *
     * # 반환값
     *
     * * `Some(char)`: 입력된 `jamo`로 인해 이전 음절 조합이 완료된 경우, 완성된 한글 음절.
     * * `None`: 조합이 계속 진행 중인 경우.
     */
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        // 현재 큐 상태를 복사하여 실패 시 복원 용도로 사용
        let original_queue = self.base_composer.jamo_queue().clone();

        // 새로운 자모 추가
        self.base_composer.jamo_queue().push_back(jamo);

        // 조합 시도
        if self.compose_hangul() {
            // 조합 성공: 조합 계속 진행
            None
        } else {
            // 조합 실패: 이전 음절 완성 및 새 음절 시작

            // 1. 추가했던 자모 제거 (큐 복원으로 대체)
            *self.base_composer.jamo_queue() = original_queue;

            // 2. 이전 상태로 `current_hangul` 복원 (조합 재실행)
            self.compose_hangul(); // 실패할 수 없음 (이전 상태는 유효했으므로)

            // 3. 완성된 음절 얻기
            let complete_hangul = self.base_composer.current_hangul().get_syllable().ok(); // Linter 오류 수정: .ok() 추가

            // 4. 이전 큐 상태를 last_jamo_queue에 저장
            let current_jamo_queue_content: Vec<_> =
                self.base_composer.jamo_queue().iter().copied().collect();
            self.base_composer.last_jamo_queue().clear();
            self.base_composer
                .last_jamo_queue()
                .extend(current_jamo_queue_content); // 복사된 내용 사용

            // 5. 현재 큐를 비우고 새 자모만 추가
            self.base_composer.jamo_queue().clear();
            self.base_composer.jamo_queue().push_back(jamo);

            // 6. current_hangul 상태 초기화 (clear_jamo 호출)
            self.clear_jamo(); // BaseHangulComposer의 clear_jamo 호출

            // 7. 새 자모로 current_hangul 상태 설정
            self.compose_hangul(); // 새 자모 하나로 조합

            // 8. 완성된 이전 음절 반환
            complete_hangul
        }
    }

    /**
     * 마지막으로 입력된 한글 자모를 제거하고 조합 상태를 갱신합니다.
     *
     * `BaseHangulComposer`의 `remove_jamo` 구현을 그대로 사용합니다.
     * 큐에서 마지막 자모를 제거하고, 남은 자모로 `compose_hangul`을 호출하여
     * `current_hangul` 상태를 업데이트합니다.
     *
     * # 반환값
     *
     * * `Some(JamoEnum)`: 성공적으로 제거된 자모.
     * * `None`: 제거할 자모가 없는 경우 (조합 큐가 비어 있음).
     */
    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        self.base_composer.remove_jamo()
    }

    /**
     * 현재 `jamo_queue`에 저장된 자모들을 바탕으로 3벌식 규칙에 따라 한글 음절을 조합합니다.
     *
     * 먼저, 큐가 비어있는지 확인하고 비어있다면 상태를 초기화하고 `true`를 반환합니다.
     *
     * 그 다음, 3벌식에서 유효하지 않은 자모 시퀀스를 검사합니다:
     * 1. 중성이 아직 채워지지 않았는데 종성이 입력된 경우 (`!is_filled_jung` && `Jong`)
     * 2. 중성이나 종성 다음에 초성이 입력된 경우 (`Jung`|`Jong` -> `Cho`)
     * 3. 종성 다음에 중성이 입력된 경우 (`Jong` -> `Jung`)
     *
     * 위의 규칙에 위배되면 조합에 실패한 것으로 간주하고 `false`를 반환합니다.
     *
     * 3벌식 특수 규칙 검사를 통과하면, `BaseHangulComposer`의 `compose_cho`, `compose_jung`,
     * `compose_jong` 메서드를 순서대로 호출하여 실제 자모 조합을 수행하고
     * `current_hangul_char` 상태를 업데이트합니다. 이 과정 중 하나라도 실패하면 `false`를 반환합니다.
     *
     * 모든 조합 과정이 성공하면 `true`를 반환합니다.
     *
     * # 반환값
     *
     * * `true`: 조합에 성공했거나, 큐가 비어 있어 초기화된 경우.
     * * `false`: 3벌식 자모 조합 규칙에 맞지 않거나 내부 조합 로직(초/중/종성 조합)에서 실패한 경우.
     */
    fn compose_hangul(&mut self) -> bool {
        // 큐가 비어있는지 먼저 확인
        if self.base_composer.jamo_queue().is_empty() {
            self.clear_jamo();
            return true;
        }

        // 마지막 두 자모와 현재 중성 채움 상태 확인
        let (last_jamo, last_prev_jamo) = {
            let queue = self.base_composer.jamo_queue(); // 첫 번째 mutable borrow 시작
            let last = *queue.back().unwrap(); // 값 복사 (JamoEnum은 Copy)
            let prev = if queue.len() > 1 {
                Some(queue[queue.len() - 2]) // 값 복사
            } else {
                None
            };
            (last, prev) // 첫 번째 mutable borrow 끝
        };
        let is_filled_jung = {
            // current_hangul()이 &mut HangulChar를 반환하므로 mutable borrow 발생
            self.base_composer.current_hangul().is_filled_jung() // 두 번째 mutable borrow 시작 및 끝
        };

        // 3벌식 특수 규칙 검사
        match (last_prev_jamo, last_jamo) {
            // 중성 없이 종성이 먼저 오는 경우 (첫 자모가 종성이거나, 초성 다음에 종성)
            (_, JamoEnum::Jong(_))
                if !is_filled_jung
                    && (last_prev_jamo.is_none()
                        || matches!(last_prev_jamo, Some(JamoEnum::Cho(_)))) =>
            {
                return false;
            }
            // 중성이나 종성 다음에 초성이 오는 경우
            (Some(JamoEnum::Jung(_) | JamoEnum::Jong(_)), JamoEnum::Cho(_)) => return false,
            // 종성 다음에 중성이 오는 경우
            (Some(JamoEnum::Jong(_)), JamoEnum::Jung(_)) => return false,
            // 그 외 경우는 3벌식 특수 규칙에 해당하지 않음
            _ => {}
        }

        // 기본 조합 로직 위임 (초성, 중성, 종성 순서로 조합)
        if !self.base_composer.compose_cho() {
            return false;
        }
        if !self.base_composer.compose_jung() {
            return false;
        }
        if !self.base_composer.compose_jong() {
            return false;
        }

        // 모든 검사와 조합이 성공
        true
    }

    /**
     * 현재까지 입력된 자모들을 강제로 조합하여 완성된 한글 음절을 반환하고, 조합 상태를 초기화합니다.
     *
     * `BaseHangulComposer`의 `force_compose_hangul` 구현을 그대로 사용합니다.
     * 현재 조합 중(`is_compose()`가 `true`)일 때만 동작하며, 성공 시 조합 상태를 모두 초기화합니다.
     *
     * # 반환값
     *
     * * `Some(char)`: 조합이 성공한 경우, 완성된 한글 음절.
     * * `None`: 조합 중인 상태가 아니거나 조합에 실패한 경우.
     */
    fn force_compose_hangul(&mut self) -> Option<char> {
        self.base_composer.force_compose_hangul()
    }

    /**
     * 현재 한글 조합이 진행 중인지 여부를 확인합니다.
     *
     * `BaseHangulComposer`의 `is_compose` 구현을 그대로 사용합니다.
     * `jamo_queue`에 자모가 하나 이상 있으면 조합 중인 것으로 간주합니다.
     *
     * # 반환값
     *
     * * `true`: 조합 중인 경우.
     * * `false`: 조합 중이 아닌 경우 (큐가 비어 있음).
     */
    fn is_compose(&self) -> bool {
        self.base_composer.is_compose()
    }

    /**
     * 다음에 입력될 자모가 새로운 음절을 시작해야 하는지 여부를 판단합니다.
     *
     * `BaseHangulComposer`의 `is_new_syllable` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * * `true`: 새로운 음절 시작 조건에 맞는 경우.
     * * `false`: 그렇지 않은 경우.
     */
    fn is_new_syllable(&self) -> bool {
        self.base_composer.is_new_syllable()
    }

    /**
     * 한글 초성 조합을 수행합니다. (내부 사용)
     *
     * `BaseHangulComposer`의 `compose_cho` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * * `true`: 초성 조합 성공 또는 초성 없음.
     * * `false`: 유효하지 않은 초성 조합 시도.
     */
    fn compose_cho(&mut self) -> bool {
        self.base_composer.compose_cho()
    }

    /**
     * 한글 중성 조합을 수행합니다. (내부 사용)
     *
     * `BaseHangulComposer`의 `compose_jung` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * * `true`: 중성 조합 성공 또는 중성 없음.
     * * `false`: 유효하지 않은 중성 조합 시도.
     */
    fn compose_jung(&mut self) -> bool {
        self.base_composer.compose_jung()
    }

    /**
     * 한글 종성 조합을 수행합니다. (내부 사용)
     *
     * `BaseHangulComposer`의 `compose_jong` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * * `true`: 종성 조합 성공 또는 종성 없음.
     * * `false`: 유효하지 않은 종성 조합 시도.
     */
    fn compose_jong(&mut self) -> bool {
        self.base_composer.compose_jong()
    }

    /**
     * 현재 조합 중인 한글 문자(`current_hangul`)의 자모를 모두 지웁니다.
     *
     * `BaseHangulComposer`의 `clear_jamo` 구현을 그대로 사용합니다.
     */
    fn clear_jamo(&mut self) {
        self.base_composer.clear_jamo()
    }

    /**
     * 현재 조합된 초성을 얻습니다.
     *
     * `BaseHangulComposer`의 `get_current_cho` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 현재 조합된 초성 (`Option<Cho>`).
     */
    fn get_current_cho(&self) -> Option<Cho> {
        self.base_composer.get_current_cho()
    }

    /**
     * 현재 조합된 중성을 얻습니다.
     *
     * `BaseHangulComposer`의 `get_current_jung` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 현재 조합된 중성 (`Option<Jung>`).
     */
    fn get_current_jung(&self) -> Option<Jung> {
        self.base_composer.get_current_jung()
    }

    /**
     * 현재 조합된 종성을 얻습니다.
     *
     * `BaseHangulComposer`의 `get_current_jong` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 현재 조합된 종성 (`Option<Jong>`).
     */
    fn get_current_jong(&self) -> Option<Jong> {
        self.base_composer.get_current_jong()
    }

    /**
     * 현재 조합 중인 한글 문자의 초성을 설정합니다.
     *
     * `BaseHangulComposer`의 `set_current_cho` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 설정 성공 여부 (현재 구현에서는 항상 `true`).
     */
    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool {
        self.base_composer.set_current_cho(cho)
    }

    /**
     * 현재 조합 중인 한글 문자의 중성을 설정합니다.
     *
     * `BaseHangulComposer`의 `set_current_jung` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 설정 성공 여부 (현재 구현에서는 항상 `true`).
     */
    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool {
        self.base_composer.set_current_jung(jung)
    }

    /**
     * 현재 조합 중인 한글 문자의 종성을 설정합니다.
     *
     * `BaseHangulComposer`의 `set_current_jong` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 설정 성공 여부 (현재 구현에서는 항상 `true`).
     */
    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool {
        self.base_composer.set_current_jong(jong)
    }

    /**
     * 내부적으로 사용되는 자모 조합 테이블에 대한 읽기 전용 참조를 반환합니다.
     *
     * `BaseHangulComposer`의 `get_combined_jamo` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 자모 조합 규칙을 담고 있는 해시맵에 대한 참조.
     */
    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_composer.get_combined_jamo()
    }

    /**
     * 현재 조합 중인 자모들이 순서대로 저장된 큐에 대한 가변 참조를 반환합니다.
     *
     * `BaseHangulComposer`의 `jamo_queue` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 자모 큐 (`VecDeque<JamoEnum>`)에 대한 가변 참조.
     */
    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_composer.jamo_queue()
    }

    /**
     * 직전에 완성된 음절을 구성했던 자모 큐에 대한 가변 참조를 반환합니다.
     *
     * `BaseHangulComposer`의 `last_jamo_queue` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 이전 자모 큐 (`VecDeque<JamoEnum>`)에 대한 가변 참조.
     */
    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_composer.last_jamo_queue()
    }

    /**
     * 내부적으로 사용되는 자모 조합 테이블에 대한 가변 참조를 반환합니다.
     *
     * `BaseHangulComposer`의 `combined_jamo` 구현을 그대로 사용합니다.
     * (주의: 이 메서드는 `initialize_combined_jamo` 외에는 직접 사용할 일이 거의 없습니다.)
     *
     * # 반환값
     *
     * 자모 조합 규칙을 담고 있는 해시맵에 대한 가변 참조.
     */
    fn combined_jamo(&mut self) -> &mut HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        self.base_composer.combined_jamo()
    }

    /**
     * 현재 조합 중인 한글 문자(`HangulChar`)의 상태를 나타내는 구조체에 대한 가변 참조를 반환합니다.
     *
     * `BaseHangulComposer`의 `current_hangul` 구현을 그대로 사용합니다.
     *
     * # 반환값
     *
     * 현재 조합 중인 `HangulChar`에 대한 가변 참조.
     */
    fn current_hangul(&mut self) -> &mut HangulChar {
        self.base_composer.current_hangul()
    }
}
