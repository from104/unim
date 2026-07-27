use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use rust_i18n::t;
use std::fs::File;
use std::io::{self, BufRead, BufReader, Write};
use std::path::{Path, PathBuf};
use std::process;
use unim::config::{
    english_layout_display_name, korean_layout_display_name, normalize_english_layout_name,
    normalize_korean_layout_name, CommitUnit, Config as UnimConfig, InputCategory, KoreanConfig,
    ModeSharingMode,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN, ENGLISH_LAYOUT_BUILTINS,
    KOREAN_LAYOUT_BUILTINS, KOREAN_LAYOUT_DUBEOLSIK, KOREAN_LAYOUT_SEBEOLSIK_390,
    KOREAN_LAYOUT_SEBEOLSIK_391, KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT,
};
use unim::hangul::composer_with_2bul::HangulComposer2Bul;
use unim::hangul::composer_with_3bul::HangulComposer3Bul;
use unim::keystroke::keyboard_map::KeyboardMap;
use unim::keystroke::keystrokes_to_korean::keystrokes_to_korean;
use unim::keystroke::korean_to_keystrokes::korean_to_keystrokes;
use unim::keystroke::profile::{
    build_combined_jamo_map, parse_profile_str, resolve_inherits, ProfileRegistry,
};
use unim::typefix_blacklist::Blacklist;
use unim::typefix_userdict::UserDictionary;
use unim::unim_log;

rust_i18n::i18n!("locales");

// ─────────────────────────────────────────────────────────────────────────────
// Help text helpers (i18n via rust-i18n)
//
// clap derive의 about/long_about/help/long_help 속성은 expr를 받으므로 함수
// 호출로 런타임 i18n 키를 해석할 수 있다. main()에서 LANG/LC_ALL 기반 로케일을
// `Cli::parse()` 보다 먼저 설정하므로, 여기서 호출되는 t!() 매크로는 사용자
// 환경 언어로 해석된다.
// ─────────────────────────────────────────────────────────────────────────────
fn h(key: &str) -> String {
    t!(key).to_string()
}

// ─────────────────────────────────────────────────────────────────────────────
// 표시값 로케일 어댑터 (M-16 / UX-LINUX-V01)
//
// `config.rs`의 `*_display_name()` / `display_name()` 계열은 라벨이 아니라
// "값"인데도 한국어 &'static str 리터럴을 반환한다(core 는 로케일 개념이
// 없다). core pub 시그니처는 그대로 두고, 여기서 로케일 키로 다시 매핑해
// `config show`/대화형 선택기에 노출되는 값 자체도 LANG 을 따르게 한다.
// 영어 자판 이름(QWERTY/Dvorak/...)은 고유명사라 번역 대상이 아니므로 제외.
// ─────────────────────────────────────────────────────────────────────────────
fn korean_layout_display_name_localized(name: &str) -> String {
    match normalize_korean_layout_name(name).as_str() {
        KOREAN_LAYOUT_DUBEOLSIK => t!("layout_ko_2bulstd").to_string(),
        KOREAN_LAYOUT_SEBEOLSIK_390 => t!("layout_ko_3bul390").to_string(),
        KOREAN_LAYOUT_SEBEOLSIK_391 => t!("layout_ko_3bul391").to_string(),
        KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT => t!("layout_ko_3bul_noshift").to_string(),
        // 사용자 정의 프로필 — core 와 동일하게 빈 문자열(GUI/CLI 가 프로필
        // metadata 의 display_name 을 별도 resolve).
        _ => korean_layout_display_name(name).to_string(),
    }
}

fn mode_sharing_display_name_localized(mode: ModeSharingMode) -> String {
    match mode {
        ModeSharingMode::Global => t!("mode_sharing_global").to_string(),
        ModeSharingMode::PerApp => t!("mode_sharing_perapp").to_string(),
    }
}

fn commit_unit_display_name_localized(unit: CommitUnit) -> String {
    match unit {
        CommitUnit::Syllable => t!("commit_unit_syllable").to_string(),
        CommitUnit::Word => t!("commit_unit_word").to_string(),
        CommitUnit::Smart => t!("commit_unit_smart").to_string(),
    }
}

/// UNIM-cli: Korean/English keyboard converter + settings manager
#[derive(Parser, Debug)]
#[command(
    author,
    version,
    about = h("help_cli_about"),
    long_about = h("help_cli_long_about"),
    args_conflicts_with_subcommands = true
)]
struct Cli {
    #[arg(
        name = "FILE",
        help = h("help_cli_files_short"),
        long_help = h("help_cli_files_long"),
    )]
    input_files: Vec<String>,

    #[arg(
        short, long, value_name = "FILE",
        help = h("help_cli_output_short"),
        long_help = h("help_cli_output_long"),
    )]
    output: Option<String>,

    #[arg(
        short, long, group = "conversion", default_value_t = true,
        help = h("help_cli_compose_short"),
        long_help = h("help_cli_compose_long"),
    )]
    compose: bool,

    #[arg(
        short, long, group = "conversion",
        help = h("help_cli_decompose_short"),
        long_help = h("help_cli_decompose_long"),
    )]
    decompose: bool,

    #[arg(
        short = 'k', long = "korean-keyboard", value_enum,
        default_value_t = KeyboardMode::TwoBulStd,
        help = h("help_cli_korean_keyboard_short"),
        long_help = h("help_cli_korean_keyboard_long"),
    )]
    korean_keyboard: KeyboardMode,

    #[arg(
        short = 'e', long = "english-keyboard", value_enum,
        default_value_t = EnglishKeyboardMode::Qwerty,
        help = h("help_cli_english_keyboard_short"),
        long_help = h("help_cli_english_keyboard_long"),
    )]
    english_keyboard: EnglishKeyboardMode,

    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
    /// Manage settings (placeholder; overridden by command attributes)
    #[command(
        about = h("help_cmd_config_about"),
        long_about = h("help_cmd_config_long"),
    )]
    Config {
        #[command(subcommand)]
        command: Option<ConfigCommands>,
    },
    /// Trigger an action on the daemon (placeholder; overridden)
    #[command(
        about = h("help_cmd_trigger_about"),
        long_about = h("help_cmd_trigger_long"),
    )]
    Trigger {
        #[arg(
            help = h("help_arg_trigger_action_short"),
            long_help = h("help_arg_trigger_action_long"),
        )]
        action: String,
    },
    /// Query daemon runtime state
    #[command(about = h("help_cmd_daemon_about"))]
    Daemon {
        #[command(subcommand)]
        command: DaemonCommands,
    },
}

#[derive(Subcommand, Debug)]
enum DaemonCommands {
    /// List currently registered frontends
    #[command(about = h("help_daemon_frontends_about"))]
    Frontends,
}

#[derive(Subcommand, Debug)]
enum ConfigCommands {
    /// Show current settings (placeholder)
    #[command(
        about = h("help_cfg_show_about"),
        long_about = h("help_cfg_show_long"),
    )]
    Show,
    /// Set a config value (placeholder)
    #[command(
        about = h("help_cfg_set_about"),
        long_about = h("help_cfg_set_long"),
    )]
    Set {
        #[arg(
            value_enum,
            help = h("help_cfg_set_key_short"),
            long_help = h("help_cfg_set_key_long"),
        )]
        key: ConfigKey,
        #[arg(
            help = h("help_cfg_set_value_short"),
            long_help = h("help_cfg_set_value_long"),
        )]
        value: String,
    },
    /// Show config file path (placeholder)
    #[command(
        about = h("help_cfg_path_about"),
        long_about = h("help_cfg_path_long"),
    )]
    Path,
    /// Reset settings to defaults (placeholder)
    #[command(
        about = h("help_cfg_reset_about"),
        long_about = h("help_cfg_reset_long"),
    )]
    Reset,
    /// Start interactive editor (placeholder)
    #[command(
        about = h("help_cfg_interactive_about"),
        long_about = h("help_cfg_interactive_long"),
    )]
    Interactive,
    /// Inspect/validate keyboard layout profiles (placeholder)
    #[command(
        about = h("help_cfg_layout_about"),
        long_about = h("help_cfg_layout_long"),
    )]
    Layout {
        #[command(subcommand)]
        action: LayoutAction,
    },
    /// Manage reverse user dictionary (placeholder)
    #[command(
        about = h("help_cfg_userdict_about"),
        long_about = h("help_cfg_userdict_long"),
    )]
    UserDict {
        #[command(subcommand)]
        action: UserDictCommand,
    },
    /// Manage AutoTypeFix blacklist (learned correction suppressions)
    #[command(
        about = h("help_cmd_blacklist_about"),
        long_about = h("help_cmd_blacklist_long"),
    )]
    Blacklist {
        #[command(subcommand)]
        action: BlacklistCommand,
    },
}

/// 역방향 사용자 사전 (AutoTypeFix reverse 전용 whitelist) 관리 서브커맨드.
#[derive(Subcommand, Debug)]
enum UserDictCommand {
    /// List dictionary words (placeholder)
    #[command(
        about = h("help_ud_list_about"),
        long_about = h("help_ud_list_long"),
    )]
    List,
    /// Add a word (placeholder)
    #[command(
        about = h("help_ud_add_about"),
        long_about = h("help_ud_add_long"),
    )]
    Add {
        #[arg(
            help = h("help_ud_add_word_short"),
            long_help = h("help_ud_add_word_long"),
        )]
        word: String,
        #[arg(
            long,
            help = h("help_ud_add_note_short"),
            long_help = h("help_ud_add_note_long"),
        )]
        note: Option<String>,
    },
    /// Remove a word (placeholder)
    #[command(
        about = h("help_ud_remove_about"),
        long_about = h("help_ud_remove_long"),
    )]
    Remove {
        #[arg(
            help = h("help_ud_remove_word_short"),
            long_help = h("help_ud_remove_word_long"),
        )]
        word: String,
    },
    /// Clear the dictionary (placeholder)
    #[command(
        about = h("help_ud_clear_about"),
        long_about = h("help_ud_clear_long"),
    )]
    Clear,
    /// Show dictionary file path (placeholder)
    #[command(
        about = h("help_ud_path_about"),
        long_about = h("help_ud_path_long"),
    )]
    Path,
}

/// AutoTypeFix 블랙리스트 관리 서브커맨드.
#[derive(Subcommand, Debug)]
enum BlacklistCommand {
    /// List blacklist entries (placeholder)
    #[command(
        about = h("help_bl_list_about"),
        long_about = h("help_bl_list_long"),
    )]
    List,
    /// Remove a blacklist entry by index (placeholder)
    #[command(
        about = h("help_bl_remove_about"),
        long_about = h("help_bl_remove_long"),
    )]
    Remove {
        #[arg(
            help = h("help_bl_remove_idx_short"),
            long_help = h("help_bl_remove_idx_long"),
        )]
        index: usize,
    },
    /// Clear all blacklist entries (placeholder)
    #[command(
        about = h("help_bl_clear_about"),
        long_about = h("help_bl_clear_long"),
    )]
    Clear,
    /// Show blacklist file path (placeholder)
    #[command(
        about = h("help_bl_path_about"),
        long_about = h("help_bl_path_long"),
    )]
    Path,
}

#[derive(Subcommand, Debug)]
enum LayoutAction {
    /// List builtin and user profiles (placeholder)
    #[command(
        about = h("help_layout_list_about"),
        long_about = h("help_layout_list_long"),
    )]
    List,
    /// Describe a profile (placeholder)
    #[command(
        about = h("help_layout_describe_about"),
        long_about = h("help_layout_describe_long"),
    )]
    Describe {
        #[arg(
            help = h("help_layout_describe_name_short"),
            long_help = h("help_layout_describe_name_long"),
        )]
        name: String,
    },
    /// Validate a profile file (placeholder)
    #[command(
        about = h("help_layout_validate_about"),
        long_about = h("help_layout_validate_long"),
    )]
    Validate {
        #[arg(
            help = h("help_layout_validate_file_short"),
            long_help = h("help_layout_validate_file_long"),
        )]
        file: PathBuf,
    },
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
    /// 쿼티형 세벌식 자판
    #[value(name = "3bul_qwerty")]
    ThreeBulQwerty,
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
    /// 한국어 레이아웃 (2bul, 3bul390, 3bul391, 3bul_noshift, 3bul_qwerty)
    #[value(name = "korean-layout", help = h("help_ck_korean_layout"))]
    KoreanLayout,
    /// 영어 레이아웃 (qwerty, dvorak, colemak, colemak_dh, workman)
    #[value(name = "english-layout", help = h("help_ck_english_layout"))]
    EnglishLayout,
    /// 한국어 자판 활성 규칙 세트 (쉼표 구분, 빈 문자열 = 프로필 기본값 사용)
    #[value(
        name = "korean-active-rule-sets",
        help = h("help_ck_korean_active_rule_sets")
    )]
    KoreanActiveRuleSets,
    /// 초기 입력 모드 (korean, english)
    #[value(name = "default-category", help = h("help_ck_default_category"))]
    DefaultCategory,
    /// 모드 공유 방식 (global, per-app)
    #[value(name = "mode-sharing", help = h("help_ck_mode_sharing"))]
    ModeSharing,
    /// 한/영 전환 키 (예: Korean,RightAlt)
    #[value(name = "toggle-keys", help = h("help_ck_toggle_keys"))]
    ToggleKeys,
    /// 한자/특수문자 키 (예: Hanja,F9)
    #[value(name = "hanja-keys", help = h("help_ck_hanja_keys"))]
    HanjaKeys,
    /// 자동 오타 교정 활성화 (true, false)
    #[value(name = "auto-typefix", help = h("help_ck_auto_typefix"))]
    AutoTypeFix,
    /// 자동 오타 교정: 한글 음절 임계값 (2~6)
    #[value(
        name = "auto-typefix-kor-threshold",
        help = h("help_ck_auto_typefix_kor_threshold")
    )]
    AutoTypeFixKorThreshold,
    /// 자동 오타 교정: 영문 단어 최소 길이 (3~8)
    #[value(
        name = "auto-typefix-eng-min-length",
        help = h("help_ck_auto_typefix_eng_min_length")
    )]
    AutoTypeFixEngMinLength,
    /// 자동 오타 교정: 순방향 시간 윈도우 (500~5000 ms)
    #[value(
        name = "auto-typefix-forward-time-window-ms",
        help = h("help_ck_auto_typefix_forward_time_window_ms")
    )]
    AutoTypeFixForwardTimeWindow,
    /// 자동 오타 교정: 역방향 시간 윈도우 (500~5000 ms)
    #[value(
        name = "auto-typefix-reverse-time-window-ms",
        help = h("help_ck_auto_typefix_reverse_time_window_ms")
    )]
    AutoTypeFixReverseTimeWindow,
    /// 자동 오타 교정: 순방향 (영→한) 교정 (true, false)
    #[value(
        name = "auto-typefix-forward",
        help = h("help_ck_auto_typefix_forward")
    )]
    AutoTypeFixForward,
    /// 자동 오타 교정: 역방향 (한→영) 교정 (true, false)
    #[value(
        name = "auto-typefix-reverse",
        help = h("help_ck_auto_typefix_reverse")
    )]
    AutoTypeFixReverse,
    /// 자동 오타 교정: 영단어 매칭 시 억제 (true, false)
    #[value(
        name = "auto-typefix-skip-english-word",
        help = h("help_ck_auto_typefix_skip_english_word")
    )]
    AutoTypeFixSkipEnglishWord,
    /// 자동 오타 교정: 온전한 음절 매칭 시 억제 (true, false)
    #[value(
        name = "auto-typefix-skip-complete-syllable",
        help = h("help_ck_auto_typefix_skip_complete_syllable")
    )]
    AutoTypeFixSkipCompleteSyllable,
    /// 자동 오타 교정: 재트리거 기반 학습형 억제 (true, false)
    #[value(
        name = "auto-typefix-rollback-detection",
        help = h("help_ck_auto_typefix_rollback_detection")
    )]
    AutoTypeFixRollbackDetection,
    /// 자동 오타 교정: 임시 억제 단어 만료 기간 (1~12 시간)
    #[value(
        name = "auto-typefix-tentative-expiry-hours",
        help = h("help_ck_auto_typefix_tentative_expiry_hours")
    )]
    AutoTypeFixTentativeExpiryHours,
    /// 자동 오타 교정: 재트리거 관찰 창 (5~15 초)
    #[value(
        name = "auto-typefix-observation-timeout-secs",
        help = h("help_ck_auto_typefix_observation_timeout_secs")
    )]
    AutoTypeFixObservationTimeoutSecs,
    /// 자동 오타 교정: 역방향 사용자 사전 활성화 (true, false)
    #[value(
        name = "auto-typefix-user-dict",
        help = h("help_ck_auto_typefix_user_dict")
    )]
    AutoTypeFixUserDictEnabled,
    /// 자동 오타 교정: 전체 토글 단축키 (쉼표 구분 단일 키, 비우면 사용 안 함. 예: F10, Shift+F9)
    #[value(
        name = "auto-typefix-toggle-keys",
        help = h("help_ck_auto_typefix_toggle_keys")
    )]
    AutoTypeFixToggleKeys,
    /// 자동 오타 교정: 순방향(영→한) 토글 단축키 (쉼표 구분 단일 키, 비우면 사용 안 함)
    #[value(
        name = "auto-typefix-forward-toggle-keys",
        help = h("help_ck_auto_typefix_forward_toggle_keys")
    )]
    AutoTypeFixForwardToggleKeys,
    /// 자동 오타 교정: 역방향(한→영) 토글 단축키 (쉼표 구분 단일 키, 비우면 사용 안 함)
    #[value(
        name = "auto-typefix-reverse-toggle-keys",
        help = h("help_ck_auto_typefix_reverse_toggle_keys")
    )]
    AutoTypeFixReverseToggleKeys,
    /// 자동 영문 모드 전환 활성화 (true, false)
    #[value(name = "auto-english", help = h("help_ck_auto_english"))]
    AutoEnglish,
    /// 자동 영문 전환 트리거 키 (예: key:Escape,char:/,char:,)
    #[value(name = "auto-english-keys", help = h("help_ck_auto_english_keys"))]
    AutoEnglishKeys,
    /// 앱별 모드 규칙 (JSON 형식)
    #[value(name = "app-rules", help = h("help_ck_app_rules"))]
    AppRules,
    /// 모아치기 양방향 자모 결합 (true, false). supports_moachigi 자판 전용.
    #[value(
        name = "korean-bidirectional-combine",
        help = h("help_ck_korean_bidirectional_combine")
    )]
    KoreanBidirectionalCombine,
    /// 모아치기 화음 윈도우 (ms, 0=OFF). supports_moachigi 자판 전용. Phase 4 예약.
    #[value(
        name = "korean-chord-window-ms",
        help = h("help_ck_korean_chord_window_ms")
    )]
    KoreanChordWindowMs,
    /// 조합 확정 단위 (syllable, word, smart). 단어 단위는 터미널·XIM 비대상, 스마트는 word-mode-apps 목록 앱만 단어 조합.
    #[value(name = "commit-unit", help = h("help_ck_commit_unit"))]
    CommitUnit,
    /// 단어 모드 앱 목록 (쉼표 구분, 실행 파일명 정확일치). Smart 확정 단위에서 단어 조합할 앱 (Windows: winword.exe, Linux: soffice 등).
    #[value(name = "word-mode-apps", help = h("help_ck_word_mode_apps"))]
    WordModeApps,
    /// 한/영 전환 소리 알림 (true, false). 접근성 — 전환 시 차등 비프음(한글=높은 음, 영문=낮은 음).
    #[value(
        name = "toggle-announce-beep",
        help = h("help_ck_toggle_announce_beep")
    )]
    ToggleAnnounceBeep,
    /// 조합키 자동반복 억제 (true, false). 접근성(지체장애) — 키 홀드 시 연타·토글 진동 방지. Windows·Linux 공통 집행.
    #[value(name = "ignore-key-repeat", help = h("help_ck_ignore_key_repeat"))]
    IgnoreKeyRepeat,
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
        KeyboardMode::ThreeBulQwerty => "ko_3bul_qwerty",
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
        KeyboardMode::ThreeBul390
            | KeyboardMode::ThreeBul391
            | KeyboardMode::ThreeBulNoShift
            | KeyboardMode::ThreeBulQwerty
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

    let korean_name = korean_layout_display_name_localized(&config.engine.korean.layout);
    let english_name = english_layout_display_name(&config.engine.english.layout);

    let default_category_name = match config.engine.default_category {
        InputCategory::Korean => t!("korean_mode"),
        InputCategory::English => t!("english_mode"),
    };

    let mode_sharing_name = mode_sharing_display_name_localized(config.engine.mode_sharing);

    println!("{}", t!("settings_title"));
    println!("================");
    println!(
        "{}: {} ({})",
        t!("korean_layout_label"),
        korean_name,
        config.engine.korean.layout
    );
    {
        // None         → 프로필 기본값 사용
        // Some(vec![]) → 사용자가 명시적으로 모두 OFF
        // Some(list)   → 명시 활성 목록
        let display = match &config.engine.korean.active_rule_sets {
            None => t!("profile_default").to_string(),
            Some(list) if list.is_empty() => t!("korean_active_rule_sets_all_off").to_string(),
            Some(list) => list.join(", "),
        };
        println!("{}: {}", t!("korean_active_rule_sets_label"), display);
    }
    println!(
        "{}: {} ({})",
        t!("english_layout_label"),
        english_name,
        config.engine.english.layout
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
        "{}: {} {}",
        t!("commit_unit_label"),
        commit_unit_display_name_localized(config.engine.korean.commit_unit),
        t!("commit_unit_note")
    );
    println!(
        "{}: {}",
        t!("word_mode_apps_label"),
        config.engine.korean.word_mode_apps.join(", ")
    );
    let auto_typefix_status = if config.engine.auto_typefix.enabled {
        t!("enabled")
    } else {
        t!("disabled")
    };
    println!("{}: {}", t!("auto_typefix_label"), auto_typefix_status);
    {
        // 토글 단축키 3종 — ATF off 상태에서도 전체 켜기용으로 쓰이므로 항상 표시.
        let atf = &config.engine.auto_typefix;
        println!(
            "{}",
            t!(
                "config_atfix_toggle_keys_line",
                all = display_keys(&atf.toggle_enabled_keys),
                fwd = display_keys(&atf.toggle_forward_keys),
                rev = display_keys(&atf.toggle_reverse_keys)
            )
        );
    }
    if config.engine.auto_typefix.enabled {
        let atf = &config.engine.auto_typefix;
        println!(
            "{}",
            t!(
                "config_atfix_status_line",
                fwd = if atf.forward { "ON" } else { "OFF" },
                rev = if atf.reverse { "ON" } else { "OFF" }
            )
        );
        println!(
            "{}",
            t!(
                "config_atfix_threshold_line",
                kor = atf.kor_syllable_threshold.to_string(),
                eng = atf.eng_word_min_length.to_string()
            )
        );
        println!(
            "{}",
            t!(
                "config_atfix_window_line",
                fwd_ms = atf.forward_time_window_ms.to_string(),
                rev_ms = atf.reverse_time_window_ms.to_string()
            )
        );
        println!(
            "{}",
            t!(
                "config_atfix_retrigger_line",
                rb = if atf.rollback_detection { "ON" } else { "OFF" },
                obs = atf.observation_timeout_secs.to_string(),
                exp = atf.tentative_expiry_hours.to_string()
            )
        );
        let ud = UserDictionary::load_from_default_path();
        println!(
            "{}",
            t!(
                "config_atfix_userdict_line",
                state = if atf.user_dict_enabled { "ON" } else { "OFF" },
                count = ud.len().to_string()
            )
        );
    }
    let auto_english_status = if config.engine.auto_english.enabled {
        t!("enabled")
    } else {
        t!("disabled")
    };
    println!("{}: {}", t!("auto_english_label"), auto_english_status);
    if config.engine.auto_english.enabled {
        println!(
            "  - {}: {}",
            t!("auto_english_keys_label"),
            config.engine.auto_english.trigger_keys.join(", ")
        );
    }
    let toggle_beep_status = if config.engine.toggle_announce_beep {
        t!("enabled")
    } else {
        t!("disabled")
    };
    println!(
        "{}: {}",
        t!("toggle_announce_beep_label"),
        toggle_beep_status
    );
    let ignore_repeat_status = if config.engine.ignore_key_repeat {
        t!("enabled")
    } else {
        t!("disabled")
    };
    println!(
        "{}: {}",
        t!("ignore_key_repeat_label"),
        ignore_repeat_status
    );
    println!(
        "{}: {}",
        t!("app_rules_label"),
        if config.engine.app_rules.is_empty() {
            t!("not_set").to_string()
        } else {
            format!("{} rules", config.engine.app_rules.len())
        }
    );
    // 모아치기 설정 (supports_moachigi 자판에서만 유효)
    if config.engine.korean.bidirectional_combine.is_some()
        || config.engine.korean.chord_window_ms.is_some()
    {
        println!("-- 모아치기 (moachigi) --");
        match config.engine.korean.bidirectional_combine {
            Some(v) => println!("  korean-bidirectional-combine: {v}"),
            None => println!("  korean-bidirectional-combine: 자판 기본값"),
        }
        match config.engine.korean.chord_window_ms {
            Some(v) => println!("  korean-chord-window-ms: {v}ms"),
            None => println!("  korean-chord-window-ms: 자판 기본값"),
        }
    }
    println!();
    if let Some(path) = UnimConfig::default_config_path() {
        println!("{}: {}", t!("config_file_label"), path.display());
    }
}

/// 단축키 목록 표시용 — 비어 있으면 "(사용 안 함)" 라벨로 노출한다(옵트인 키 3종 공통).
fn display_keys(keys: &[String]) -> String {
    if keys.is_empty() {
        t!("keys_unset").to_string()
    } else {
        keys.join(", ")
    }
}

/// ATF 토글 핫키 값(키 이름 목록)을 검증해 경고 목록과 `Unknown` 포함 여부를 돌려준다.
///
/// 차단하지 않고 경고만 낸다(toggle_keys 규약과 정합 — 저장은 호출부에서 그대로 수행).
/// * 파싱 불가 표기 → 오타/미지원 경고. 호출부는 이 경우 성공 에코를 억제한다.
/// * 수정자 없는 문자·편집키(A~Z, Space/Enter/Backspace/Tab/Delete) → 그 키로 입력이
///   불가능해지는 풋건 경고. 수정자 조합(`Ctrl+A` 등)은 맨 키 입력을 막지 않으므로 제외.
/// * 기존 한/영(`toggle_keys`)·한자(`hanja_keys`) 키와 중복 → 역할 충돌 경고.
///   수정자가 붙으면 맨 키와 갈리므로(`Shift+F9` vs `F9`) 충돌이 아니다.
///
/// 표기 해석은 엔진의 `parse_atf_hotkey` 를 그대로 쓴다 — CLI 가 자체 파싱하면
/// 엔진이 받아들이는 표기를 CLI 가 오탐하는 어긋남이 생긴다(실제로 `Shift+F9` 가
/// Unknown 으로 보고되던 버그의 원인).
fn atf_hotkey_warnings(keys: &[String], config: &UnimConfig) -> (Vec<String>, bool) {
    use unim::input_engine::InputEngine;
    use unim::keycode::KeyCode;
    let mut warnings = Vec::new();
    let mut any_unknown = false;
    for name in keys {
        let Some(hotkey) = InputEngine::parse_atf_hotkey(name) else {
            any_unknown = true;
            warnings.push(t!("atf_hotkey_warn_unknown", key = name.clone()).to_string());
            continue;
        };
        let has_modifier = hotkey.ctrl || hotkey.alt || hotkey.super_key || hotkey.shift;
        let kc = hotkey.code;
        let is_input_key = kc.is_character_key()
            || matches!(
                kc,
                KeyCode::Enter | KeyCode::Backspace | KeyCode::Tab | KeyCode::Delete
            );
        if is_input_key && !has_modifier {
            warnings.push(t!("atf_hotkey_warn_input_key", key = name.clone()).to_string());
        }
        // 중복 판정도 수정자까지 본다 — `Shift+F9` 같은 조합은 한자키 `F9` 와 공존한다.
        let dup = !has_modifier
            && config
                .engine
                .toggle_keys
                .iter()
                .chain(config.engine.hanja_keys.iter())
                .any(|k| KeyCode::from_name(k) == kc);
        if dup {
            warnings.push(t!("atf_hotkey_warn_duplicate", key = name.clone()).to_string());
        }
    }
    (warnings, any_unknown)
}

/// 한/영 전환키·한자키가 지정될 역할 — 경고 문구 선택과 중복 판정 대상 결정에 쓴다.
#[derive(Clone, Copy, PartialEq, Eq)]
enum SwitchKeyRole {
    Toggle,
    Hanja,
}

/// 한/영 전환키(`toggle_keys`)·한자키(`hanja_keys`) 목록을 검증해 경고 목록과
/// `Unknown` 포함 여부를 돌려준다.
///
/// 정책은 `atf_hotkey_warnings` 와 동일 — 차단하지 않고 경고만 내며, 호출부는
/// `any_unknown` 이면 성공 에코를 억제한다. 표기 해석은 엔진 `parse_switch_key` 를
/// 그대로 쓴다(CLI 자체 파싱 금지 — 엔진과 어긋나면 오탐이 난다).
///
/// * 파싱 불가 → `switch_key_warn_unknown`. 이 두 필드는 ATF 핫키와 달리 조합 표기
///   (`Ctrl+X`)를 지원하지 않으므로 문구도 전용이다.
/// * **다른 역할**(반대편 전환키 + 수정자 없는 ATF 토글 키)과 중복 →
///   `switch_key_warn_duplicate`. 같은 역할의 기존 목록은 지금 교체되는 값이므로
///   비교 대상에서 뺀다(호출부는 대입 **전**에 이 함수를 부른다).
fn switch_key_warnings(
    keys: &[String],
    role: SwitchKeyRole,
    config: &UnimConfig,
) -> (Vec<String>, bool) {
    use unim::input_engine::InputEngine;
    use unim::keycode::KeyCode;
    let mut warnings = Vec::new();
    let mut any_unknown = false;

    // 반대편 역할의 전환키 목록 (같은 역할은 지금 교체되므로 제외).
    let other_switch: &[String] = match role {
        SwitchKeyRole::Toggle => &config.engine.hanja_keys,
        SwitchKeyRole::Hanja => &config.engine.toggle_keys,
    };

    for name in keys {
        let Some(code) = InputEngine::parse_switch_key(name) else {
            any_unknown = true;
            warnings.push(t!("switch_key_warn_unknown", key = name.clone()).to_string());
            continue;
        };
        // 문자·편집 키 풋건 — ATF 의 `is_input_key` 와 동일 판정식.
        // ATF 와 달리 `has_modifier` 항이 없다: 전환키/한자키는 조합 표기를 지원하지 않아
        // (`parse_switch_key`) 애초에 수정자가 붙은 표기는 위에서 unknown 으로 걸러진다.
        // 수정자 키 자체(RightAlt/LeftShift 등)는 `is_character_key()` 가 false 이고
        // 편집 키 목록에도 없어 오탐되지 않는다 — 출하 기본값 `["Korean", "RightAlt"]`,
        // `["Hanja", "F9"]` 는 경고 0 (기존 회귀 가드 테스트가 고정).
        let is_input_key = code.is_character_key()
            || matches!(
                code,
                KeyCode::Enter | KeyCode::Backspace | KeyCode::Tab | KeyCode::Delete
            );
        if is_input_key {
            warnings.push(t!("switch_key_warn_input_key", key = name.clone()).to_string());
        }
        // 반대편 전환키와의 중복.
        let dup_switch = other_switch
            .iter()
            .any(|k| InputEngine::parse_switch_key(k) == Some(code));
        // ATF 토글 핫키와의 중복 — 수정자가 붙은 조합(`Shift+F9`)은 맨 키와 갈리므로 제외.
        let dup_atf = config
            .engine
            .auto_typefix
            .toggle_enabled_keys
            .iter()
            .chain(config.engine.auto_typefix.toggle_forward_keys.iter())
            .chain(config.engine.auto_typefix.toggle_reverse_keys.iter())
            .filter_map(|k| InputEngine::parse_atf_hotkey(k))
            .any(|h| {
                h.code == code && !(h.ctrl || h.alt || h.super_key || h.shift)
            });
        if dup_switch || dup_atf {
            warnings.push(t!("switch_key_warn_duplicate", key = name.clone()).to_string());
        }
    }
    (warnings, any_unknown)
}

/// 자동 영문 전환 트리거 표기를 검증해 경고 목록과 무효 표기 포함 여부를 돌려준다.
///
/// 표기 체계(`key:`/`char:`/legacy)가 달라 전용 문구를 쓴다 — ATF 문구를 재활용하면
/// 사용자를 오도한다. 판정은 엔진 `is_valid_auto_english_key` 단일 진실 공급원.
/// 중복·풋건 판정은 하지 않는다(문자 트리거 `char:/` 가 정상 용법이므로 풋건 개념이
/// 성립하지 않는다).
fn auto_english_key_warnings(keys: &[String]) -> (Vec<String>, bool) {
    use unim::input_engine::InputEngine;
    let mut warnings = Vec::new();
    let mut any_unknown = false;
    for name in keys {
        if !InputEngine::is_valid_auto_english_key(name) {
            any_unknown = true;
            warnings.push(t!("auto_english_key_warn_unknown", key = name.clone()).to_string());
        }
    }
    (warnings, any_unknown)
}

/// 목록의 **모든** 항목이 파서에서 거부되는지 — 필수 키 목록이 통째로 비는 저장을 차단.
///
/// 부분 무효는 경고 후 저장(warn-not-block)하지만, 전 항목 무효는 엔진 파서가 전부
/// 걸러 빈 목록 저장과 결과가 같아진다(예: toggle_keys 전무효 → 한/영 전환 불능).
/// 명시적 빈 목록이 "At least one key required" 하드 에러인 것과 동일하게 반려한다.
/// ATF 토글 3종은 빈 목록이 정상 옵트아웃이므로 이 검사를 적용하지 않는다.
fn all_keys_invalid(keys: &[String], is_valid: impl Fn(&str) -> bool) -> bool {
    !keys.is_empty() && keys.iter().all(|k| !is_valid(k))
}

fn config_set(key: ConfigKey, value: &str) -> Result<(), String> {
    let mut config = UnimConfig::load_from_default_path();

    match key {
        ConfigKey::KoreanLayout => {
            let normalized = normalize_korean_layout_name(value);
            // ProfileRegistry 기반 검증 — `unim layout list`(동일 registry)와 통일.
            // 내장 한글 자판(ko_3bul_anmatae 등)뿐 아니라 사용자 한글 프로필까지 수용하고,
            // 정적 KOREAN_LAYOUT_BUILTINS 화이트리스트의 자기모순(list엔 보이나 set은 거부)을
            // 해소한다. 프로필 language 메타데이터가 "korean"이면 허용.
            let reg = ProfileRegistry::new();
            let is_korean = |name: &str| {
                reg.find_raw(name)
                    .map(|p| p.language == "korean")
                    .unwrap_or(false)
            };
            if !is_korean(&normalized) {
                let allowed: Vec<String> = reg
                    .list_names()
                    .into_iter()
                    .filter(|n| is_korean(n))
                    .collect();
                let kind = t!("korean_layout_label").to_string();
                return Err(t!(
                    "error_invalid_layout",
                    kind = kind,
                    value = value,
                    allowed = allowed.join(", ")
                )
                .to_string());
            }
            let kind = t!("korean_layout_label").to_string();
            println!(
                "{}",
                t!("layout_changed", kind = kind, layout = normalized.as_str())
            );
            // GTK 설정과 동일한 캐시 동작 — 이전 자판의 active_rule_sets 보존,
            // 새 자판의 캐시된 값(있으면) 복원. 사용자가 자판을 왕복해도 룰셋 ON/OFF
            // 의도가 잃지 않는다. (CLI는 프로필 객체가 없어 stale 정리는 생략 — daemon이
            // 다음 로드 시 처리.)
            config.engine.korean.switch_layout(&normalized, None);
        }
        ConfigKey::EnglishLayout => {
            let normalized = normalize_english_layout_name(value);
            if !ENGLISH_LAYOUT_BUILTINS.contains(&normalized.as_str()) {
                let kind = t!("english_layout_label").to_string();
                return Err(t!(
                    "error_invalid_layout",
                    kind = kind,
                    value = value,
                    allowed = ENGLISH_LAYOUT_BUILTINS.join(", ")
                )
                .to_string());
            }
            let kind = t!("english_layout_label").to_string();
            println!(
                "{}",
                t!("layout_changed", kind = kind, layout = normalized.as_str())
            );
            config.engine.english.layout = normalized;
        }
        ConfigKey::KoreanActiveRuleSets => {
            // value 의미 (engine `LayoutProfile.active_rule_sets`와 동치):
            //   "default" sentinel → None, 프로필 기본값 사용
            //   "" (빈 문자열)      → Some(vec![]), 사용자 명시적 All OFF
            //   "a,b,c"           → Some(vec!["a","b","c"]), 명시 활성 목록
            let trimmed = value.trim();
            let (new_value, display) = if trimmed.eq_ignore_ascii_case("default") {
                (None, t!("profile_default").to_string())
            } else {
                let names: Vec<String> = trimmed
                    .split(',')
                    .map(|s| s.trim().to_string())
                    .filter(|s| !s.is_empty())
                    .collect();
                let disp = if names.is_empty() {
                    t!("korean_active_rule_sets_all_off").to_string()
                } else {
                    names.join(", ")
                };
                (Some(names), disp)
            };
            config.engine.korean.active_rule_sets = new_value;
            // 현재 자판의 캐시도 동기화 — 다음 자판 전환 시 이 상태가 보존된다.
            // None일 때 캐시 entry는 제거되어 "프로필 기본값" 의도가 보존된다.
            config.engine.korean.cache_active_rule_sets();
            println!("{}: {}", t!("korean_active_rule_sets_label"), display);
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
            println!(
                "{}",
                t!(
                    "mode_sharing_changed",
                    mode = mode_sharing_display_name_localized(mode)
                )
            );
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
            if all_keys_invalid(&keys, |k| {
                unim::input_engine::InputEngine::parse_switch_key(k).is_some()
            }) {
                return Err(t!("error_no_valid_key").to_string());
            }
            // 경고 계산은 대입 **전** — 반대편 역할(한자키) 비교가 구값 기준으로 성립한다.
            let (warnings, any_unknown) =
                switch_key_warnings(&keys, SwitchKeyRole::Toggle, &config);
            config.engine.toggle_keys = keys;
            for w in &warnings {
                eprintln!("{}", w);
            }
            // 미지 키가 있으면 성공 에코를 억제(오타를 성공으로 오인 방지 — ATF 3키와 동일 정책).
            if !any_unknown {
                println!(
                    "{}: {}",
                    t!("toggle_keys_label"),
                    config.engine.toggle_keys.join(", ")
                );
            }
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
            if all_keys_invalid(&keys, |k| {
                unim::input_engine::InputEngine::parse_switch_key(k).is_some()
            }) {
                return Err(t!("error_no_valid_key").to_string());
            }
            let (warnings, any_unknown) = switch_key_warnings(&keys, SwitchKeyRole::Hanja, &config);
            config.engine.hanja_keys = keys;
            for w in &warnings {
                eprintln!("{}", w);
            }
            if !any_unknown {
                println!(
                    "{}: {}",
                    t!("hanja_keys_label"),
                    config.engine.hanja_keys.join(", ")
                );
            }
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
                "{}",
                t!(
                    "typefix_forward_status",
                    state = if enabled { "ON" } else { "OFF" }
                )
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
                "{}",
                t!(
                    "typefix_reverse_status",
                    state = if enabled { "ON" } else { "OFF" }
                )
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
        ConfigKey::AutoTypeFixUserDictEnabled => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.user_dict_enabled = enabled;
            println!(
                "{}: {}",
                t!("auto_typefix_user_dict_enabled_label"),
                if enabled { "ON" } else { "OFF" }
            );
        }
        // 오타 교정 토글 단축키 3종 — toggle-keys 와 달리 빈 목록 허용(옵트인, 비우면 사용 안 함).
        ConfigKey::AutoTypeFixToggleKeys => {
            let keys: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let (warnings, any_unknown) = atf_hotkey_warnings(&keys, &config);
            config.engine.auto_typefix.toggle_enabled_keys = keys;
            for w in &warnings {
                eprintln!("{}", w);
            }
            // Unknown 키가 있으면 성공 에코를 억제(오타/미지원을 성공으로 오인 방지).
            if !any_unknown {
                println!(
                    "{}: {}",
                    t!("auto_typefix_toggle_keys_label"),
                    display_keys(&config.engine.auto_typefix.toggle_enabled_keys)
                );
            }
        }
        ConfigKey::AutoTypeFixForwardToggleKeys => {
            let keys: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let (warnings, any_unknown) = atf_hotkey_warnings(&keys, &config);
            config.engine.auto_typefix.toggle_forward_keys = keys;
            for w in &warnings {
                eprintln!("{}", w);
            }
            if !any_unknown {
                println!(
                    "{}: {}",
                    t!("auto_typefix_forward_toggle_keys_label"),
                    display_keys(&config.engine.auto_typefix.toggle_forward_keys)
                );
            }
        }
        ConfigKey::AutoTypeFixReverseToggleKeys => {
            let keys: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            let (warnings, any_unknown) = atf_hotkey_warnings(&keys, &config);
            config.engine.auto_typefix.toggle_reverse_keys = keys;
            for w in &warnings {
                eprintln!("{}", w);
            }
            if !any_unknown {
                println!(
                    "{}: {}",
                    t!("auto_typefix_reverse_toggle_keys_label"),
                    display_keys(&config.engine.auto_typefix.toggle_reverse_keys)
                );
            }
        }
        ConfigKey::AutoEnglish => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid value for auto-english: {}", value)),
            };
            config.engine.auto_english.enabled = enabled;
            let status = if enabled {
                t!("enabled")
            } else {
                t!("disabled")
            };
            println!("{}", t!("auto_english_changed", status = status));
        }
        ConfigKey::AutoEnglishKeys => {
            let keys: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if keys.is_empty() {
                return Err("At least one key required".to_string());
            }
            if all_keys_invalid(&keys, |k| {
                unim::input_engine::InputEngine::is_valid_auto_english_key(k)
            }) {
                return Err(t!("error_no_valid_key").to_string());
            }
            let (warnings, any_unknown) = auto_english_key_warnings(&keys);
            config.engine.auto_english.trigger_keys = keys;
            for w in &warnings {
                eprintln!("{}", w);
            }
            if !any_unknown {
                println!(
                    "{}: {}",
                    t!("auto_english_keys_label"),
                    config.engine.auto_english.trigger_keys.join(", ")
                );
            }
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
        ConfigKey::KoreanBidirectionalCombine => {
            if value.is_empty() {
                config.engine.korean.bidirectional_combine = None;
                println!("korean.bidirectional_combine: 미설정 (OFF)");
            } else {
                let enabled: bool = value
                    .parse()
                    .map_err(|_| "Invalid value, use true/false (or empty to reset)".to_string())?;
                config.engine.korean.bidirectional_combine = Some(enabled);
                let status = if enabled { "활성화" } else { "비활성화" };
                println!("korean.bidirectional_combine: {status}");
            }
        }
        ConfigKey::KoreanChordWindowMs => {
            if value.is_empty() {
                config.engine.korean.chord_window_ms = None;
                println!("korean.chord_window_ms: 미설정 (모아치기 OFF)");
            } else {
                let ms: u16 = value
                    .parse()
                    .map_err(|_| "Invalid value, use 0 (OFF) or 10-200 ms (or empty to reset)".to_string())?;
                KoreanConfig::validate_chord_window_ms(Some(ms))?;
                config.engine.korean.chord_window_ms = Some(ms);
                println!("korean.chord_window_ms: {ms}ms (범위 10-200, 0=OFF)");
            }
        }
        ConfigKey::CommitUnit => {
            // ModeSharing 패턴 미러 — 영문 키워드 + 한국어 별칭 수용.
            let unit = match value.to_lowercase().as_str() {
                "syllable" | "음절" => CommitUnit::Syllable,
                "word" | "단어" => CommitUnit::Word,
                "smart" | "스마트" => CommitUnit::Smart,
                _ => {
                    return Err(t!(
                        "error_invalid_commit_unit",
                        value = value,
                        allowed = "syllable, word, smart"
                    )
                    .to_string());
                }
            };
            config.engine.korean.commit_unit = unit;
            println!(
                "{}",
                t!(
                    "commit_unit_changed",
                    unit = commit_unit_display_name_localized(unit)
                )
            );
        }
        ConfigKey::WordModeApps => {
            // 실행 파일명 정확일치 목록. 빈 문자열/빈 목록도 유효(= Smart 게이트가 어떤
            // 앱도 단어 모드로 켜지 않음). toggle-keys 와 달리 최소 1개 강제 없음.
            let apps: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            config.engine.korean.word_mode_apps = apps;
            println!(
                "{}: {}",
                t!("word_mode_apps_label"),
                config.engine.korean.word_mode_apps.join(", ")
            );
        }
        ConfigKey::ToggleAnnounceBeep => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.toggle_announce_beep = enabled;
            println!(
                "{}: {}",
                t!("toggle_announce_beep_label"),
                if enabled { t!("enabled") } else { t!("disabled") }
            );
        }
        ConfigKey::IgnoreKeyRepeat => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.ignore_key_repeat = enabled;
            println!(
                "{}: {}",
                t!("ignore_key_repeat_label"),
                if enabled { t!("enabled") } else { t!("disabled") }
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
    // 저장 직전 재로드 비교용 스냅샷(GAP-config-05) — 세션 시작 이후 외부에서 파일이
    // 바뀌었는지 판정한다. Config 는 PartialEq 를 파생하지 않으므로 Debug 문자열로 비교.
    let session_start_snapshot = format!("{:?}", config.engine);
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
                let layouts = KOREAN_LAYOUT_BUILTINS;
                let layout_names: Vec<String> = layouts
                    .iter()
                    .map(|n| korean_layout_display_name_localized(n))
                    .collect();
                let current_idx = layouts
                    .iter()
                    .position(|n| *n == config.engine.korean.layout.as_str())
                    .unwrap_or(0);
                let s = Select::with_theme(&theme)
                    .with_prompt(t!("select_korean_layout").to_string())
                    .items(&layout_names)
                    .default(current_idx)
                    .interact()
                    .unwrap();

                config.engine.korean.layout = layouts[s].to_string();
            }
            1 => {
                let layouts = ENGLISH_LAYOUT_BUILTINS;
                let layout_names: Vec<&str> = layouts
                    .iter()
                    .map(|n| english_layout_display_name(n))
                    .collect();
                let current_idx = layouts
                    .iter()
                    .position(|n| *n == config.engine.english.layout.as_str())
                    .unwrap_or(0);
                let s = Select::with_theme(&theme)
                    .with_prompt(t!("select_english_layout").to_string())
                    .items(&layout_names)
                    .default(current_idx)
                    .interact()
                    .unwrap();

                config.engine.english.layout = layouts[s].to_string();
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
                let mode_names: Vec<String> = modes
                    .iter()
                    .map(|m| mode_sharing_display_name_localized(*m))
                    .collect();
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
                // 저장 직전 재로드 + diff 경고(GAP-config-05) — 대화형 세션이 열려 있는
                // 동안 외부(다른 unim-cli 인스턴스, 데몬, 설정 GUI 등)에서 파일이 바뀌면
                // 이 세션의 저장이 그 변경을 그대로 덮어쓴다. 병합은 하지 않고 경고만 한다.
                let latest_on_disk = UnimConfig::load_from_default_path();
                if format!("{:?}", latest_on_disk.engine) != session_start_snapshot {
                    eprintln!("{}", t!("interactive_external_change_warning"));
                }
                if let Err(e) = config.save_to_default_path() {
                    eprintln!("{}", t!("execution_error", error = e.to_string()));
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
                eprintln!("{}", t!("execution_error", error = e));
                process::exit(1);
            }
        }
        Some(ConfigCommands::Path) => config_path(),
        Some(ConfigCommands::Reset) => {
            if let Err(e) = config_reset() {
                eprintln!("{}", t!("execution_error", error = e));
                process::exit(1);
            }
        }
        Some(ConfigCommands::Interactive) => config_interactive(),
        Some(ConfigCommands::Layout { action }) => match action {
            LayoutAction::List => layout_list(),
            LayoutAction::Describe { name } => layout_describe(&name),
            LayoutAction::Validate { file } => {
                let code = layout_validate(&file);
                process::exit(code);
            }
        },
        Some(ConfigCommands::UserDict { action }) => match action {
            UserDictCommand::List => user_dict_list(),
            UserDictCommand::Add { word, note } => {
                if let Err(e) = user_dict_add(&word, note) {
                    eprintln!("{}: {}", t!("error_label"), e);
                    process::exit(1);
                }
            }
            UserDictCommand::Remove { word } => {
                if let Err(e) = user_dict_remove(&word) {
                    eprintln!("{}: {}", t!("error_label"), e);
                    process::exit(1);
                }
            }
            UserDictCommand::Clear => {
                if let Err(e) = user_dict_clear() {
                    eprintln!("{}: {}", t!("error_label"), e);
                    process::exit(1);
                }
            }
            UserDictCommand::Path => user_dict_path(),
        },
        Some(ConfigCommands::Blacklist { action }) => handle_blacklist(action),
        None => {
            config_show();
            println!("\n{}", t!("help_hint"));
        }
    }
}

// ============================================================================
// User Dictionary 서브커맨드 (PR #6)
// ============================================================================

fn user_dict_list() {
    let ud = UserDictionary::load_from_default_path();
    if ud.is_empty() {
        println!("{}", t!("user_dict_empty"));
        if let Some(path) = UserDictionary::default_path() {
            println!("({}: {})", t!("user_dict_path_label"), path.display());
        }
        return;
    }
    println!("{} ({}):", t!("user_dict_list_title"), ud.len());
    println!("{}", "-".repeat(60));
    for (i, e) in ud.reverse_words.iter().enumerate() {
        let note = e.note.as_deref().unwrap_or("");
        if note.is_empty() {
            println!("  {:>3}. {}", i + 1, e.word);
        } else {
            println!("  {:>3}. {:<16}  — {}", i + 1, e.word, note);
        }
    }
}

fn user_dict_add(word: &str, note: Option<String>) -> Result<(), String> {
    let mut ud = UserDictionary::load_from_default_path();
    if !ud.add(word, note) {
        return Err(t!("user_dict_add_failed", word = word.to_string()).to_string());
    }
    ud.save_to_default_path()
        .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
    println!("{}", t!("user_dict_added", word = word.to_string()));
    Ok(())
}

fn user_dict_remove(word: &str) -> Result<(), String> {
    let mut ud = UserDictionary::load_from_default_path();
    if !ud.remove_by_word(word) {
        return Err(t!("user_dict_not_found", word = word.to_string()).to_string());
    }
    ud.save_to_default_path()
        .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
    println!("{}", t!("user_dict_removed", word = word.to_string()));
    Ok(())
}

fn user_dict_clear() -> Result<(), String> {
    let mut ud = UserDictionary::load_from_default_path();
    if ud.is_empty() {
        println!("{}", t!("user_dict_empty"));
        return Ok(());
    }
    let count = ud.len();
    let theme = ColorfulTheme::default();
    let ok = Confirm::with_theme(&theme)
        .with_prompt(t!("user_dict_clear_confirm", count = count.to_string()).to_string())
        .default(false)
        .interact()
        .unwrap_or(false);
    if !ok {
        println!("{}", t!("exit_canceled"));
        return Ok(());
    }
    ud.clear();
    ud.save_to_default_path()
        .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
    println!("{}", t!("user_dict_cleared", count = count.to_string()));
    Ok(())
}

fn user_dict_path() {
    if let Some(path) = UserDictionary::default_path() {
        println!("{}", path.display());
    } else {
        eprintln!("{}", t!("error_path_not_found"));
    }
}

// ============================================================================
// Blacklist 서브커맨드 (AutoTypeFix 학습형 오탐 억제 목록)
// ============================================================================

fn handle_blacklist(action: BlacklistCommand) {
    match action {
        BlacklistCommand::List => blacklist_list(),
        BlacklistCommand::Remove { index } => {
            if let Err(e) = blacklist_remove(index) {
                eprintln!("{}: {}", t!("error_label"), e);
                process::exit(1);
            }
        }
        BlacklistCommand::Clear => {
            if let Err(e) = blacklist_clear() {
                eprintln!("{}: {}", t!("error_label"), e);
                process::exit(1);
            }
        }
        BlacklistCommand::Path => blacklist_path(),
    }
}

fn blacklist_list() {
    let bl = Blacklist::load_from_default_path();
    if bl.entries.is_empty() {
        println!("{}", t!("blacklist_empty"));
        if let Some(path) = Blacklist::default_path() {
            println!("({}: {})", t!("blacklist_path_label"), path.display());
        }
        return;
    }
    println!("{} ({}):", t!("blacklist_list_title"), bl.entries.len());
    println!("{}", "-".repeat(80));
    for (i, e) in bl.entries.iter().enumerate() {
        println!(
            "  {:>3}. [{:?}] [{:?}] {} | {} / {} | hits:{}",
            i + 1,
            e.status,
            e.direction,
            e.ascii,
            e.korean_layout,
            e.english_layout,
            e.hit_count
        );
    }
}

fn blacklist_remove(index: usize) -> Result<(), String> {
    if index == 0 {
        return Err(t!("blacklist_index_one_based").to_string());
    }
    let mut bl = Blacklist::load_from_default_path();
    let idx = index - 1;
    if idx >= bl.entries.len() {
        return Err(t!(
            "blacklist_index_out_of_range",
            max = bl.entries.len().to_string()
        )
        .to_string());
    }
    bl.remove(idx);
    bl.save_to_default_path()
        .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
    println!("{}", t!("blacklist_removed", index = index.to_string()));
    Ok(())
}

fn blacklist_clear() -> Result<(), String> {
    let mut bl = Blacklist::load_from_default_path();
    if bl.entries.is_empty() {
        println!("{}", t!("blacklist_empty"));
        return Ok(());
    }
    let count = bl.entries.len();
    let theme = ColorfulTheme::default();
    let ok = Confirm::with_theme(&theme)
        .with_prompt(t!("blacklist_clear_confirm", count = count.to_string()).to_string())
        .default(false)
        .interact()
        .unwrap_or(false);
    if !ok {
        println!("{}", t!("exit_canceled"));
        return Ok(());
    }
    bl.entries.clear();
    bl.save_to_default_path()
        .map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
    println!("{}", t!("blacklist_cleared", count = count.to_string()));
    Ok(())
}

fn blacklist_path() {
    if let Some(path) = Blacklist::default_path() {
        println!("{}", path.display());
    } else {
        eprintln!("{}", t!("error_path_not_found"));
    }
}

// ============================================================================
// Layout 서브커맨드 (자판 프로필 관리)
// ============================================================================

fn layout_list() {
    let reg = ProfileRegistry::new();
    let names = reg.list_names();

    println!("{}", t!("layout_list_title"));
    if let Some(dir) = reg.user_dir() {
        println!("  {}: {}", t!("layout_user_dir_label"), dir.display());
    }
    println!();

    let locale = rust_i18n::locale().to_string();
    for name in &names {
        let source_label = if reg.is_user_override(name) {
            t!("layout_source_user")
        } else {
            t!("layout_source_builtin")
        };
        let display = reg
            .find_raw(name)
            .map(|p| {
                let dn = p
                    .metadata
                    .display_name
                    .as_ref()
                    .map(|d| d.resolve(&locale).to_string())
                    .unwrap_or_else(|| name.clone());
                format!("{dn} [{}]", p.layout_type)
            })
            .unwrap_or_else(|| name.clone());

        println!("  [{}] {:<20} {}", source_label, name, display);
    }
    println!();
    println!("{}: {}", t!("layout_total_label"), names.len());
}

fn layout_describe(name: &str) {
    let reg = ProfileRegistry::new();
    let raw = match reg.find_raw(name) {
        Some(p) => p,
        None => {
            eprintln!(
                "{}: {}",
                t!("error_label"),
                t!("layout_not_found", name = name)
            );
            process::exit(1);
        }
    };

    let profile = match resolve_inherits(&raw, &reg) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "{}: {}",
                t!("error_label"),
                t!("layout_resolve_failed", err = e.to_string())
            );
            process::exit(2);
        }
    };

    println!("{}: {}", t!("layout_name_label"), profile.name);
    println!("{}: v{}", t!("layout_schema_label"), profile.schema_version);
    println!("{}: {}", t!("layout_type_label"), profile.layout_type);
    println!("{}: {}", t!("layout_language_label"), profile.language);

    let locale = rust_i18n::locale().to_string();
    if let Some(dn) = profile.metadata.display_name.as_ref() {
        println!(
            "{}: {}",
            t!("layout_display_name_label"),
            dn.resolve(&locale)
        );
    }
    if let Some(author) = profile.metadata.author.as_deref() {
        println!("{}: {}", t!("layout_author_label"), author);
    }
    if let Some(ver) = profile.metadata.version.as_deref() {
        println!("{}: {}", t!("layout_version_label"), ver);
    }
    if let Some(license) = profile.metadata.license.as_deref() {
        println!("{}: {}", t!("layout_license_label"), license);
    }
    if let Some(desc) = profile.metadata.description.as_ref() {
        println!(
            "{}: {}",
            t!("layout_description_label"),
            desc.resolve(&locale)
        );
    }
    if !profile.metadata.tags.is_empty() {
        println!(
            "{}: {}",
            t!("layout_tags_label"),
            profile.metadata.tags.join(", ")
        );
    }

    println!();
    if let Some(combos) = profile.combinations.as_ref() {
        println!(
            "{}: cho {}, jung {}, jong {}",
            t!("layout_combinations_label"),
            combos.cho.len(),
            combos.jung.len(),
            combos.jong.len()
        );
    } else {
        println!(
            "{}: {}",
            t!("layout_combinations_label"),
            t!("layout_combinations_fallback")
        );
    }

    if profile.rule_sets.is_empty() {
        println!("{}: {}", t!("layout_rule_sets_label"), t!("not_set"));
    } else {
        println!("{}:", t!("layout_rule_sets_label"));
        for (set_name, rs) in &profile.rule_sets {
            let active_mark = if rs.active { "✓" } else { "·" };
            let desc = rs
                .description
                .as_ref()
                .map(|d| format!(" — {}", d.resolve(&locale)))
                .unwrap_or_default();
            println!(
                "  {} {} ({} combinations){}",
                active_mark,
                set_name,
                rs.combinations.len(),
                desc
            );
        }
    }

    if let Some(active) = profile.active_rule_sets.as_ref() {
        println!("{}: {}", t!("layout_active_sets_label"), active.join(", "));
    }
}

/// Validate result code: 0=ok, 1=warnings only, 2=errors.
fn layout_validate(file: &std::path::Path) -> i32 {
    let content = match std::fs::read_to_string(file) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "{}: {}",
                t!("error_label"),
                t!(
                    "layout_validate_read_err",
                    file = file.display().to_string(),
                    err = e.to_string()
                )
            );
            return 2;
        }
    };

    let profile = match parse_profile_str(&content) {
        Ok(p) => p,
        Err(e) => {
            eprintln!(
                "{}: {}",
                t!("error_label"),
                t!("layout_validate_parse_err", err = e.to_string())
            );
            return 2;
        }
    };

    let mut errors: Vec<String> = Vec::new();
    let mut warnings: Vec<String> = Vec::new();

    let resolved = if profile.inherits.is_some() {
        let reg = ProfileRegistry::new();
        match resolve_inherits(&profile, &reg) {
            Ok(p) => p,
            Err(e) => {
                warnings
                    .push(t!("layout_validate_inherits_warn", err = e.to_string()).into_owned());
                profile.clone()
            }
        }
    } else {
        profile.clone()
    };

    if let Err(e) = build_combined_jamo_map(&resolved) {
        errors.push(t!("layout_validate_combinations_err", err = format!("{e:?}")).into_owned());
    }

    if let Some(active) = resolved.active_rule_sets.as_ref() {
        for name in active {
            if !resolved.rule_sets.contains_key(name) {
                warnings
                    .push(t!("layout_validate_unknown_set", name = name.to_string()).into_owned());
            }
        }
    }

    println!("{}: {}", t!("layout_validate_file_label"), file.display());
    println!(
        "{}: {} (v{})",
        t!("layout_name_label"),
        resolved.name,
        resolved.schema_version
    );

    if errors.is_empty() && warnings.is_empty() {
        println!("{}", t!("layout_validate_ok"));
        return 0;
    }

    for w in &warnings {
        println!("  {} {}", t!("layout_validate_warn_prefix"), w);
    }
    for e in &errors {
        eprintln!("  {} {}", t!("layout_validate_err_prefix"), e);
    }

    if !errors.is_empty() {
        eprintln!(
            "{} ({} errors, {} warnings)",
            t!("layout_validate_failed"),
            errors.len(),
            warnings.len()
        );
        2
    } else {
        println!(
            "{} ({} warnings)",
            t!("layout_validate_warnings_only"),
            warnings.len()
        );
        1
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// Daemon query path
// ─────────────────────────────────────────────────────────────────────────────

/// 데몬의 `GetActiveFrontends` RPC를 호출하여 현재 등록된 프런트엔드 목록 출력.
async fn run_daemon_frontends() -> Result<(), String> {
    let client = unim_dbus::client::UnimClient::connect()
        .await
        .map_err(|e| format!("daemon unreachable: {}", e))?;
    let im = client
        .input_method()
        .await
        .map_err(|e| format!("input_method proxy failed: {}", e))?;
    let frontends = im
        .get_active_frontends()
        .await
        .map_err(|e| format!("GetActiveFrontends failed: {}", e))?;
    if frontends.is_empty() {
        println!("(none)");
    } else {
        for name in &frontends {
            println!("{}", name);
        }
    }
    Ok(())
}

// Trigger path — universal entry point for OS shortcut tools (KDE, Hyprland, AHK ...)
// ─────────────────────────────────────────────────────────────────────────────

/// 데몬의 `org.atit.unim.InputMethod.TriggerAction` RPC를 호출.
///
/// GNOME extension처럼 자체 InputContext를 가지는 클라이언트와 달리, CLI는
/// 어느 InputContext인지 알 수 없으므로 InputMethod-level RPC를 사용한다.
/// 데몬은 마지막으로 보고된 cursor 좌표로 `ShowEmojiPopup` 시그널을 발행한다.
async fn run_trigger(action: &str) -> Result<(), String> {
    let client = unim_dbus::client::UnimClient::connect()
        .await
        .map_err(|e| t!("trigger_error_daemon_unreachable", err = e.to_string()).to_string())?;
    let im = client
        .input_method()
        .await
        .map_err(|e| t!("trigger_error_daemon_unreachable", err = e.to_string()).to_string())?;
    im.trigger_action(action).await.map_err(|e| {
        t!(
            "trigger_error_action_failed",
            action = action,
            err = e.to_string()
        )
        .to_string()
    })?;
    println!("{}", t!("trigger_success", action = action));
    Ok(())
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
        Some(Commands::Trigger { action }) => {
            // tokio 런타임은 trigger 호출 시에만 빌드 (변환·config 경로는 sync 유지)
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| io::Error::other(format!("tokio runtime build failed: {}", e)))?;
            if let Err(e) = runtime.block_on(run_trigger(&action)) {
                eprintln!("{}", t!("execution_error", error = e));
                process::exit(1);
            }
            Ok(())
        }
        Some(Commands::Daemon { command }) => {
            let runtime = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| io::Error::other(format!("tokio runtime build failed: {}", e)))?;
            match command {
                DaemonCommands::Frontends => {
                    if let Err(e) = runtime.block_on(run_daemon_frontends()) {
                        eprintln!("{}", t!("execution_error", error = e));
                        process::exit(1);
                    }
                }
            }
            Ok(())
        }
        None => {
            let config = ConvertConfig::from_cli(&cli);

            for warning in &config.warnings {
                eprintln!("{}", t!("warning_label", warning = warning));
            }

            if let Err(e) = run_convert(config) {
                eprintln!("{}", t!("execution_error", error = e.to_string()));
                process::exit(1);
            }

            Ok(())
        }
    }
}

// ─────────────────────────────────────────────────────────────────────────────
// 단축키 성격 필드 검증 헬퍼 단위 테스트.
//
// 판정은 전부 엔진 파서(단일 진실 공급원)에 위임하므로, 여기서 고정하는 것은
// **CLI 정책**이다: 차단하지 않고 경고만 내되 미지 표기가 있으면 성공 에코를
// 억제한다(`any_unknown`), 중복 판정은 "다른 역할"만 본다.
// ─────────────────────────────────────────────────────────────────────────────
#[cfg(test)]
mod tests {
    use super::*;

    fn keys(xs: &[&str]) -> Vec<String> {
        xs.iter().map(|s| s.to_string()).collect()
    }

    #[test]
    fn switch_key_warnings_clean_for_defaults() {
        // 출하 기본값은 경고 0 — 신규 설치가 경고를 뱉으면 안 된다.
        // `RightAlt`(수정자 키 자체)가 유효해야 성립한다(ATF 파서 재사용 금지 회귀 가드).
        let config = UnimConfig::default();
        let (w, unknown) = switch_key_warnings(
            &keys(&["Korean", "RightAlt"]),
            SwitchKeyRole::Toggle,
            &config,
        );
        assert!(w.is_empty(), "기본 toggle_keys 경고 없음: {w:?}");
        assert!(!unknown);

        let (w, unknown) =
            switch_key_warnings(&keys(&["Hanja", "F9"]), SwitchKeyRole::Hanja, &config);
        assert!(w.is_empty(), "기본 hanja_keys 경고 없음: {w:?}");
        assert!(!unknown);
    }

    #[test]
    fn switch_key_warnings_flags_unknown_and_suppresses_echo() {
        let config = UnimConfig::default();
        let (w, unknown) =
            switch_key_warnings(&keys(&["RigthAlt"]), SwitchKeyRole::Toggle, &config);
        assert_eq!(w.len(), 1, "오타 키 1건 경고: {w:?}");
        assert!(unknown, "성공 에코 억제 플래그");
    }

    #[test]
    fn switch_key_warnings_flags_combo_notation() {
        // 전환키/한자키는 조합 표기를 지원하지 않는다 — ATF 핫키와 다른 문법.
        let config = UnimConfig::default();
        let (w, unknown) =
            switch_key_warnings(&keys(&["Shift+F9"]), SwitchKeyRole::Hanja, &config);
        assert_eq!(w.len(), 1, "조합 표기는 미지 이름: {w:?}");
        assert!(unknown);
    }

    #[test]
    fn switch_key_warnings_detects_cross_role_duplicate() {
        // 기본 hanja_keys 에 F9 가 있으므로 toggle 로 F9 를 주면 역할 충돌 경고.
        let config = UnimConfig::default();
        let (w, unknown) = switch_key_warnings(&keys(&["F9"]), SwitchKeyRole::Toggle, &config);
        assert_eq!(w.len(), 1, "중복 경고 1건: {w:?}");
        assert!(!unknown, "중복은 유효 표기 — 에코 억제 대상 아님");
    }

    #[test]
    fn switch_key_warnings_ignores_same_role_list() {
        // 같은 역할의 기존 목록은 지금 교체되는 값이므로 중복 판정에서 뺀다.
        let mut config = UnimConfig::default();
        config.engine.toggle_keys = keys(&["Korean"]);
        config.engine.hanja_keys = keys(&["Hanja"]);
        let (w, unknown) = switch_key_warnings(&keys(&["Korean"]), SwitchKeyRole::Toggle, &config);
        assert!(w.is_empty(), "자기 역할 중복은 경고 아님: {w:?}");
        assert!(!unknown);
    }

    #[test]
    fn switch_key_warnings_detects_bare_atf_duplicate() {
        // 수정자 없는 ATF 토글 키와 겹치면 충돌 경고, 조합 표기(Shift+F8)는 아니다.
        let mut config = UnimConfig::default();
        config.engine.auto_typefix.toggle_enabled_keys = keys(&["F10"]);
        config.engine.auto_typefix.toggle_forward_keys = keys(&["Shift+F11"]);
        config.engine.auto_typefix.toggle_reverse_keys = Vec::new();

        let (w, _) = switch_key_warnings(&keys(&["F10"]), SwitchKeyRole::Toggle, &config);
        assert_eq!(w.len(), 1, "맨 F10 은 ATF 토글과 충돌: {w:?}");

        let (w, _) = switch_key_warnings(&keys(&["F11"]), SwitchKeyRole::Toggle, &config);
        assert!(w.is_empty(), "Shift+F11 조합은 맨 F11 과 갈린다: {w:?}");
    }

    #[test]
    fn switch_key_warnings_flags_character_key_footgun() {
        let config = UnimConfig::default();
        let (w, unknown) = switch_key_warnings(&keys(&["A"]), SwitchKeyRole::Toggle, &config);
        assert_eq!(w.len(), 1, "문자 키는 풋건 경고: {w:?}");
        assert!(!unknown, "유효 표기 — 성공 에코 억제 대상 아님");
    }

    #[test]
    fn switch_key_warnings_flags_space_and_edit_keys() {
        let config = UnimConfig::default();
        for k in ["Space", "Enter", "Backspace", "Tab", "Delete"] {
            let (w, _) = switch_key_warnings(&keys(&[k]), SwitchKeyRole::Toggle, &config);
            assert_eq!(w.len(), 1, "{k} 는 풋건 경고 대상: {w:?}");
        }
    }

    #[test]
    fn switch_key_warnings_no_footgun_for_modifier_or_function_keys() {
        // 수정자 키·기능 키·전용 키는 오탐 금지 (기본값 회귀 가드의 확장판).
        let mut config = UnimConfig::default();
        config.engine.hanja_keys.clear(); // 반대 역할 중복 잡음 제거
        config.engine.auto_typefix.toggle_enabled_keys.clear();
        config.engine.auto_typefix.toggle_forward_keys.clear();
        config.engine.auto_typefix.toggle_reverse_keys.clear();
        let (w, unknown) = switch_key_warnings(
            &keys(&["Korean", "Hanja", "RightAlt", "LeftShift", "F1", "F12"]),
            SwitchKeyRole::Toggle,
            &config,
        );
        assert!(w.is_empty(), "수정자·기능 키 오탐 금지: {w:?}");
        assert!(!unknown);
    }

    #[test]
    fn switch_key_warnings_footgun_and_duplicate_stack() {
        let mut config = UnimConfig::default();
        config.engine.hanja_keys = keys(&["A"]);
        let (w, _) = switch_key_warnings(&keys(&["A"]), SwitchKeyRole::Toggle, &config);
        assert_eq!(w.len(), 2, "풋건 + 역할 충돌 2건: {w:?}");
    }

    #[test]
    fn auto_english_key_warnings_flags_unknown() {
        let (w, unknown) = auto_english_key_warnings(&keys(&["key:Ctrl+"]));
        assert_eq!(w.len(), 1, "무효 트리거 1건: {w:?}");
        assert!(unknown);

        // 출하 기본값은 경고 0.
        let (w, unknown) = auto_english_key_warnings(&keys(&["key:Escape", "char:/"]));
        assert!(w.is_empty(), "기본 트리거 경고 없음: {w:?}");
        assert!(!unknown);
    }

    #[test]
    fn all_keys_invalid_blocks_only_full_invalid_list() {
        use unim::input_engine::InputEngine;
        let valid = |k: &str| InputEngine::parse_switch_key(k).is_some();
        // 전 항목 무효 → 차단 (엔진 파서가 전부 걸러 빈 목록 저장과 동일해지는 케이스).
        assert!(all_keys_invalid(&keys(&["NoSuchKey", "AlsoBad"]), valid));
        // 부분 무효 → warn-not-block 유지.
        assert!(!all_keys_invalid(&keys(&["NoSuchKey", "Korean"]), valid));
        // 전 항목 유효(출하 기본값) → 통과.
        assert!(!all_keys_invalid(&keys(&["Korean", "RightAlt"]), valid));
        // 빈 목록은 별도의 "At least one key required" 경로 담당 — 여기선 false.
        assert!(!all_keys_invalid(&keys(&[]), valid));
    }

    #[test]
    fn atf_hotkey_warnings_unchanged_for_valid_combo() {
        // 기존 계약 회귀 가드 — 수정자 조합은 미지 표기가 아니고 풋건도 아니다.
        let config = UnimConfig::default();
        let (w, unknown) = atf_hotkey_warnings(&keys(&["Shift+F9"]), &config);
        assert!(w.is_empty(), "Shift+F9 는 무경고: {w:?}");
        assert!(!unknown);

        let (w, unknown) = atf_hotkey_warnings(&keys(&["ScrollLock"]), &config);
        assert_eq!(w.len(), 1, "존재하지 않는 키 이름은 미지 경고: {w:?}");
        assert!(unknown);
    }
}
