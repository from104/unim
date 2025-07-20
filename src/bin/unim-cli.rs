use clap::{Parser, ValueEnum};
/// UNIM-cli (Universal Next-generation Input Method for command-line) - 한/영 자판 변환기
///
/// 이 프로그램은 한글과 영문 자판 간의 변환을 수행합니다.
/// - 영문 타이핑을 한글로 변환
/// - 한글 타이핑을 영문으로 변환
// use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process;
use unim::hangul::composer_with_2bul::HangulComposer2Bul;
use unim::hangul::composer_with_3bul::HangulComposer3Bul;
use unim::keystroke::hangul_to_keystrokes::hangul_to_keystrokes;
use unim::keystroke::keyboard_map::KeyboardMap;
use unim::keystroke::keystrokes_to_hangul::keystrokes_to_hangul;

/// 한글 자판 모드
///
/// 사용할 한글 자판의 종류를 정의합니다.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum KeyboardMode {
    /// 두벌식 표준 자판
    #[value(name = "2bul")]
    TwoBulStd,
    /// 세벌식 390 자판
    #[value(name = "390")]
    ThreeBul390,
    /// 세벌식 391 자판
    #[value(name = "391")]
    ThreeBul391,
}

/// 영문 자판 모드
///
/// 사용할 영문 자판의 종류를 정의합니다.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum EnglishKeyboardMode {
    /// QWERTY 자판
    #[value(name = "qwerty")]
    Qwerty,
    /// Dvorak 자판
    #[value(name = "dvorak")]
    Dvorak,
}

/// 변환 모드
///
/// 한/영 변환 방향을 정의합니다.
#[derive(Clone, Copy, Debug, ValueEnum)]
enum ConversionMode {
    /// 한글을 영문으로 변환
    #[value(name = "ko2en")]
    KoreanToEnglish,
    /// 영문을 한글로 변환
    #[value(name = "en2ko")]
    EnglishToKorean,
}

/// 유니코드 입력 방식 변환 프로그램 (UNIM)
///
/// 한글과 영문 자판 간의 변환을 수행합니다.
#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Cli {
    /// 입력 파일들 (지정하지 않으면 표준 입력을 사용)
    #[arg(name = "FILE")]
    input_files: Vec<String>,

    /// 출력 파일 (지정하지 않으면 표준 출력을 사용)
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// 영문자판 스트림을 한글로 조합 (기본 모드)
    #[arg(short, long, group = "conversion", default_value_t = true)]
    compose: bool,

    /// 한글을 영문자판 스트림으로 분해
    #[arg(short, long, group = "conversion")]
    decompose: bool,

    /// 한글 자판 레이아웃 (기본 2bul)
    #[arg(short = 'k', long = "korean-keyboard", value_enum, default_value_t = KeyboardMode::TwoBulStd)]
    korean_keyboard: KeyboardMode,

    /// 영문 자판 레이아웃 (기본 qwerty)
    #[arg(short = 'e', long = "english-keyboard", value_enum, default_value_t = EnglishKeyboardMode::Qwerty)]
    english_keyboard: EnglishKeyboardMode,
}

/// 프로그램 설정
///
/// 명령행 인자로부터 파싱된 프로그램 설정을 포함합니다.
struct Config {
    /// 입력 파일 경로 목록 (비어 있는 경우 표준 입력 사용)
    input_files: Vec<String>,
    /// 출력 파일 경로 (None인 경우 표준 출력 사용)
    output_file: Option<String>,
    /// 한글 자판 모드
    keyboard_mode: KeyboardMode,
    /// 영문 자판 모드
    english_keyboard_mode: EnglishKeyboardMode,
    /// 변환 모드
    conversion_mode: ConversionMode,
    /// 경고 메시지 목록
    warnings: Vec<String>,
}

impl Config {
    /// Clap 파서로부터 설정을 생성합니다.
    ///
    /// # 인자
    ///
    /// * `cli` - Clap 파서로 파싱된 명령행 인자
    ///
    /// # 반환 값
    ///
    /// * `Config` - 설정 인스턴스
    fn from_cli(cli: Cli) -> Self {
        let mut input_files = Vec::new();
        let mut warnings = Vec::new();
        let mut has_stdin_input = false;

        // 입력 파일 처리
        if cli.input_files.is_empty() {
            // 입력 파일이 없으면 표준 입력을 사용
            input_files.push("-".to_string());
        } else {
            // 입력 파일 처리, 중복된 표준 입력은 경고로 처리
            for file in cli.input_files {
                if file == "-" {
                    if has_stdin_input {
                        warnings.push(
                            "표준 입력('-')이 여러 번 지정되었습니다. 중복된 항목은 무시됩니다."
                                .to_string(),
                        );
                    } else {
                        has_stdin_input = true;
                        input_files.push(file);
                    }
                } else {
                    input_files.push(file);
                }
            }
        }

        // 변환 모드 결정
        let conversion_mode = if cli.decompose {
            ConversionMode::KoreanToEnglish
        } else {
            ConversionMode::EnglishToKorean
        };

        Config {
            input_files,
            output_file: cli.output,
            keyboard_mode: cli.korean_keyboard,
            english_keyboard_mode: cli.english_keyboard,
            conversion_mode,
            warnings,
        }
    }
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
    let mut inputs: Vec<Box<dyn BufRead>> = Vec::new();
    if config.input_files.is_empty() {
        // 입력 파일이 없으면 표준 입력 사용
        inputs.push(Box::new(BufReader::new(io::stdin())));
    } else {
        // 입력 파일들을 열어서 추가
        for file in &config.input_files {
            if file == "-" {
                // 파일 이름이 '-'인 경우 표준 입력을 사용
                inputs.push(Box::new(BufReader::new(io::stdin())));
            } else {
                let input = File::open(Path::new(file))?;
                inputs.push(Box::new(BufReader::new(input)));
            }
        }
    }

    // 출력 스트림 설정
    let mut output: Box<dyn Write> = match &config.output_file {
        Some(file) if file == "-" => Box::new(io::stdout()),
        Some(file) => Box::new(File::create(Path::new(file))?),
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
                process_with_3bul(inputs, &mut output, english_keymap, korean_keymap)?;
            } else {
                process_with_2bul(inputs, &mut output, english_keymap, korean_keymap)?;
            }
        }
        ConversionMode::KoreanToEnglish => {
            // 한글 -> 영문 변환
            process_korean_to_english(
                inputs,
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
/// * `inputs` - 입력 스트림 목록
/// * `output` - 출력 스트림
/// * `en_keymap` - 영문 키맵 파일 경로
/// * `ko_keymap` - 한글 키맵 파일 경로
///
/// # 반환 값
///
/// * `io::Result<()>` - 성공 시 Ok(()), 실패 시 IO 오류
fn process_with_2bul(
    inputs: Vec<Box<dyn BufRead>>,
    output: &mut Box<dyn Write>,
    en_keymap: &str,
    ko_keymap: &str,
) -> io::Result<()> {
    let mut hangul_composer = HangulComposer2Bul::new();
    let keyboard_map = KeyboardMap::create_keyboard_map(en_keymap, ko_keymap, false);

    for input in inputs {
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
    }

    Ok(())
}

/// 세벌식 한글 변환 처리를 수행합니다.
///
/// 영문 키 입력을 세벌식 한글로 변환합니다.
///
/// # 인자
///
/// * `inputs` - 입력 스트림 목록
/// * `output` - 출력 스트림
/// * `en_keymap` - 영문 키맵 파일 경로
/// * `ko_keymap` - 한글 키맵 파일 경로
///
/// # 반환 값
///
/// * `io::Result<()>` - 성공 시 Ok(()), 실패 시 IO 오류
fn process_with_3bul(
    inputs: Vec<Box<dyn BufRead>>,
    output: &mut Box<dyn Write>,
    en_keymap: &str,
    ko_keymap: &str,
) -> io::Result<()> {
    let mut hangul_composer = HangulComposer3Bul::new();
    let keyboard_map = KeyboardMap::create_keyboard_map(en_keymap, ko_keymap, true);

    for input in inputs {
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
    }

    Ok(())
}

/// 한글을 영문으로 변환 처리를 수행합니다.
///
/// 한글 입력을 영문 키 입력으로 변환합니다.
///
/// # 인자
///
/// * `inputs` - 입력 스트림 목록
/// * `output` - 출력 스트림
/// * `en_keymap` - 영문 키맵 파일 경로
/// * `ko_keymap` - 한글 키맵 파일 경로
/// * `is_three_bul` - 세벌식 여부
///
/// # 반환 값
///
/// * `io::Result<()>` - 성공 시 Ok(()), 실패 시 IO 오류
fn process_korean_to_english(
    inputs: Vec<Box<dyn BufRead>>,
    output: &mut Box<dyn Write>,
    en_keymap: &str,
    ko_keymap: &str,
    is_three_bul: bool,
) -> io::Result<()> {
    let keyboard_map = KeyboardMap::create_keyboard_map(en_keymap, ko_keymap, is_three_bul);

    for input in inputs {
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
    let cli = Cli::parse();
    let config = Config::from_cli(cli);

    // 경고 메시지가 있으면 출력
    for warning in &config.warnings {
        eprintln!("경고: {}", warning);
    }

    if let Err(e) = run(config) {
        eprintln!("실행 오류: {}", e);
        process::exit(1);
    }

    Ok(())
}
