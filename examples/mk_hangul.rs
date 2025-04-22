use unin::hangul::char::{CHOSEONG_NUMBER, JONGSEONG_NUMBER, JUNGSEONG_NUMBER};
use unin::hangul::HangulChar;

fn main() {
    // 초성 (0..19), 중성 (0..21), 종성 (0..28) 순서로 반복
    for cho_seq in 0..CHOSEONG_NUMBER {
        for jung_seq in 0..JUNGSEONG_NUMBER {
            for jong_seq in 0..JONGSEONG_NUMBER {
                // 유니코드 한글 음절 계산 공식
                let syllable = HangulChar::from_jamo_sequences(
                    cho_seq as i32,
                    jung_seq as i32,
                    jong_seq as i32,
                );
                print!("{}", syllable);
            }
            println!();
        }
    }
}
