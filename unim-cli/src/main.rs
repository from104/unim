use clap::{Parser, ValueEnum};
use rust_i18n::t;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process;
use unim::korean::composer_with_2bul::HangulComposer2Bul;
use unim::korean::composer_with_3bul::HangulComposer3Bul;
use unim::keystroke::korean_to_keystrokes::korean_to_keystrokes;
use unim::keystroke::keyboard_map::KeyboardMap;
use unim::keystroke::keystrokes_to_korean::keystrokes_to_korean;

// i18n 초기화
rust_i18n::i18n!("locales");

/// UNIM-cli (Universal Next-generation Input Method for command-line)
#[derive(Parser, Debug)]
#[command(author, version, about = "UNIM-cli - Korean/English Keyboard Converter")]
struct Cli {
    /// Input files (uses standard input if not specified)
    #[arg(name = "FILE")]
    input_files: Vec<String>,

    /// Output file (uses standard output if not specified)
    #[arg(short, long, value_name = "FILE")]
    output: Option<String>,

    /// Compose English keyboard stream into Korean (default mode)
    #[arg(short, long, group = "conversion", default_value_t = true)]
    compose: bool,

    /// Decompose Korean into English keyboard stream
    #[arg(short, long, group = "conversion")]
    decompose: bool,

    /// Korean keyboard layout (default: 2bul)
    #[arg(short = 'k', long = "korean-keyboard", value_enum, default_value_t = KeyboardMode::TwoBulStd)]
    korean_keyboard: KeyboardMode,

    /// English keyboard layout (default: qwerty)
    #[arg(short = 'e', long = "english-keyboard", value_enum, default_value_t = EnglishKeyboardMode::Qwerty)]
    english_keyboard: EnglishKeyboardMode,
}

/// 한국어 자판 모드
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

/// 영어 자판 모드
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
#[derive(Clone, Copy, Debug)]
enum ConversionMode {
    KoreanToEnglish,
    EnglishToKorean,
}

struct ConvertConfig {
    input_files: Vec<String>,
    output_file: Option<String>,
    keyboard_mode: KeyboardMode,
    english_keyboard_mode: EnglishKeyboardMode,
    conversion_mode: ConversionMode,
    warnings: Vec<String>,
}

impl ConvertConfig {
    fn from_cli(cli: &Cli) -> Self {
        let mut input_files = Vec::new();
        let mut warnings = Vec::new();
        let mut has_stdin_input = false;

        if cli.input_files.is_empty() {
            input_files.push("-".to_string());
        } else {
            for file in &cli.input_files {
                if file == "-" {
                    if has_stdin_input {
                        warnings.push(t!("warning_multiple_stdin").to_string());
                    } else {
                        has_stdin_input = true;
                        input_files.push(file.clone());
                    }
                } else {
                    input_files.push(file.clone());
                }
            }
        }

        let conversion_mode = if cli.decompose {
            ConversionMode::KoreanToEnglish
        } else {
            ConversionMode::EnglishToKorean
        };

        ConvertConfig {
            input_files,
            output_file: cli.output.clone(),
            keyboard_mode: cli.korean_keyboard,
            english_keyboard_mode: cli.english_keyboard,
            conversion_mode,
            warnings,
        }
    }
}

fn run_convert(config: ConvertConfig) -> io::Result<()> {
    let mut inputs: Vec<Box<dyn BufRead>> = Vec::new();
    if config.input_files.is_empty() {
        inputs.push(Box::new(BufReader::new(io::stdin())));
    } else {
        for file in &config.input_files {
            if file == "-" {
                inputs.push(Box::new(BufReader::new(io::stdin())));
            } else {
                let input = File::open(Path::new(file))?;
                inputs.push(Box::new(BufReader::new(input)));
            }
        }
    }

    let mut output: Box<dyn Write> = match &config.output_file {
        Some(file) if file == "-" => Box::new(io::stdout()),
        Some(file) => Box::new(File::create(Path::new(file))?),
        None => Box::new(io::stdout()),
    };

    let en_keymap_name = match config.english_keyboard_mode {
        EnglishKeyboardMode::Qwerty => "en_qwerty",
        EnglishKeyboardMode::Dvorak => "en_dvorak",
    };

    let korean_keymap_name = match config.keyboard_mode {
        KeyboardMode::TwoBulStd => "ko_2bulstd",
        KeyboardMode::ThreeBul390 => "ko_3bul390",
        KeyboardMode::ThreeBul391 => "ko_3bul391",
    };
    
    let en_json = unim::keystroke::get_keymap_json(en_keymap_name);
    let ko_json = unim::keystroke::get_keymap_json(korean_keymap_name);

    let is_three_bul = matches!(
        config.keyboard_mode,
        KeyboardMode::ThreeBul390 | KeyboardMode::ThreeBul391
    );

    match config.conversion_mode {
        ConversionMode::EnglishToKorean => {
            if is_three_bul {
                process_with_3bul(inputs, &mut output, en_json, ko_json)?;
            } else {
                process_with_2bul(inputs, &mut output, en_json, ko_json)?;
            }
        }
        ConversionMode::KoreanToEnglish => {
            process_korean_to_english(inputs, &mut output, en_json, ko_json, is_three_bul)?;
        }
    }

    Ok(())
}

fn process_with_2bul(
    inputs: Vec<Box<dyn BufRead>>,
    output: &mut Box<dyn Write>,
    en_json: &str,
    ko_json: &str,
) -> io::Result<()> {
    let mut korean_composer = HangulComposer2Bul::new();
    let keyboard_map = KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, false);

    for input in inputs {
        for line in input.lines() {
            let input_line = line?;
            if input_line.is_empty() {
                writeln!(output)?;
                continue;
            }
            let result = keystrokes_to_korean(&input_line, &keyboard_map, &mut korean_composer);
            writeln!(output, "{}", result)?;
        }
    }
    Ok(())
}

fn process_with_3bul(
    inputs: Vec<Box<dyn BufRead>>,
    output: &mut Box<dyn Write>,
    en_json: &str,
    ko_json: &str,
) -> io::Result<()> {
    let mut korean_composer = HangulComposer3Bul::new();
    let keyboard_map = KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, true);

    for input in inputs {
        for line in input.lines() {
            let input_line = line?;
            if input_line.is_empty() {
                writeln!(output)?;
                continue;
            }
            let result = keystrokes_to_korean(&input_line, &keyboard_map, &mut korean_composer);
            writeln!(output, "{}", result)?;
        }
    }
    Ok(())
}

fn process_korean_to_english(
    inputs: Vec<Box<dyn BufRead>>,
    output: &mut Box<dyn Write>,
    en_json: &str,
    ko_json: &str,
    is_three_bul: bool,
) -> io::Result<()> {
    let keyboard_map = KeyboardMap::create_keyboard_map_from_str(en_json, ko_json, is_three_bul);

    for input in inputs {
        for line in input.lines() {
            let input_line = line?;
            if input_line.is_empty() {
                writeln!(output)?;
                continue;
            }
            let result = korean_to_keystrokes(&input_line, &keyboard_map, is_three_bul);
            writeln!(output, "{}", result)?;
        }
    }
    Ok(())
}

fn main() -> io::Result<()> {
    // 로케일 설정
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_else(|_| "en".to_string());
    let locale = locale.split('.').next().unwrap_or("en");
    let locale = locale.split('_').next().unwrap_or("en");
    rust_i18n::set_locale(locale);

    let cli = Cli::parse();
    let config = ConvertConfig::from_cli(&cli);
    
    for warning in &config.warnings {
        eprintln!("{}", t!("warning_label", warning = warning));
    }
    
    if let Err(e) = run_convert(config) {
        eprintln!("{}", t!("error_label", error = e));
        process::exit(1);
    }

    Ok(())
}
