# UNIM MSI Diagnosis — `YYYY-MM-DD`

> 본 양식을 복사해 `MSI_DIAGNOSIS_2026-MM-DD.md` 로 저장 후 채워 넣는다.

## 환경

- MSI 파일: `unim-<version>-x64.msi` (Artifact run #___, commit `___`)
- OS: Windows 11 ___H2 (build ___)
- 아키텍처: x64
- 한국어 입력 사전 설치 여부: ☐ Y / ☐ N
- VM / 실기: ___
- 동시 활성 IME: ___

## 결과 요약

| 단계 | 통과 / 실패 | 비고 |
|------|-------------|------|
| 1. 설치 (msiexec /i) | | |
| 2. 레지스트리 무결성 | | |
| 3. TIP 발견 | | |
| 4.1 두벌식 입력 | | |
| 4.2 BS 제거 | | |
| 4.3 한자 변환 | | |
| 4.4 한영 토글 | | |
| 4.5 x86 앱 | | |
| 4.6 UWP 앱 | | |
| 5. 트레이/설정 UI | | |
| 6. 제거 (msiexec /x) | | |

## 1. install.log (마지막 50줄)

```text
<paste here>
```

## 2. 레지스트리 덤프

### 2.1 `HKCR\CLSID\{UNIM_CLSID}` (전체)

```reg
<paste reg query output>
```

### 2.2 `InProcServer32` default value

```text
<expected: REG_SZ "C:\Program Files\UNIM\unim_tsf.dll">
<actual:   ___>
```

### 2.3 `HKLM\...\CTF\TIP\{UNIM_CLSID}` (전체)

```reg
<paste reg query output>
```

### 2.4 LanguageProfile Enable

```text
<expected: REG_DWORD 0x1>
<actual:   ___>
```

### 2.5 Category 키 (TIP_KEYBOARD)

```reg
<paste — Item 마지막 sub-key 가 {UNIM_CLSID} 여야 함>
```

## 3. TIP 발견 (PowerShell)

```text
<Get-WinUserLanguageList 출력>
```

## 4. 입력 동작 노트

(실패한 케이스만 자세히)

- 4._: 입력 시퀀스 = `___`, 기대 = `___`, 실제 = `___`. 스크린샷: `___.png`.

## 5. uninstall.log (마지막 30줄)

```text
<paste>
```

## 6. 원인 분류 (plan.md §2 표 참조)

해당 항목에 ☑

- ☐ P0-1 mingw GNU ABI ≠ Windows COM (MSVC) ABI
- ☐ P0-2 wixl `[#File.Id]` 토큰 치환 실패
- ☐ P0-3 wixl 빌드 산출물 누락/손상
- ☐ P1-1 Category 키 구조 오류
- ☐ P1-2 Profile Description sub-key 누락
- ☐ P1-3 globals.rs ↔ wxs GUID 불일치
- ☐ P2-1 regsvr32 자동 호출 누락
- ☐ P2-2 MSI scope=perMachine 권한 처리
- ☐ P3   EmbedCab 압축 손상
- ☐ 기타 (서술)

## 7. 다음 액션

(원인이 확정되면 fix 위치 file_path:line 로 명시)
