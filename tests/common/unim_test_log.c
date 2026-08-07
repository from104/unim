/**
 * UNIM 테스트 앱 — 구조화 로거 구현
 *
 * 설계 근거: docs/dev/testing/TEST_APPS.md §4
 */

/* clock_gettime · localtime_r — 엄격 ISO C 모드에서도 보이게 */
#ifndef _POSIX_C_SOURCE
#define _POSIX_C_SOURCE 200809L
#endif

#include "unim_test_log.h"

#include <stdio.h>
#include <stdlib.h>
#include <string.h>
#include <time.h>
#include <unistd.h>

/* ─── 상태 ────────────────────────────────────────────────────────────── */

#define LOG_TEXT_MAX  4096
/* 이스케이프 후 상한. 제어문자가 대부분인 병적 입력(\u00XX 로 6배)은
 * 잘린다 — 로그이므로 허용한다. unim_log_json_escape 가 경계를 지킨다. */
#define LOG_ESC_MAX   (LOG_TEXT_MAX * 2 + 8)
#define LOG_KV_MAX    (LOG_ESC_MAX * 3 + 1024)
#define LOG_LINE_MAX  (LOG_KV_MAX + 256)

/* 아래 버퍼들은 전부 static 이다. 로거는 단일 스레드 GUI 콜백에서만 쓰이고
 * 자기 자신을 재진입하지 않으므로 안전하며, GUI 콜백 스택을 아끼게 된다. */

enum { FMT_JSON = 1, FMT_HUMAN = 2 };

static struct {
    int         inited;
    FILE       *jsonl;          /* UNIM_TEST_LOG 파일 (없으면 NULL) */
    long        seq;
    long long   last_ms;
    long long   start_ms;
    char        app[64];
    int         format;         /* FMT_* 비트마스크 */
    int         skip_key;       /* UNIM_TEST_LOG_LEVEL=no-key */
    int         color;          /* stdout 이 tty 인가 */
    UnimLogSink sink;
    void       *sink_user;
} L;

/* ─── 시간 ────────────────────────────────────────────────────────────── */

static long long now_ms(void) {
    struct timespec ts;
    clock_gettime(CLOCK_REALTIME, &ts);
    return (long long)ts.tv_sec * 1000 + ts.tv_nsec / 1000000;
}

static void clock_str(long long ms, char *out, size_t n) {
    time_t sec = (time_t)(ms / 1000);
    struct tm tm;
    localtime_r(&sec, &tm);
    snprintf(out, n, "%02d:%02d:%02d.%03d",
             tm.tm_hour, tm.tm_min, tm.tm_sec, (int)(ms % 1000));
}

/* ─── UTF-8 ───────────────────────────────────────────────────────────── */

size_t unim_log_utf8_len(const char *s) {
    if (!s) return 0;
    size_t n = 0;
    for (const unsigned char *p = (const unsigned char *)s; *p; p++)
        if ((*p & 0xC0) != 0x80) n++;
    return n;
}

char *unim_log_json_escape(const char *in, char *out, size_t out_size) {
    size_t o = 0;
    if (out_size == 0) return out;
    if (!in) { out[0] = '\0'; return out; }

    for (const unsigned char *p = (const unsigned char *)in; *p; p++) {
        /* 최악의 경우(\u00XX) 6바이트 + NUL 여유를 항상 남긴다 */
        if (o + 7 >= out_size) break;
        switch (*p) {
            case '"':  out[o++] = '\\'; out[o++] = '"';  break;
            case '\\': out[o++] = '\\'; out[o++] = '\\'; break;
            case '\n': out[o++] = '\\'; out[o++] = 'n';  break;
            case '\r': out[o++] = '\\'; out[o++] = 'r';  break;
            case '\t': out[o++] = '\\'; out[o++] = 't';  break;
            case '\b': out[o++] = '\\'; out[o++] = 'b';  break;
            case '\f': out[o++] = '\\'; out[o++] = 'f';  break;
            default:
                if (*p < 0x20) {
                    o += (size_t)snprintf(out + o, out_size - o, "\\u%04x", *p);
                } else {
                    out[o++] = (char)*p;   /* UTF-8 은 그대로 통과 */
                }
        }
    }
    out[o] = '\0';
    return out;
}

/* NULL 을 빈 문자열로. 로그에 "(null)" 이 새는 것을 막는다. */
static const char *nn(const char *s) { return s ? s : ""; }

/* ─── 색 ──────────────────────────────────────────────────────────────── */

static const char *color_of(const char *ev) {
    if (!L.color) return NULL;
    if (strncmp(ev, "error", 5) == 0)          return "\x1b[0;31m";
    if (strncmp(ev, "warn", 4) == 0)           return "\x1b[0;33m";
    if (strcmp(ev, "commit") == 0)             return "\x1b[1;32m";
    if (strncmp(ev, "preedit", 7) == 0)        return "\x1b[0;36m";
    if (strcmp(ev, "field.render") == 0)       return "\x1b[1;37m";
    if (strncmp(ev, "key.", 4) == 0)           return "\x1b[2m";
    if (strncmp(ev, "focus", 5) == 0)          return "\x1b[0;35m";
    if (strncmp(ev, "dbus", 4) == 0)           return "\x1b[2;34m";
    if (strncmp(ev, "app.", 4) == 0)           return "\x1b[1;34m";
    return NULL;
}

/* ─── 발행 ────────────────────────────────────────────────────────────── */

/**
 * @param ev       사건 이름
 * @param json_kv  JSON 오브젝트 본문 조각 (이미 이스케이프됨, 없으면 NULL)
 * @param human    사람이 읽는 본문 (사건 이름 뒤에 붙는다)
 */
static void emit(const char *ev, const char *json_kv, const char *human) {
    if (!L.inited) return;

    long long t  = now_ms();
    long long dt = L.last_ms ? (t - L.last_ms) : 0;
    L.last_ms = t;
    L.seq++;

    if (L.jsonl) {
        fprintf(L.jsonl,
                "{\"seq\":%ld,\"t\":%lld,\"dt\":%lld,\"app\":\"%s\",\"ev\":\"%s\"%s%s}\n",
                L.seq, t, dt, L.app, ev,
                (json_kv && *json_kv) ? "," : "", nn(json_kv));
        fflush(L.jsonl);   /* 하네스가 tail 한다 — 버퍼링 금지 */
    }

    if ((L.format & FMT_HUMAN) || L.sink) {
        char clk[32];
        clock_str(t, clk, sizeof clk);

        static char line[LOG_LINE_MAX];
        snprintf(line, sizeof line, "%5ld %s %+5lldms  %-16s %s",
                 L.seq, clk, dt, ev, nn(human));

        if (L.format & FMT_HUMAN) {
            const char *c = color_of(ev);
            if (c) printf("%s%s\x1b[0m\n", c, line);
            else   printf("%s\n", line);
        }
        if (L.sink) L.sink(line, L.sink_user);
    }

    if (L.format & FMT_JSON) {
        /* stdout 으로도 JSONL 을 흘린다 (파일 없이 파이프로 받는 경우) */
        printf("{\"seq\":%ld,\"t\":%lld,\"dt\":%lld,\"app\":\"%s\",\"ev\":\"%s\"%s%s}\n",
               L.seq, t, dt, L.app, ev,
               (json_kv && *json_kv) ? "," : "", nn(json_kv));
    }
}

/* ─── 수명 ────────────────────────────────────────────────────────────── */

void unim_log_init(const char *app_name, int argc, char **argv) {
    if (L.inited) return;

    memset(&L, 0, sizeof L);
    snprintf(L.app, sizeof L.app, "%s", nn(app_name));
    L.start_ms = now_ms();
    L.color    = isatty(STDOUT_FILENO);

    const char *fmt = getenv("UNIM_TEST_LOG_FORMAT");
    if (!fmt || !*fmt)                 L.format = FMT_HUMAN | FMT_JSON;
    else if (strcmp(fmt, "json") == 0) L.format = FMT_JSON;
    else if (strcmp(fmt, "human") == 0)L.format = FMT_HUMAN;
    else                               L.format = FMT_HUMAN | FMT_JSON;

    const char *lvl = getenv("UNIM_TEST_LOG_LEVEL");
    L.skip_key = (lvl && strcmp(lvl, "no-key") == 0);

    const char *path = getenv("UNIM_TEST_LOG");
    if (path && *path) {
        L.jsonl = fopen(path, "w");
        if (!L.jsonl)
            fprintf(stderr, "⚠️  UNIM_TEST_LOG 열기 실패: %s\n", path);
    }

    /* 파이프로 받아도 즉시 흐르게. `stdbuf -oL` 이 필요 없어야 한다. */
    setvbuf(stdout, NULL, _IOLBF, 0);

    L.inited = 1;

    char args[1024] = "";
    for (int i = 0; i < argc && argv; i++) {
        static char esc[256];
        unim_log_json_escape(argv[i], esc, sizeof esc);
        size_t used = strlen(args);
        snprintf(args + used, sizeof args - used, "%s\\\"%s\\\"",
                 i ? " " : "", esc);
    }

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv, "\"pid\":%d,\"argv\":\"%s\"", (int)getpid(), args);
    emit("app.start", kv, L.app);
}

void unim_log_ready(void) {
    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv, "\"boot_ms\":%lld", now_ms() - L.start_ms);
    emit("app.ready", kv, "UI 준비 완료 — 키 주입 가능");
}

void unim_log_shutdown(void) {
    if (!L.inited) return;
    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv, "\"uptime_ms\":%lld,\"events\":%ld",
             now_ms() - L.start_ms, L.seq);
    emit("app.exit", kv, "종료");
    if (L.jsonl) { fclose(L.jsonl); L.jsonl = NULL; }
    fflush(stdout);
    L.inited = 0;
}

long unim_log_seq(void) { return L.seq; }

void unim_log_set_sink(UnimLogSink sink, void *user_data) {
    L.sink = sink;
    L.sink_user = user_data;
}

/* ─── 환경 ────────────────────────────────────────────────────────────── */

void unim_log_env(const char *toolkit_version) {
    static const char *const KEYS[] = {
        "GTK_IM_MODULE", "QT_IM_MODULE", "XMODIFIERS", "GDK_BACKEND",
        "QT_QPA_PLATFORM", "XDG_SESSION_TYPE", "WAYLAND_DISPLAY", "DISPLAY",
        "XDG_CURRENT_DESKTOP", "GTK_DEBUG", "LANG",
    };
    char kv[2048] = "";
    size_t used = 0;

    for (size_t i = 0; i < sizeof KEYS / sizeof KEYS[0]; i++) {
        const char *v = getenv(KEYS[i]);
        static char esc[256];
        unim_log_json_escape(v ? v : "", esc, sizeof esc);
        used += (size_t)snprintf(kv + used, sizeof kv - used,
                                 "%s\"%s\":\"%s\"", i ? "," : "", KEYS[i], esc);
        if (used >= sizeof kv) break;
    }
    char tv[128];
    unim_log_json_escape(nn(toolkit_version), tv, sizeof tv);
    snprintf(kv + used, sizeof kv - used, ",\"toolkit_version\":\"%s\"", tv);

    /* 사람용으로는 IM 경로 결정에 실제로 쓰이는 것만 추려서 보여준다 */
    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human,
             "GTK_IM_MODULE=%s QT_IM_MODULE=%s XMODIFIERS=%s "
             "GDK_BACKEND=%s SESSION=%s toolkit=%s",
             nn(getenv("GTK_IM_MODULE")), nn(getenv("QT_IM_MODULE")),
             nn(getenv("XMODIFIERS")), nn(getenv("GDK_BACKEND")),
             nn(getenv("XDG_SESSION_TYPE")), nn(toolkit_version));

    emit("env", kv, human);
}

/* ─── 키 ──────────────────────────────────────────────────────────────── */

void unim_log_key(const char *phase, unsigned keyval, unsigned keysym,
                  unsigned hw, unsigned state, const char *str, int filtered) {
    if (L.skip_key) return;

    static char esc[LOG_ESC_MAX];
    unim_log_json_escape(nn(str), esc, sizeof esc);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv,
             "\"keyval\":%u,\"keysym\":%u,\"hw_keycode\":%u,\"state\":%u,"
             "\"string\":\"%s\",\"filtered\":%d",
             keyval, keysym, hw, state, esc, filtered);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "keyval=0x%x hw=%u state=0x%x \"%s\" %s",
             keyval, hw, state, nn(str),
             filtered > 0 ? "→IM 삼킴" : filtered == 0 ? "→앱" : "");

    char ev[32];
    snprintf(ev, sizeof ev, "key.%s", nn(phase));
    emit(ev, kv, human);
}

void unim_log_im(const char *phase, const char *field,
                 const char *result, double elapsed_ms) {
    static char fe[256], re[256];
    unim_log_json_escape(nn(field), fe, sizeof fe);
    unim_log_json_escape(nn(result), re, sizeof re);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv,
             "\"field\":\"%s\",\"result\":\"%s\",\"elapsed_ms\":%.3f",
             fe, re, elapsed_ms);

    static char human[LOG_KV_MAX];
    if (elapsed_ms > 0)
        snprintf(human, sizeof human, "%s %s (%.1fms)",
                 nn(field), nn(result), elapsed_ms);
    else
        snprintf(human, sizeof human, "%s %s", nn(field), nn(result));

    char ev[32];
    snprintf(ev, sizeof ev, "im.filter.%s", nn(phase));
    emit(ev, kv, human);
}

/* ─── 조합·확정 ───────────────────────────────────────────────────────── */

void unim_log_preedit(const char *phase, const char *field,
                      const char *text, int cursor, const char *attrs) {
    static char te[LOG_ESC_MAX], fe[256], ae[256];
    unim_log_json_escape(nn(text), te, sizeof te);
    unim_log_json_escape(nn(field), fe, sizeof fe);
    unim_log_json_escape(nn(attrs), ae, sizeof ae);

    size_t chars = unim_log_utf8_len(text);
    size_t bytes = text ? strlen(text) : 0;

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv,
             "\"field\":\"%s\",\"text\":\"%s\",\"chars\":%zu,\"bytes\":%zu,"
             "\"cursor\":%d,\"attrs\":\"%s\"",
             fe, te, chars, bytes, cursor, ae);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "%-14s \"%s\" (%zu자 %zuB 커서%d)%s%s",
             nn(field), nn(text), chars, bytes, cursor,
             (attrs && *attrs) ? " " : "", nn(attrs));

    char ev[32];
    snprintf(ev, sizeof ev, "preedit.%s", nn(phase));
    emit(ev, kv, human);
}

void unim_log_commit(const char *field, const char *text) {
    static char te[LOG_ESC_MAX], fe[256];
    unim_log_json_escape(nn(text), te, sizeof te);
    unim_log_json_escape(nn(field), fe, sizeof fe);

    size_t chars = unim_log_utf8_len(text);
    size_t bytes = text ? strlen(text) : 0;

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv,
             "\"field\":\"%s\",\"text\":\"%s\",\"chars\":%zu,\"bytes\":%zu",
             fe, te, chars, bytes);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "%-14s \"%s\" (%zu자 %zuB)",
             nn(field), nn(text), chars, bytes);

    emit("commit", kv, human);
}

void unim_log_field_render(const char *field, const char *committed,
                           const char *preedit, int caret,
                           const char *rendered) {
    static char fe[256], ce[LOG_ESC_MAX], pe[LOG_ESC_MAX], re[LOG_ESC_MAX];
    unim_log_json_escape(nn(field), fe, sizeof fe);
    unim_log_json_escape(nn(committed), ce, sizeof ce);
    unim_log_json_escape(nn(preedit), pe, sizeof pe);
    unim_log_json_escape(nn(rendered), re, sizeof re);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv,
             "\"field\":\"%s\",\"committed\":\"%s\",\"preedit\":\"%s\","
             "\"caret\":%d,\"rendered\":\"%s\"",
             fe, ce, pe, caret, re);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "%-14s 화면=\"%s\"  확정=\"%s\" 조합=\"%s\" 캐럿%d",
             nn(field), nn(rendered), nn(committed), nn(preedit), caret);

    emit("field.render", kv, human);
}

/* ─── 포커스·클릭·리셋 ────────────────────────────────────────────────── */

void unim_log_focus(const char *phase, const char *field, const char *prev) {
    static char fe[256], pe[256];
    unim_log_json_escape(nn(field), fe, sizeof fe);
    unim_log_json_escape(nn(prev), pe, sizeof pe);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv, "\"field\":\"%s\",\"prev\":\"%s\"", fe, pe);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "%s%s%s", nn(field),
             (prev && *prev) ? "  ← " : "", nn(prev));

    char ev[32];
    snprintf(ev, sizeof ev, "focus.%s", nn(phase));
    emit(ev, kv, human);
}

void unim_log_click(int x, int y, const char *field,
                    int caret_before, int caret_after) {
    static char fe[256];
    unim_log_json_escape(nn(field), fe, sizeof fe);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv,
             "\"x\":%d,\"y\":%d,\"field\":\"%s\","
             "\"caret_before\":%d,\"caret_after\":%d",
             x, y, fe, caret_before, caret_after);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "(%d,%d) %s 캐럿 %d→%d",
             x, y, nn(field), caret_before, caret_after);

    emit("click", kv, human);
}

void unim_log_reset(const char *field, const char *reason) {
    static char fe[256], re[256];
    unim_log_json_escape(nn(field), fe, sizeof fe);
    unim_log_json_escape(nn(reason), re, sizeof re);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv, "\"field\":\"%s\",\"reason\":\"%s\"", fe, re);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "%s (%s)", nn(field), nn(reason));

    emit("reset", kv, human);
}

/* ─── DBus·주변문맥 ───────────────────────────────────────────────────── */

void unim_log_dbus(const char *kind, const char *iface, const char *member,
                   const char *detail, double elapsed_ms) {
    static char ie[256], me[256], de[LOG_ESC_MAX];
    unim_log_json_escape(nn(iface), ie, sizeof ie);
    unim_log_json_escape(nn(member), me, sizeof me);
    unim_log_json_escape(nn(detail), de, sizeof de);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv,
             "\"iface\":\"%s\",\"member\":\"%s\",\"detail\":\"%s\","
             "\"elapsed_ms\":%.3f",
             ie, me, de, elapsed_ms);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "%s%s%s %s%s",
             nn(iface), (member && *member) ? "." : "", nn(member),
             nn(detail), elapsed_ms > 0 ? "" : "");
    if (elapsed_ms > 0) {
        size_t u = strlen(human);
        snprintf(human + u, sizeof human - u, " (%.1fms)", elapsed_ms);
    }

    char ev[32];
    snprintf(ev, sizeof ev, "dbus.%s", nn(kind));
    emit(ev, kv, human);
}

void unim_log_surrounding(const char *kind, const char *text, int cursor,
                          int offset, int n_chars) {
    static char te[LOG_ESC_MAX];
    unim_log_json_escape(nn(text), te, sizeof te);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv,
             "\"text\":\"%s\",\"cursor\":%d,\"offset\":%d,\"n_chars\":%d",
             te, cursor, offset, n_chars);

    static char human[LOG_KV_MAX];
    snprintf(human, sizeof human, "\"%s\" 커서%d offset%d n%d",
             nn(text), cursor, offset, n_chars);

    char ev[40];
    snprintf(ev, sizeof ev, "surrounding.%s", nn(kind));
    emit(ev, kv, human);
}

/* ─── 자유 진단 ───────────────────────────────────────────────────────── */

static void note_v(const char *ev, const char *fmt, va_list ap) {
    static char msg[LOG_TEXT_MAX];
    vsnprintf(msg, sizeof msg, fmt, ap);

    static char esc[LOG_ESC_MAX];
    unim_log_json_escape(msg, esc, sizeof esc);

    static char kv[LOG_KV_MAX];
    snprintf(kv, sizeof kv, "\"msg\":\"%s\"", esc);

    emit(ev, kv, msg);
}

void unim_log_note(const char *fmt, ...) {
    va_list ap; va_start(ap, fmt); note_v("note",  fmt, ap); va_end(ap);
}
void unim_log_warn(const char *fmt, ...) {
    va_list ap; va_start(ap, fmt); note_v("warn",  fmt, ap); va_end(ap);
}
void unim_log_error(const char *fmt, ...) {
    va_list ap; va_start(ap, fmt); note_v("error", fmt, ap); va_end(ap);
}

void unim_log_raw(const char *ev, const char *json_kv) {
    emit(nn(ev), json_kv, nn(json_kv));
}
