//! 시스템 트레이 (StatusNotifierItem)
//!
//! ksni 기반 트레이 아이콘 구현.
//! 툴킷에 무관한 코드로, 향후 `unim-gui-common`으로 추출될 대상입니다.

use std::sync::mpsc::Sender;
use std::sync::{Arc, RwLock};

use tokio::sync::mpsc::Sender as TokioSender;

use ksni::menu::*;
use rust_i18n::t;
use unim::status::InputCategory;
use unim::unim_log;

use crate::types::{GuiAction, IndicatorState, SETTINGS_TX};

/// ksni 트레이 구현
#[derive(Debug)]
pub struct UnimTray {
    pub state: Arc<RwLock<IndicatorState>>,
    pub popup_tx: Sender<GuiAction>,
    /// 트레이 → DBus watcher: SetGlobalMode 전달 채널 (tokio mpsc)
    pub dbus_action_tx: TokioSender<GuiAction>,
}

impl ksni::Tray for UnimTray {
    fn id(&self) -> String {
        "unim-gui".into()
    }

    fn icon_theme_path(&self) -> String {
        "/usr/share/icons/hicolor/scalable/apps".into()
    }

    fn icon_name(&self) -> String {
        let category = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::English);
        match category {
            InputCategory::Korean => "unim-korean".into(),
            InputCategory::English => "unim-english".into(),
        }
    }

    fn title(&self) -> String {
        let category = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::English);
        match category {
            InputCategory::Korean => t!("tray_title_korean").into(),
            InputCategory::English => t!("tray_title_english").into(),
        }
    }

    fn tool_tip(&self) -> ksni::ToolTip {
        let category = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::English);

        let mode_desc = match category {
            InputCategory::Korean => t!("tray_tooltip_korean_mode"),
            InputCategory::English => t!("tray_tooltip_english_mode"),
        };

        ksni::ToolTip {
            title: t!("tray_tooltip_title").into(),
            description: mode_desc.into(),
            ..Default::default()
        }
    }

    fn activate(&mut self, _x: i32, _y: i32) {
        // 좌클릭 = 한/영 토글: 현재 모드의 반대값을 엔진에 전달
        let current = self
            .state
            .read()
            .map(|s| s.category)
            .unwrap_or(InputCategory::English);
        let next_korean = match current {
            InputCategory::Korean => false,
            InputCategory::English => true,
        };
        unim_log!("INDICATOR", "트레이 좌클릭 → 모드 토글: korean={}", next_korean);
        let _ = self.dbus_action_tx.blocking_send(GuiAction::SetGlobalMode(next_korean));
    }

    fn menu(&self) -> Vec<ksni::MenuItem<Self>> {
        vec![
            {
                let current_category = self
                    .state
                    .read()
                    .map(|s| s.category)
                    .unwrap_or(InputCategory::English);
                let label_body = t!("tray_menu_korean");
                let korean_label = if current_category == InputCategory::Korean {
                    format!("✓ {}", label_body)
                } else {
                    format!("   {}", label_body)
                };
                StandardItem {
                    label: korean_label,
                    activate: Box::new(|this: &mut Self| {
                        unim_log!("INDICATOR", "메뉴 → 한국어 모드로 전환 요청");
                        let _ = this.dbus_action_tx.blocking_send(GuiAction::SetGlobalMode(true));
                    }),
                    ..Default::default()
                }
            }
            .into(),
            {
                let current_category = self
                    .state
                    .read()
                    .map(|s| s.category)
                    .unwrap_or(InputCategory::English);
                let label_body = t!("tray_menu_english");
                let english_label = if current_category == InputCategory::English {
                    format!("✓ {}", label_body)
                } else {
                    format!("   {}", label_body)
                };
                StandardItem {
                    label: english_label,
                    activate: Box::new(|this: &mut Self| {
                        unim_log!("INDICATOR", "메뉴 → 영어 모드로 전환 요청");
                        let _ = this.dbus_action_tx.blocking_send(GuiAction::SetGlobalMode(false));
                    }),
                    ..Default::default()
                }
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: t!("tray_menu_settings").into(),
                activate: Box::new(|_: &mut Self| {
                    open_settings();
                }),
                ..Default::default()
            }
            .into(),
            ksni::MenuItem::Separator,
            StandardItem {
                label: t!("tray_menu_quit").into(),
                icon_name: "application-exit".into(),
                activate: Box::new(|_: &mut Self| {
                    unim_log!("INDICATOR", "인디케이터 종료");
                    std::process::exit(0);
                }),
                ..Default::default()
            }
            .into(),
        ]
    }
}

/// 내장 설정 다이얼로그를 GTK 이벤트 루프에 GuiAction으로 요청
fn open_settings() {
    if let Ok(tx) = SETTINGS_TX.lock() {
        if let Some(tx) = tx.as_ref() {
            let _ = tx.send(GuiAction::OpenSettings);
        }
    }
}
