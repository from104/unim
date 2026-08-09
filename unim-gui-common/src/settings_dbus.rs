//! toolkit-free DBus 헬퍼 — 설정 저장 전파
//!
//! Config 직렬화 + DBus `SetConfigYaml` fire-and-forget.
//! GTK `save_and_notify`와 미래 Qt settings 다이얼로그 양쪽에서 재사용.

use unim::config::Config;
use unim::unim_log;

/// Config를 YAML로 직렬화하여 DBus `SetConfigYaml`로 전파 (fire-and-forget).
///
/// - 파일 저장은 호출자 책임 (`Config::save_to_default_path`).
/// - 이 함수는 직렬화 + DBus 전파만 담당.
/// - 실패해도 메인 스레드를 차단하지 않는다.
pub fn save_config_via_dbus(config: &Config, label: &str) {
    match serde_yaml::to_string(config) {
        Ok(yaml) => spawn_set_config_yaml(yaml, label.to_string()),
        Err(e) => unim_log!("INDICATOR", "[Settings] YAML 직렬화 실패: {}", e),
    }
}

/// DBus SetConfigYaml fire-and-forget.
///
/// 새로운 tokio current-thread 런타임을 임시로 생성하여 호출.
/// 메인 스레드를 차단하지 않기 위해 별도 OS 스레드에 위임.
pub fn spawn_set_config_yaml(yaml: String, label: String) {
    std::thread::spawn(move || {
        let rt = match tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
        {
            Ok(rt) => rt,
            Err(e) => {
                unim_log!("INDICATOR", "[Settings] tokio 런타임 생성 실패: {}", e);
                return;
            }
        };
        rt.block_on(async move {
            match send_set_config_yaml(&yaml).await {
                Ok(()) => unim_log!(
                    "INDICATOR",
                    "[Settings] DBus SetConfigYaml 성공 ({})",
                    label
                ),
                Err(e) => unim_log!(
                    "INDICATOR",
                    "[Settings] DBus SetConfigYaml 실패 ({}): {}",
                    label,
                    e
                ),
            }
        });
    });
}

async fn send_set_config_yaml(yaml: &str) -> zbus::Result<()> {
    use unim_dbus::client::InputMethodProxy;
    let conn = zbus::Connection::session().await?;
    let proxy = InputMethodProxy::new(&conn).await?;
    proxy.set_config_yaml(yaml).await
}
