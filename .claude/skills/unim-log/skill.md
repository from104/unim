# UNIM Log Analyzer

Analyze logs under `~/.unim-log/` for debugging UNIM input method issues.

## Usage

```
/unim-log                    # 가장 최근 프로세스의 로그 요약 (마지막 100줄)
/unim-log 500                # 마지막 N줄 분석
/unim-log clear              # 현재 세션·날짜의 모든 로그 파일 초기화
/unim-log <keyword>          # 키워드 필터 (예: POPUP, ENGINE, DBUS, XIM, GNOME_EXT)
/unim-log all                # ~/.unim-log/ 의 모든 파일 목록과 크기
/unim-log app <name>         # 특정 앱(progname)의 로그만 (예: /unim-log app gnome-shell)
```

## Instructions

UNIM 로그는 `~/.unim-log/` 아래에 **윈도우 세션 · 날짜 · 프로세스(앱)** 단위로 분리되어 저장됩니다.
파일명: `{session-tag}_{YYYY-MM-DD}_{progname}-{pid}.log`
- `session-tag` 우선순위: `XDG_SESSION_ID` → `WAYLAND_DISPLAY` → `DISPLAY`.
- `progname` 은 `/proc/self/comm` 의 호스트 프로세스 이름. GTK/Qt IM 모듈은 호스트 앱(예: `konsole`, `kate`) 안에서 동작하므로 자연스럽게 앱별로 분리됩니다. 데몬은 `unim-daemon`, GNOME extension은 `gnome-shell`.
- `pid` 는 호스트 프로세스 PID.

로그는 `UNIM_DEVELOP=1` 환경 변수가 설정된 경우에만 활성화됩니다.

Log format: `[YYYY/MM/DD HH:MM:SS] - [MODULE] - message`

Modules: `ENGINE`, `HANGUL`, `COMPOSER`, `CONTEXT`, `CHAR`, `DAEMON`, `DBUS`, `ENGINE_WORKER`, `XIM`, `XIM_HANDLER`, `XIM_DBUS`, `WAYLAND`, `CLI`, `INDICATOR`, `GTK_IM`, `QT_IM`, `EXTENSION`, `GNOME_EXT/HANJA`, `GNOME_EXT/SPECIAL`, `GNOME_EXT/DBUS_IME`, `GNOME_EXT/INDICATOR`

### Resolving the current session log

가장 최근에 수정된 파일을 현재 세션 로그로 간주합니다:

```bash
ls -t ~/.unim-log/*.log 2>/dev/null | head -1
```

### Analysis Steps

1. **Read the log** using `ctx_execute` to avoid flooding context:
   - 기본 대상은 `~/.unim-log/` 의 최근 mtime 파일
   - If argument is a number N: read last N lines
   - If argument is "clear": truncate the resolved file with `: > <path>`
   - If argument is "all": `ls -lt ~/.unim-log/` 로 파일 목록만 출력
   - If argument is a keyword: filter by that keyword
   - Default: last 100 lines

2. **Summarize** the log in Korean with these sections:
   - **Timeline**: Key events in chronological order (first/last timestamp, duration)
   - **Signal Flow**: Trace the data flow (KeyPress -> Engine -> DBus -> Frontend -> Popup)
   - **Errors**: Any ERROR lines, exceptions, or unexpected behavior
   - **Popup Status**: ShowHanjaPopup/ShowSpecialPopup/PopupNavigate/HidePopup signal status
   - **Focus Events**: FocusIn/FocusOut transitions
   - **Diagnosis**: Root cause analysis if errors found

3. **Cross-reference** with source code when needed:
   - `unim-dbus/src/service.rs` for DBus signal emission
   - `unim-gui-gtk/src/` for popup display
   - `unim-gnome-extension/` for GNOME Shell extension
   - `src/input_engine.rs` for key processing
   - `src/hangul/` for composition logic

4. **Output format**:
   - Keep summaries concise (under 300 words)
   - Use tables for signal flows
   - Highlight anomalies with **bold**
   - Suggest specific file:line to investigate

### Common Patterns to Detect

| Pattern | Meaning |
|---------|---------|
| `consumed=true` but no preedit/commit | Key swallowed without effect |
| `ShowHanjaPopup` without `[INDICATOR]` receipt | Signal not reaching unim-gui-gtk |
| `[GNOME_EXT/DBUS_IME] - ERROR` | Extension JS error |
| `PopupNavigate` without subsequent selection | Navigation broken |
| `FocusOut` immediately after popup show | Focus stolen by popup window |
| Missing `CursorRect` before popup | Position data unavailable |
| HiDPI coordinates (>2000) with small display | Scale factor mismatch |
