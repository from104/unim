use futures_util::stream::StreamExt;
use std::collections::HashMap;
use zbus::zvariant::Value;
use zbus::{proxy, Connection, Result};

// 일반적인 IBus 프록시 정의
#[proxy(
    interface = "org.freedesktop.IBus",
    default_service = "org.freedesktop.IBus",
    default_path = "/org/freedesktop/IBus/Bus"
)]
trait IBus {
    #[zbus(signal)]
    async fn global_engine_changed(&self, engine_name: String) -> Result<()>;
}

// 일반적인 DBus Properties 인터페이스 프록시 정의
#[proxy(
    interface = "org.freedesktop.DBus.Properties",
    default_service = "org.freedesktop.IBus.Panel", // IBus Panel 서비스에 맞게 설정
    default_path = "/org/freedesktop/IBus/Panel"    // IBus Panel 경로에 맞게 설정
)]
trait Properties {
    // PropertiesChanged 시그널 정의
    #[zbus(signal)]
    async fn properties_changed(
        &self,
        interface_name: String,
        changed_properties: HashMap<String, Value<'_>>,
        invalidated_properties: Vec<String>,
    ) -> Result<()>;
}

#[tokio::main]
async fn main() -> Result<()> {
    // D-Bus 세션 연결 생성
    let connection = Connection::session().await?;

    // IBus 프록시 생성
    let ibus_proxy = IBusProxy::new(&connection).await?;

    println!("IBus GlobalEngineChanged 시그널 대기 중...");

    // GlobalEngineChanged 시그널 스트림 수신
    let mut engine_stream = ibus_proxy.receive_global_engine_changed().await?;

    // 현재 한글 입력기 사용 중인지 여부
    let mut using_hangul = false;

    // Properties 프록시와 스트림은 필요할 때 생성
    let mut props_proxy: Option<PropertiesProxy<'_>> = None;
    let mut props_changed_stream = None;

    // 무한 루프로 시그널 처리
    loop {
        tokio::select! {
            // 엔진 변경 시그널 수신
            Some(signal) = engine_stream.next() => {
                match signal.args() {
                    Ok(args) => {
                        let engine_name = &args.engine_name;
                        println!("IBus 엔진이 변경되었습니다: {}", engine_name);

                        // 한글 엔진으로 변경되었는지 확인
                        if engine_name == "hangul" && !using_hangul {
                            println!("한글 엔진을 사용합니다. Properties 모니터링을 시작합니다...");
                            using_hangul = true;

                            // 아직 프록시가 없으면 생성
                            if props_proxy.is_none() {
                                let panel_proxy = PropertiesProxy::new(&connection).await?;
                                props_changed_stream = Some(panel_proxy.receive_properties_changed().await?);
                                props_proxy = Some(panel_proxy);
                            }
                        } else if engine_name != "hangul" && using_hangul {
                            println!("한글 엔진에서 다른 엔진으로 변경되었습니다. 모니터링을 중지합니다.");
                            using_hangul = false;
                        }
                    }
                    Err(e) => eprintln!("GlobalEngineChanged 시그널 인수 처리 오류: {}", e),
                }
            },

            // Properties 변경 시그널 수신 (스트림이 있는 경우에만)
            Some(props_signal) = async {
                if let Some(stream) = &mut props_changed_stream {
                    stream.next().await
                } else {
                    futures_util::future::pending().await
                }
            }, if props_changed_stream.is_some() => {
                // 시그널에서 인수 추출
                match props_signal.args() {
                    Ok(args) => {
                        let interface_name = &args.interface_name;
                        let changed_props = &args.changed_properties;

                        println!("인터페이스 '{}' 속성 변경:", interface_name);

                        // 변경된 속성 출력
                        for (key, value) in changed_props.iter() {
                            println!("  변경된 속성: {} = {:?}", key, value);

                            // 한/영 상태와 관련된 것으로 추정되는 속성
                            if key == "InputMode" || key == "State" || key == "Aux" || key == "AuxText" {
                                println!("  --> 이 속성이 한/영 모드를 나타낼 수 있습니다!");
                            }
                        }

                        // 무효화된 속성 출력
                        let invalidated = &args.invalidated_properties;
                        if !invalidated.is_empty() {
                            println!("  무효화된 속성:");
                            for prop in invalidated {
                                println!("    {}", prop);
                            }
                        }
                    }
                    Err(e) => eprintln!("PropertiesChanged 시그널 인수 처리 오류: {}", e),
                }
            },

            // 기타 경우 처리
            else => {}
        }
    }
}
