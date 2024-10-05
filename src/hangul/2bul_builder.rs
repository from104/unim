use std::collections::HashMap;
use super::hangul::Hangul;
use super::phoneme::{Onset, Nucleus, Coda, Jamo};

/// 2벌식 한글 입력 방식을 구현한 빌더 구조체
pub struct HangulBuilder2Bul {
    jamo_stack: Vec<Jamo>,          // 현재 입력 중인 자모를 저장하는 스택
    last_jamo_stack: Vec<Jamo>,     // 이전에 입력한 자모를 저장하는 스택
    combined_jamo: HashMap<Jamo, HashMap<Jamo, Jamo>>, // 자모 조합 규칙을 저장하는 해시맵
    current_hangul: Option<Hangul>, // 현재 조합 중인 한글 음절
}

impl HangulBuilder2Bul {
    /// 새로운 HangulBuilder2Bul 인스턴스를 생성합니다.
    pub fn new() -> Self {
        let mut builder = HangulBuilder2Bul {
            jamo_stack: Vec::new(),
            last_jamo_stack: Vec::new(),
            combined_jamo: HashMap::new(),
            current_hangul: None,
        };
        builder.init_combined_jamo();
        builder
    }

    /// 자모 조합 규칙을 초기화합니다.
    fn init_combined_jamo(&mut self) {
        // 모음 조합 규칙 설정
        let mut o = HashMap::new();
        o.insert(Nucleus::A, Nucleus::Wa);   // ㅗ + ㅏ = ㅘ
        o.insert(Nucleus::Ae, Nucleus::Wae); // ㅗ + ㅐ = ㅙ
        o.insert(Nucleus::I, Nucleus::Oe);   // ㅗ + ㅣ = ㅚ
        self.combined_jamo.insert(Nucleus::O, o);

        let mut u = HashMap::new();
        u.insert(Nucleus::Eo, Nucleus::Wo);  // ㅜ + ㅓ = ㅝ
        u.insert(Nucleus::E, Nucleus::We);   // ㅜ + ㅔ = ㅞ
        u.insert(Nucleus::I, Nucleus::Wi);   // ㅜ + ㅣ = ㅟ
        self.combined_jamo.insert(Nucleus::U, u);

        let mut eu = HashMap::new();
        eu.insert(Nucleus::I, Nucleus::Ui);  // ㅡ + ㅣ = ㅢ
        self.combined_jamo.insert(Nucleus::Eu, eu);

        // 자음 조합 규칙 설정
        let mut g = HashMap::new();
        g.insert(Coda::S, Coda::Gs);         // ㄱ + ㅅ = ㄳ
        self.combined_jamo.insert(Coda::G, g);

        let mut n = HashMap::new();
        n.insert(Coda::J, Coda::Nj);         // ㄴ + ㅈ = ㄵ
        n.insert(Coda::H, Coda::Nh);         // ㄴ + ㅎ = ㄶ
        self.combined_jamo.insert(Coda::N, n);

        let mut l = HashMap::new();
        l.insert(Coda::G, Coda::Lg);         // ㄹ + ㄱ = ㄺ
        l.insert(Coda::M, Coda::Lm);         // ㄹ + ㅁ = ㄻ
        l.insert(Coda::B, Coda::Lb);         // ㄹ + ㅂ = ㄼ
        l.insert(Coda::S, Coda::Ls);         // ㄹ + ㅅ = ㄽ
        l.insert(Coda::T, Coda::Lt);         // ㄹ + ㅌ = ㄾ
        l.insert(Coda::P, Coda::Lp);         // ㄹ + ㅍ = ㄿ
        l.insert(Coda::H, Coda::Lh);         // ㄹ + ㅎ = ㅀ
        self.combined_jamo.insert(Coda::L, l);

        let mut b = HashMap::new();
        b.insert(Coda::S, Coda::Bs);         // ㅂ + ㅅ = ㅄ
        self.combined_jamo.insert(Coda::B, b);
    }

    /// 새로운 자모를 추가하고 한글을 조합합니다.
    pub fn add_jamo(&mut self, jamo: Jamo) -> Option<Hangul> {
        self.jamo_stack.push(jamo);
        if self.build_hangul() {
            None // 자모가 성공적으로 조합되었으면 None 반환
        } else {
            self.jamo_stack.pop();
            // 도깨비불 현상 처리
            if let Some(Jamo::Coda(_)) = self.jamo_stack.last() {
                if let Jamo::Nucleus(_) = jamo {
                    let coda = self.jamo_stack.pop().unwrap();
                    self.build_hangul();
                    let completed_hangul = self.current_hangul.take();
                    self.last_jamo_stack = self.jamo_stack.clone();
                    self.jamo_stack.clear();
                    self.jamo_stack.push(coda.to_onset());
                    self.jamo_stack.push(jamo);
                    self.build_hangul();
                    return completed_hangul;
                }
            }
            let hangul = self.force_build_hangul();
            self.last_jamo_stack = self.jamo_stack.clone();
            self.jamo_stack.clear();
            self.jamo_stack.push(jamo);
            self.build_hangul();
            hangul
        }
    }

    /// 현재 자모 스택을 사용하여 한글을 조합합니다.
    fn build_hangul(&mut self) -> bool {
        let mut hangul = Hangul::new();
        let mut combined = false;

        for (i, jamo) in self.jamo_stack.iter().enumerate() {
            match (i, jamo) {
                (0, Jamo::Onset(onset)) => hangul.set_onset(Some(*onset)),
                (1, Jamo::Nucleus(nucleus)) => hangul.set_nucleus(Some(*nucleus)),
                (2, Jamo::Coda(coda)) => hangul.set_coda(Some(*coda)),
                (3, Jamo::Coda(new_coda)) => {
                    if let Some(current_coda) = hangul.coda() {
                        if let Some(combined_coda) = self.combined_jamo.get(&Jamo::Coda(current_coda))
                            .and_then(|map| map.get(&Jamo::Coda(*new_coda))) {
                            if let Jamo::Coda(combined) = combined_coda {
                                hangul.set_coda(Some(*combined));
                                combined = true;
                            }
                        } else {
                            return false;
                        }
                    } else {
                        return false;
                    }
                }
                _ => return false,
            }
        }

        self.current_hangul = Some(hangul);
        combined
    }

    /// 현재 자모 스택을 강제로 한글로 조합합니다.
    fn force_build_hangul(&mut self) -> Option<Hangul> {
        if self.jamo_stack.is_empty() {
            return None;
        }

        let mut hangul = Hangul::new();
        
        for (i, jamo) in self.jamo_stack.iter().enumerate() {
            match (i, jamo) {
                (0, Jamo::Onset(onset)) => hangul.set_onset(Some(*onset)),
                (1, Jamo::Nucleus(nucleus)) => hangul.set_nucleus(Some(*nucleus)),
                (2, Jamo::Coda(coda)) => hangul.set_coda(Some(*coda)),
                _ => break,
            }
        }

        Some(hangul)
    }

    /// 현재 조합 중인 한글을 초기화합니다.
    fn clear(&mut self) {
        self.current_hangul = None;
    }
}