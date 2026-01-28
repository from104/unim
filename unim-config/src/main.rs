use clap::{Parser, Subcommand, ValueEnum};
use dialoguer::{theme::ColorfulTheme, Confirm, Input, Select};
use rust_i18n::t;
use std::process;
use unim::config::{Config as UnimConfig, EnglishLayout, KoreanLayout};

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
    /// 한국어 레이아웃 (2bul, 3bul390, 3bul391)
    #[value(name = "korean-layout")]
    KoreanLayout,
    /// 영어 레이아웃 (qwerty, dvorak)
    #[value(name = "english-layout")]
    EnglishLayout,
    /// 자동 전환 활성화 (true, false)
    #[value(name = "auto-switch")]
    AutoSwitch,
    /// 자동 전환 임계값 (0.0 ~ 1.0)
    #[value(name = "auto-switch-threshold")]
    AutoSwitchThreshold,
}

fn config_show() {
    let config = UnimConfig::load_from_default_path();

    let korean_name = config.engine.korean.layout.display_name();

    let english_name = config.engine.english.layout.display_name();

    let auto_switch_status = if config.engine.auto_switch.enabled {
        t!("enabled")
    } else {
        t!("disabled")
    };

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
    println!("{}: {}", t!("auto_switch_label"), auto_switch_status);
    println!(
        "{}: {:.2}",
        t!("auto_switch_threshold_label"),
        config.engine.auto_switch.threshold
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
        ConfigKey::AutoSwitch => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid value for auto-switch: {}", value)),
            };
            config.engine.auto_switch.enabled = enabled;
            let status = if enabled {
                t!("enabled")
            } else {
                t!("disabled")
            };
            println!("{}", t!("auto_switch_changed", status = status));
        }
        ConfigKey::AutoSwitchThreshold => {
            let threshold: f32 = value
                .parse()
                .map_err(|_| t!("error_invalid_threshold", value = value).to_string())?;
            if !(0.0..=1.0).contains(&threshold) {
                return Err(t!("error_invalid_threshold", value = value).to_string());
            }
            config.engine.auto_switch.threshold = threshold;
            println!(
                "{}",
                t!("threshold_changed", value = format!("{:.2}", threshold))
            );
        }
    }

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
            t!("auto_switch_label").to_string(),
            t!("auto_switch_threshold_label").to_string(),
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
                config.engine.auto_switch.enabled = Confirm::with_theme(&theme)
                    .with_prompt(t!("enable_auto_switch").to_string())
                    .default(config.engine.auto_switch.enabled)
                    .interact()
                    .unwrap();
            }
            3 => {
                let threshold: f32 = Input::with_theme(&theme)
                    .with_prompt(t!("enter_threshold").to_string())
                    .default(config.engine.auto_switch.threshold)
                    .validate_with(|input: &f32| {
                        if (0.0..=1.0).contains(input) {
                            Ok(())
                        } else {
                            Err(t!("error_invalid_threshold", value = input).to_string())
                        }
                    })
                    .interact_text()
                    .unwrap();
                config.engine.auto_switch.threshold = threshold;
            }
            4 => {
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
            5 => {
                if let Err(e) = config.save_to_default_path() {
                    eprintln!("{}: {}", t!("error_label"), e);
                } else {
                    println!("{}", t!("config_saved"));
                }
                break;
            }
            6 => {
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
