/*
 * UNIM Qt 입력기 테스트 — Qt5/Qt6 통합 소스
 *
 * --auto  : 자동 회귀 테스트 (GUI 없이 DBus 테스트 후 종료)
 * --verbose / -v : 상세 출력
 *
 * CMake에서 Qt5/Qt6 각각의 바이너리로 빌드됩니다.
 */

#include <QApplication>
#include <QWidget>
#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGridLayout>
#include <QLabel>
#include <QLineEdit>
#include <QTextEdit>
#include <QSpinBox>
#include <QPushButton>
#include <QGroupBox>
#include <QFrame>
#include <QScrollArea>
#include <QDateTime>
#include <QInputMethodEvent>
#include <QDebug>
#include <QDBusConnection>
#include <QDBusInterface>
#include <QDBusReply>
#include <cstdlib>
#include <cstring>

/* 자동 테스트 (C 라이브러리) — Qt signals 매크로와 GLib 충돌 방지 */
#undef signals
extern "C" {
#include "unim_test.h"
}
#define signals Q_SIGNALS

/* Qt 버전 태그 */
#if QT_VERSION >= QT_VERSION_CHECK(6, 0, 0)
#define QT_TAG "[UNIM-QT6]"
#define QT_TITLE "UNIM Qt6 입력기 테스트"
#else
#define QT_TAG "[UNIM-QT5]"
#define QT_TITLE "UNIM Qt5 입력기 테스트"
#endif

/* ─── 디버그 로깅 ─────────────────────────────────────────────── */

static bool unim_debug_enabled = false;

#define UNIM_DEBUG(...) \
    do { if (unim_debug_enabled) qDebug() << QT_TAG << __VA_ARGS__; } while (0)

static void unim_check_debug_env() {
    static bool checked = false;
    if (!checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && strcmp(env, "1") == 0) {
            unim_debug_enabled = true;
            qDebug() << QT_TAG << "디버그 모드 활성화 (UNIM_DEVELOP=1)";
        }
        checked = true;
    }
}

/* ─── 자동 테스트 ─────────────────────────────────────────────── */

static int run_auto_test(bool verbose) {
    long log_mark = unim_test_log_mark();

    UnimTestRunner *runner = unim_test_runner_new(verbose);
    if (!runner) return 1;

    unim_test_run_suite(runner, UNIM_TEST_SUITE_AUTO);
    unim_test_log_check(runner, log_mark);

    int ret = runner->failed > 0 ? 1 : 0;
    unim_test_runner_free(runner);
    return ret;
}

/* ─── 메인 윈도우 ─────────────────────────────────────────────── */

class TestWindow : public QWidget {
    Q_OBJECT

public:
    TestWindow(QWidget *parent = nullptr) : QWidget(parent) {
        unim_check_debug_env();
        setWindowTitle(QT_TITLE);
        resize(700, 800);

        setupUI();
        setupDBus();

        logMessage(QString("%1 앱 시작").arg(QT_TITLE));
        QString imModule = qEnvironmentVariable("QT_IM_MODULE");
        logMessage(QString("QT_IM_MODULE=%1").arg(imModule.isEmpty() ? "(unset)" : imModule));
        queryInitialMode();
    }

private slots:
    void onClearLog() { logTextEdit->clear(); }

    void onFocusChanged(QWidget * /*old*/, QWidget *now) {
        if (now && !now->property("wn").isNull()) {
            QString name = now->property("wn").toString();
            focusLabel->setText(name);
            logMessage(QString("Focus: %1").arg(name));
        }
    }

    void onTextChanged(const QString &text) {
        QObject *s = QObject::sender();
        if (s) logMessage(QString("%1: \"%2\"").arg(s->property("wn").toString(), text));
    }

    void onGlobalModeChanged(bool isHangul) {
        updateModeDisplay(isHangul);
        logMessage(QString("모드: %1").arg(isHangul ? "한글" : "영문"));
    }

    void onToggleMode() {
        if (!m_dbus) return;
        QDBusReply<bool> reply = m_dbus->call("GetGlobalMode");
        if (reply.isValid()) m_dbus->call("SetGlobalMode", !reply.value());
    }

private:
    QLabel *focusLabel, *statusLabel, *dbusStatusLabel;
    QTextEdit *logTextEdit;
    QDBusInterface *m_dbus = nullptr;

    void setupDBus() {
        QDBusConnection bus = QDBusConnection::sessionBus();
        if (!bus.isConnected()) {
            dbusStatusLabel->setText("❌ DBus 실패");
            return;
        }

        m_dbus = new QDBusInterface(
            "org.atit.unim.InputMethod",
            "/org/atit/unim/InputMethod",
            "org.atit.unim.InputMethod",
            bus, this
        );

        if (!m_dbus->isValid()) {
            dbusStatusLabel->setText("❌ 데몬 없음");
            return;
        }

        bus.connect(
            "org.atit.unim.InputMethod",
            "/org/atit/unim/InputMethod",
            "org.atit.unim.InputMethod",
            "GlobalModeChanged",
            this, SLOT(onGlobalModeChanged(bool))
        );

        dbusStatusLabel->setText("✅ DBus 연결됨");
    }

    void queryInitialMode() {
        if (!m_dbus || !m_dbus->isValid()) { statusLabel->setText("(알 수 없음)"); return; }
        QDBusReply<bool> reply = m_dbus->call("GetGlobalMode");
        if (reply.isValid()) updateModeDisplay(reply.value());
    }

    void updateModeDisplay(bool isHangul) {
        statusLabel->setText(isHangul ? "🇰🇷 한글" : "🔤 영문");
        statusLabel->setStyleSheet(isHangul
            ? "color: #3498db; font-weight: bold;"
            : "color: #27ae60; font-weight: bold;");
    }

    void setupUI() {
        auto *main = new QVBoxLayout(this);
        main->setContentsMargins(0, 0, 0, 0);
        main->setSpacing(0);

        auto *scroll = new QScrollArea;
        scroll->setWidgetResizable(true);
        scroll->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);

        auto *content = new QWidget;
        auto *cl = new QVBoxLayout(content);
        cl->setSpacing(10);
        cl->setContentsMargins(10, 10, 10, 10);

        /* 상태 패널 */
        auto *sg = new QGroupBox("입력기 상태");
        auto *grid = new QGridLayout(sg);
        grid->setColumnStretch(1, 1);

        grid->addWidget(new QLabel("DBus:"), 0, 0);
        dbusStatusLabel = new QLabel("(연결 중...)");
        grid->addWidget(dbusStatusLabel, 0, 1);

        grid->addWidget(new QLabel("포커스:"), 1, 0);
        focusLabel = new QLabel("(없음)");
        grid->addWidget(focusLabel, 1, 1);

        grid->addWidget(new QLabel("모드:"), 2, 0);
        statusLabel = new QLabel("(감지 중...)");
        grid->addWidget(statusLabel, 2, 1);

        cl->addWidget(sg);

        /* 입력 위젯들 */
        cl->addWidget(mkSep());
        cl->addWidget(new QLabel("<b>Entry 위젯</b>"));
        cl->addWidget(mkEntry("일반 Entry:", "Entry-일반", "한글/영문 입력"));
        cl->addWidget(mkEntry("숫자 Entry:", "Entry-숫자", "123"));
        cl->addWidget(mkPassword("비밀번호:", "Password"));
        cl->addWidget(mkSpin("SpinButton:", "Spin"));
        cl->addWidget(mkSep());
        cl->addWidget(new QLabel("<b>TextEdit</b>"));
        cl->addWidget(mkTextEdit("멀티라인", "TextEdit", 120));
        cl->addWidget(mkSep());

        /* 로그 */
        auto *lg = new QGroupBox("이벤트 로그");
        auto *ll = new QVBoxLayout(lg);
        logTextEdit = new QTextEdit;
        logTextEdit->setReadOnly(true);
        logTextEdit->setFont(QFont("Monospace"));
        ll->addWidget(logTextEdit);
        cl->addWidget(lg, 1);

        scroll->setWidget(content);
        main->addWidget(scroll);

        /* 버튼 바 */
        auto *bl = new QHBoxLayout;
        bl->setContentsMargins(10, 5, 10, 5);
        auto *toggleBtn = new QPushButton("한/영 전환");
        connect(toggleBtn, &QPushButton::clicked, this, &TestWindow::onToggleMode);
        bl->addWidget(toggleBtn);
        bl->addStretch();
        auto *clearBtn = new QPushButton("로그 지우기");
        connect(clearBtn, &QPushButton::clicked, this, &TestWindow::onClearLog);
        bl->addWidget(clearBtn);
        main->addLayout(bl);

        connect(qApp, &QApplication::focusChanged, this, &TestWindow::onFocusChanged);
    }

    /* 위젯 팩토리 */
    QFrame *mkSep() {
        auto *f = new QFrame; f->setFrameShape(QFrame::HLine); return f;
    }

    QWidget *mkEntry(const char *label, const char *name, const char *ph) {
        auto *w = new QWidget; auto *l = new QHBoxLayout(w);
        l->setContentsMargins(0, 0, 0, 0);
        auto *lb = new QLabel(label); lb->setFixedWidth(120); l->addWidget(lb);
        auto *e = new QLineEdit; e->setPlaceholderText(ph);
        e->setProperty("wn", name);
        connect(e, &QLineEdit::textChanged, this, &TestWindow::onTextChanged);
        l->addWidget(e);
        return w;
    }

    QWidget *mkPassword(const char *label, const char *name) {
        auto *w = new QWidget; auto *l = new QHBoxLayout(w);
        l->setContentsMargins(0, 0, 0, 0);
        auto *lb = new QLabel(label); lb->setFixedWidth(120); l->addWidget(lb);
        auto *e = new QLineEdit; e->setEchoMode(QLineEdit::Password);
        e->setProperty("wn", name);
        connect(e, &QLineEdit::textChanged, this, &TestWindow::onTextChanged);
        l->addWidget(e);
        return w;
    }

    QWidget *mkSpin(const char *label, const char *name) {
        auto *w = new QWidget; auto *l = new QHBoxLayout(w);
        l->setContentsMargins(0, 0, 0, 0);
        auto *lb = new QLabel(label); lb->setFixedWidth(120); l->addWidget(lb);
        auto *s = new QSpinBox; s->setRange(0, 100);
        s->setProperty("wn", name);
        l->addWidget(s); l->addStretch();
        return w;
    }

    QGroupBox *mkTextEdit(const char *title, const char *name, int h) {
        auto *g = new QGroupBox(title);
        auto *l = new QVBoxLayout(g);
        auto *t = new QTextEdit; t->setMinimumHeight(h);
        t->setProperty("wn", name);
        l->addWidget(t);
        return g;
    }

    void logMessage(const QString &msg) {
        if (logTextEdit)
            logTextEdit->append(QDateTime::currentDateTime().toString("[hh:mm:ss] ") + msg);
    }
};

#include "main.moc"

/* ─── main ────────────────────────────────────────────────────── */

int main(int argc, char *argv[]) {
    /* --auto 모드: GUI 없이 자동 테스트 */
    bool autoMode = false, verbose = false;
    for (int i = 1; i < argc; i++) {
        if (strcmp(argv[i], "--auto") == 0) autoMode = true;
        if (strcmp(argv[i], "--verbose") == 0 || strcmp(argv[i], "-v") == 0) verbose = true;
    }

    if (autoMode) {
        return run_auto_test(verbose);
    }

    /* GUI 모드 */
    QApplication app(argc, argv);
    TestWindow window;
    window.show();
    return app.exec();
}
