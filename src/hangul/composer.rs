use crate::hangul::char::HangulChar;
use crate::hangul::jamo::JamoEnum;
use crate::hangul::jamo::*;
/**
 * 한글 조합 취상위 클래스
 * @author "KiHyeon Seo" <from104@gmail.com>
 */
// builder.rs
use std::collections::{HashMap, VecDeque};

/// 한글 자모를 조합하여 한글 음절을 만드는 기능을 정의하는 트레이트입니다.
///
/// 이 트레이트는 자모 입력, 삭제, 조합 상태 확인 등의 기본적인 인터페이스를 제공합니다.
/// 구체적인 조합 로직은 이 트레이트를 구현하는 타입에서 정의됩니다.
pub trait HangulComposer {
    /// 한글 자모를 입력받아 현재 조합 상태에 추가합니다.
    ///
    /// 입력된 자모로 인해 새로운 음절 조합이 시작되어 이전 음절이 완성되면,
    /// 완성된 한글 음절 문자를 `Some(char)`로 반환합니다.
    /// 조합이 계속 진행 중이면 `None`을 반환합니다.
    ///
    /// # 매개변수
    ///
    /// * `jamo`: 입력할 한글 자모 (`JamoEnum`). 초성, 중성, 종성 또는 특수 문자일 수 있습니다.
    ///
    /// # 반환값
    ///
    /// * `Some(char)`: 입력된 자모로 인해 이전 음절 조합이 완료된 경우, 완성된 한글 음절.
    /// * `None`: 조합이 계속 진행 중인 경우.
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char>;

    /// 마지막으로 입력된 한글 자모를 제거하고 조합 상태를 갱신합니다.
    ///
    /// 제거 후 조합 상태가 변경됩니다.
    ///
    /// # 반환값
    ///
    /// * `Some(JamoEnum)`: 성공적으로 제거된 자모.
    /// * `None`: 제거할 자모가 없는 경우 (조합 큐가 비어 있는 경우).
    fn remove_jamo(&mut self) -> Option<JamoEnum>;

    /// 현재 `jamo_queue`에 저장된 자모들을 바탕으로 한글 음절을 조합합니다.
    ///
    /// 내부적으로 `compose_cho`, `compose_jung`, `compose_jong`을 호출하여
    /// `current_hangul_char`의 상태를 업데이트합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 조합에 성공했거나, 큐가 비어 있어 초기화된 경우.
    /// * `false`: 자모 조합 규칙에 맞지 않아 조합에 실패한 경우.
    fn compose_hangul(&mut self) -> bool;

    /// 현재까지 입력된 자모들을 강제로 조합하여 완성된 한글 음절을 반환하고, 조합 상태를 초기화합니다.
    ///
    /// 조합 중인 상태(`is_compose()`가 `true`인 경우)에만 동작합니다.
    /// 성공적으로 조합되면 현재 조합 상태(`jamo_queue`, `last_jamo_queue`, `current_hangul_char`)가 모두 초기화됩니다.
    ///
    /// # 반환값
    ///
    /// * `Some(char)`: 조합이 성공한 경우, 완성된 한글 음절.
    /// * `None`: 조합 중인 상태가 아니거나 조합에 실패한 경우.
    fn force_compose_hangul(&mut self) -> Option<char>;

    /// 현재 한글 조합이 진행 중인지 여부를 확인합니다.
    ///
    /// `jamo_queue`에 자모가 하나 이상 있으면 조합 중인 것으로 간주합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 조합 중인 경우.
    /// * `false`: 조합 중이 아닌 경우 (큐가 비어 있음).
    fn is_compose(&self) -> bool;

    /// 다음에 입력될 자모가 새로운 음절을 시작해야 하는지 여부를 판단합니다.
    ///
    /// 현재 조합 상태를 기준으로 판단하며, 구체적인 로직은 구현체에 따라 다를 수 있습니다.
    /// 예를 들어, 마지막 입력이 초성이었고 현재 중성이 채워져 있다면 새로운 음절 시작으로 볼 수 있습니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 새로운 음절 시작 조건에 맞는 경우.
    /// * `false`: 그렇지 않은 경우.
    fn is_new_syllable(&self) -> bool;

    // --- 내부적으로 사용되는 함수들 (Java의 protected methods) ---

    /// 한글 초성 조합 (내부 사용)
    ///
    /// # 반환값
    ///
    /// * `true`: 성공 또는 실패
    fn compose_cho(&mut self) -> bool;

    /// 한글 중성 조합 (내부 사용)
    ///
    /// # 반환값
    ///
    /// * `true`: 성공 또는 실패
    fn compose_jung(&mut self) -> bool;

    /// 한글 종성 조합 (내부 사용)
    ///
    /// # 반환값
    ///
    /// * `true`: 성공 또는 실패
    fn compose_jong(&mut self) -> bool;

    /// 자모 모두 지우기 (HangulChar의 clear() 와 유사, 필요에 따라 트레잇에 추가하거나 구현체에서 제공)
    fn clear_jamo(&mut self);

    /// 현재 조합된 초성 얻기 (HangulChar의 get_cho() 와 유사)
    fn get_current_cho(&self) -> Option<Cho>;

    /// 현재 조합된 중성 얻기 (HangulChar의 get_jung() 와 유사)
    fn get_current_jung(&self) -> Option<Jung>;

    /// 현재 조합된 종성 얻기 (HangulChar의 get_jong() 와 유사)
    fn get_current_jong(&self) -> Option<Jong>;

    /// 초성 설정 (HangulChar의 set_cho_object() 와 유사)
    ///
    /// # 반환값
    ///
    /// 설정 성공 여부 (현재 구현에서는 항상 `true`).
    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool;

    /// 중성 설정 (HangulChar의 set_jung_object() 와 유사)
    ///
    /// # 반환값
    ///
    /// 설정 성공 여부 (현재 구현에서는 항상 `true`).
    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool;

    /// 종성 설정 (HangulChar의 set_jong_object() 와 유사)
    ///
    /// # 반환값
    ///
    /// 설정 성공 여부 (현재 구현에서는 항상 `true`).
    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool;

    /// 자모 조합 테이블 접근 (필요한 경우)
    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>>;

    // 자모가 입력되는 순서대로 저장하는 큐
    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum>;

    // 직전 큐
    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum>;

    // 자모 조합 테이블
    fn combined_jamo(&mut self) -> &mut HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>>;

    // 현재 조합 중인 한글
    fn current_hangul(&mut self) -> &mut HangulChar;
}

/// `HangulComposer` 트레이트의 기본 구현을 제공하는 구조체입니다.
///
/// 이 구조체는 한글 자모를 조합하여 한글 음절을 생성하는 기본적인 기능을 구현합니다.
/// 자모 입력, 삭제, 조합 상태 확인 등의 기능을 제공하며, 한글 입력기나 텍스트 편집기에서
/// 사용할 수 있습니다.
///
/// # 필드
///
/// * `jamo_queue` - 현재 입력 중인 자모들을 순서대로 저장하는 큐
/// * `last_jamo_queue` - 직전에 입력된 자모들을 저장하는 큐
/// * `combined_jamo` - 자모 조합 규칙을 정의하는 테이블
/// * `current_hangul_char` - 현재 조합 중인 한글 음절
#[derive(Debug, Default)]
pub struct BaseHangulComposer {
    jamo_queue: VecDeque<JamoEnum>,
    last_jamo_queue: VecDeque<JamoEnum>,
    combined_jamo: HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>>,
    current_hangul_char: HangulChar,
}

impl BaseHangulComposer {
    /// 새로운 `BaseHangulComposer` 인스턴스를 생성합니다.
    ///
    /// # 반환값
    ///
    /// 초기화된 `BaseHangulComposer` 인스턴스
    pub fn new() -> Self {
        BaseHangulComposer::default()
    }

    /// 내부적으로 새로운 음절 시작 여부를 판단합니다.
    ///
    /// 마지막 입력이 초성이었고 현재 중성이 채워져 있는 경우 새로운 음절 시작으로 간주합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 새로운 음절 시작 조건에 맞는 경우
    /// * `false`: 그렇지 않은 경우
    pub fn is_new_syllable_internal(&self) -> bool {
        self.jamo_queue
            .back().is_some_and(|last_jamo| matches!(last_jamo, JamoEnum::Cho(_) if self.current_hangul_char.is_filled_jung()))
    }

    /// 자모 큐에 접근할 수 있는 가변 참조를 반환합니다.
    ///
    /// # 반환값
    ///
    /// 자모 큐의 가변 참조
    pub fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        &mut self.jamo_queue
    }

    /// 현재 조합된 초성을 반환합니다.
    ///
    /// # 반환값
    ///
    /// * `Some(Cho)`: 초성이 설정된 경우
    /// * `None`: 초성이 설정되지 않은 경우
    pub fn get_cho(&self) -> Option<Cho> {
        self.current_hangul_char.get_cho()
    }

    /// 현재 조합된 중성을 반환합니다.
    ///
    /// # 반환값
    ///
    /// * `Some(Jung)`: 중성이 설정된 경우
    /// * `None`: 중성이 설정되지 않은 경우
    pub fn get_jung(&self) -> Option<Jung> {
        self.current_hangul_char.get_jung()
    }

    /// 현재 조합된 종성을 반환합니다.
    ///
    /// # 반환값
    ///
    /// * `Some(Jong)`: 종성이 설정된 경우
    /// * `None`: 종성이 설정되지 않은 경우
    pub fn get_jong(&self) -> Option<Jong> {
        self.current_hangul_char.get_jong()
    }

    /// 초성을 설정합니다.
    ///
    /// # 매개변수
    ///
    /// * `cho` - 설정할 초성 값
    pub fn set_cho(&mut self, cho: Option<Cho>) {
        self.current_hangul_char.set_cho_object(cho);
    }

    /// 중성을 설정합니다.
    ///
    /// # 매개변수
    ///
    /// * `jung` - 설정할 중성 값
    pub fn set_jung(&mut self, jung: Option<Jung>) {
        self.current_hangul_char.set_jung_object(jung);
    }

    /// 종성을 설정합니다.
    ///
    /// # 매개변수
    ///
    /// * `jong` - 설정할 종성 값
    pub fn set_jong(&mut self, jong: Option<Jong>) {
        self.current_hangul_char.set_jong_object(jong);
    }

    /// 초성을 초기화합니다.
    pub fn clear_cho(&mut self) {
        self.current_hangul_char.clear_cho();
    }

    /// 중성을 초기화합니다.
    pub fn clear_jung(&mut self) {
        self.current_hangul_char.clear_jung();
    }

    /// 종성을 초기화합니다.
    pub fn clear_jong(&mut self) {
        self.current_hangul_char.clear_jong();
    }

    /// 모든 자모를 초기화합니다.
    pub fn clear(&mut self) {
        self.current_hangul_char.clear();
    }

    /// 초성이 설정되어 있는지 확인합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 초성이 설정된 경우
    /// * `false`: 초성이 설정되지 않은 경우
    pub fn is_filled_cho(&self) -> bool {
        self.current_hangul_char.is_filled_cho()
    }

    /// 중성이 설정되어 있는지 확인합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 중성이 설정된 경우
    /// * `false`: 중성이 설정되지 않은 경우
    pub fn is_filled_jung(&self) -> bool {
        self.current_hangul_char.is_filled_jung()
    }

    /// 종성이 설정되어 있는지 확인합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 종성이 설정된 경우
    /// * `false`: 종성이 설정되지 않은 경우
    pub fn is_filled_jong(&self) -> bool {
        self.current_hangul_char.is_filled_jong()
    }

    /// 초성을 조합합니다.
    ///
    /// 자모 큐에서 초성만 추출하여 조합 규칙에 따라 초성을 설정합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 조합에 성공한 경우
    /// * `false`: 조합에 실패한 경우
    fn compose_cho(&mut self) -> bool {
        let mut cho_vec = Vec::new();

        // 초성만 걸러냄
        for jamo in &self.jamo_queue {
            if let JamoEnum::Cho(cho) = jamo {
                cho_vec.push(*cho);
            }
        }

        if cho_vec.is_empty() {
            self.clear_cho();
        } else {
            self.set_cho(Some(cho_vec[0]));
            if cho_vec.len() > 1 {
                cho_vec.remove(0);
                for cho in cho_vec {
                    let first_jamo = JamoEnum::Cho(self.get_cho().unwrap());
                    let second_jamo = JamoEnum::Cho(cho);

                    if let Some(combined_map) = self.combined_jamo.get(&first_jamo) {
                        if let Some(JamoEnum::Cho(combined_cho)) = combined_map.get(&second_jamo) {
                            self.set_cho(Some(*combined_cho));
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// 중성을 조합합니다.
    ///
    /// 자모 큐에서 중성만 추출하여 조합 규칙에 따라 중성을 설정합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 조합에 성공한 경우
    /// * `false`: 조합에 실패한 경우
    fn compose_jung(&mut self) -> bool {
        let mut jung_vec = Vec::new();

        // 중성만 걸러냄
        for jamo in &self.jamo_queue {
            if let JamoEnum::Jung(jung) = jamo {
                jung_vec.push(*jung);
            }
        }

        if jung_vec.is_empty() {
            self.clear_jung();
        } else {
            self.set_jung(Some(jung_vec[0]));
            if jung_vec.len() > 1 {
                jung_vec.remove(0);
                for jung in jung_vec {
                    let first_jamo = JamoEnum::Jung(self.get_jung().unwrap());
                    let second_jamo = JamoEnum::Jung(jung);

                    if let Some(combined_map) = self.combined_jamo.get(&first_jamo) {
                        if let Some(JamoEnum::Jung(combined_jung)) = combined_map.get(&second_jamo)
                        {
                            self.set_jung(Some(*combined_jung));
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// 종성을 조합합니다.
    ///
    /// 자모 큐에서 종성만 추출하여 조합 규칙에 따라 종성을 설정합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 조합에 성공한 경우
    /// * `false`: 조합에 실패한 경우
    fn compose_jong(&mut self) -> bool {
        let mut jong_vec = Vec::new();

        // 종성만 걸러냄
        for jamo in &self.jamo_queue {
            if let JamoEnum::Jong(jong) = jamo {
                jong_vec.push(*jong);
            }
        }

        if jong_vec.is_empty() {
            self.clear_jong();
        } else {
            self.set_jong(Some(jong_vec[0]));
            if jong_vec.len() > 1 {
                jong_vec.remove(0);
                for jong in jong_vec {
                    let first_jamo = JamoEnum::Jong(self.get_jong().unwrap());
                    let second_jamo = JamoEnum::Jong(jong);

                    if let Some(combined_map) = self.combined_jamo.get(&first_jamo) {
                        if let Some(JamoEnum::Jong(combined_jong)) = combined_map.get(&second_jamo)
                        {
                            self.set_jong(Some(*combined_jong));
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
            }
        }
        true
    }

    /// 도깨비불 현상 처리 함수
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
    pub fn handle_dokkaebi_effect(&mut self, jamo: JamoEnum) -> Option<Option<char>> {
        // 마지막 입력된 자모가 종성인지 확인
        let last_jamo = self.jamo_queue.back().copied();
        if let Some(JamoEnum::Jong(jong)) = last_jamo {
            // 새로 입력된 자모가 중성인지 확인
            if matches!(jamo, JamoEnum::Jung(_)) {
                // 마지막 종성을 큐에서 제거
                self.jamo_queue.pop_back();
                // 현재까지 조합된 글자를 강제로 완성 (초성+중성 상태)
                let current_char = self.force_compose_hangul();

                // 제거된 종성을 초성으로 변환하여 새로운 글자의 초성으로 추가
                if let Ok(new_cho) = jong.to_cho() {
                    self.add_jamo(JamoEnum::Cho(new_cho));

                    // 새로 입력된 중성을 추가
                    self.add_jamo(jamo);

                    // 완성된 이전 글자를 반환
                    return Some(current_char);
                }
            }
        }
        None
    }

    /// 중성 뒤 초성 입력 처리 함수
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
    pub fn handle_cho_after_jung(&mut self, jamo: JamoEnum) -> Option<Option<char>> {
        if self.is_filled_jung() {
            if let JamoEnum::Cho(cho) = jamo {
                // 입력된 초성을 종성으로 변환 시도
                if let Ok(jong) = cho.to_jong() {
                    // 변환 성공 시, 종성으로 기본 조합 로직 호출
                    return Some(self.add_jamo(JamoEnum::Jong(jong)));
                }
                // 변환 실패 시 기본 로직으로 처리
            }
        }
        None
    }

    /// 입력된 자모가 유효한 초성, 중성, 종성인지 확인합니다.
    ///
    /// # 매개변수
    ///
    /// * `jamo` - 검사할 자모
    ///
    /// # 반환값
    ///
    /// * `true` - 유효한 자모
    /// * `false` - 유효하지 않은 자모
    pub fn is_valid_jamo(&self, jamo: &JamoEnum) -> bool {
        matches!(
            jamo,
            JamoEnum::Cho(_) | JamoEnum::Jung(_) | JamoEnum::Jong(_)
        )
    }

    /// 중성 조합 규칙(복모음)을 초기화합니다.
    ///
    /// # 반환값
    ///
    /// 중성 조합 규칙이 담긴 해시맵
    pub fn initialize_jung_combinations(&self) -> HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        let mut combined_jamo = HashMap::new();

        // 'ㅗ' + 'ㅏ' -> 'ㅘ'
        // 'ㅗ' + 'ㅐ' -> 'ㅙ'
        // 'ㅗ' + 'ㅣ' -> 'ㅚ'
        let mut o_map = HashMap::new();
        o_map.insert(JamoEnum::Jung(Jung::A), JamoEnum::Jung(Jung::WA));
        o_map.insert(JamoEnum::Jung(Jung::AE), JamoEnum::Jung(Jung::WAE));
        o_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::OE));
        combined_jamo.insert(JamoEnum::Jung(Jung::O), o_map);

        // 'ㅜ' + 'ㅓ' -> 'ㅝ'
        // 'ㅜ' + 'ㅔ' -> 'ㅞ'
        // 'ㅜ' + 'ㅣ' -> 'ㅟ'
        let mut u_map = HashMap::new();
        u_map.insert(JamoEnum::Jung(Jung::EO), JamoEnum::Jung(Jung::WEO));
        u_map.insert(JamoEnum::Jung(Jung::E), JamoEnum::Jung(Jung::WE));
        u_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::WI));
        combined_jamo.insert(JamoEnum::Jung(Jung::U), u_map);

        // 'ㅡ' + 'ㅣ' -> 'ㅢ'
        let mut eu_map = HashMap::new();
        eu_map.insert(JamoEnum::Jung(Jung::I), JamoEnum::Jung(Jung::YI));
        combined_jamo.insert(JamoEnum::Jung(Jung::EU), eu_map);

        combined_jamo
    }

    /// 종성 조합 규칙(겹받침)을 초기화합니다.
    ///
    /// # 반환값
    ///
    /// 종성 조합 규칙이 담긴 해시맵
    pub fn initialize_jong_combinations(&self) -> HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        let mut combined_jamo = HashMap::new();

        // 'ㄱ' + 'ㄱ' -> 'ㄲ'
        // 'ㄱ' + 'ㅅ' -> 'ㄳ'
        let mut g_map = HashMap::new();
        g_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::GG));
        g_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::GS));
        combined_jamo.insert(JamoEnum::Jong(Jong::G), g_map);

        // 'ㄴ' + 'ㅈ' -> 'ㄵ'
        // 'ㄴ' + 'ㅎ' -> 'ㄶ'
        let mut n_map = HashMap::new();
        n_map.insert(JamoEnum::Jong(Jong::J), JamoEnum::Jong(Jong::NJ));
        n_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::NH));
        combined_jamo.insert(JamoEnum::Jong(Jong::N), n_map);

        // 'ㄹ' + 'ㄱ' -> 'ㄺ'
        // 'ㄹ' + 'ㅁ' -> 'ㄻ'
        // 'ㄹ' + 'ㅂ' -> 'ㄼ'
        // 'ㄹ' + 'ㅅ' -> 'ㄽ'
        // 'ㄹ' + 'ㅌ' -> 'ㄾ'
        // 'ㄹ' + 'ㅍ' -> 'ㄿ'
        // 'ㄹ' + 'ㅎ' -> 'ㅀ'
        let mut l_map = HashMap::new();
        l_map.insert(JamoEnum::Jong(Jong::G), JamoEnum::Jong(Jong::LG));
        l_map.insert(JamoEnum::Jong(Jong::M), JamoEnum::Jong(Jong::LM));
        l_map.insert(JamoEnum::Jong(Jong::B), JamoEnum::Jong(Jong::LB));
        l_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::LS));
        l_map.insert(JamoEnum::Jong(Jong::T), JamoEnum::Jong(Jong::LT));
        l_map.insert(JamoEnum::Jong(Jong::P), JamoEnum::Jong(Jong::LP));
        l_map.insert(JamoEnum::Jong(Jong::H), JamoEnum::Jong(Jong::LH));
        combined_jamo.insert(JamoEnum::Jong(Jong::L), l_map);

        // 'ㅂ' + 'ㅅ' -> 'ㅄ'
        let mut b_map = HashMap::new();
        b_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::BS));
        combined_jamo.insert(JamoEnum::Jong(Jong::B), b_map);

        // 'ㅅ' + 'ㅅ' -> 'ㅆ'
        let mut s_map = HashMap::new();
        s_map.insert(JamoEnum::Jong(Jong::S), JamoEnum::Jong(Jong::SS));
        combined_jamo.insert(JamoEnum::Jong(Jong::S), s_map);

        combined_jamo
    }

    /// 초성 조합 규칙(쌍자음)을 초기화합니다. 3벌식 전용입니다.
    ///
    /// # 반환값
    ///
    /// 초성 조합 규칙이 담긴 해시맵
    pub fn initialize_cho_combinations(&self) -> HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        let mut combined_jamo = HashMap::new();

        // 'ㄱ' + 'ㄱ' -> 'ㄲ'
        let mut g_map = HashMap::new();
        g_map.insert(JamoEnum::Cho(Cho::G), JamoEnum::Cho(Cho::GG));
        combined_jamo.insert(JamoEnum::Cho(Cho::G), g_map);

        // 'ㄷ' + 'ㄷ' -> 'ㄸ'
        let mut d_map = HashMap::new();
        d_map.insert(JamoEnum::Cho(Cho::D), JamoEnum::Cho(Cho::DD));
        combined_jamo.insert(JamoEnum::Cho(Cho::D), d_map);

        // 'ㅂ' + 'ㅂ' -> 'ㅃ'
        let mut b_map = HashMap::new();
        b_map.insert(JamoEnum::Cho(Cho::B), JamoEnum::Cho(Cho::BB));
        combined_jamo.insert(JamoEnum::Cho(Cho::B), b_map);

        // 'ㅅ' + 'ㅅ' -> 'ㅆ'
        let mut s_map = HashMap::new();
        s_map.insert(JamoEnum::Cho(Cho::S), JamoEnum::Cho(Cho::SS));
        combined_jamo.insert(JamoEnum::Cho(Cho::S), s_map);

        // 'ㅈ' + 'ㅈ' -> 'ㅉ'
        let mut j_map = HashMap::new();
        j_map.insert(JamoEnum::Cho(Cho::J), JamoEnum::Cho(Cho::JJ));
        combined_jamo.insert(JamoEnum::Cho(Cho::J), j_map);

        combined_jamo
    }

    /// 자모 조합 테이블을 초기화합니다.
    ///
    /// 중성, 종성 조합 규칙을 초기화하고, 필요에 따라 초성 조합 규칙도 추가합니다.
    ///
    /// # 매개변수
    ///
    /// * `with_cho_combinations` - 초성 조합 규칙을 추가할지 여부 (3벌식의 경우 true)
    pub fn initialize_combined_jamo(&mut self, with_cho_combinations: bool) {
        let mut combined_jamo = HashMap::new();

        // 1. 중성 조합 규칙 초기화
        for (key, value) in self.initialize_jung_combinations() {
            combined_jamo.insert(key, value);
        }

        // 2. 종성 조합 규칙 초기화
        for (key, value) in self.initialize_jong_combinations() {
            combined_jamo.insert(key, value);
        }

        // 3. 필요한 경우 초성 조합 규칙 초기화 (3벌식용)
        if with_cho_combinations {
            for (key, value) in self.initialize_cho_combinations() {
                combined_jamo.insert(key, value);
            }
        }

        *self.combined_jamo() = combined_jamo;
    }
}

impl HangulComposer for BaseHangulComposer {
    /// 한글 자모를 입력받아 현재 조합 상태에 추가합니다.
    ///
    /// 입력된 자모로 인해 새로운 음절 조합이 시작되어 이전 음절이 완성되면,
    /// 완성된 한글 음절 문자를 `Some(char)`로 반환합니다.
    /// 조합이 계속 진행 중이면 `None`을 반환합니다.
    ///
    /// # 매개변수
    ///
    /// * `jamo` - 입력할 한글 자모 (`JamoEnum`). 초성, 중성, 종성 또는 특수 문자일 수 있습니다.
    ///
    /// # 반환값
    ///
    /// * `Some(char)` - 입력된 자모로 인해 이전 음절 조합이 완료된 경우, 완성된 한글 음절.
    /// * `None` - 조합이 계속 진행 중인 경우.
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        self.jamo_queue.push_back(jamo);
        if !self.compose_hangul() {
            self.jamo_queue.pop_back();
            self.compose_hangul();
            let complete_hangul = self.current_hangul_char.get_syllable();
            self.last_jamo_queue.clear();
            self.last_jamo_queue.extend(&self.jamo_queue);
            self.jamo_queue.clear();
            self.jamo_queue.push_back(jamo);
            self.clear();
            self.compose_hangul();
            complete_hangul.ok()
        } else {
            None
        }
    }

    /// 마지막으로 입력된 한글 자모를 제거하고 조합 상태를 갱신합니다.
    ///
    /// # 반환값
    ///
    /// * `Some(JamoEnum)` - 성공적으로 제거된 자모.
    /// * `None` - 제거할 자모가 없는 경우 (조합 큐가 비어 있는 경우).
    fn remove_jamo(&mut self) -> Option<JamoEnum> {
        if self.jamo_queue.is_empty() {
            None
        } else {
            let jamo = self.jamo_queue.pop_back();
            self.compose_hangul();
            jamo
        }
    }

    /// 현재 `jamo_queue`에 저장된 자모들을 바탕으로 한글 음절을 조합합니다.
    ///
    /// 내부적으로 `compose_cho`, `compose_jung`, `compose_jong`을 호출하여
    /// `current_hangul_char`의 상태를 업데이트합니다.
    ///
    /// # 반환값
    ///
    /// * `true` - 조합에 성공했거나, 큐가 비어 있어 초기화된 경우.
    /// * `false` - 자모 조합 규칙에 맞지 않아 조합에 실패한 경우.
    fn compose_hangul(&mut self) -> bool {
        if self.jamo_queue.is_empty() {
            self.clear();
            return true;
        }

        if !self.compose_cho() || !self.compose_jung() || !self.compose_jong() {
            return false;
        }

        true
    }

    /// 현재까지 입력된 자모들을 강제로 조합하여 완성된 한글 음절을 반환하고, 조합 상태를 초기화합니다.
    ///
    /// 조합 중인 상태(`is_compose()`가 `true`인 경우)에만 동작합니다.
    /// 성공적으로 조합되면 현재 조합 상태(`jamo_queue`, `last_jamo_queue`, `current_hangul_char`)가 모두 초기화됩니다.
    ///
    /// # 반환값
    ///
    /// * `Some(char)` - 조합이 성공한 경우, 완성된 한글 음절.
    /// * `None` - 조합 중인 상태가 아니거나 조합에 실패한 경우.
    fn force_compose_hangul(&mut self) -> Option<char> {
        if self.is_compose() {
            self.compose_hangul();
            let complete_hangul = self.current_hangul_char.get_syllable();
            self.clear();
            self.jamo_queue.clear();
            self.last_jamo_queue.clear();
            complete_hangul.ok()
        } else {
            None
        }
    }

    /// 현재 한글 조합이 진행 중인지 여부를 확인합니다.
    ///
    /// # 반환값
    ///
    /// * `true` - 조합 중인 경우.
    /// * `false` - 조합 중이 아닌 경우 (큐가 비어 있음).
    fn is_compose(&self) -> bool {
        !self.jamo_queue.is_empty()
    }

    /// 다음에 입력될 자모가 새로운 음절을 시작해야 하는지 여부를 판단합니다.
    ///
    /// # 반환값
    ///
    /// * `true` - 새로운 음절 시작 조건에 맞는 경우.
    /// * `false` - 그렇지 않은 경우.
    fn is_new_syllable(&self) -> bool {
        self.is_new_syllable_internal()
    }

    /// 한글 초성 조합 (내부 사용)
    ///
    /// # 반환값
    ///
    /// * `true` - 조합에 성공한 경우
    /// * `false` - 조합에 실패한 경우
    fn compose_cho(&mut self) -> bool {
        let cho_phonemes: Vec<Cho> = self
            .jamo_queue
            .iter()
            .filter_map(|p| {
                if let JamoEnum::Cho(c) = p {
                    Some(*c)
                } else {
                    None
                }
            })
            .collect();

        if cho_phonemes.is_empty() {
            self.current_hangul_char.clear_cho();
        } else {
            let mut cho = cho_phonemes[0];
            for next_cho in cho_phonemes.iter().skip(1) {
                if let Some(combined_map) = self.combined_jamo.get(&JamoEnum::Cho(cho)) {
                    if let Some(JamoEnum::Cho(new_cho)) =
                        combined_map.get(&JamoEnum::Cho(*next_cho))
                    {
                        cho = *new_cho;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul_char.set_cho_object(Some(cho));
        }
        true
    }

    /// 한글 중성 조합 (내부 사용)
    ///
    /// # 반환값
    ///
    /// * `true` - 조합에 성공한 경우
    /// * `false` - 조합에 실패한 경우
    fn compose_jung(&mut self) -> bool {
        let jung_phonemes: Vec<Jung> = self
            .jamo_queue
            .iter()
            .filter_map(|p| {
                if let JamoEnum::Jung(j) = p {
                    Some(*j)
                } else {
                    None
                }
            })
            .collect();

        if jung_phonemes.is_empty() {
            self.current_hangul_char.clear_jung();
        } else {
            let mut jung = jung_phonemes[0];
            for next_jung in jung_phonemes.iter().skip(1) {
                if let Some(combined_map) = self.combined_jamo.get(&JamoEnum::Jung(jung)) {
                    if let Some(JamoEnum::Jung(new_jung)) =
                        combined_map.get(&JamoEnum::Jung(*next_jung))
                    {
                        jung = *new_jung;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul_char.set_jung_object(Some(jung));
        }
        true
    }

    /// 한글 종성 조합 (내부 사용)
    ///
    /// # 반환값
    ///
    /// * `true` - 조합에 성공한 경우
    /// * `false` - 조합에 실패한 경우
    fn compose_jong(&mut self) -> bool {
        let jong_phonemes: Vec<Jong> = self
            .jamo_queue
            .iter()
            .filter_map(|p| {
                if let JamoEnum::Jong(j) = p {
                    Some(*j)
                } else {
                    None
                }
            })
            .collect();

        if jong_phonemes.is_empty() {
            self.current_hangul_char.clear_jong();
        } else {
            let mut jong = jong_phonemes[0];
            for next_jong in jong_phonemes.iter().skip(1) {
                if let Some(combined_map) = self.combined_jamo.get(&JamoEnum::Jong(jong)) {
                    if let Some(JamoEnum::Jong(new_jong)) =
                        combined_map.get(&JamoEnum::Jong(*next_jong))
                    {
                        jong = *new_jong;
                    } else {
                        return false;
                    }
                } else {
                    return false;
                }
            }
            self.current_hangul_char.set_jong_object(Some(jong));
        }
        true
    }

    /// 자모 모두 지우기
    fn clear_jamo(&mut self) {
        self.current_hangul_char.clear();
    }

    /// 현재 조합된 초성 얻기
    ///
    /// # 반환값
    ///
    /// * `Some(Cho)` - 초성이 설정된 경우
    /// * `None` - 초성이 설정되지 않은 경우
    fn get_current_cho(&self) -> Option<Cho> {
        self.current_hangul_char.get_cho()
    }

    /// 현재 조합된 중성 얻기
    ///
    /// # 반환값
    ///
    /// * `Some(Jung)` - 중성이 설정된 경우
    /// * `None` - 중성이 설정되지 않은 경우
    fn get_current_jung(&self) -> Option<Jung> {
        self.current_hangul_char.get_jung()
    }

    /// 현재 조합된 종성 얻기
    ///
    /// # 반환값
    ///
    /// * `Some(Jong)` - 종성이 설정된 경우
    /// * `None` - 종성이 설정되지 않은 경우
    fn get_current_jong(&self) -> Option<Jong> {
        self.current_hangul_char.get_jong()
    }

    /// 초성 설정
    ///
    /// # 매개변수
    ///
    /// * `cho` - 설정할 초성 값
    ///
    /// # 반환값
    ///
    /// 설정 성공 여부 (현재 구현에서는 항상 `true`)
    fn set_current_cho(&mut self, cho: Option<Cho>) -> bool {
        self.current_hangul_char.set_cho_object(cho)
    }

    /// 중성 설정
    ///
    /// # 매개변수
    ///
    /// * `jung` - 설정할 중성 값
    ///
    /// # 반환값
    ///
    /// 설정 성공 여부 (현재 구현에서는 항상 `true`)
    fn set_current_jung(&mut self, jung: Option<Jung>) -> bool {
        self.current_hangul_char.set_jung_object(jung)
    }

    /// 종성 설정
    ///
    /// # 매개변수
    ///
    /// * `jong` - 설정할 종성 값
    ///
    /// # 반환값
    ///
    /// 설정 성공 여부 (현재 구현에서는 항상 `true`)
    fn set_current_jong(&mut self, jong: Option<Jong>) -> bool {
        self.current_hangul_char.set_jong_object(jong)
    }

    /// 자모 조합 테이블 접근
    ///
    /// # 반환값
    ///
    /// 자모 조합 테이블의 참조
    fn get_combined_jamo(&self) -> &HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        &self.combined_jamo
    }

    /// 자모가 입력되는 순서대로 저장하는 큐에 접근
    ///
    /// # 반환값
    ///
    /// 자모 큐의 가변 참조
    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        &mut self.jamo_queue
    }

    /// 직전 큐에 접근
    ///
    /// # 반환값
    ///
    /// 직전 큐의 가변 참조
    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum> {
        &mut self.last_jamo_queue
    }

    /// 자모 조합 테이블에 접근
    ///
    /// # 반환값
    ///
    /// 자모 조합 테이블의 가변 참조
    fn combined_jamo(&mut self) -> &mut HashMap<JamoEnum, HashMap<JamoEnum, JamoEnum>> {
        &mut self.combined_jamo
    }

    /// 현재 조합 중인 한글에 접근
    ///
    /// # 반환값
    ///
    /// 현재 조합 중인 한글의 가변 참조
    fn current_hangul(&mut self) -> &mut HangulChar {
        &mut self.current_hangul_char
    }
}
