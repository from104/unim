use hangul::builder_2bul::HangulBuilder2Bul;
use hangul::builder_3bul::HangulBuilder3Bul;
use std::env;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process;

pub mod hangul;

enum KeyboardMode {
    TwoBul,
    ThreeBul,
}

enum ConversionMode {
    KoreanToEnglish,
    EnglishToKorean,
}

struct Config {
    input_file: Option<String>,
    output_file: Option<String>,
    keyboard_mode: KeyboardMode,
    conversion_mode: ConversionMode,
}

impl Config {
    fn new(args: env::Args) -> Result<Config, &'static str> {
        let args: Vec<String> = args.collect();

        let mut input_file = None;
        let mut output_file = None;
        let mut keyboard_mode = KeyboardMode::TwoBul;
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
                    keyboard_mode = KeyboardMode::TwoBul;
                }
                "-3" | "--3bulsik" => {
                    keyboard_mode = KeyboardMode::ThreeBul;
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
            conversion_mode,
        })
    }
}

fn print_help() {
    println!("UNIN (Unicode Input) - 한/영 자판 변환기");
    println!();
    println!("사용법: unin [옵션] [입력파일]");
    println!();
    println!("옵션:");
    println!("  -h, --help       이 도움말을 표시합니다");
    println!("  -k, --korean     영문 타자를 한글로 변환 (기본값)");
    println!("  -e, --english    한글 타자를 영문으로 변환");
    println!("  -2, --2bulsik    두벌식 모드 (기본값)");
    println!("  -3, --3bulsik    세벌식 모드 (390)");
    println!("  -o, --output <FILE>  출력 파일 지정");
}

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

    match config.conversion_mode {
        ConversionMode::EnglishToKorean => {
            // 영문 -> 한글 변환
            match config.keyboard_mode {
                KeyboardMode::TwoBul => {
                    process_with_2bul(input, &mut output)?;
                }
                KeyboardMode::ThreeBul => {
                    process_with_3bul(input, &mut output)?;
                }
            }
        }
        ConversionMode::KoreanToEnglish => {
            // 한글 -> 영문 변환은 향후 구현
            writeln!(output, "한글 -> 영문 변환은 아직 지원되지 않습니다.")?;
        }
    }

    Ok(())
}

fn process_with_2bul(input: Box<dyn BufRead>, output: &mut Box<dyn Write>) -> io::Result<()> {
    let mut hangul_builder = HangulBuilder2Bul::new();

    for line in input.lines() {
        let input_line = line?;

        // 비어있는 줄은 그대로 출력
        if input_line.is_empty() {
            writeln!(output, "")?;
            continue;
        }

        let result = hangul_builder.convert_string(&input_line);
        writeln!(output, "{}", result)?;
    }

    Ok(())
}

fn process_with_3bul(input: Box<dyn BufRead>, output: &mut Box<dyn Write>) -> io::Result<()> {
    let mut hangul_builder = HangulBuilder3Bul::new();

    for line in input.lines() {
        let input_line = line?;

        // 비어있는 줄은 그대로 출력
        if input_line.is_empty() {
            writeln!(output, "")?;
            continue;
        }

        let result = hangul_builder.convert_string(&input_line);
        writeln!(output, "{}", result)?;
    }

    Ok(())
}

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
