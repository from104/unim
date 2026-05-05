use crate::hangul::char::HangulChar;
use crate::hangul::jamo::JamoEnum;
use crate::hangul::jamo::*;
use crate::keystroke::profile::MoachigiSpec;
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

/// 안마태/모아치기 자판에서 키별로 고정되는 자모 영역.
///
/// 자판 JSON의 layout 셀에서 해당 자모가 속한 영역을 사전 계산해 `KeyToRegionMap`에 보관.
/// `HangulComposer3BulMoachigi::add_jamo_with_region`에서 음절 경계 결정에 사용.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Region {
    /// 초성 영역.
    Cho,
    /// 중성 영역.
    Jung,
    /// 종성 영역.
    Jong,
}

/// 자모 큐에 함께 보관되는 키-출처 메타데이터.
///
/// 한 키에 여러 의미를 부여할 수 있는 자판(예: 세벌식 390/391)에서
/// "이 자모가 이중모음 head로 결합 가능한가" 같은 키-별 속성을 추적한다.
///
/// # 룰 A (vowel_combine_head)
/// 세벌식 390/391의 lower row4 4번째(v 키)·5번째(b 키)에 매핑된 ㅗ·ㅜ는
/// 단순 모음만 — 후속 ㅏ/ㅐ/ㅣ/ㅓ/ㅔ가 와도 합용 시도 안 함, 새 음절로 분리.
/// 반면 같은 자판의 `/`·`9` 키 ㅗ·ㅜ는 ㅘ/ㅙ/ㅚ/ㅝ/ㅞ/ㅟ로 결합 가능.
///
/// 기본값(`true`)은 두벌식 호환 — 자판 JSON `key_meta.vowel_combine_head` 명시 부재 시
/// 모든 ㅗ/ㅜ가 결합 가능으로 해석된다.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct JamoMeta {
    /// 이 자모가 이중모음의 첫 모음으로 결합 가능한지 여부.
    /// `false`면 후속 jung이 와도 합용하지 않고 음절 분리.
    pub vowel_combine_head: bool,
}

impl Default for JamoMeta {
    fn default() -> Self {
        Self {
            vowel_combine_head: true,
        }
    }
}

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

    /// `add_jamo`의 메타데이터 확장 버전.
    ///
    /// 룰 A(이중모음 head 결합 가부) 같은 키-출처 정보를 함께 큐에 보관하기 위함.
    /// 기본 구현은 `meta`를 무시하고 `add_jamo`로 위임하여 기존 호출 호환성을 보존.
    /// 룰 A를 지원하는 구현(`BaseHangulComposer`/`HangulComposer2Bul`/`HangulComposer3Bul`)은
    /// override하여 `meta`를 큐에 함께 push해야 한다.
    fn add_jamo_with_meta(&mut self, jamo: JamoEnum, _meta: JamoMeta) -> Option<char> {
        self.add_jamo(jamo)
    }

    /// `add_jamo`의 영역 정보 확장 버전 — 안마태/모아치기 전용.
    ///
    /// `region`은 이 키가 자판 JSON에서 속하는 고정 영역(Cho/Jung/Jong).
    /// `spec`은 현재 자판의 모아치기 파라미터.
    ///
    /// 기본 구현은 `region`/`spec`을 무시하고 `add_jamo_with_meta`로 위임하므로
    /// 기존 2벌식·3벌식 구현체에 영향 없음.
    /// `HangulComposer3BulMoachigi`만 이 메서드를 override하여 영역 기반 경계 알고리즘 적용.
    fn add_jamo_with_region(
        &mut self,
        jamo: JamoEnum,
        _region: Region,
        _spec: &MoachigiSpec,
    ) -> Option<char> {
        self.add_jamo_with_meta(jamo, JamoMeta::default())
    }

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
    /// `jamo_queue`와 길이·인덱스가 항상 일치하는 평행 메타데이터 큐.
    /// 룰 A(vowel_combine_head 검사)에서 사용. 모든 push/pop은 sync helper로 강제.
    meta_queue: VecDeque<JamoMeta>,
    last_meta_queue: VecDeque<JamoMeta>,
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
            meta_queue: VecDeque::with_capacity(6),
            last_meta_queue: VecDeque::with_capacity(6),
            combined_jamo: HashMap::new(),
            current_korean_char: HangulChar::default(),
        }
    }

    // ========================================================================
    // 큐 sync helpers — jamo_queue와 meta_queue를 항상 같은 길이로 유지.
    // 외부에서 `jamo_queue()` 가변 참조로 직접 mutate하는 곳이 1곳뿐(2bul 도깨비불).
    // 그 호출점만 `pop_back_synced()`로 옮기면 평행 큐 무결성 유지.
    // ========================================================================

    /// jamo와 meta를 한 쌍으로 큐에 push.
    pub fn push_back_synced(&mut self, jamo: JamoEnum, meta: JamoMeta) {
        self.jamo_queue.push_back(jamo);
        self.meta_queue.push_back(meta);
    }

    /// jamo와 meta를 한 쌍으로 큐에서 pop.
    pub fn pop_back_synced(&mut self) -> Option<(JamoEnum, JamoMeta)> {
        let j = self.jamo_queue.pop_back()?;
        let m = self.meta_queue.pop_back().unwrap_or_default();
        Some((j, m))
    }

    /// 두 큐를 모두 비움.
    pub fn clear_queues_synced(&mut self) {
        self.jamo_queue.clear();
        self.meta_queue.clear();
    }

    /// 현재 큐를 last로 백업하고 두 큐를 모두 비움.
    pub fn backup_to_last_synced(&mut self) {
        self.last_jamo_queue.clear();
        self.last_jamo_queue.extend(&self.jamo_queue);
        self.last_meta_queue.clear();
        self.last_meta_queue.extend(&self.meta_queue);
        self.jamo_queue.clear();
        self.meta_queue.clear();
    }

    /// 현재 jung_phoneme의 인덱스(자모 큐 기준 첫 번째 jung 위치)에 해당하는 meta를 반환.
    /// 룰 A 검사에서 "첫 번째 jung이 결합 head 가능한가" 판정에 사용.
    pub fn first_jung_meta(&self) -> Option<JamoMeta> {
        self.jamo_queue
            .iter()
            .zip(self.meta_queue.iter())
            .find_map(|(j, m)| match j {
                JamoEnum::Jung(_) => Some(*m),
                _ => None,
            })
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

    // NOTE: 과거 inherent `compose_cho`/`compose_jung`/`compose_jong`이 여기 있었으나
    // trait impl(아래 `impl HangulComposer for BaseHangulComposer`)와 중복이고,
    // method resolution이 inherent 우선이라 trait impl 코드가 dead로 묻혀있었다.
    // 룰 A를 trait impl `compose_jung`에 추가하면서 inherent를 제거 — 이제 모든 호출이
    // trait method로 dispatch되어 룰 A가 일관 적용된다.

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

    /// 콜백 기반 자모 추가 메서드 (룰 A meta 포함).
    ///
    /// 외부에서 주입된 `compose_fn` 함수를 사용하여 조합 규칙을 적용합니다.
    /// 이를 통해 2벌식/3벌식이 각자의 compose_korean 로직을 사용할 수 있습니다.
    ///
    /// # 매개변수
    ///
    /// * `jamo` - 추가할 자모
    /// * `meta` - 자모 큐에 함께 보관될 메타데이터 (룰 A: vowel_combine_head).
    ///   기본 호출에서는 `JamoMeta::default()`(=결합 가능) 사용.
    /// * `compose_fn` - 조합을 시도하는 클로저 (성공 시 true, 실패 시 false 반환)
    ///
    /// # 반환값
    ///
    /// * `Some(char)` - 이전 음절이 완성된 경우
    /// * `None` - 조합이 계속 진행 중인 경우
    pub fn add_jamo_with<F>(
        &mut self,
        jamo: JamoEnum,
        meta: JamoMeta,
        compose_fn: F,
    ) -> Option<char>
    where
        F: Fn(&mut Self) -> bool,
    {
        unim_log!(
            "COMPOSER",
            "BaseComposer.add_jamo_with_meta: {:?} meta={:?}",
            jamo,
            meta
        );

        if !self.is_valid_jamo(&jamo) {
            return None;
        }

        self.push_back_synced(jamo, meta);

        if compose_fn(self) {
            unim_log!(
                "COMPOSER",
                "  -> 조합 계속: current_korean={:?}",
                self.current_korean_char
            );
            None
        } else {
            // 조합 실패 -> 이전 글자 완성 후 새 글자 시작
            self.pop_back_synced();
            compose_fn(self);
            let complete_korean = self
                .current_korean_char
                .get_syllable()
                .ok()
                .or_else(|| extract_incomplete_compat_char(&self.current_korean_char));
            unim_log!("COMPOSER", "  -> 음절 분리: complete={:?}", complete_korean);

            // 큐 상태 백업 및 초기화 (jamo_queue + meta_queue 동시)
            self.backup_to_last_synced();
            self.push_back_synced(jamo, meta);
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
        self.add_jamo_with_meta(jamo, JamoMeta::default())
    }

    fn add_jamo_with_meta(&mut self, jamo: JamoEnum, meta: JamoMeta) -> Option<char> {
        unim_log!(
            "COMPOSER",
            "BaseComposer.add_jamo_with_meta: {:?} meta={:?}",
            jamo,
            meta
        );
        self.push_back_synced(jamo, meta);
        if !self.compose_korean() {
            self.pop_back_synced();
            self.compose_korean();
            let complete_korean = self
                .current_korean_char
                .get_syllable()
                .ok()
                .or_else(|| extract_incomplete_compat_char(&self.current_korean_char));
            unim_log!("COMPOSER", "  -> 음절 분리: complete={:?}", complete_korean);
            self.backup_to_last_synced();
            self.push_back_synced(jamo, meta);
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
            let popped = self.pop_back_synced().map(|(j, _)| j);
            self.compose_korean();
            popped
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
            // jamo_queue와 meta_queue를 동시 초기화 + last_*도 동시 초기화
            self.clear_queues_synced();
            self.last_jamo_queue.clear();
            self.last_meta_queue.clear();
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
    /// # 룰 A — vowel_combine_head 검사
    /// 두 개 이상의 jung이 큐에 있을 때, 첫 번째 jung의 `meta.vowel_combine_head`가 `false`면
    /// 결합 거부 → `false` 반환 → 음절 분리 path로 진입.
    /// 첫 번째 jung이 결합 가능(`true`)이어야만 다음 jung과 합용 시도.
    ///
    /// # 반환값
    ///
    /// * `true` - 조합에 성공한 경우
    /// * `false` - 조합에 실패한 경우
    fn compose_jung(&mut self) -> bool {
        // jung phoneme과 그 meta를 평행하게 수집 (jamo_queue와 meta_queue는 항상 동일 길이).
        let jung_with_meta: Vec<(Jung, JamoMeta)> = self
            .jamo_queue
            .iter()
            .zip(self.meta_queue.iter())
            .filter_map(|(p, m)| match p {
                JamoEnum::Jung(j) => Some((*j, *m)),
                _ => None,
            })
            .collect();

        if jung_with_meta.is_empty() {
            self.current_korean_char.clear_jung();
        } else {
            let (mut jung, first_meta) = jung_with_meta[0];
            // 두 번째 이후 jung이 있으면 결합 시도 직전에 룰 A 검사.
            // 첫 jung이 vowel_combine_head=false면 즉시 결합 거부.
            if jung_with_meta.len() > 1 && !first_meta.vowel_combine_head {
                unim_log!(
                    "COMPOSER",
                    "compose_jung: 룰 A 결합 거부 (first.vowel_combine_head=false)"
                );
                return false;
            }
            for (next_jung, _) in jung_with_meta.iter().skip(1) {
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
