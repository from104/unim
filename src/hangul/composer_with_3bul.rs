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
 * # 특징
 * - 초성, 중성, 종성이 별도의 키로 할당됨
 * - 종성은 중성이 입력된 후에만 입력 가능
 * - 중성이나 종성 다음에 초성이 오면 새로운 음절 시작
 * - 종성 다음에 중성이 오면 새로운 음절 시작
 *
 * # 주요 기능
 * - [`JamoEnum`] 자모를 입력받아 조합 (`add_jamo`)
 * - 마지막 입력 자모 제거 (`remove_jamo`)
 * - 현재 큐의 자모를 바탕으로 음절 조합 시도 (`compose_hangul`)
 * - 강제 음절 완성 및 상태 초기화 (`force_compose_hangul`)
 * - 3벌식 조합 규칙에 따른 자모 조합 테이블 초기화 (`initialize_combined_jamo`)
 *
 * # 예시
 * ```
 * use unin::hangul::composer_with_3bul::HangulComposer3Bul;
 * use unin::hangul::jamo::*;
 *
 * let mut composer = HangulComposer3Bul::new();
 * composer.add_jamo(JamoEnum::Cho(Cho::G));  // 'ㄱ'
 * composer.add_jamo(JamoEnum::Jung(Jung::A)); // 'ㅏ'
 * assert_eq!(composer.force_compose_hangul(), Some('가'));
 * ```
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
        // ㄱ + ㄱ -> ㄲ
        let mut g_map = HashMap::new();
        g_map.insert(JamoEnum::Cho(Cho::G), JamoEnum::Cho(Cho::GG));
        combined_jamo.insert(JamoEnum::Cho(Cho::G), g_map);

        // ㄷ + ㄷ -> ㄸ
        let mut d_map = HashMap::new();
        d_map.insert(JamoEnum::Cho(Cho::D), JamoEnum::Cho(Cho::DD));
        combined_jamo.insert(JamoEnum::Cho(Cho::D), d_map);

        // ㅂ + ㅂ -> ㅃ
        let mut b_map = HashMap::new();
        b_map.insert(JamoEnum::Cho(Cho::B), JamoEnum::Cho(Cho::BB));
        combined_jamo.insert(JamoEnum::Cho(Cho::B), b_map);

        // ㅅ + ㅅ -> ㅆ
        let mut s_map = HashMap::new();
        s_map.insert(JamoEnum::Cho(Cho::S), JamoEnum::Cho(Cho::SS));
        combined_jamo.insert(JamoEnum::Cho(Cho::S), s_map);

        // ㅈ + ㅈ -> ㅉ
        let mut j_map = HashMap::new();
        j_map.insert(JamoEnum::Cho(Cho::J), JamoEnum::Cho(Cho::JJ));
        combined_jamo.insert(JamoEnum::Cho(Cho::J), j_map);

        // --- 중성 조합 규칙 ---
        // ㅗ + ㅏ -> ㅘ
        // ㅗ + ㅐ -> ㅙ
        // ㅗ + ㅣ -> ㅚ
        let mut o_map = HashMap::new();
        o_map.insert(JamoEnum::Jung(Jung::A), JamoEnum::Jung(Jung::WA));
        o_map.insert(JamoEnum::Jung(Jung::AE), JamoEnum::Jung(Jung::WAE));
        o_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::OE));
        combined_jamo.insert(JamoEnum::Jung(Jung::O), o_map);

        // ㅜ + ㅓ -> ㅝ
        // ㅜ + ㅔ -> ㅞ
        // ㅜ + ㅣ -> ㅟ
        let mut u_map = HashMap::new();
        u_map.insert(JamoEnum::Jung(Jung::EO), JamoEnum::Jung(Jung::WEO));
        u_map.insert(JamoEnum::Jung(Jung::E), JamoEnum::Jung(Jung::WE));
        u_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::WI));
        combined_jamo.insert(JamoEnum::Jung(Jung::U), u_map);

        // ㅡ + ㅣ -> ㅢ
        let mut eu_map = HashMap::new();
        eu_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::YI));
        combined_jamo.insert(JamoEnum::Jung(Jung::EU), eu_map);

        // --- 종성 조합 규칙 ---
        // ㄱ + ㄱ -> ㄲ
        // ㄱ + ㅅ -> ㄳ
        let mut jong_g_map = HashMap::new();
        jong_g_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::GG));
        jong_g_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::GS));
        combined_jamo.insert(JamoEnum::Jong(Jong::G), jong_g_map);

        // ㄴ + ㅈ -> ㄵ
        // ㄴ + ㅎ -> ㄶ
        let mut n_map = HashMap::new();
        n_map.insert(JamoEnum::Jong(Jong::J), JamoEnum::Jong(Jong::NJ));
        n_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::NH));
        combined_jamo.insert(JamoEnum::Jong(Jong::N), n_map);

        // ㄹ + ㄱ -> ㄺ
        // ㄹ + ㅁ -> ㄻ
        // ㄹ + ㅂ -> ㄼ
        // ㄹ + ㅅ -> ㄽ
        // ㄹ + ㅌ -> ㄾ
        // ㄹ + ㅍ -> ㄿ
        // ㄹ + ㅎ -> ㅀ
        let mut l_map = HashMap::new();
        l_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::LG));
        l_map.insert(JamoEnum::Jong(Jong::M), JamoEnum::Jong(Jong::LM));
        l_map.insert(JamoEnum::Jong(Jong::B), JamoEnum::Jong(Jong::LB));
        l_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::LS));
        l_map.insert(JamoEnum::Jong(Jong::T), JamoEnum::Jong(Jong::LT));
        l_map.insert(JamoEnum::Jong(Jong::P), JamoEnum::Jong(Jong::LP));
        l_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::LH));
        combined_jamo.insert(JamoEnum::Jong(Jong::L), l_map);

        // ㅂ + ㅅ -> ㅄ
        let mut jong_b_map = HashMap::new();
        jong_b_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::BS));
        combined_jamo.insert(JamoEnum::Jong(Jong::B), jong_b_map);

        // ㅅ + ㅅ -> ㅆ
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
     * # 동작 방식
     * 1. 입력된 자모를 현재 조합 중인 자모 시퀀스에 추가
     * 2. 조합 시도
     * 3. 조합 실패 시:
     *    - 이전 상태로 복원
     *    - 완성된 음절 생성
     *    - 새로운 음절 시작을 위한 상태 초기화
     *
     * # 매개변수
     * * `jamo` - 입력할 한글 자모
     *
     * # 반환값
     * * `Some(char)` - 이전 음절이 완성된 경우, 완성된 한글 음절
     * * `None` - 조합이 계속 진행 중인 경우
     *
     * # 예시
     * ```
     * use unin::hangul::composer_with_3bul::HangulComposer3Bul;
     * use unin::hangul::jamo::*;
     *
     * let mut composer = HangulComposer3Bul::new();
     * assert_eq!(composer.add_jamo(JamoEnum::Cho(Cho::G)), None);  // 'ㄱ'
     * assert_eq!(composer.add_jamo(JamoEnum::Jung(Jung::A)), Some('가')); // 'ㅏ'
     * ```
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
     * # 동작 방식
     * 1. 큐에서 마지막 자모 제거
     * 2. 남은 자모로 조합 상태 갱신
     *
     * # 반환값
     * * `Some(JamoEnum)` - 제거된 자모
     * * `None` - 제거할 자모가 없는 경우
     *
     * # 예시
     * ```
     * use unin::hangul::composer_with_3bul::HangulComposer3Bul;
     * use unin::hangul::jamo::*;
     *
     * let mut composer = HangulComposer3Bul::new();
     * composer.add_jamo(JamoEnum::Cho(Cho::G));  // 'ㄱ'
     * assert_eq!(composer.remove_jamo(), Some(JamoEnum::Cho(Cho::G)));
     * ```
     */
    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        self.base_composer.remove_jamo()
    }

    /**
     * 현재 자모 큐의 내용을 바탕으로 한글 음절을 조합합니다.
     *
     * # 동작 방식
     * 1. 큐가 비어있는지 확인
     * 2. 3벌식 특수 규칙 검사:
     *    - 중성 없이 종성 입력 시도
     *    - 중성/종성 다음 초성 입력
     *    - 종성 다음 중성 입력
     * 3. 자모 조합 수행:
     *    - 초성 조합
     *    - 중성 조합
     *    - 종성 조합
     *
     * # 반환값
     * * `true` - 조합 성공
     * * `false` - 조합 실패
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
     * 현재 조합 중인 자모들을 강제로 음절로 완성합니다.
     *
     * # 동작 방식
     * 1. 현재 자모 큐의 내용으로 음절 조합 시도
     * 2. 조합 성공 시 완성된 음절 반환
     * 3. 조합 실패 시 `None` 반환
     *
     * # 반환값
     * * `Some(char)` - 완성된 한글 음절
     * * `None` - 조합 실패
     *
     * # 예시
     * ```
     * use unin::hangul::composer_with_3bul::HangulComposer3Bul;
     * use unin::hangul::jamo::*;
     *
     * let mut composer = HangulComposer3Bul::new();
     * composer.add_jamo(JamoEnum::Cho(Cho::G));  // 'ㄱ'
     * composer.add_jamo(JamoEnum::Jung(Jung::A)); // 'ㅏ'
     * assert_eq!(composer.force_compose_hangul(), Some('가'));
     * ```
     */
    fn force_compose_hangul(&mut self) -> Option<char> {
        self.base_composer.force_compose_hangul()
    }

    /**
     * 현재 자모 큐에 조합 가능한 자모가 있는지 확인합니다.
     *
     * # 반환값
     * * `true` - 조합 가능한 자모가 있음
     * * `false` - 조합 가능한 자모가 없음
     */
    fn is_compose(&self) -> bool {
        self.base_composer.is_compose()
    }

    /**
     * 새로운 음절의 시작인지 확인합니다.
     *
     * # 반환값
     * * `true` - 새로운 음절의 시작
     * * `false` - 기존 음절의 연속
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
