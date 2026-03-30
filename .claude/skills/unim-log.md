# UNIM Log Analyzer

Analyze `~/.unim-errors.log` for debugging UNIM input method issues.

## Usage

```
/unim-log                    # 최근 로그 요약 (마지막 100줄)
/unim-log 500                # 마지막 N줄 분석
/unim-log clear              # 로그 초기화
/unim-log <keyword>          # 키워드 필터 (예: POPUP, ENGINE, DBUS, XIM, GNOME_EXT)
```

## Instructions

The UNIM error log is at `~/.unim-errors.log`. It is only active when `UNIM_DEVELOP=1` is set.

Log format: `[YYYY/MM/DD HH:MM:SS] - [MODULE] - message`

Modules: `ENGINE`, `HANGUL`, `COMPOSER`, `CONTEXT`, `CHAR`, `DAEMON`, `DBUS`, `ENGINE_WORKER`, `XIM`, `XIM_HANDLER`, `XIM_DBUS`, `WAYLAND`, `CLI`, `INDICATOR`, `GTK_IM`, `QT_IM`, `EXTENSION`, `GNOME_EXT/HANJA`, `GNOME_EXT/SPECIAL`, `GNOME_EXT/DBUS_IME`, `GNOME_EXT/INDICATOR`

### Analysis Steps

1. **Read the log** using `ctx_execute` to avoid flooding context:
   - If argument is a number N: read last N lines
   - If argument is "clear": truncate the file with `> ~/.unim-errors.log`
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
