use unin::hangul::char::{CHOSEONG_NUMBER, JONGSEONG_NUMBER, JUNGSEONG_NUMBER, SYLLABLE_BASE};

fn main() {
    // 초성 (0..19), 중성 (0..21), 종성 (0..28) 순서로 반복
    for cho_seq in 0..CHOSEONG_NUMBER {
        for jung_seq in 0..JUNGSEONG_NUMBER {
            for jong_seq in 0..JONGSEONG_NUMBER {
                // 유니코드 한글 음절 계산 공식
                let code = SYLLABLE_BASE
                    + (cho_seq * JUNGSEONG_NUMBER * JONGSEONG_NUMBER
                        + jung_seq * JONGSEONG_NUMBER
                        + jong_seq) as u32;

                if let Some(syllable) = std::char::from_u32(code) {
                    print!("{}", syllable);
                }
            }
            println!();
        }
    }
    println!(); // 마지막에 줄바꿈 추가
}
