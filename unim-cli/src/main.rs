use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use rust_i18n::t;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::Path;
use std::process;
use unim::config::{
    Config as UnimConfig, EnglishLayout, InputCategory, KoreanLayout, ModeSharingMode,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};
use unim::hangul::composer_with_2bul::HangulComposer2Bul;
use unim::hangul::composer_with_3bul::HangulComposer3Bul;
use unim::keystroke::keyboard_map::KeyboardMap;
use unim::keystroke::keystrokes_to_korean::keystrokes_to_korean;
use unim::keystroke::korean_to_keystrokes::korean_to_keystrokes;
use unim::unim_log;

rust_i18n::i18n!("locales");

/// UNIM-cli: Korean/English keyboard converter + settings manager
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = "UNIM-cli - Korean/English Keyboard Converter & Settings Manager",
    args_conflicts_with_subcommands = true
)]
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

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// 설정 관리 (Manage settings)
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },
}

#[derive(Subcommand, Debug)]
enum ConfigCommands {
    /// 현재 설정 표시
    Show,
    /// 설정 값 변경
    Set {
        /// 설정 항목 이름
        #[arg(value_enum)]
        key: ConfigKey,
        /// 설정 값
        value: String,
    },
    /// 설정 파일 경로 표시
    Path,
    /// 설정을 기본값으로 초기화
    Reset,
    /// 인터렉티브 설정 모드 시작
    Interactive,
}

/// 한국어 자판 모드 (변환용)
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
    /// 세벌식 순아래 자판
    #[value(name = "noshift")]
    ThreeBulNoShift,
}

/// 영어 자판 모드 (변환용)
#[derive(Clone, Copy, Debug, ValueEnum)]
enum EnglishKeyboardMode {
    /// QWERTY 자판
    #[value(name = "qwerty")]
    Qwerty,
    /// Dvorak 자판
    #[value(name = "dvorak")]
    Dvorak,
    /// Colemak 자판
    #[value(name = "colemak")]
    Colemak,
    /// Colemak-DH 자판
    #[value(name = "colemak_dh")]
    ColemakDh,
    /// Workman 자판
    #[value(name = "workman")]
    Workman,
}

/// 변환 모드
#[derive(Clone, Copy, Debug)]
enum ConversionMode {
    KoreanToEnglish,
    EnglishToKorean,
}

#[derive(Clone, Debug, ValueEnum)]
enum ConfigKey {
    /// 한국어 레이아웃 (2bul, 3bul390, 3bul391, 3bul_noshift)
    #[value(name = "korean-layout")]
    KoreanLayout,
    /// 영어 레이아웃 (qwerty, dvorak, colemak, colemak_dh, workman)
    #[value(name = "english-layout")]
    EnglishLayout,
    /// 초기 입력 모드 (korean, english)
    #[value(name = "default-category")]
    DefaultCategory,
    /// 모드 공유 방식 (global, per-app)
    #[value(name = "mode-sharing")]
    ModeSharing,
    /// 한/영 전환 키 (예: Korean,RightAlt)
    #[value(name = "toggle-keys")]
    ToggleKeys,
    /// 한자/특수문자 키 (예: Hanja,F9)
    #[value(name = "hanja-keys")]
    HanjaKeys,
    /// 팝업 표시 방식 (standalone, embedded)
    #[value(name = "popup-mode")]
    PopupMode,
    /// 자동 오타 교정 활성화 (true, false)
    #[value(name = "auto-typefix")]
    AutoTypeFix,
    /// 자동 오타 교정: 한글 음절 임계값 (2~6)
    #[value(name = "auto-typefix-kor-threshold")]
    AutoTypeFixKorThreshold,
    /// 자동 오타 교정: 영문 단어 최소 길이 (3~8)
    #[value(name = "auto-typefix-eng-min-length")]
    AutoTypeFixEngMinLength,
    /// 자동 오타 교정: 순방향 시간 윈도우 (500~5000 ms)
    #[value(name = "auto-typefix-forward-time-window-ms")]
    AutoTypeFixForwardTimeWindow,
    /// 자동 오타 교정: 역방향 시간 윈도우 (500~5000 ms)
    #[value(name = "auto-typefix-reverse-time-window-ms")]
    AutoTypeFixReverseTimeWindow,
    /// 자동 오타 교정: 순방향 (영→한) 교정 (true, false)
    #[value(name = "auto-typefix-forward")]
    AutoTypeFixForward,
    /// 자동 오타 교정: 역방향 (한→영) 교정 (true, false)
    #[value(name = "auto-typefix-reverse")]
    AutoTypeFixReverse,
    /// 자동 오타 교정: 영단어 매칭 시 억제 (true, false)
    #[value(name = "auto-typefix-skip-english-word")]
    AutoTypeFixSkipEnglishWord,
    /// 자동 오타 교정: 온전한 음절 매칭 시 억제 (true, false)
    #[value(name = "auto-typefix-skip-complete-syllable")]
    AutoTypeFixSkipCompleteSyllable,
    /// 자동 오타 교정: 재트리거 기반 학습형 억제 (true, false)
    #[value(name = "auto-typefix-rollback-detection")]
    AutoTypeFixRollbackDetection,
    /// 자동 오타 교정: 임시 억제 단어 만료 기간 (1~12 시간)
    #[value(name = "auto-typefix-tentative-expiry-hours")]
    AutoTypeFixTentativeExpiryHours,
    /// 자동 오타 교정: 재트리거 관찰 창 (5~15 초)
    #[value(name = "auto-typefix-observation-timeout-secs")]
    AutoTypeFixObservationTimeoutSecs,
    /// 앱별 모드 규칙 (JSON 형식)
    #[value(name = "app-rules")]
    AppRules,
}

// ─────────────────────────────────────────────────────────────────────────────
// Conversion path
// ─────────────────────────────────────────────────────────────────────────────

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
        EnglishKeyboardMode::Colemak => "en_colemak",
        EnglishKeyboardMode::ColemakDh => "en_colemak_dh",
        EnglishKeyboardMode::Workman => "en_workman",
    };

    let korean_keymap_name = match config.keyboard_mode {
        KeyboardMode::TwoBulStd => "ko_2bulstd",
        KeyboardMode::ThreeBul390 => "ko_3bul390",
        KeyboardMode::ThreeBul391 => "ko_3bul391",
        KeyboardMode::ThreeBulNoShift => "ko_3bul_noshift",
    };

    unim_log!(
        "CLI",
        "변환 시작: 영어자판={}, 한글자판={}",
        en_keymap_name,
        korean_keymap_name
    );

    let en_json = unim::keystroke::get_keymap_json(en_keymap_name);
    let ko_json = unim::keystroke::get_keymap_json(korean_keymap_name);

    let is_three_bul = matches!(
        config.keyboard_mode,
        KeyboardMode::ThreeBul390 | KeyboardMode::ThreeBul391 | KeyboardMode::ThreeBulNoShift
    );

    match config.conversion_mode {
        ConversionMode::EnglishToKorean => {
            unim_log!("CLI", "변환 모드: 영어 -> 한글");
            if is_three_bul {
                process_with_3bul(inputs, &mut output, en_json, ko_json)?;
            } else {
                process_with_2bul(inputs, &mut output, en_json, ko_json)?;
            }
        }
        ConversionMode::KoreanToEnglish => {
            unim_log!("CLI", "변환 모드: 한글 -> 영어");
            process_korean_to_english(inputs, &mut output, en_json, ko_json, is_three_bul)?;
        }
    }

    unim_log!("CLI", "변환 완료");
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

// ─────────────────────────────────────────────────────────────────────────────
// Config path
// ─────────────────────────────────────────────────────────────────────────────

fn config_show() {
    let config = UnimConfig::load_from_default_path();

    let korean_name = config.engine.korean.layout.display_name();
    let english_name = config.engine.english.layout.display_name();

    let default_category_name = match config.engine.default_category {
        InputCategory::Korean => t!("korean_mode"),
        InputCategory::English => t!("english_mode"),
    };

    let mode_sharing_name = config.engine.mode_sharing.display_name();

    println!("{}", t!("settings_title"));
    println!("================");
    println!(
        "{}: {} ({})",
        t!("korean_layout_label"),
        korean_name,
        config.engine.korean.layout.name()
    );
    println!(
        "{}: {} ({})",
        t!("english_layout_label"),
        english_name,
        config.engine.english.layout.name()
    );
    println!(
        "{}: {}",
        t!("default_category_label"),
        default_category_name
    );
    println!("{}: {}", t!("mode_sharing_label"), mode_sharing_name);
    println!(
        "{}: {}",
        t!("toggle_keys_label"),
        config.engine.toggle_keys.join(", ")
    );
    println!(
        "{}: {}",
        t!("hanja_keys_label"),
        config.engine.hanja_keys.join(", ")
    );
    println!(
        "{}: {}",
        t!("popup_mode_label"),
        config.engine.popup_mode.name()
    );
    let auto_typefix_status = if config.engine.auto_typefix.enabled {
        t!("enabled")
    } else {
        t!("disabled")
    };
    println!("{}: {}", t!("auto_typefix_label"), auto_typefix_status);
    if config.engine.auto_typefix.enabled {
        let atf = &config.engine.auto_typefix;
        println!(
            "  - 순방향(영→한): {}, 역방향(한→영): {}",
            if atf.forward { "ON" } else { "OFF" },
            if atf.reverse { "ON" } else { "OFF" }
        );
        println!(
            "  - 한글 음절 임계값: {}, 영문 최소 길이: {}",
            atf.kor_syllable_threshold, atf.eng_word_min_length
        );
        println!(
            "  - 시간 윈도우: 순방향 {}ms / 역방향 {}ms",
            atf.forward_time_window_ms, atf.reverse_time_window_ms
        );
        println!(
            "  - 재트리거 감지: {} / 관찰 창: {}초 / 임시 억제 만료: {}시간",
            if atf.rollback_detection { "ON" } else { "OFF" },
            atf.observation_timeout_secs,
            atf.tentative_expiry_hours
        );
    }
    println!(
        "{}: {}",
        t!("app_rules_label"),
        if config.engine.app_rules.is_empty() {
            t!("not_set").to_string()
        } else {
            format!("{} rules", config.engine.app_rules.len())
        }
    );
    println!();
    if let Some(path) = UnimConfig::default_config_path() {
        println!("{}: {}", t!("config_file_label"), path.display());
    }
}

fn config_set(key: ConfigKey, value: &str) -> Result<(), String> {
    let mut config = UnimConfig::load_from_default_path();

    match key {
        ConfigKey::KoreanLayout => {
            let layout = match value {
                "2bul" | "dubeolsik" => KoreanLayout::Dubeolsik,
                "3bul390" | "390" => KoreanLayout::Sebeolsik390,
                "3bul391" | "391" => KoreanLayout::Sebeolsik391,
                "3bul_noshift" | "noshift" => KoreanLayout::SebeolsikNoShift,
                _ => {
                    let kind = t!("korean_layout_label").to_string();
                    return Err(t!(
                        "error_invalid_layout",
                        kind = kind,
                        value = value,
                        allowed = "2bul, 3bul390, 3bul391, 3bul_noshift"
                    )
                    .to_string());
                }
            };
            config.engine.korean.layout = layout;
            let kind = t!("korean_layout_label").to_string();
            println!(
                "{}",
                t!("layout_changed", kind = kind, layout = layout.name())
            );
        }
        ConfigKey::EnglishLayout => {
            let layout = match value {
                "qwerty" => EnglishLayout::Qwerty,
                "dvorak" => EnglishLayout::Dvorak,
                "colemak" => EnglishLayout::Colemak,
                "colemak_dh" | "colemak-dh" => EnglishLayout::ColemakDh,
                "workman" => EnglishLayout::Workman,
                _ => {
                    let kind = t!("english_layout_label").to_string();
                    return Err(t!(
                        "error_invalid_layout",
                        kind = kind,
                        value = value,
                        allowed = "qwerty, dvorak, colemak, colemak_dh, workman"
                    )
                    .to_string());
                }
            };
            config.engine.english.layout = layout;
            let kind = t!("english_layout_label").to_string();
            println!(
                "{}",
                t!("layout_changed", kind = kind, layout = layout.name())
            );
        }
        ConfigKey::DefaultCategory => {
            let category = match value.to_lowercase().as_str() {
                "korean" | "ko" | "한글" | "한국어" => InputCategory::Korean,
                "english" | "en" | "영어" => InputCategory::English,
                _ => {
                    return Err(t!(
                        "error_invalid_category",
                        value = value,
                        allowed = "korean, english"
                    )
                    .to_string());
                }
            };
            config.engine.default_category = category;
            let category_name = match category {
                InputCategory::Korean => t!("korean_mode"),
                InputCategory::English => t!("english_mode"),
            };
            println!(
                "{}",
                t!("default_category_changed", category = category_name)
            );
        }
        ConfigKey::ModeSharing => {
            let mode = match value.to_lowercase().as_str() {
                "global" | "전역" => ModeSharingMode::Global,
                "per-app" | "perapp" | "앱별" => ModeSharingMode::PerApp,
                _ => {
                    return Err(t!(
                        "error_invalid_mode_sharing",
                        value = value,
                        allowed = "global, per-app"
                    )
                    .to_string());
                }
            };
            config.engine.mode_sharing = mode;
            println!("{}", t!("mode_sharing_changed", mode = mode.display_name()));
        }
        ConfigKey::ToggleKeys => {
            let keys: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if keys.is_empty() {
                return Err("At least one key required".to_string());
            }
            config.engine.toggle_keys = keys;
            println!(
                "{}: {}",
                t!("toggle_keys_label"),
                config.engine.toggle_keys.join(", ")
            );
        }
        ConfigKey::HanjaKeys => {
            let keys: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if keys.is_empty() {
                return Err("At least one key required".to_string());
            }
            config.engine.hanja_keys = keys;
            println!(
                "{}: {}",
                t!("hanja_keys_label"),
                config.engine.hanja_keys.join(", ")
            );
        }
        ConfigKey::PopupMode => {
            let mode = match value {
                "standalone" | "Standalone" => unim::config::PopupMode::Standalone,
                "embedded" | "Embedded" => unim::config::PopupMode::Embedded,
                _ => {
                    return Err(format!(
                        "Invalid popup mode: {}. Allowed: standalone, embedded",
                        value
                    ));
                }
            };
            config.engine.popup_mode = mode;
            println!(
                "{}: {}",
                t!("popup_mode_label"),
                config.engine.popup_mode.name()
            );
        }
        ConfigKey::AutoTypeFix => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid value for auto-typefix: {}", value)),
            };
            config.engine.auto_typefix.enabled = enabled;
            let status = if enabled {
                t!("enabled")
            } else {
                t!("disabled")
            };
            println!("{}", t!("auto_typefix_changed", status = status));
        }
        ConfigKey::AutoTypeFixKorThreshold => {
            let v: u8 = value
                .parse()
                .map_err(|_| format!("Invalid number: {}", value))?;
            if !(AUTO_TYPEFIX_KOR_THRESHOLD_MIN..=AUTO_TYPEFIX_KOR_THRESHOLD_MAX).contains(&v) {
                return Err(format!(
                    "Range {}~{}, got {}",
                    AUTO_TYPEFIX_KOR_THRESHOLD_MIN, AUTO_TYPEFIX_KOR_THRESHOLD_MAX, v
                ));
            }
            config.engine.auto_typefix.kor_syllable_threshold = v;
            println!("한글 음절 임계값: {}", v);
        }
        ConfigKey::AutoTypeFixEngMinLength => {
            let v: u8 = value
                .parse()
                .map_err(|_| format!("Invalid number: {}", value))?;
            if !(AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN..=AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX).contains(&v) {
                return Err(format!(
                    "Range {}~{}, got {}",
                    AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN, AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, v
                ));
            }
            config.engine.auto_typefix.eng_word_min_length = v;
            println!("영문 단어 최소 길이: {}", v);
        }
        ConfigKey::AutoTypeFixForwardTimeWindow => {
            let v: u32 = value
                .parse()
                .map_err(|_| format!("Invalid number: {}", value))?;
            if !(AUTO_TYPEFIX_TIME_WINDOW_MIN..=AUTO_TYPEFIX_TIME_WINDOW_MAX).contains(&v) {
                return Err(format!(
                    "Range {}~{}, got {}",
                    AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX, v
                ));
            }
            config.engine.auto_typefix.forward_time_window_ms = v;
            println!("순방향 시간 윈도우: {}ms", v);
        }
        ConfigKey::AutoTypeFixReverseTimeWindow => {
            let v: u32 = value
                .parse()
                .map_err(|_| format!("Invalid number: {}", value))?;
            if !(AUTO_TYPEFIX_TIME_WINDOW_MIN..=AUTO_TYPEFIX_TIME_WINDOW_MAX).contains(&v) {
                return Err(format!(
                    "Range {}~{}, got {}",
                    AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX, v
                ));
            }
            config.engine.auto_typefix.reverse_time_window_ms = v;
            println!("역방향 시간 윈도우: {}ms", v);
        }
        ConfigKey::AutoTypeFixForward => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.forward = enabled;
            println!(
                "순방향(영→한) 교정: {}",
                if enabled { "ON" } else { "OFF" }
            );
        }
        ConfigKey::AutoTypeFixReverse => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.reverse = enabled;
            println!(
                "역방향(한→영) 교정: {}",
                if enabled { "ON" } else { "OFF" }
            );
        }
        ConfigKey::AutoTypeFixSkipEnglishWord => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.skip_on_english_word = enabled;
            println!(
                "{}: {}",
                t!("auto_typefix_skip_english_word_label"),
                if enabled { "ON" } else { "OFF" }
            );
        }
        ConfigKey::AutoTypeFixSkipCompleteSyllable => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.skip_on_complete_syllable = enabled;
            println!(
                "{}: {}",
                t!("auto_typefix_skip_complete_syllable_label"),
                if enabled { "ON" } else { "OFF" }
            );
        }
        ConfigKey::AutoTypeFixRollbackDetection => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.rollback_detection = enabled;
            println!(
                "{}: {}",
                t!("auto_typefix_rollback_detection_label"),
                if enabled { "ON" } else { "OFF" }
            );
        }
        ConfigKey::AutoTypeFixTentativeExpiryHours => {
            let hours: u16 = value
                .parse()
                .map_err(|_| format!("Invalid number: {}", value))?;
            if !(AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN..=AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX)
                .contains(&hours)
            {
                return Err(format!(
                    "Value must be between {} and {}",
                    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX
                ));
            }
            config.engine.auto_typefix.tentative_expiry_hours = hours;
            println!(
                "{}: {} {}",
                t!("auto_typefix_tentative_expiry_hours_label"),
                hours,
                t!("unit_hours")
            );
        }
        ConfigKey::AutoTypeFixObservationTimeoutSecs => {
            let secs: u8 = value
                .parse()
                .map_err(|_| format!("Invalid number: {}", value))?;
            if !(AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN..=AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX)
                .contains(&secs)
            {
                return Err(format!(
                    "Value must be between {} and {}",
                    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX
                ));
            }
            config.engine.auto_typefix.observation_timeout_secs = secs;
            println!(
                "{}: {} {}",
                t!("auto_typefix_observation_timeout_secs_label"),
                secs,
                t!("unit_secs")
            );
        }
        ConfigKey::AppRules => {
            let rules: Vec<unim::config::AppRule> =
                serde_json::from_str(value).map_err(|e| format!("Invalid JSON: {}", e))?;
            config.engine.app_rules = rules;
            println!(
                "{}: {} rules",
                t!("app_rules_label"),
                config.engine.app_rules.len()
            );
        }
    }

    config.engine.auto_typefix.clamp_ranges();

    config
        .save_to_default_path()
        .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
    println!("{}", t!("config_saved"));
    Ok(())
}

fn config_path() {
    if let Some(path) = UnimConfig::default_config_path() {
        println!("{}", path.display());
    } else {
        eprintln!("{}", t!("error_path_not_found"));
    }
}

fn config_reset() -> Result<(), String> {
    let config = UnimConfig::default();
    config
        .save_to_default_path()
        .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
    println!("{}", t!("config_reset_done"));
    config_show();
    Ok(())
}

fn config_interactive() {
    let mut config = UnimConfig::load_from_default_path();
    let theme = ColorfulTheme::default();

    loop {
        println!("\x1B[2J\x1B[1;1H");
        config_show();

        let options = vec![
            t!("korean_layout_label").to_string(),
            t!("english_layout_label").to_string(),
            t!("default_category_label").to_string(),
            t!("mode_sharing_label").to_string(),
            t!("toggle_keys_label").to_string(),
            t!("hanja_keys_label").to_string(),
            t!("config_reset_desc").to_string(),
            t!("save_and_exit").to_string(),
            t!("exit_without_save").to_string(),
        ];

        let selection = Select::with_theme(&theme)
            .with_prompt(t!("select_setting").to_string())
            .default(0)
            .items(&options)
            .interact()
            .unwrap();

        match selection {
            0 => {
                let layouts = KoreanLayout::all();
                let layout_names: Vec<&str> = layouts.iter().map(|l| l.display_name()).collect();
                let current_idx = layouts
                    .iter()
                    .position(|l| *l == config.engine.korean.layout)
                    .unwrap_or(0);
                let s = Select::with_theme(&theme)
                    .with_prompt(t!("select_korean_layout").to_string())
                    .items(&layout_names)
                    .default(current_idx)
                    .interact()
                    .unwrap();

                config.engine.korean.layout = layouts[s];
            }
            1 => {
                let layouts = EnglishLayout::all();
                let layout_names: Vec<&str> = layouts.iter().map(|l| l.display_name()).collect();
                let current_idx = layouts
                    .iter()
                    .position(|l| *l == config.engine.english.layout)
                    .unwrap_or(0);
                let s = Select::with_theme(&theme)
                    .with_prompt(t!("select_english_layout").to_string())
                    .items(&layout_names)
                    .default(current_idx)
                    .interact()
                    .unwrap();

                config.engine.english.layout = layouts[s];
            }
            2 => {
                let categories = [InputCategory::Korean, InputCategory::English];
                let category_names: Vec<String> = categories
                    .iter()
                    .map(|c| match c {
                        InputCategory::Korean => t!("korean_mode").to_string(),
                        InputCategory::English => t!("english_mode").to_string(),
                    })
                    .collect();
                let current_idx = categories
                    .iter()
                    .position(|c| *c == config.engine.default_category)
                    .unwrap_or(0);
                let s = Select::with_theme(&theme)
                    .with_prompt(t!("select_default_category").to_string())
                    .items(&category_names)
                    .default(current_idx)
                    .interact()
                    .unwrap();

                config.engine.default_category = categories[s];
            }
            3 => {
                let modes = ModeSharingMode::all();
                let mode_names: Vec<&str> = modes.iter().map(|m| m.display_name()).collect();
                let current_idx = modes
                    .iter()
                    .position(|m| *m == config.engine.mode_sharing)
                    .unwrap_or(0);
                let s = Select::with_theme(&theme)
                    .with_prompt(t!("select_mode_sharing").to_string())
                    .items(&mode_names)
                    .default(current_idx)
                    .interact()
                    .unwrap();

                config.engine.mode_sharing = modes[s];
            }
            4 => {
                let current = config.engine.toggle_keys.join(",");
                let input: String = Input::with_theme(&theme)
                    .with_prompt(t!("toggle_keys_label").to_string())
                    .default(current)
                    .interact_text()
                    .unwrap();
                let keys: Vec<String> = input
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !keys.is_empty() {
                    config.engine.toggle_keys = keys;
                }
            }
            5 => {
                let current = config.engine.hanja_keys.join(",");
                let input: String = Input::with_theme(&theme)
                    .with_prompt(t!("hanja_keys_label").to_string())
                    .default(current)
                    .interact_text()
                    .unwrap();
                let keys: Vec<String> = input
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                if !keys.is_empty() {
                    config.engine.hanja_keys = keys;
                }
            }
            6 => {
                if Confirm::with_theme(&theme)
                    .with_prompt(t!("confirm_reset").to_string())
                    .default(false)
                    .interact()
                    .unwrap()
                {
                    config = UnimConfig::default();
                    println!("{}", t!("config_reset_done"));
                }
            }
            7 => {
                if let Err(e) = config.save_to_default_path() {
                    eprintln!("{}", t!("error_label", error = e.to_string()));
                } else {
                    println!("{}", t!("config_saved"));
                }
                break;
            }
            8 => {
                println!("{}", t!("exit_canceled"));
                break;
            }
            _ => unreachable!(),
        }
    }
}

fn handle_config(command: Option<ConfigCommands>) {
    match command {
        Some(ConfigCommands::Show) => config_show(),
        Some(ConfigCommands::Set { key, value }) => {
            if let Err(e) = config_set(key, &value) {
                eprintln!("{}", t!("error_label", error = e));
                process::exit(1);
            }
        }
        Some(ConfigCommands::Path) => config_path(),
        Some(ConfigCommands::Reset) => {
            if let Err(e) = config_reset() {
                eprintln!("{}", t!("error_label", error = e));
                process::exit(1);
            }
        }
        Some(ConfigCommands::Interactive) => config_interactive(),
        None => {
            config_show();
            println!("\n{}", t!("help_hint"));
        }
    }
}

fn main() -> io::Result<()> {
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_else(|_| "en".to_string());
    let locale = locale.split('.').next().unwrap_or("en");
    let locale = locale.split('_').next().unwrap_or("en");
    rust_i18n::set_locale(locale);

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Config { command }) => {
            handle_config(command);
            Ok(())
        }
        None => {
            let config = ConvertConfig::from_cli(&cli);

            for warning in &config.warnings {
                eprintln!("{}", t!("warning_label", warning = warning));
            }

            if let Err(e) = run_convert(config) {
                eprintln!("{}", t!("error_label", error = e.to_string()));
                process::exit(1);
            }

            Ok(())
        }
    }
}
