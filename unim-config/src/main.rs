use clap::{Parser, Subcommand, ValueEnum};
use rust_i18n::t;
use std::process;
use unim::config::{Config as UnimConfig, HangulLayout, LatinLayout};

// i18n 초기화
rust_i18n::i18n!("locales");

/// UNIM 입력기 설정 관리 도구
#[derive(Parser, Debug)]
#[command(author, version, about = "UNIM Settings Manager")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
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
}

#[derive(Clone, Debug, ValueEnum)]
enum ConfigKey {
    /// 한글 레이아웃 (2bul, 3bul390, 3bul391)
    #[value(name = "hangul-layout")]
    HangulLayout,
    /// 영문 레이아웃 (qwerty, dvorak)
    #[value(name = "latin-layout")]
    LatinLayout,
    /// 자동 전환 활성화 (true, false)
    #[value(name = "auto-switch")]
    AutoSwitch,
    /// 자동 전환 임계값 (0.0 ~ 1.0)
    #[value(name = "auto-switch-threshold")]
    AutoSwitchThreshold,
}

fn config_show() {
    let config = UnimConfig::load_from_default_path();
    
    let hangul_name = match config.engine.hangul.layout {
        HangulLayout::Dubeolsik => t!("twobul_std"),
        HangulLayout::Sebeolsik390 => t!("threebul_390"),
        HangulLayout::Sebeolsik391 => t!("threebul_391"),
    };
    
    let latin_name = match config.engine.latin.layout {
        LatinLayout::Qwerty => t!("qwerty"),
        LatinLayout::Dvorak => t!("dvorak"),
    };
    
    let auto_switch_status = if config.engine.auto_switch.enabled { t!("enabled") } else { t!("disabled") };
    
    println!("{}", t!("settings_title"));
    println!("================");
    println!("{}: {} ({})", t!("hangul_layout_label"), hangul_name, config.engine.hangul.layout.name());
    println!("{}: {} ({})", t!("latin_layout_label"), latin_name, config.engine.latin.layout.name());
    println!("{}: {}", t!("auto_switch_label"), auto_switch_status);
    println!("{}: {:.2}", t!("auto_switch_threshold_label"), config.engine.auto_switch.threshold);
    println!();
    if let Some(path) = UnimConfig::default_config_path() {
        println!("{}: {}", t!("config_file_label"), path.display());
    }
}

fn config_set(key: ConfigKey, value: &str) -> Result<(), String> {
    let mut config = UnimConfig::load_from_default_path();
    
    match key {
        ConfigKey::HangulLayout => {
            let layout = match value {
                "2bul" | "dubeolsik" => HangulLayout::Dubeolsik,
                "3bul390" | "390" => HangulLayout::Sebeolsik390,
                "3bul391" | "391" => HangulLayout::Sebeolsik391,
                _ => {
                    let kind = t!("hangul_layout_label").to_string();
                    return Err(t!("error_invalid_layout", kind = kind, value = value, allowed = "2bul, 3bul390, 3bul391").to_string());
                }
            };
            config.engine.hangul.layout = layout;
            let kind = t!("hangul_layout_label").to_string();
            println!("{}", t!("layout_changed", kind = kind, layout = layout.name()));
        }
        ConfigKey::LatinLayout => {
            let layout = match value {
                "qwerty" => LatinLayout::Qwerty,
                "dvorak" => LatinLayout::Dvorak,
                _ => {
                    let kind = t!("latin_layout_label").to_string();
                    return Err(t!("error_invalid_layout", kind = kind, value = value, allowed = "qwerty, dvorak").to_string());
                }
            };
            config.engine.latin.layout = layout;
            let kind = t!("latin_layout_label").to_string();
            println!("{}", t!("layout_changed", kind = kind, layout = layout.name()));
        }
        ConfigKey::AutoSwitch => {
            let enabled = match value.to_lowercase().as_str() {
                "true" | "on" | "1" | "yes" => true,
                "false" | "off" | "0" | "no" => false,
                _ => return Err(format!("Invalid value for auto-switch: {}", value)),
            };
            config.engine.auto_switch.enabled = enabled;
            let status = if enabled { t!("enabled") } else { t!("disabled") };
            println!("{}", t!("auto_switch_changed", status = status));
        }
        ConfigKey::AutoSwitchThreshold => {
            let threshold: f32 = value.parse()
                .map_err(|_| t!("error_invalid_threshold", value = value).to_string())?;
            if !(0.0..=1.0).contains(&threshold) {
                return Err(t!("error_invalid_threshold", value = value).to_string());
            }
            config.engine.auto_switch.threshold = threshold;
            println!("{}", t!("threshold_changed", value = format!("{:.2}", threshold)));
        }
    }
    
    config.save_to_default_path().map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
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
    config.save_to_default_path().map_err(|e| t!("error_save_failed", error = e.to_string()).to_string())?;
    println!("{}", t!("config_reset_done"));
    config_show();
    Ok(())
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
        Commands::Show => config_show(),
        Commands::Set { key, value } => {
            if let Err(e) = config_set(key, &value) {
                eprintln!("{}: {}", t!("error_label"), e);
                process::exit(1);
            }
        }
        Commands::Path => config_path(),
        Commands::Reset => {
            if let Err(e) = config_reset() {
                eprintln!("{}: {}", t!("error_label"), e);
                process::exit(1);
            }
        }
    }
}
