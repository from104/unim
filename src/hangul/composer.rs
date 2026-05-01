use crate::hangul::char::HangulChar;
use crate::hangul::jamo::JamoEnum;
use crate::hangul::jamo::*;
use crate::unim_log;
/**
 * 한글 조합 취상위 클래스
 * @author "KiHyeon Seo" <from104@gmail.com>
 */
// builder.rs
use std::collections::{HashMap, VecDeque};

/// 자모 조합 규칙을 정의하는 해시맵 타입 앨리어스입니다.
/// 튜플 키 `(첫번째 자모, 두번째 자모)`를 사용하여 조합된 자모를 조회합니다.
pub type CombinedJamoMap = HashMap<(JamoEnum, JamoEnum), JamoEnum>;

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
    /// `current_korean_char`의 상태를 업데이트합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 조합에 성공했거나, 큐가 비어 있어 초기화된 경우.
    /// * `false`: 자모 조합 규칙에 맞지 않아 조합에 실패한 경우.
    fn compose_korean(&mut self) -> bool;

    /// 현재까지 입력된 자모들을 강제로 조합하여 완성된 한글 음절을 반환하고, 조합 상태를 초기화합니다.
    ///
    /// 조합 중인 상태(`is_compose()`가 `true`인 경우)에만 동작합니다.
    /// 성공적으로 조합되면 현재 조합 상태(`jamo_queue`, `last_jamo_queue`, `current_korean_char`)가 모두 초기화됩니다.
    ///
    /// # 반환값
    ///
    /// * `Some(char)`: 조합이 성공한 경우, 완성된 한글 음절.
    /// * `None`: 조합 중인 상태가 아니거나 조합에 실패한 경우.
    fn force_compose_korean(&mut self) -> Option<char>;

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
    fn get_combined_jamo(&self) -> &CombinedJamoMap;

    // 자모가 입력되는 순서대로 저장하는 큐
    fn jamo_queue(&mut self) -> &mut VecDeque<JamoEnum>;

    // 직전 큐
    fn last_jamo_queue(&mut self) -> &mut VecDeque<JamoEnum>;

    // 자모 조합 테이블
    fn combined_jamo(&mut self) -> &mut CombinedJamoMap;

    // 현재 조합 중인 한글
    fn current_korean(&mut self) -> &mut HangulChar;
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
/// * `current_korean_char` - 현재 조합 중인 한글 음절
#[derive(Debug, Default)]
pub struct BaseHangulComposer {
    jamo_queue: VecDeque<JamoEnum>,
    last_jamo_queue: VecDeque<JamoEnum>,
    combined_jamo: CombinedJamoMap,
    current_korean_char: HangulChar,
}

impl BaseHangulComposer {
    /// 새로운 `BaseHangulComposer` 인스턴스를 생성합니다.
    ///
    /// # 반환값
    ///
    /// 초기화된 `BaseHangulComposer` 인스턴스
    pub fn new() -> Self {
        BaseHangulComposer {
            jamo_queue: VecDeque::with_capacity(6),
            last_jamo_queue: VecDeque::with_capacity(6),
            combined_jamo: HashMap::new(),
            current_korean_char: HangulChar::default(),
        }
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
            .back().is_some_and(|last_jamo| matches!(last_jamo, JamoEnum::Cho(_) if self.current_korean_char.is_filled_jung()))
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
        self.current_korean_char.get_cho()
    }

    /// 현재 조합된 중성을 반환합니다.
    ///
    /// # 반환값
    ///
    /// * `Some(Jung)`: 중성이 설정된 경우
    /// * `None`: 중성이 설정되지 않은 경우
    pub fn get_jung(&self) -> Option<Jung> {
        self.current_korean_char.get_jung()
    }

    /// 현재 조합된 종성을 반환합니다.
    ///
    /// # 반환값
    ///
    /// * `Some(Jong)`: 종성이 설정된 경우
    /// * `None`: 종성이 설정되지 않은 경우
    pub fn get_jong(&self) -> Option<Jong> {
        self.current_korean_char.get_jong()
    }

    /// 초성을 설정합니다.
    ///
    /// # 매개변수
    ///
    /// * `cho` - 설정할 초성 값
    pub fn set_cho(&mut self, cho: Option<Cho>) {
        self.current_korean_char.set_cho_object(cho);
    }

    /// 중성을 설정합니다.
    ///
    /// # 매개변수
    ///
    /// * `jung` - 설정할 중성 값
    pub fn set_jung(&mut self, jung: Option<Jung>) {
        self.current_korean_char.set_jung_object(jung);
    }

    /// 종성을 설정합니다.
    ///
    /// # 매개변수
    ///
    /// * `jong` - 설정할 종성 값
    pub fn set_jong(&mut self, jong: Option<Jong>) {
        self.current_korean_char.set_jong_object(jong);
    }

    /// 초성을 초기화합니다.
    pub fn clear_cho(&mut self) {
        self.current_korean_char.clear_cho();
    }

    /// 중성을 초기화합니다.
    pub fn clear_jung(&mut self) {
        self.current_korean_char.clear_jung();
    }

    /// 종성을 초기화합니다.
    pub fn clear_jong(&mut self) {
        self.current_korean_char.clear_jong();
    }

    /// 모든 자모를 초기화합니다.
    pub fn clear(&mut self) {
        self.current_korean_char.clear();
    }

    /// 초성이 설정되어 있는지 확인합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 초성이 설정된 경우
    /// * `false`: 초성이 설정되지 않은 경우
    pub fn is_filled_cho(&self) -> bool {
        self.current_korean_char.is_filled_cho()
    }

    /// 중성이 설정되어 있는지 확인합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 중성이 설정된 경우
    /// * `false`: 중성이 설정되지 않은 경우
    pub fn is_filled_jung(&self) -> bool {
        self.current_korean_char.is_filled_jung()
    }

    /// 종성이 설정되어 있는지 확인합니다.
    ///
    /// # 반환값
    ///
    /// * `true`: 종성이 설정된 경우
    /// * `false`: 종성이 설정되지 않은 경우
    pub fn is_filled_jong(&self) -> bool {
        self.current_korean_char.is_filled_jong()
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

                    if let Some(JamoEnum::Cho(combined_cho)) =
                        self.combined_jamo.get(&(first_jamo, second_jamo))
                    {
                        self.set_cho(Some(*combined_cho));
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

                    if let Some(JamoEnum::Jung(combined_jung)) =
                        self.combined_jamo.get(&(first_jamo, second_jamo))
                    {
                        self.set_jung(Some(*combined_jung));
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

                    if let Some(JamoEnum::Jong(combined_jong)) =
                        self.combined_jamo.get(&(first_jamo, second_jamo))
                    {
                        self.set_jong(Some(*combined_jong));
                    } else {
                        return false;
                    }
                }
            }
        }
        true
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

    /// 콜백 기반 자모 추가 메서드
    ///
    /// 외부에서 주입된 `compose_fn` 함수를 사용하여 조합 규칙을 적용합니다.
    /// 이를 통해 2벌식/3벌식이 각자의 compose_korean 로직을 사용할 수 있습니다.
    ///
    /// # 매개변수
    ///
    /// * `jamo` - 추가할 자모
    /// * `compose_fn` - 조합을 시도하는 클로저 (성공 시 true, 실패 시 false 반환)
    ///
    /// # 반환값
    ///
    /// * `Some(char)` - 이전 음절이 완성된 경우
    /// * `None` - 조합이 계속 진행 중인 경우
    pub fn add_jamo_with<F>(&mut self, jamo: JamoEnum, compose_fn: F) -> Option<char>
    where
        F: Fn(&mut Self) -> bool,
    {
        unim_log!("COMPOSER", "BaseComposer.add_jamo_with: {:?}", jamo);

        if !self.is_valid_jamo(&jamo) {
            return None;
        }

        self.jamo_queue.push_back(jamo);

        if compose_fn(self) {
            unim_log!(
                "COMPOSER",
                "  -> 조합 계속: current_korean={:?}",
                self.current_korean_char
            );
            None
        } else {
            // 조합 실패 -> 이전 글자 완성 후 새 글자 시작
            self.jamo_queue.pop_back();
            compose_fn(self);
            let complete_korean = self
                .current_korean_char
                .get_syllable()
                .ok()
                .or_else(|| extract_incomplete_compat_char(&self.current_korean_char));
            unim_log!("COMPOSER", "  -> 음절 분리: complete={:?}", complete_korean);

            // 큐 상태 백업 및 초기화
            self.last_jamo_queue.clear();
            self.last_jamo_queue.extend(&self.jamo_queue);
            self.jamo_queue.clear();
            self.jamo_queue.push_back(jamo);
            self.clear();

            // 새 음절 시작 시 compose_fn을 다시 호출하여 상태 업데이트
            compose_fn(self);

            unim_log!(
                "COMPOSER",
                "  -> 새 current_korean: {:?}",
                self.current_korean_char
            );

            complete_korean
        }
    }
}

/// 음절이 완성되지 않은 `HangulChar`(예: 초성만, 중성만, 종성만 채워진 상태)에서
/// 호환용 자모(compat jamo) 한 글자를 추출한다.
///
/// `add_jamo`/`add_jamo_with`의 분리 경로에서, 직전 음절이 완성 음절이 아닌
/// 단일 자모만 채워진 경우에도 frontend가 commit 신호를 받을 수 있도록
/// 호환자모로 흘려보낼 때 사용된다.
///
/// # 반환값
/// * `Some(char)` - 호환자모 문자열이 정확히 한 글자(= cho 단독, jung 단독, jong 단독)일 때
/// * `None` - 비어 있거나 두 글자 이상(예: cho+jung)일 때 (corner case 보존)
fn extract_incomplete_compat_char(current: &HangulChar) -> Option<char> {
    let s = current.to_compat_jamo_string();
    let mut iter = s.chars();
    let first = iter.next()?;
    if iter.next().is_some() {
        // 두 자모 이상은 분리 정책이 모호 → 기존 동작(None) 유지
        return None;
    }
    Some(first)
}

impl HangulComposer for BaseHangulComposer {
    /// 한국어 자모를 입력받아 현재 조합 상태에 추가합니다.
    ///
    /// 입력된 자모로 인해 새로운 음절 조합이 시작되어 이전 음절이 완성되면,
    /// 완성된 한국어 음절 문자를 `Some(char)`로 반환합니다.
    /// 조합이 계속 진행 중이면 `None`을 반환합니다.
    ///
    /// # 매개변수
    ///
    /// * `jamo` - 입력할 한국어 자모 (`JamoEnum`). 초성, 중성, 종성 또는 특수 문자일 수 있습니다.
    ///
    /// # 반환값
    ///
    /// * `Some(char)` - 입력된 자모로 인해 이전 음절 조합이 완료된 경우, 완성된 한국어 음절.
    /// * `None` - 조합이 계속 진행 중인 경우.
    fn add_jamo(&mut self, jamo: JamoEnum) -> Option<char> {
        unim_log!("COMPOSER", "BaseComposer.add_jamo: {:?}", jamo);
        self.jamo_queue.push_back(jamo);
        if !self.compose_korean() {
            self.jamo_queue.pop_back();
            self.compose_korean();
            let complete_korean = self
                .current_korean_char
                .get_syllable()
                .ok()
                .or_else(|| extract_incomplete_compat_char(&self.current_korean_char));
            unim_log!("COMPOSER", "  -> 음절 분리: complete={:?}", complete_korean);
            self.last_jamo_queue.clear();
            self.last_jamo_queue.extend(&self.jamo_queue);
            self.jamo_queue.clear();
            self.jamo_queue.push_back(jamo);
            self.clear();
            self.compose_korean();
            unim_log!(
                "COMPOSER",
                "  -> 새 current_korean: {:?}",
                self.current_korean_char
            );
            complete_korean
        } else {
            unim_log!(
                "COMPOSER",
                "  -> 조합 계속: current_korean={:?}",
                self.current_korean_char
            );
            None
        }
    }

    /// 마지막으로 입력된 한국어 자모를 제거하고 조합 상태를 갱신합니다.
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
            self.compose_korean();
            jamo
        }
    }

    /// 현재 `jamo_queue`에 저장된 자모들을 바탕으로 한국어 음절을 조합합니다.
    ///
    /// 내부적으로 `compose_cho`, `compose_jung`, `compose_jong`을 호출하여
    /// `current_korean_char`의 상태를 업데이트합니다.
    ///
    /// # 반환값
    ///
    /// * `true` - 조합에 성공했거나, 큐가 비어 있어 초기화된 경우.
    /// * `false` - 자모 조합 규칙에 맞지 않아 조합에 실패한 경우.
    fn compose_korean(&mut self) -> bool {
        unim_log!(
            "COMPOSER",
            "BaseComposer.compose_korean: queue={:?}",
            self.jamo_queue
        );
        if self.jamo_queue.is_empty() {
            self.clear();
            unim_log!("COMPOSER", "  -> 큐 비어있음, true");
            return true;
        }

        let cho_ok = self.compose_cho();
        let jung_ok = self.compose_jung();
        let jong_ok = self.compose_jong();
        unim_log!(
            "COMPOSER",
            "  -> compose_cho={}, compose_jung={}, compose_jong={}",
            cho_ok,
            jung_ok,
            jong_ok
        );
        unim_log!(
            "COMPOSER",
            "  -> current_korean: {:?}",
            self.current_korean_char
        );

        if !cho_ok || !jung_ok || !jong_ok {
            return false;
        }

        true
    }

    /// 현재까지 입력된 자모들을 강제로 조합하여 완성된 한국어 음절을 반환하고, 조합 상태를 초기화합니다.
    ///
    /// 조합 중인 상태(`is_compose()`가 `true`인 경우)에만 동작합니다.
    /// 성공적으로 조합되면 현재 조합 상태(`jamo_queue`, `last_jamo_queue`, `current_korean_char`)가 모두 초기화됩니다.
    ///
    /// # 반환값
    ///
    /// * `Some(char)` - 조합이 성공한 경우, 완성된 한국어 음절.
    /// * `None` - 조합 중인 상태가 아니거나 조합에 실패한 경우.
    fn force_compose_korean(&mut self) -> Option<char> {
        if self.is_compose() {
            self.compose_korean();
            let complete_korean = self.current_korean_char.get_syllable();
            self.clear();
            self.jamo_queue.clear();
            self.last_jamo_queue.clear();
            complete_korean.ok()
        } else {
            None
        }
    }

    /// 현재 한국어 조합이 진행 중인지 여부를 확인합니다.
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

    /// 한국어 초성 조합 (내부 사용)
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
            self.current_korean_char.clear_cho();
        } else {
            let mut cho = cho_phonemes[0];
            for next_cho in cho_phonemes.iter().skip(1) {
                if let Some(JamoEnum::Cho(new_cho)) = self
                    .combined_jamo
                    .get(&(JamoEnum::Cho(cho), JamoEnum::Cho(*next_cho)))
                {
                    cho = *new_cho;
                } else {
                    return false;
                }
            }
            self.current_korean_char.set_cho_object(Some(cho));
        }
        true
    }

    /// 한국어 중성 조합 (내부 사용)
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
            self.current_korean_char.clear_jung();
        } else {
            let mut jung = jung_phonemes[0];
            for next_jung in jung_phonemes.iter().skip(1) {
                if let Some(JamoEnum::Jung(new_jung)) = self
                    .combined_jamo
                    .get(&(JamoEnum::Jung(jung), JamoEnum::Jung(*next_jung)))
                {
                    jung = *new_jung;
                } else {
                    return false;
                }
            }
            self.current_korean_char.set_jung_object(Some(jung));
        }
        true
    }

    /// 한국어 종성 조합 (내부 사용)
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
            self.current_korean_char.clear_jong();
        } else {
            let mut jong = jong_phonemes[0];
            for next_jong in jong_phonemes.iter().skip(1) {
                if let Some(JamoEnum::Jong(new_jong)) = self
                    .combined_jamo
                    .get(&(JamoEnum::Jong(jong), JamoEnum::Jong(*next_jong)))
                {
                    jong = *new_jong;
                } else {
                    return false;
                }
            }
            self.current_korean_char.set_jong_object(Some(jong));
        }
        true
    }

    /// 자모 모두 지우기
    fn clear_jamo(&mut self) {
        self.current_korean_char.clear();
    }

    /// 현재 조합된 초성 얻기
    ///
    /// # 반환값
    ///
    /// * `Some(Cho)` - 초성이 설정된 경우
    /// * `None` - 초성이 설정되지 않은 경우
    fn get_current_cho(&self) -> Option<Cho> {
        self.current_korean_char.get_cho()
    }

    /// 현재 조합된 중성 얻기
    ///
    /// # 반환값
    ///
    /// * `Some(Jung)` - 중성이 설정된 경우
    /// * `None` - 중성이 설정되지 않은 경우
    fn get_current_jung(&self) -> Option<Jung> {
        self.current_korean_char.get_jung()
    }

    /// 현재 조합된 종성 얻기
    ///
    /// # 반환값
    ///
    /// * `Some(Jong)` - 종성이 설정된 경우
    /// * `None` - 종성이 설정되지 않은 경우
    fn get_current_jong(&self) -> Option<Jong> {
        self.current_korean_char.get_jong()
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
        self.current_korean_char.set_cho_object(cho)
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
        self.current_korean_char.set_jung_object(jung)
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
        self.current_korean_char.set_jong_object(jong)
    }

    /// 자모 조합 테이블 접근
    ///
    /// # 반환값
    ///
    /// 자모 조합 테이블의 참조
    fn get_combined_jamo(&self) -> &CombinedJamoMap {
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
    fn combined_jamo(&mut self) -> &mut CombinedJamoMap {
        &mut self.combined_jamo
    }

    /// 현재 조합 중인 한국어에 접근
    ///
    /// # 반환값
    ///
    /// 현재 조합 중인 한국어의 가변 참조
    fn current_korean(&mut self) -> &mut HangulChar {
        &mut self.current_korean_char
    }
}
