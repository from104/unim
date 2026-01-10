// builder2bul.rs
use crate::hangul::char::HangulChar;
use crate::hangul::composer::BaseHangulComposer;
use crate::hangul::composer::CombinedJamoMap;
use crate::hangul::composer::HangulComposer;
use crate::hangul::jamo::*;
use once_cell::sync::Lazy;
use std::collections::{HashMap, VecDeque};

/// 2벌식 자모 조합 테이블 (중성 + 종성)
/// 프로그램 시작 시 한 번만 초기화됩니다.
static COMBINED_JAMO_2BUL: Lazy<CombinedJamoMap> = Lazy::new(|| {
    let mut map = HashMap::new();

    // === 중성 조합 (복모음) ===
    map.insert((JamoEnum::Jung(Jung::O), JamoEnum::Jung(Jung::A)), JamoEnum::Jung(Jung::WA));
    map.insert((JamoEnum::Jung(Jung::O), JamoEnum::Jung(Jung::AE)), JamoEnum::Jung(Jung::WAE));
    map.insert((JamoEnum::Jung(Jung::O), JamoEnum::Jung(Jung::I)), JamoEnum::Jung(Jung::OE));
    map.insert((JamoEnum::Jung(Jung::U), JamoEnum::Jung(Jung::EO)), JamoEnum::Jung(Jung::WEO));
    map.insert((JamoEnum::Jung(Jung::U), JamoEnum::Jung(Jung::E)), JamoEnum::Jung(Jung::WE));
    map.insert((JamoEnum::Jung(Jung::U), JamoEnum::Jung(Jung::I)), JamoEnum::Jung(Jung::WI));
    map.insert((JamoEnum::Jung(Jung::EU), JamoEnum::Jung(Jung::I)), JamoEnum::Jung(Jung::YI));

    // === 종성 조합 (겹받침) ===
    map.insert((JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::G)), JamoEnum::Jong(Jong::GG));
    map.insert((JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::S)), JamoEnum::Jong(Jong::GS));
    map.insert((JamoEnum::Jong(Jong::N), JamoEnum::Jong(Jong::J)), JamoEnum::Jong(Jong::NJ));
    map.insert((JamoEnum::Jong(Jong::N), JamoEnum::Jong(Jong::H)), JamoEnum::Jong(Jong::NH));
    map.insert((JamoEnum::Jong(Jong::L), JamoEnum::Jong(Jong::G)), JamoEnum::Jong(Jong::LG));
    map.insert((JamoEnum::Jong(Jong::L), JamoEnum::Jong(Jong::M)), JamoEnum::Jong(Jong::LM));
    map.insert((JamoEnum::Jong(Jong::L), JamoEnum::Jong(Jong::B)), JamoEnum::Jong(Jong::LB));
    map.insert((JamoEnum::Jong(Jong::L), JamoEnum::Jong(Jong::S)), JamoEnum::Jong(Jong::LS));
    map.insert((JamoEnum::Jong(Jong::L), JamoEnum::Jong(Jong::T)), JamoEnum::Jong(Jong::LT));
    map.insert((JamoEnum::Jong(Jong::L), JamoEnum::Jong(Jong::P)), JamoEnum::Jong(Jong::LP));
    map.insert((JamoEnum::Jong(Jong::L), JamoEnum::Jong(Jong::H)), JamoEnum::Jong(Jong::LH));
    map.insert((JamoEnum::Jong(Jong::B), JamoEnum::Jong(Jong::S)), JamoEnum::Jong(Jong::BS));
    map.insert((JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::S)), JamoEnum::Jong(Jong::SS));

    map
});


/**
 * 두벌식 한글 입력 방식의 조합 로직을 구현한 한글 컴포저입니다.
 *
 * `BaseHangulComposer`를 기반으로 두벌식 키보드 입력의 특징적인 동작,
 * 특히 '도깨비불 현상'과 중성 뒤 초성 입력 시 종성 변환 규칙을 처리합니다.
 *
 * `HangulComposer` 트레이트를 구현하여 기본적인 한글 조합 인터페이스를 제공합니다.
 */
#[derive(Debug, Default)]
pub struct HangulComposer2Bul {
    /// 기본적인 한글 조합 상태와 로직을 관리하는 내부 컴포저입니다.
    /// `HangulComposer2Bul`은 두벌식 특화 로직을 추가하고,
    /// 기본적인 조합 기능은 `base_composer`에 위임합니다.
    base_composer: BaseHangulComposer,
}

impl HangulComposer2Bul {
    /**
     * 새로운 `HangulComposer2Bul` 인스턴스를 생성합니다.
     *
     * 생성 시 `base_composer`를 초기화하고, 두벌식에 필요한
     * 자모 조합 규칙(`combined_jamo`)을 설정합니다.
     *
     * # 반환값
     *
     * 초기화된 `HangulComposer2Bul` 인스턴스.
     */
    pub fn new() -> Self {
        let mut composer = HangulComposer2Bul {
            base_composer: BaseHangulComposer::new(),
        };
        // 정적 2벌식 조합 테이블을 복제하여 사용
        *composer.combined_jamo() = COMBINED_JAMO_2BUL.clone();
        composer
    }

    /// 도깨비불 현상 처리 함수 (2벌식 전용)
    ///
    /// 종성 다음에 중성이 입력되었을 때(도깨비불 현상) 처리하는 함수입니다.
    /// 마지막 종성을 제거하고 새로운 음절의 초성으로 변환 후 조합합니다.
    ///
    /// # 매개변수
    ///
    /// * `jamo` - 입력된 중성 자모 (`JamoEnum::Jung`)
    ///
    /// # 반환값
    ///
    /// * `Some(Option<char>)` - 처리됨. 내부 값은 완성된 글자(있는 경우)
    /// * `None` - 도깨비불 현상이 아님
    fn handle_dokkaebi_effect(&mut self, jamo: JamoEnum) -> Option<Option<char>> {
        // 마지막 입력된 자모가 종성인지 확인
        let last_jamo = self.base_composer.jamo_queue().back().copied(); // Read access is okay
        if let Some(JamoEnum::Jong(jong)) = last_jamo {
            // 새로 입력된 자모가 중성인지 확인
            if matches!(jamo, JamoEnum::Jung(_)) {
                // 마지막 종성을 큐에서 제거
                self.base_composer.jamo_queue().pop_back();
                // 현재까지 조합된 글자를 강제로 완성 (초성+중성 상태)
                let current_char = self.force_compose_hangul(); // Uses self, which is fine

                // 제거된 종성을 초성으로 변환하여 새로운 글자의 초성으로 추가
                if let Ok(new_cho) = jong.to_cho() {
                    self.add_jamo(JamoEnum::Cho(new_cho)); // Recursive call - potential issue, but let's keep the logic for now

                    // 새로 입력된 중성을 추가
                    self.add_jamo(jamo); // Recursive call - potential issue

                    // 완성된 이전 글자를 반환
                    return Some(current_char);
                }
            }
        }
        None
    }

    /// 중성 뒤 초성 입력 처리 함수 (2벌식 전용)
    ///
    /// 중성이 채워진 상태에서 초성이 입력되었을 때, 초성을 종성으로 변환하여 처리합니다.
    ///
    /// # 매개변수
    ///
    /// * `jamo` - 입력된 초성 자모 (`JamoEnum::Cho`)
    ///
    /// # 반환값
    ///
    /// * `Some(Option<char>)` - 처리됨. 내부 값은 완성된 글자(있는 경우)
    /// * `None` - 처리 안됨(중성 뒤 초성 입력이 아님)
    fn handle_cho_after_jung(&mut self, jamo: JamoEnum) -> Option<Option<char>> {
        if self.base_composer.is_filled_jung() {
            // Read access is okay
            if let JamoEnum::Cho(cho) = jamo {
                // 입력된 초성을 종성으로 변환 시도
                if let Ok(jong) = cho.to_jong() {
                    // 변환 성공 시, 종성으로 기본 조합 로직 호출 (이 부분이 핵심)
                    // add_jamo를 직접 호출하는 대신, base_composer의 add_jamo를 사용해야 할 수 있음
                    // 하지만 일단 self.add_jamo로 시도
                    return Some(self.add_jamo(JamoEnum::Jong(jong))); // Recursive call
                }
                // 변환 실패 시 기본 로직으로 처리 (None 반환)
            }
        }
        None
    }
}

// `HangulComposer` 트레이트 구현
impl HangulComposer for HangulComposer2Bul {
    /**
     * 현재 조합 중인 자모들을 저장하는 큐에 대한 가변 참조를 반환합니다.
     * 이 큐는 `BaseHangulComposer`에 의해 관리됩니다.
     */
    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_composer.jamo_queue()
    }

    /**
     * 직전에 완성된 음절의 자모 큐에 대한 가변 참조를 반환합니다.
     * 이 큐는 `BaseHangulComposer`에 의해 관리됩니다.
     */
    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        self.base_composer.last_jamo_queue()
    }

    /**
     * 자모 조합 규칙이 정의된 해시맵에 대한 가변 참조를 반환합니다.
     * 이 맵은 `BaseHangulComposer`에 의해 관리됩니다.
     */
    fn combined_jamo(&mut self) -> &mut CombinedJamoMap {
        self.base_composer.combined_jamo()
    }

    /**
     * 현재 조합 중인 한글 음절(`HangulChar`)에 대한 가변 참조를 반환합니다.
     * 이 객체는 `BaseHangulComposer`에 의해 관리됩니다.
     */
    fn current_hangul(&mut self) -> &mut HangulChar {
        self.base_composer.current_hangul()
    }

    /**
     * 새로운 자모를 입력받아 두벌식 조합 로직에 따라 처리합니다.
     *
     * 두벌식의 특수한 입력 규칙을 처리합니다:
     * 1. **중성 뒤 초성 입력**: 현재 중성이 채워진 상태에서 초성이 입력되면,
     *    입력된 초성을 종성으로 변환하여 추가하려고 시도합니다. (예: '가' + 'ㄱ' -> '각')
     *    변환은 `Cho::to_jong()` 메서드를 사용합니다.
     * 2. **도깨비불 현상 (종성 + 중성 입력)**: 현재 종성이 채워진 상태에서 중성이 입력되면,
     *    마지막 종성을 큐에서 제거하고, 현재까지 조합된 글자를 완성합니다.
     *    제거된 종성은 초성으로 변환(`Jong::to_cho()`)되어 새로운 음절의 시작 초성이 되고,
     *    새로 입력된 중성이 그 뒤를 따릅니다. (예: '각' + 'ㅏ' -> '가', 'ㄱㅏ')
     *
     * 위의 특수 규칙에 해당하지 않으면, `BaseHangulComposer::add_jamo`를 호출하여
     * 기본적인 자모 추가 및 조합 로직을 수행합니다.
     *
     * # 매개변수
     *
     * * `jamo`: 입력할 한글 자모 (`JamoEnum`).
     *
     * # 반환값
     *
     * * `Some(char)`: 자모 입력으로 인해 이전 음절 조합이 완료된 경우, 완성된 한글 음절.
     *                 (주로 도깨비불 현상 발생 시 반환됨)
     * * `None`: 조합이 계속 진행 중이거나, 특수 입력 변환에 실패한 경우.
     */
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        // 입력된 자모가 유효한지 확인
        if !self.base_composer.is_valid_jamo(&jamo) {
            return None;
        }

        // 1. 중성 뒤 초성 입력 처리
        if let Some(result_opt) = self.handle_cho_after_jung(jamo) {
            return result_opt;
        }

        // 2. 도깨비불 현상 처리 (종성 + 중성 입력)
        if let Some(result_opt) = self.handle_dokkaebi_effect(jamo) {
            return result_opt;
        }

        // 위 특수 경우에 해당하지 않으면 기본 자모 추가 로직 수행
        self.base_composer.add_jamo(jamo)
    }

    /**
     * 마지막으로 입력된 자모를 제거하고 조합 상태를 갱신합니다.
     * `BaseHangulComposer::remove_jamo`에 작업을 위임합니다.
     *
     * # 반환값
     *
     * * `Some(JamoEnum)`: 성공적으로 제거된 자모.
     * * `None`: 제거할 자모가 없는 경우.
     */
    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        self.base_composer.remove_jamo()
    }

    /**
     * 현재 `jamo_queue`의 자모들을 조합하여 `current_hangul` 상태를 업데이트합니다.
     *
     * 두벌식에서는 특정 조합을 허용하지 않는 추가 검사를 수행합니다:
     * 1. 초성이 없는 상태에서 중성 다음에 종성이 오는 경우 (예: 'ㅏㄱ')
     * 2. 종성 다음에 중성이 오는 경우 (예: 'ㄱㅏ') -> 도깨비불 현상으로 처리되어야 함
     *
     * 위 조건에 해당하면 조합 실패(`false`)를 반환합니다.
     * 그 외의 경우는 `BaseHangulComposer::compose_hangul`에 작업을 위임합니다.
     *
     * # 반환값
     *
     * * `true`: 조합 성공 또는 큐가 비어 초기화됨.
     * * `false`: 조합 규칙 위반으로 실패.
     */
    fn compose_hangul(&mut self) -> bool {
        // 큐가 비어있으면 초기화하고 성공 반환
        if self.base_composer.jamo_queue().is_empty() {
            self.base_composer.clear();
            return true;
        }

        // 마지막 두 자모 확인
        let queue = self.base_composer.jamo_queue();
        let last_jamo = *queue.back().unwrap();
        let second_last_jamo = if queue.len() > 1 {
            Some(*queue.get(queue.len() - 2).unwrap())
        } else {
            None
        };

        // 1. 초성 없이 [중성, 종성] 순서 확인
        if !self.base_composer.is_filled_cho() // 현재 초성이 없고
            && second_last_jamo.is_some_and(|j| matches!(j, JamoEnum::Jung(_))) // 이전 자모가 중성이며
            && matches!(last_jamo, JamoEnum::Jong(_))
        // 마지막 자모가 종성이면
        {
            // 유효하지 않은 조합 (예: 'ㅏㄱ')
            return false;
        }

        // 2. [종성, 중성] 순서 확인 (도깨비불 현상)
        if second_last_jamo.is_some_and(|j| matches!(j, JamoEnum::Jong(_))) // 이전 자모가 종성이며
            && matches!(last_jamo, JamoEnum::Jung(_))
        // 마지막 자모가 중성이면
        {
            // add_jamo에서 처리되어야 할 상태이므로, 여기서는 조합 실패로 간주
            return false;
        }

        // 위의 추가 검사를 통과하면 기본 조합 로직 수행
        self.base_composer.compose_hangul()
    }

    /**
     * 현재까지 입력된 자모를 강제로 조합하여 완성된 음절을 반환하고 상태를 초기화합니다.
     * `BaseHangulComposer::force_compose_hangul`에 작업을 위임합니다.
     *
     * # 반환값
     *
     * * `Some(char)`: 조합 성공 시 완성된 음절.
     * * `None`: 조합 중이 아니거나 실패한 경우.
     */
    fn force_compose_hangul(&mut self) -> Option<char> {
        self.base_composer.force_compose_hangul()
    }

    /**
     * 현재 한글 조합이 진행 중인지 확인합니다.
     * `BaseHangulComposer::is_compose`에 작업을 위임합니다.
     *
     * # 반환값
     *
     * * `true`: 조합 중 (`jamo_queue`에 자모가 있음).
     * * `false`: 조합 중 아님.
     */
    fn is_compose(&self) -> bool {
        self.base_composer.is_compose()
    }

    // --- 내부 조합 함수 위임 ---
    // 아래 함수들은 `BaseHangulComposer`의 구현을 그대로 사용합니다.
    // 각 함수의 상세 설명은 `BaseHangulComposer` 또는 `HangulComposer` 트레이트 주석 참고.

    /**
     * 초성 조합 로직을 수행합니다. (내부 사용)
     * `BaseHangulComposer::compose_cho`에 위임합니다.
     */
    fn compose_cho(&mut self) -> bool {
        self.base_composer.compose_cho()
    }

    /**
     * 중성 조합 로직을 수행합니다. (내부 사용)
     * `BaseHangulComposer::compose_jung`에 위임합니다.
     */
    fn compose_jung(&mut self) -> bool {
        self.base_composer.compose_jung()
    }

    /**
     * 종성 조합 로직을 수행합니다. (내부 사용)
     * `BaseHangulComposer::compose_jong`에 위임합니다.
     */
    fn compose_jong(&mut self) -> bool {
        self.base_composer.compose_jong()
    }

    /**
     * 현재 조합 중인 한글 음절의 자모를 모두 지웁니다.
     * `BaseHangulComposer::clear_jamo`에 위임합니다.
     */
    fn clear_jamo(&mut self) {
        self.base_composer.clear_jamo()
    }

    /**
     * 현재 조합된 초성을 반환합니다.
     * `BaseHangulComposer::get_current_cho`에 위임합니다.
     */
    fn get_current_cho(&self) -> Option<Cho> {
        self.base_composer.get_current_cho()
    }

    /**
     * 현재 조합된 중성을 반환합니다.
     * `BaseHangulComposer::get_current_jung`에 위임합니다.
     */
    fn get_current_jung(&self) -> Option<Jung> {
        self.base_composer.get_current_jung()
    }

    /**
     * 현재 조합된 종성을 반환합니다.
     * `BaseHangulComposer::get_current_jong`에 위임합니다.
     */
    fn get_current_jong(&self) -> Option<Jong> {
        self.base_composer.get_current_jong()
    }

    /**
     * 현재 조합 중인 음절의 초성을 설정합니다.
     * `BaseHangulComposer::set_current_cho`에 위임합니다.
     */
    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool {
        self.base_composer.set_current_cho(cho)
    }

    /**
     * 현재 조합 중인 음절의 중성을 설정합니다.
     * `BaseHangulComposer::set_current_jung`에 위임합니다.
     */
    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool {
        self.base_composer.set_current_jung(jung)
    }

    /**
     * 현재 조합 중인 음절의 종성을 설정합니다.
     * `BaseHangulComposer::set_current_jong`에 위임합니다.
     */
    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool {
        self.base_composer.set_current_jong(jong)
    }

    /**
     * 자모 조합 규칙이 정의된 해시맵에 대한 불변 참조를 반환합니다.
     * `BaseHangulComposer::get_combined_jamo`에 위임합니다.
     */
    fn get_combined_jamo(&self) -> &CombinedJamoMap {
        self.base_composer.get_combined_jamo()
    }

    /**
     * 다음에 입력될 자모가 새로운 음절을 시작해야 하는지 판단합니다.
     * `BaseHangulComposer::is_new_syllable`에 위임합니다.
     * (두벌식의 경우 `add_jamo`에서 관련 로직을 직접 처리하므로,
     * 이 함수의 반환값이 결정적인 역할을 하지는 않을 수 있습니다.)
     */
    fn is_new_syllable(&self) -> bool {
        self.base_composer.is_new_syllable()
    }
}
