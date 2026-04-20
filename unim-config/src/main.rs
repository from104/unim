use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use rust_i18n::t;
use std::process;
use unim::config::{
    Config as UnimConfig, EnglishLayout, InputCategory, KoreanLayout, ModeSharingMode,
    AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN,
    AUTO_TYPEFIX_KOR_THRESHOLD_MAX, AUTO_TYPEFIX_KOR_THRESHOLD_MIN,
    AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MAX, AUTO_TYPEFIX_OBSERVATION_TIMEOUT_MIN,
    AUTO_TYPEFIX_TENTATIVE_EXPIRY_MAX, AUTO_TYPEFIX_TENTATIVE_EXPIRY_MIN,
    AUTO_TYPEFIX_TIME_WINDOW_MAX, AUTO_TYPEFIX_TIME_WINDOW_MIN,
};

// i18n 초기화
rust_i18n::i18n!("locales");

/// UNIM 입력기 설정 관리 도구
#[derive(Parser, Debug)]
#[command(author, version, about = "UNIM Settings Manager")]
struct Cli {
    #[command(subcommand)]
    command: Option<Commands>,
}

#[derive(Subcommand, Debug)]
enum Commands {
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
    /// 자동 오타 교정: 시간 윈도우 (500~5000 ms)
    #[value(name = "auto-typefix-time-window")]
    AutoTypeFixTimeWindow,
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
    /// 자동 오타 교정: 접두사 충돌 시 보류 (true, false)
    #[value(name = "auto-typefix-skip-on-prefix-collision")]
    AutoTypeFixSkipOnPrefixCollision,
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
    /// 이모지 팝업 활성화 (true, false)
    #[value(name = "emoji-popup")]
    EmojiPopup,
    /// 이모지 팝업 트리거 키 (예: Super+Period)
    #[value(name = "emoji-popup-keys")]
    EmojiPopupKeys,
}

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
        println!("  - 순방향(영→한): {}, 역방향(한→영): {}",
            if atf.forward { "ON" } else { "OFF" },
            if atf.reverse { "ON" } else { "OFF" });
        println!("  - 한글 음절 임계값: {}, 영문 최소 길이: {}",
            atf.kor_syllable_threshold, atf.eng_word_min_length);
        println!("  - 시간 윈도우: {}ms", atf.time_window_ms);
        println!("  - 재트리거 감지: {} / 관찰 창: {}초 / 임시 억제 만료: {}시간",
            if atf.rollback_detection { "ON" } else { "OFF" },
            atf.observation_timeout_secs,
            atf.tentative_expiry_hours);
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
    let emoji_status = if config.engine.emoji_popup.enabled {
        t!("enabled")
    } else {
        t!("disabled")
    };
    println!("{}: {}", t!("emoji_popup_label"), emoji_status);
    println!(
        "{}: {}",
        t!("emoji_popup_keys_label"),
        config.engine.emoji_popup.trigger_keys.join(", ")
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
            let v: u8 = value.parse().map_err(|_| format!("Invalid number: {}", value))?;
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
            let v: u8 = value.parse().map_err(|_| format!("Invalid number: {}", value))?;
            if !(AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN..=AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX).contains(&v) {
                return Err(format!(
                    "Range {}~{}, got {}",
                    AUTO_TYPEFIX_ENG_MIN_LENGTH_MIN, AUTO_TYPEFIX_ENG_MIN_LENGTH_MAX, v
                ));
            }
            config.engine.auto_typefix.eng_word_min_length = v;
            println!("영문 단어 최소 길이: {}", v);
        }
        ConfigKey::AutoTypeFixTimeWindow => {
            let v: u32 = value.parse().map_err(|_| format!("Invalid number: {}", value))?;
            if !(AUTO_TYPEFIX_TIME_WINDOW_MIN..=AUTO_TYPEFIX_TIME_WINDOW_MAX).contains(&v) {
                return Err(format!(
                    "Range {}~{}, got {}",
                    AUTO_TYPEFIX_TIME_WINDOW_MIN, AUTO_TYPEFIX_TIME_WINDOW_MAX, v
                ));
            }
            config.engine.auto_typefix.time_window_ms = v;
            println!("시간 윈도우: {}ms", v);
        }
        ConfigKey::AutoTypeFixForward => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.forward = enabled;
            println!("순방향(영→한) 교정: {}", if enabled { "ON" } else { "OFF" });
        }
        ConfigKey::AutoTypeFixReverse => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.reverse = enabled;
            println!("역방향(한→영) 교정: {}", if enabled { "ON" } else { "OFF" });
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
        ConfigKey::AutoTypeFixSkipOnPrefixCollision => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid bool: {}", value)),
            };
            config.engine.auto_typefix.skip_on_prefix_collision = enabled;
            println!(
                "{}: {}",
                t!("auto_typefix_skip_prefix_collision_label"),
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
            println!("{}: {} {}",
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
            println!("{}: {} {}",
                t!("auto_typefix_observation_timeout_secs_label"),
                secs,
                t!("unit_secs")
            );
        }
        ConfigKey::AppRules => {
            let rules: Vec<unim::config::AppRule> =
                serde_json::from_str(value).map_err(|e| format!("Invalid JSON: {}", e))?;
            config.engine.app_rules = rules;
            println!("{}: {} rules", t!("app_rules_label"), config.engine.app_rules.len());
        }
        ConfigKey::EmojiPopup => {
            let enabled: bool = value.parse().map_err(|_| "Invalid value, use true/false".to_string())?;
            config.engine.emoji_popup.enabled = enabled;
            let status = if enabled { t!("enabled") } else { t!("disabled") };
            println!("{}: {}", t!("emoji_popup_label"), status);
        }
        ConfigKey::EmojiPopupKeys => {
            let keys: Vec<String> = value
                .split(',')
                .map(|s| s.trim().to_string())
                .filter(|s| !s.is_empty())
                .collect();
            if keys.is_empty() {
                return Err("At least one trigger required".to_string());
            }
            config.engine.emoji_popup.trigger_keys = keys;
            println!(
                "{}: {}",
                t!("emoji_popup_keys_label"),
                config.engine.emoji_popup.trigger_keys.join(", ")
            );
        }
    }

    // 방어: AutoTypeFix 범위 클램프 (명시적 범위 체크 통과 후에도 SSoT 보강)
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
        println!("\x1B[2J\x1B[1;1H"); // Clear screen
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
                    eprintln!("{}: {}", t!("error_label"), e);
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

fn main() {
    // 로케일 설정
    let locale = std::env::var("LANG")
        .or_else(|_| std::env::var("LC_ALL"))
        .unwrap_or_else(|_| "en".to_string());
    let locale = locale.split('.').next().unwrap_or("en");
    let locale = locale.split('_').next().unwrap_or("en");
    rust_i18n::set_locale(locale);

    let cli = Cli::parse();

    match cli.command {
        Some(Commands::Show) => config_show(),
        Some(Commands::Set { key, value }) => {
            if let Err(e) = config_set(key, &value) {
                eprintln!("{}: {}", t!("error_label"), e);
                process::exit(1);
            }
        }
        Some(Commands::Path) => config_path(),
        Some(Commands::Reset) => {
            if let Err(e) = config_reset() {
                eprintln!("{}: {}", t!("error_label"), e);
                process::exit(1);
            }
        }
        Some(Commands::Interactive) => config_interactive(),
        None => {
            config_show();
            println!("\n{}", t!("help_hint"));
        }
    }
}
