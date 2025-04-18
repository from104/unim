/// UNIN (Unicode Input) - 한/영 자판 변환기
///
/// 이 프로그램은 한글과 영문 자판 간의 변환을 수행합니다.
/// - 영문 타이핑을 한글로 변환
/// - 한글 타이핑을 영문으로 변환
/// - 다양한 자판 지원 (두벌식 표준, 세벌식 390, 세벌식 391)
/// - 다양한 영문 자판 지원 (QWERTY, Dvorak)
use crate::keystroke::hangul_to_keystrokes::hangul_to_keystrokes;
use crate::keystroke::keyboard_map::KeyboardMap;
use crate::keystroke::keystrokes_to_hangul::keystrokes_to_hangul;
use hangul::composer_with_2bul::HangulComposer2Bul;
use hangul::composer_with_3bul::HangulComposer3Bul;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process;

pub mod hangul;
pub mod keystroke;

/// 한글 자판 모드
///
/// 사용할 한글 자판의 종류를 정의합니다.
#[derive(Clone, Copy)]
enum KeyboardMode {
    /// 두벌식 표준 자판
    TwoBulStd,
    /// 세벌식 390 자판
    ThreeBul390,
    /// 세벌식 391 자판
    ThreeBul391,
}

/// 영문 자판 모드
///
/// 사용할 영문 자판의 종류를 정의합니다.
#[derive(Clone, Copy)]
enum EnglishKeyboardMode {
    /// QWERTY 자판
    Qwerty,
    /// Dvorak 자판
    Dvorak,
}

/// 변환 모드
///
/// 한/영 변환 방향을 정의합니다.
#[derive(Clone, Copy)]
enum ConversionMode {
    /// 한글을 영문으로 변환
    KoreanToEnglish,
    /// 영문을 한글로 변환
    EnglishToKorean,
}

/// 프로그램 설정
///
/// 명령행 인자로부터 파싱된 프로그램 설정을 포함합니다.
struct Config {
    /// 입력 파일 경로 (None인 경우 표준 입력 사용)
    input_file: Option<String>,
    /// 출력 파일 경로 (None인 경우 표준 출력 사용)
    output_file: Option<String>,
    /// 한글 자판 모드
    keyboard_mode: KeyboardMode,
    /// 영문 자판 모드
    english_keyboard_mode: EnglishKeyboardMode,
    /// 변환 모드
    conversion_mode: ConversionMode,
}

impl Config {
    /// 명령행 인자로부터 설정을 생성합니다.
    ///
    /// # 인자
    ///
    /// * `args` - 명령행 인자
    ///
    /// # 반환 값
    ///
    /// * `Result<Config, &'static str>` - 성공 시 Config 인스턴스, 실패 시 오류 메시지
    fn new(args: env::Args) -> Result<Config, &'static str> {
        let args: Vec<String> = args.collect();

        let mut input_file = None;
        let mut output_file = None;
        let mut keyboard_mode = KeyboardMode::TwoBulStd;
        let mut english_keyboard_mode = EnglishKeyboardMode::Qwerty;
        let mut conversion_mode = ConversionMode::EnglishToKorean;

        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "-h" | "--help" => {
                    print_help();
                    process::exit(0);
                }
                "-k" | "--korean" => {
                    conversion_mode = ConversionMode::EnglishToKorean;
                }
                "-e" | "--english" => {
                    conversion_mode = ConversionMode::KoreanToEnglish;
                }
                "-2" | "--2bulsik" => {
                    keyboard_mode = KeyboardMode::TwoBulStd;
                }
                "-3" | "--3bulsik" => {
                    keyboard_mode = KeyboardMode::ThreeBul390;
                }
                "-390" | "--3bul390" => {
                    keyboard_mode = KeyboardMode::ThreeBul390;
                }
                "-391" | "--3bul391" => {
                    keyboard_mode = KeyboardMode::ThreeBul391;
                }
                "-q" | "--qwerty" => {
                    english_keyboard_mode = EnglishKeyboardMode::Qwerty;
                }
                "-d" | "--dvorak" => {
                    english_keyboard_mode = EnglishKeyboardMode::Dvorak;
                }
                "-o" | "--output" => {
                    if i + 1 < args.len() {
                        output_file = Some(args[i + 1].clone());
                        i += 1;
                    } else {
                        return Err("출력 파일 이름이 필요합니다");
                    }
                }
                _ => {
                    // 입력 파일로 간주
                    if !args[i].starts_with('-') && input_file.is_none() {
                        input_file = Some(args[i].clone());
                    } else {
                        return Err("알 수 없는 인자: 입력 파일은 하나만 지정할 수 있습니다");
                    }
                }
            }
            i += 1;
        }

        Ok(Config {
            input_file,
            output_file,
            keyboard_mode,
            english_keyboard_mode,
            conversion_mode,
        })
    }
}

/// 도움말 메시지를 출력합니다.
fn print_help() {
    println!("UNIN (Unicode Input) - 한/영 자판 변환기");
    println!();
    println!("사용법: unin [옵션] [입력파일]");
    println!();
    println!("옵션:");
    println!("  -h, --help       이 도움말을 표시합니다");
    println!("  -k, --korean     영문 타자를 한글로 변환 (기본값)");
    println!("  -e, --english    한글 타자를 영문으로 변환");
    println!("  영어 자판:");
    println!("    -q, --qwerty   QWERTY 자판 (기본값)");
    println!("    -d, --dvorak   Dvorak 자판");
    println!("  한글 자판:");
    println!("    -2, --2bulsik  두벌식 표준 (기본값)");
    println!("    -3, --3bulsik  세벌식 390");
    println!("    -390, --3bul390  세벌식 390");
    println!("    -391, --3bul391  세벌식 391");
    println!("  -o, --output <FILE>  출력 파일 지정");
}

/// 프로그램의 주요 실행 함수입니다.
///
/// 설정에 따라 적절한 변환 처리를 수행합니다.
///
/// # 인자
///
/// * `config` - 프로그램 설정
///
/// # 반환 값
///
/// * `io::Result<()>` - 성공 시 Ok(()), 실패 시 IO 오류
fn run(config: Config) -> io::Result<()> {
    // 입력 스트림 설정
    let input: Box<dyn BufRead> = match config.input_file {
        Some(file) => {
            let file = File::open(Path::new(&file))?;
            Box::new(BufReader::new(file))
        }
        None => Box::new(BufReader::new(io::stdin())),
    };

    // 출력 스트림 설정
    let mut output: Box<dyn Write> = match config.output_file {
        Some(file) => Box::new(File::create(Path::new(&file))?),
        None => Box::new(io::stdout()),
    };

    // 키맵 파일 경로 설정
    let english_keymap = match config.english_keyboard_mode {
        EnglishKeyboardMode::Qwerty => "src/keystroke/keymap/en_qwerty.json",
        EnglishKeyboardMode::Dvorak => "src/keystroke/keymap/en_dvorak.json",
    };

    let korean_keymap = match config.keyboard_mode {
        KeyboardMode::TwoBulStd => "src/keystroke/keymap/ko_2bulstd.json",
        KeyboardMode::ThreeBul390 => "src/keystroke/keymap/ko_3bul390.json",
        KeyboardMode::ThreeBul391 => "src/keystroke/keymap/ko_3bul391.json",
    };

    // 변환 모드에 따른 처리
    let is_three_bul = matches!(
        config.keyboard_mode,
        KeyboardMode::ThreeBul390 | KeyboardMode::ThreeBul391
    );

    match config.conversion_mode {
        ConversionMode::EnglishToKorean => {
            // 영문 -> 한글 변환
            if is_three_bul {
                process_with_3bul(input, &mut output, english_keymap, korean_keymap)?;
            } else {
                process_with_2bul(input, &mut output, english_keymap, korean_keymap)?;
            }
        }
        ConversionMode::KoreanToEnglish => {
            // 한글 -> 영문 변환
            process_korean_to_english(
                input,
                &mut output,
                english_keymap,
                korean_keymap,
                is_three_bul,
            )?;
        }
    }

    Ok(())
}

/// 두벌식 한글 변환 처리를 수행합니다.
///
/// 영문 키 입력을 두벌식 한글로 변환합니다.
///
/// # 인자
///
/// * `input` - 입력 스트림
/// * `output` - 출력 스트림
/// * `en_keymap` - 영문 키맵 파일 경로
/// * `ko_keymap` - 한글 키맵 파일 경로
///
/// # 반환 값
///
/// * `io::Result<()>` - 성공 시 Ok(()), 실패 시 IO 오류
fn process_with_2bul(
    input: Box<dyn BufRead>,
    output: &mut Box<dyn Write>,
    en_keymap: &str,
    ko_keymap: &str,
) -> io::Result<()> {
    let mut hangul_composer = HangulComposer2Bul::new();
    let keyboard_map = KeyboardMap::create_keyboard_map(en_keymap, ko_keymap, false);

    for line in input.lines() {
        let input_line = line?;

        // 비어있는 줄은 그대로 출력
        if input_line.is_empty() {
            writeln!(output)?;
            continue;
        }

        let result = keystrokes_to_hangul(&input_line, &keyboard_map, &mut hangul_composer);
        writeln!(output, "{}", result)?;
    }

    Ok(())
}

/// 세벌식 한글 변환 처리를 수행합니다.
///
/// 영문 키 입력을 세벌식 한글로 변환합니다.
///
/// # 인자
///
/// * `input` - 입력 스트림
/// * `output` - 출력 스트림
/// * `en_keymap` - 영문 키맵 파일 경로
/// * `ko_keymap` - 한글 키맵 파일 경로
///
/// # 반환 값
///
/// * `io::Result<()>` - 성공 시 Ok(()), 실패 시 IO 오류
fn process_with_3bul(
    input: Box<dyn BufRead>,
    output: &mut Box<dyn Write>,
    en_keymap: &str,
    ko_keymap: &str,
) -> io::Result<()> {
    let mut hangul_composer = HangulComposer3Bul::new();
    let keyboard_map = KeyboardMap::create_keyboard_map(en_keymap, ko_keymap, true);

    for line in input.lines() {
        let input_line = line?;

        // 비어있는 줄은 그대로 출력
        if input_line.is_empty() {
            writeln!(output)?;
            continue;
        }

        let result = keystrokes_to_hangul(&input_line, &keyboard_map, &mut hangul_composer);
        writeln!(output, "{}", result)?;
    }

    Ok(())
}

/// 한글을 영문으로 변환 처리를 수행합니다.
///
/// 한글 입력을 영문 키 입력으로 변환합니다.
///
/// # 인자
///
/// * `input` - 입력 스트림
/// * `output` - 출력 스트림
/// * `en_keymap` - 영문 키맵 파일 경로
/// * `ko_keymap` - 한글 키맵 파일 경로
/// * `is_three_bul` - 세벌식 여부
///
/// # 반환 값
///
/// * `io::Result<()>` - 성공 시 Ok(()), 실패 시 IO 오류
fn process_korean_to_english(
    input: Box<dyn BufRead>,
    output: &mut Box<dyn Write>,
    en_keymap: &str,
    ko_keymap: &str,
    is_three_bul: bool,
) -> io::Result<()> {
    let keyboard_map = KeyboardMap::create_keyboard_map(en_keymap, ko_keymap, is_three_bul);

    for line in input.lines() {
        let input_line = line?;

        // 비어있는 줄은 그대로 출력
        if input_line.is_empty() {
            writeln!(output)?;
            continue;
        }

        let result = hangul_to_keystrokes(&input_line, &keyboard_map, is_three_bul);
        writeln!(output, "{}", result)?;
    }

    Ok(())
}

/// 프로그램의 진입점입니다.
///
/// 명령행 인자를 처리하고 프로그램을 실행합니다.
///
/// # 반환 값
///
/// * `io::Result<()>` - 성공 시 Ok(()), 실패 시 IO 오류
fn main() -> io::Result<()> {
    let config = match Config::new(env::args()) {
        Ok(config) => config,
        Err(err) => {
            eprintln!("설정 오류: {}", err);
            eprintln!("사용법을 보려면 unin --help를 실행하세요");
            process::exit(1);
        }
    };

    if let Err(e) = run(config) {
        eprintln!("실행 오류: {}", e);
        process::exit(1);
    }

    Ok(())
}
