fn main() {
    // 도움말 HTML 설치 경로. Makefile 의 PREFIX 가 가변(deb/rpm=/usr, 소스 빌드=
    // /usr/local)이라 컴파일 타임에 실제 $(DATADIR) 를 주입받는다. Makefile 은
    // `UNIM_DATADIR=$(DATADIR) cargo build --workspace` 로 워크스페이스 전체에
    // 넘기므로, 경로 해석기를 소유한 이 크레이트가 직접 받아 둔다 — 자체 build.rs
    // 가 없는 소비자(unim-indicator 트레이)도 비표준 PREFIX 를 찾아갈 수 있다.
    // 미설정이면 option_env! 가 None 이 되고 런타임 후보 순회(/usr → /usr/local →
    // 개발 폴백)가 대신 처리한다 — 즉 이 주입은 "정답을 앞에 세우는" 최적화지
    // 필수 조건이 아니다.
    println!("cargo:rerun-if-env-changed=UNIM_DATADIR");
    if let Ok(datadir) = std::env::var("UNIM_DATADIR") {
        println!("cargo:rustc-env=UNIM_DATADIR={datadir}");
    }
}
