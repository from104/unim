//! unim-tsf 빌드 스크립트 — DLL 에 시그니처 아이콘(unim.rc)을 임베드한다.
//!
//! 임베드된 RT_GROUP_ICON(ID 1)으로 LanguageProfile IconFile=DLL, IconIndex=0
//! 이 UNIM 'UN' 아이콘을 가리킨다(IME 선택기·설치 목록 표시). rc.exe 는 빌드
//! 래퍼(scripts/cargo-msvc.bat 의 vcvars)에서 PATH 에 잡힌다.

fn main() {
    #[cfg(windows)]
    {
        embed_resource::compile("unim.rc", embed_resource::NONE);
    }
}
