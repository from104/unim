/**
 * UNIM Qt 테스트 앱 (Qt5 · Qt6 공용 소스 한 벌 → 바이너리 두 개)
 *
 * 화면·필드 동작·로그는 tests/common 의 공용 코드가 정한다. GTK 판과 화면이
 * 같아야 하며(같은 `unim_test_spec.h`), 다른 것은 툴킷 API 뿐이다.
 *
 * 코어 필드는 `QWidget` + `inputMethodEvent()` 직결이다 — `QLineEdit` 는
 * preedit 을 앱에 노출하지 않으므로 화면의 진실을 로그로 남길 수 없다
 * (TEST_APPS.md §2).
 *
 * 실행:
 *   QT_IM_MODULE=unim unim-test-qt6
 *   unim-test-qt6 --auto           DBus 스모크만 돌리고 종료
 */

/* ⚠️ glib/gio 헤더를 Qt 보다 **먼저** 넣는다.
 *
 * Qt 는 `signals`·`slots` 를 매크로로 정의하는데, gio 의
 * `gdbusintrospection.h` 에 `signals` 라는 구조체 멤버가 있어 뒤에 포함하면
 * 깨진다. 순서를 바꾸지 말 것. */
#include <glib.h>

#include "unim_test.h"
#include "unim_test_dbus.h"
#include "unim_test_field.h"
#include "unim_test_log.h"
#include "unim_test_spec.h"

#include <QApplication>
#include <QFontMetrics>
#include <QGridLayout>
#include <QGuiApplication>
#include <QHBoxLayout>
#include <QInputMethod>
#include <QInputMethodEvent>
#include <QKeyEvent>
#include <QLabel>
#include <QLineEdit>
#include <QMouseEvent>
#include <QPainter>
#include <QPlainTextEdit>
#include <QTextEdit>
#include <QTimer>
#include <QVBoxLayout>
#include <QWidget>

#include <cstring>
#include <functional>

#if QT_VERSION_MAJOR >= 6
#  define APP_NAME "qt6"
#else
#  define APP_NAME "qt5"
#endif

static QColor col(unsigned rgb) {
    return QColor((rgb >> 16) & 0xff, (rgb >> 8) & 0xff, rgb & 0xff);
}

/* ─── 코어 필드 캔버스 ────────────────────────────────────────────────── */

class Canvas : public QWidget {
public:
    UnimTestField fields[UNIM_SPEC_N_CORE_FIELDS];
    int           active = 0;
    char          lastCommit[512] = "";
    std::function<void()> onChange;

    explicit Canvas(QWidget *parent = nullptr) : QWidget(parent) {
        setFocusPolicy(Qt::StrongFocus);
        setAttribute(Qt::WA_InputMethodEnabled, true);
        setAutoFillBackground(false);

        for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++)
            unim_field_init(&fields[i], &UNIM_SPEC_CORE_FIELDS[i]);

        int bottom = unim_field_layout(fields, UNIM_SPEC_N_CORE_FIELDS, 0,
                                       UNIM_SPEC_WIN_WIDTH, 1.0);
        setMinimumHeight(bottom);

        QFont f(UNIM_SPEC_FONT_UI, UNIM_SPEC_FONT_SIZE_FIELD);
        setFont(f);
        unim_field_set_focus(&fields[0], 1, nullptr);
    }

    UnimTestField *cur() { return &fields[active]; }

    /** 확정 텍스트 앞부분의 폭 — 캐럿·IM 커서 위치 계산용. */
    int measure(const char *utf8, size_t n) const {
        QFontMetrics fm(font());
        return fm.horizontalAdvance(QString::fromUtf8(utf8, int(n)));
    }

    /**
     * 창 내부 좌표계의 필드 기하를 로그로 낸다.
     *
     * 상대(`x`,`y`,`cx`,`cy`)는 논리 단위 그대로 두고, 절대
     * (`screen_cx`,`screen_cy`)는 **물리 픽셀**로 낸다 — 하네스가 이 값을
     * `xdotool` 에 넘기는데 XTEST 는 물리 픽셀을 받기 때문이다.
     *
     * `mapToGlobal()` 은 논리 좌표를 돌려주므로 devicePixelRatio 를 곱한다.
     * 이걸 빼먹어 HiDPI(배율 2)에서 클릭이 정확히 절반 지점에 떨어졌고,
     * 필드를 옮기는 시나리오(click-commit·password·multiline)만 실패했다
     * — 2026-08-09 실측. GTK 판도 같은 함정을 밟았다.
     */
    void emitGeometry() {
        QPoint o = mapTo(window(), QPoint(0, 0));
        QPoint g = mapToGlobal(QPoint(0, 0));
        const double dpr = devicePixelRatioF();
        for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++) {
            const UnimTestField *f = &fields[i];
            char kv[640];
            g_snprintf(kv, sizeof kv,
                       "\"field\":\"%s\",\"x\":%d,\"y\":%d,\"w\":%d,\"h\":%d,"
                       "\"cx\":%d,\"cy\":%d,\"screen_cx\":%d,\"screen_cy\":%d",
                       f->id, o.x() + f->x, o.y() + f->y, f->w, f->h,
                       o.x() + f->x + f->w / 2, o.y() + f->y + f->h / 2,
                       int((g.x() + f->x + f->w / 2) * dpr),
                       int((g.y() + f->y + f->h / 2) * dpr));
            unim_log_raw("geometry", kv);
        }
    }

    void focusField(int idx, const char *reason) {
        if (idx == active) return;
        UnimTestField *old = cur();
        const char *prevId = old->id;
        if (old->composing || old->preedit[0]) {
            unim_log_reset(old->id, reason);
            QGuiApplication::inputMethod()->reset();
        }
        unim_field_set_focus(old, 0, nullptr);
        active = idx;
        unim_field_set_focus(cur(), 1, prevId);
        changed();
    }

    void changed() {
        QGuiApplication::inputMethod()->update(Qt::ImCursorRectangle);
        update();
        if (onChange) onChange();
    }

protected:
    /**
     * Tab 을 가로챈다.
     *
     * Qt 는 Tab 을 **위젯 간 포커스 이동**으로 먼저 처리해서 `keyPressEvent`
     * 까지 오지 않는다. 그대로 두면 캔버스가 포커스를 잃어 코어 필드 순환이
     * 동작하지 않는다(2026-08-08 하네스가 focus-switch 실패로 잡았다).
     * GTK 판은 key-pressed 에서 먼저 가로채므로 같은 문제가 없다.
     */
    bool event(QEvent *e) override {
        if (e->type() == QEvent::KeyPress) {
            auto *ke = static_cast<QKeyEvent *>(e);
            if (ke->key() == Qt::Key_Tab || ke->key() == Qt::Key_Backtab) {
                keyPressEvent(ke);
                return true;
            }
        }
        return QWidget::event(e);
    }

    /* ── IM ── */

    void inputMethodEvent(QInputMethodEvent *e) override {
        UnimTestField *f = cur();

        if (!e->commitString().isEmpty()) {
            QByteArray c = e->commitString().toUtf8();
            g_snprintf(lastCommit, sizeof lastCommit, "%s", c.constData());
            unim_field_commit(f, c.constData());
        }

        QByteArray p = e->preeditString().toUtf8();
        if (!p.isEmpty()) {
            if (!f->composing) unim_field_preedit_start(f);
            /* Qt 는 preedit 커서를 attribute 로 준다. 없으면 끝으로 본다. */
            int caret = -1;
            for (const auto &a : e->attributes())
                if (a.type == QInputMethodEvent::Cursor)
                    caret = int(QString::fromUtf8(p.constData(), a.start)
                                    .toUtf8().size());
            unim_field_set_preedit(f, p.constData(), caret);
        } else if (f->composing || f->preedit[0]) {
            unim_field_preedit_end(f);
        }

        e->accept();
        changed();
    }

    QVariant inputMethodQuery(Qt::InputMethodQuery q) const override {
        const UnimTestField *f = &fields[active];
        switch (q) {
        case Qt::ImEnabled:
            return true;
        case Qt::ImCursorRectangle: {
            char before[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
            unim_field_before_caret(f, before, sizeof before);
            int x = f->x + UNIM_SPEC_FIELD_PAD_X + measure(before, strlen(before));
            return QRect(x, f->y, 2, f->h);
        }
        case Qt::ImSurroundingText:
            return QString::fromUtf8(f->committed);
        case Qt::ImCursorPosition:
            return int(unim_log_utf8_len(
                QByteArray(f->committed, f->caret).constData()));
        case Qt::ImAnchorPosition:
            return inputMethodQuery(Qt::ImCursorPosition);
        case Qt::ImHints:
            /* 힌트를 정확히 넘겨야 비밀번호 필드의 AutoTypeFix·팝업 억제가
             * 실제로 시험된다. */
            switch (f->hint) {
            case UNIM_HINT_PASSWORD:
                return int(Qt::ImhHiddenText | Qt::ImhSensitiveData |
                           Qt::ImhNoPredictiveText);
            case UNIM_HINT_NUMBER:    return int(Qt::ImhDigitsOnly);
            case UNIM_HINT_MULTILINE: return int(Qt::ImhMultiLine);
            default:                  return int(Qt::ImhNone);
            }
        default:
            return QWidget::inputMethodQuery(q);
        }
    }

    /* ── 키 ── */

    void keyPressEvent(QKeyEvent *e) override {
        QByteArray t = e->text().toUtf8();
        const char *utf8 = t.isEmpty() ? nullptr : t.constData();
        unim_log_key("press", uint(e->key()), uint(e->nativeVirtualKey()),
                     uint(e->nativeScanCode()), uint(e->modifiers()), utf8, 0);
        /* 여기까지 왔다는 것은 IM 이 이 키를 먹지 않았다는 뜻이다.
         * IM 이 먹은 키는 inputMethodEvent 로만 나타난다. */
        unim_log_im("leave", cur()->id, "앱으로", 0);

        if (e->key() == Qt::Key_Tab || e->key() == Qt::Key_Backtab) {
            int dir = (e->modifiers() & Qt::ShiftModifier) ? -1 : 1;
            focusField((active + dir + UNIM_SPEC_N_CORE_FIELDS)
                           % UNIM_SPEC_N_CORE_FIELDS, "tab");
            return;
        }

        UnimTestField *f = cur();
        switch (e->key()) {
        case Qt::Key_Backspace: unim_field_backspace(f);      break;
        case Qt::Key_Delete:    unim_field_delete(f);         break;
        case Qt::Key_Left:      unim_field_move_caret(f, -1); break;
        case Qt::Key_Right:     unim_field_move_caret(f, +1); break;
        case Qt::Key_Home:      unim_field_caret_home(f);     break;
        case Qt::Key_End:       unim_field_caret_end(f);      break;
        case Qt::Key_Escape:    unim_field_clear(f);          break;
        case Qt::Key_Return:
        case Qt::Key_Enter:
            if (f->hint == UNIM_HINT_MULTILINE) unim_field_insert(f, "\n");
            else focusField((active + 1) % UNIM_SPEC_N_CORE_FIELDS, "enter");
            break;
        default:
            if (utf8 && (unsigned char)utf8[0] >= 0x20 &&
                (unsigned char)utf8[0] != 0x7f)
                unim_field_insert(f, utf8);
            else
                unim_log_note("%s: 처리하지 않은 키 key=0x%x", f->id, e->key());
        }
        changed();
    }

    /* ── 마우스 ── */

    void mousePressEvent(QMouseEvent *e) override {
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
        QPoint p = e->position().toPoint();
#else
        QPoint p = e->pos();
#endif
        setFocus(Qt::MouseFocusReason);

        int hit = unim_field_hit(fields, UNIM_SPEC_N_CORE_FIELDS, p.x(), p.y());
        if (hit < 0) {
            unim_log_click(p.x(), p.y(), "(빈 곳)", -1, -1);
            return;
        }
        if (hit != active) {
            focusField(hit, "click");
        } else if (cur()->composing || cur()->preedit[0]) {
            unim_log_reset(cur()->id, "click-in-field");
            QGuiApplication::inputMethod()->reset();
        }

        UnimTestField *f = cur();
        int before = f->caret;
        f->caret = unim_field_caret_from_x(
            f, p.x(),
            [](const char *s, size_t n, void *u) {
                return static_cast<Canvas *>(u)->measure(s, n);
            },
            this);
        unim_log_click(p.x(), p.y(), f->id, before, f->caret);
        unim_field_log_render(f);
        changed();
    }

    /* ── 포커스 ── */

    void focusInEvent(QFocusEvent *) override {
        unim_field_set_focus(cur(), 1, "(네이티브 위젯)");
        changed();
    }

    void focusOutEvent(QFocusEvent *) override {
        unim_log_reset(cur()->id, "canvas-focus-out");
        QGuiApplication::inputMethod()->reset();
        unim_field_set_focus(cur(), 0, nullptr);
        changed();
    }

    /* ── 그리기 ── */

    void paintEvent(QPaintEvent *) override {
        QPainter pt(this);
        pt.fillRect(rect(), col(UNIM_SPEC_COL_BG));
        for (int i = 0; i < UNIM_SPEC_N_CORE_FIELDS; i++)
            drawField(pt, &fields[i]);
    }

private:
    void drawField(QPainter &pt, const UnimTestField *f) {
        bool focused = hasFocus() && f->focused;

        QFont labelFont(UNIM_SPEC_FONT_UI, UNIM_SPEC_FONT_SIZE_UI);
        pt.setFont(labelFont);
        pt.setPen(col(UNIM_SPEC_COL_LABEL));
        pt.drawText(QRect(UNIM_SPEC_MARGIN, f->y, UNIM_SPEC_LABEL_COL_W, f->h),
                    Qt::AlignLeft | Qt::AlignTop, QString::fromUtf8(f->label));

        QRect box(f->x, f->y, f->w, f->h);
        pt.fillRect(box, col(focused ? UNIM_SPEC_COL_FIELD_FOCUS
                                     : UNIM_SPEC_COL_FIELD_BG));
        pt.setPen(QPen(col(focused ? UNIM_SPEC_COL_BORDER_FOCUS
                                   : UNIM_SPEC_COL_BORDER),
                       focused ? 2 : 1));
        pt.drawRect(box.adjusted(0, 0, -1, -1));

        char shown[UNIM_FIELD_TEXT_MAX + UNIM_FIELD_PREEDIT_MAX];
        unim_field_display(f, shown, sizeof shown);
        QString text = QString::fromUtf8(shown);

        QFont fieldFont(UNIM_SPEC_FONT_UI, UNIM_SPEC_FONT_SIZE_FIELD);
        pt.setFont(fieldFont);
        QFontMetrics fm(fieldFont);

        int tx = f->x + UNIM_SPEC_FIELD_PAD_X;
        int ty = f->y + (f->hint == UNIM_HINT_MULTILINE ? 6 : 8);
        QRect textRect(tx, ty, f->w - 2 * UNIM_SPEC_FIELD_PAD_X, f->h);
        int flags = Qt::AlignLeft | Qt::AlignTop |
                    (f->hint == UNIM_HINT_MULTILINE ? Qt::TextWordWrap : 0);

        /* 확정 부분과 조합 부분을 나눠 그린다 — 조합은 색과 밑줄로 구분한다. */
        int caretChars = caretCharPos(f);
        int preChars   = preeditCharLen(f);
        QString head = text.left(caretChars);
        QString pre  = text.mid(caretChars, preChars);
        QString tail = text.mid(caretChars + preChars);

        pt.setPen(col(UNIM_SPEC_COL_TEXT));
        if (f->hint == UNIM_HINT_MULTILINE) {
            pt.drawText(textRect, flags, text);
        } else {
            int x = tx;
            pt.drawText(x, ty + fm.ascent(), head);
            x += fm.horizontalAdvance(head);
            if (!pre.isEmpty()) {
                QFont uf = fieldFont;
                uf.setUnderline(true);
                pt.setFont(uf);
                pt.setPen(col(UNIM_SPEC_COL_PREEDIT));
                pt.drawText(x, ty + fm.ascent(), pre);
                x += fm.horizontalAdvance(pre);
                pt.setFont(fieldFont);
                pt.setPen(col(UNIM_SPEC_COL_TEXT));
            }
            pt.drawText(x, ty + fm.ascent(), tail);
        }

        if (focused) {
            int cx = tx + fm.horizontalAdvance(
                             text.left(caretChars + preeditCaretCharPos(f)));
            pt.fillRect(QRect(cx, ty, 2, fm.height()),
                        col(UNIM_SPEC_COL_CARET));
        }
    }

    /* 화면 문자열 기준 위치들 — 비밀번호는 마스킹돼 문자 수가 달라진다. */
    static int caretCharPos(const UnimTestField *f) {
        return int(unim_log_utf8_len(
            QByteArray(f->committed, f->caret).constData()));
    }
    static int preeditCharLen(const UnimTestField *f) {
        return int(unim_log_utf8_len(f->preedit));
    }
    static int preeditCaretCharPos(const UnimTestField *f) {
        return int(unim_log_utf8_len(
            QByteArray(f->preedit, f->preedit_caret).constData()));
    }
};

/* ─── 창 ──────────────────────────────────────────────────────────────── */

class Window : public QWidget {
public:
    Canvas         *canvas;
    QLabel         *statusVal[UNIM_STATUS_N];
    QPlainTextEdit *logView;
    UnimTestDaemon *daemon = nullptr;

    Window() {
        setWindowTitle(QString::fromUtf8(UNIM_SPEC_WIN_TITLE_FMT)
                           .arg(QString::fromUtf8(APP_NAME))
                           .replace("%s", APP_NAME));
        resize(UNIM_SPEC_WIN_WIDTH, UNIM_SPEC_WIN_HEIGHT);

        auto *root = new QVBoxLayout(this);
        root->setContentsMargins(UNIM_SPEC_MARGIN, UNIM_SPEC_MARGIN,
                                 UNIM_SPEC_MARGIN, UNIM_SPEC_MARGIN);
        root->setSpacing(UNIM_SPEC_SECTION_GAP);

        root->addWidget(sectionTitle("① 상태"));
        root->addLayout(buildStatusPanel());

        root->addWidget(sectionTitle("② 코어 필드 (IM 직결 · 직접 그리기)"));
        canvas = new Canvas(this);
        canvas->onChange = [this] { refreshStatus(); };
        root->addWidget(canvas);

        root->addWidget(sectionTitle("③ 네이티브 위젯 (툴킷 기본)"));
        for (int i = 0; i < UNIM_SPEC_N_NATIVE; i++)
            root->addLayout(buildNativeRow(&UNIM_SPEC_NATIVE[i]));

        root->addWidget(sectionTitle("④ 로그"));
        logView = new QPlainTextEdit(this);
        logView->setReadOnly(true);
        logView->setMaximumBlockCount(UNIM_SPEC_LOG_LINES);
        logView->setFont(QFont(UNIM_SPEC_FONT_MONO, UNIM_SPEC_FONT_SIZE_LOG));
        logView->setMinimumHeight(UNIM_SPEC_LOG_H);
        root->addWidget(logView, 1);
    }

    void refreshStatus() {
        UnimStatusInput in{};
        in.frontend      = APP_NAME;
        in.im_path       = nullptr;
        in.focus_field   = canvas->hasFocus() ? canvas->cur()->id
                                              : "(네이티브 위젯)";
        in.preedit       = canvas->cur()->preedit;
        in.preedit_caret = canvas->cur()->preedit_caret;
        in.last_commit   = canvas->lastCommit;

        char vals[UNIM_STATUS_N][UNIM_STATUS_VALUE_MAX];
        unim_status_render(daemon, &in, vals);
        for (int i = 0; i < UNIM_STATUS_N; i++)
            statusVal[i]->setText(QString::fromUtf8(vals[i]));
    }

    void appendLog(const char *line) {
        logView->appendPlainText(QString::fromUtf8(line));
    }

private:
    static QLabel *sectionTitle(const char *t) {
        auto *l = new QLabel(QString("<b>%1</b>").arg(QString::fromUtf8(t)));
        return l;
    }

    QGridLayout *buildStatusPanel() {
        auto *grid = new QGridLayout();
        grid->setHorizontalSpacing(12);
        grid->setVerticalSpacing(4);
        for (int i = 0; i < UNIM_STATUS_N; i++) {
            auto *key = new QLabel(QString::fromUtf8(UNIM_SPEC_STATUS_LABELS[i]));
            key->setMinimumWidth(UNIM_SPEC_STATUS_LABEL_W);
            grid->addWidget(key, i, 0);
            statusVal[i] = new QLabel("…");
            statusVal[i]->setTextInteractionFlags(Qt::TextSelectableByMouse);
            grid->addWidget(statusVal[i], i, 1);
        }
        grid->setColumnStretch(1, 1);
        return grid;
    }

    QHBoxLayout *buildNativeRow(const UnimSpecNative *spec) {
        auto *row = new QHBoxLayout();
        auto *lab = new QLabel(QString::fromUtf8(spec->label));
        lab->setMinimumWidth(UNIM_SPEC_LABEL_COL_W);
        row->addWidget(lab);

        if (spec->kind == UNIM_NATIVE_MULTILINE) {
            auto *te = new QTextEdit(this);
            te->setObjectName(QString::fromUtf8(spec->id));
            te->setMaximumHeight(60);
            row->addWidget(te, 1);
        } else {
            auto *le = new QLineEdit(this);
            le->setObjectName(QString::fromUtf8(spec->id));
            if (spec->kind == UNIM_NATIVE_PASSWORD)
                le->setEchoMode(QLineEdit::Password);
            const char *id = spec->id;
            /* QLineEdit 는 preedit 을 앱에 안 준다 — 확정된 내용만 관측된다. */
            QObject::connect(le, &QLineEdit::textChanged,
                             [id](const QString &s) {
                                 QByteArray b = s.toUtf8();
                                 unim_log_field_render(id, b.constData(), "",
                                                       int(b.size()),
                                                       b.constData());
                             });
            row->addWidget(le, 1);
        }
        return row;
    }
};

static Window *g_win = nullptr;

static void log_sink(const char *line, void *) {
    if (g_win) g_win->appendLog(line);
}

static void on_daemon_changed(void *) {
    if (g_win) g_win->refreshStatus();
}

/* ─── main ────────────────────────────────────────────────────────────── */

int main(int argc, char *argv[]) {
    unim_log_init(APP_NAME, argc, argv);

    bool autoMode = false, verbose = false;
    for (int i = 1; i < argc; i++) {
        if (!std::strcmp(argv[i], "--auto")) autoMode = true;
        if (!std::strcmp(argv[i], "-v") || !std::strcmp(argv[i], "--verbose"))
            verbose = true;
    }

    char tv[64];
    g_snprintf(tv, sizeof tv, "Qt %s", qVersion());
    unim_log_env(tv);

    if (autoMode) {
        unim_log_note("--auto: DBus 스모크만 돌리고 종료한다 "
                      "(프런트엔드 경로 회귀는 tests/harness 가 본다)");
        UnimTestRunner *r = unim_test_runner_new(verbose);
        if (!r) { unim_log_error("러너 생성 실패"); unim_log_shutdown(); return 2; }
        unim_test_run_suite(r, UNIM_TEST_SUITE_AUTO);
        unim_test_print_summary(r);
        int ret = r->failed > 0 ? 1 : 0;
        unim_test_runner_free(r);
        unim_log_shutdown();
        return ret;
    }

    /*
     * HiDPI 배율을 Qt 에 맡긴다 — 켜야 위젯 좌표가 **논리 단위**가 되어
     * GTK 판·스펙(760x960 @96dpi)과 같은 화면이 나온다.
     *
     * Qt6 는 이 동작이 기본이자 강제라 속성 자체가 폐지됐지만, Qt5 는 기본이
     * **꺼져** 있어 폰트만 커지고 레이아웃은 물리 픽셀로 남는다. 그래서 같은
     * 소스인데 qt5 창만 855px(≠760)로 어긋나 있었다 — 2026-08-09 실측.
     * QApplication 생성 **전**에 설정해야 효력이 있다.
     */
#if QT_VERSION < QT_VERSION_CHECK(6, 0, 0)
    QApplication::setAttribute(Qt::AA_EnableHighDpiScaling);
    QApplication::setAttribute(Qt::AA_UseHighDpiPixmaps);
#endif

    QApplication app(argc, argv);

    /*
     * 창 전체를 스펙 색으로 칠한다.
     *
     * 코어 필드 캔버스는 우리가 직접 그려서 이미 스펙 색이지만, 창 배경·상태
     * 패널·네이티브 위젯·로그 패널은 Qt 기본 팔레트(대개 밝은색)를 따라가
     * GTK 판과 화면이 어긋났다. GTK 는 시스템 다크 테마를 따라가는 반면 Qt 는
     * 그러지 않기 때문이다. 팔레트를 스펙에서 직접 만들어 6개 앱을 맞춘다.
     */
    QPalette pal;
    pal.setColor(QPalette::Window,          col(UNIM_SPEC_COL_BG));
    pal.setColor(QPalette::WindowText,      col(UNIM_SPEC_COL_TEXT));
    pal.setColor(QPalette::Base,            col(UNIM_SPEC_COL_FIELD_BG));
    pal.setColor(QPalette::AlternateBase,   col(UNIM_SPEC_COL_PANEL));
    pal.setColor(QPalette::Text,            col(UNIM_SPEC_COL_TEXT));
    pal.setColor(QPalette::Button,          col(UNIM_SPEC_COL_PANEL));
    pal.setColor(QPalette::ButtonText,      col(UNIM_SPEC_COL_TEXT));
    pal.setColor(QPalette::ToolTipBase,     col(UNIM_SPEC_COL_PANEL));
    pal.setColor(QPalette::ToolTipText,     col(UNIM_SPEC_COL_TEXT));
    pal.setColor(QPalette::Highlight,       col(UNIM_SPEC_COL_BORDER_FOCUS));
    pal.setColor(QPalette::HighlightedText, col(UNIM_SPEC_COL_BG));
    pal.setColor(QPalette::PlaceholderText, col(UNIM_SPEC_COL_LABEL));
    pal.setColor(QPalette::Disabled, QPalette::Text, col(UNIM_SPEC_COL_LABEL));
    pal.setColor(QPalette::Disabled, QPalette::WindowText, col(UNIM_SPEC_COL_LABEL));
    app.setPalette(pal);

    /* Fusion 은 팔레트를 그대로 따르는 스타일이다. 플랫폼 기본 스타일 중에는
     * 팔레트를 무시하고 자기 색을 쓰는 것이 있어 명시적으로 고정한다. */
    app.setStyle("Fusion");

    Window w;
    g_win = &w;
    unim_log_set_sink(log_sink, nullptr);

    w.daemon = unim_daemon_connect(on_daemon_changed, nullptr);
    w.show();

    /**
     * 공용 DBus 모듈은 GDBus(gio) 를 쓴다. Qt 이벤트 루프는 GMainContext 를
     * 돌리지 않으므로 시그널이 도착하지 않는다 — 주기적으로 펌프해 준다.
     * (QtDBus 로 따로 구현하면 상태 표시가 GTK 판과 어긋난다)
     */
    QTimer glibPump;
    QObject::connect(&glibPump, &QTimer::timeout, [] {
        while (g_main_context_pending(nullptr))
            g_main_context_iteration(nullptr, FALSE);
    });
    glibPump.start(50);

    QTimer::singleShot(200, [&w] {
        w.canvas->setFocus();
        w.canvas->emitGeometry();
        w.refreshStatus();
        unim_log_ready();
    });

    int ret = app.exec();

    unim_daemon_free(w.daemon);
    unim_log_shutdown();
    return ret;
}
