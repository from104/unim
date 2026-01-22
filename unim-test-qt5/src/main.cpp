/*
 * UNIM Qt5 Input Method Test Application
 *
 * 다양한 Qt5 위젯에서 UNIM 입력기를 테스트하기 위한 앱입니다.
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
#include <QSplitter>
#include <QInputMethodEvent>
#include <QDebug>
#include <cstdlib>
#include <cstring>

/* 디버그 로깅 시스템 (GTK4 모듈과 동일한 패턴) */
static bool unim_debug_enabled = false;

#define UNIM_DEBUG(...) \
    do { \
        if (unim_debug_enabled) { \
            qDebug() << "[UNIM-QT5]" << __VA_ARGS__; \
        } \
    } while (0)

static void unim_check_debug_env()
{
    static bool checked = false;
    if (!checked) {
        const char *env = std::getenv("UNIM_DEVELOP");
        if (env && strcmp(env, "1") == 0) {
            unim_debug_enabled = true;
            qDebug() << "[UNIM-QT5] 디버그 모드 활성화 (UNIM_DEVELOP=1)";
        }
        checked = true;
    }
}

/* 메인 윈도우 클래스 */
class TestWindow : public QWidget {
    Q_OBJECT

public:
    TestWindow(QWidget *parent = nullptr) : QWidget(parent) {
        unim_check_debug_env();
        UNIM_DEBUG("TestWindow 초기화 시작");
        UNIM_DEBUG("Platform:" << QGuiApplication::platformName());

        setWindowTitle("UNIM Qt5 입력기 테스트");
        resize(700, 800);
        
        setupUI();
        
        // 초기 로그 메시지
        logMessage("UNIM Qt5 입력기 테스트 앱 시작");
        QString qtImModule = qEnvironmentVariable("QT_IM_MODULE");
        logMessage(QString("QT_IM_MODULE=%1").arg(qtImModule.isEmpty() ? "(unset)" : qtImModule));
        
        UNIM_DEBUG("TestWindow 초기화 완료");
    }

private slots:
    void onClearLog() {
        logTextEdit->clear();
    }
    
    void onFocusChanged(QWidget *old, QWidget *now) {
        (void)old;
        if (now && !now->property("widget-name").isNull()) {
            QString name = now->property("widget-name").toString();
            UNIM_DEBUG("포커스 변경:" << name);
            focusLabel->setText(name);
            logMessage(QString("Focus changed: %1").arg(name));
        }
    }
    
    void onTextChanged(const QString &text) {
        QObject *sender = QObject::sender();
        if (sender) {
            QString name = sender->property("widget-name").toString();
            UNIM_DEBUG("텍스트 변경:" << name << "->" << text);
            logMessage(QString("%1: text changed -> \"%2\"").arg(name, text));
        }
    }

private:
    QLabel *focusLabel;
    QLabel *statusLabel;
    QLabel *preeditLabel;
    QLabel *commitLabel;
    QTextEdit *logTextEdit;

    void setupUI() {
        QVBoxLayout *mainLayout = new QVBoxLayout(this);
        mainLayout->setContentsMargins(0, 0, 0, 0);
        mainLayout->setSpacing(0);
        
        // 스크롤 영역
        QScrollArea *scrollArea = new QScrollArea;
        scrollArea->setWidgetResizable(true);
        scrollArea->setHorizontalScrollBarPolicy(Qt::ScrollBarAlwaysOff);
        
        QWidget *contentWidget = new QWidget;
        QVBoxLayout *contentLayout = new QVBoxLayout(contentWidget);
        contentLayout->setSpacing(10);
        contentLayout->setContentsMargins(10, 10, 10, 10);
        
        // 상태 패널
        contentLayout->addWidget(createStatusPanel());
        
        // 구분선
        contentLayout->addWidget(createSeparator());
        
        // Entry 위젯 섹션
        QLabel *entryTitle = new QLabel("<b>Entry 위젯</b>");
        contentLayout->addWidget(entryTitle);
        
        contentLayout->addWidget(createEntryRow("일반 Entry:", "Entry-일반", "한글/영문 입력 테스트"));
        contentLayout->addWidget(createEntryRow("숫자 Entry:", "Entry-숫자", "123-456"));
        contentLayout->addWidget(createPasswordEntryRow("비밀번호:", "PasswordEntry"));
        contentLayout->addWidget(createSpinButtonRow("SpinButton:", "SpinButton"));
        
        // 구분선
        contentLayout->addWidget(createSeparator());
        
        // TextView 섹션
        QLabel *textViewTitle = new QLabel("<b>TextEdit 위젯</b>");
        contentLayout->addWidget(textViewTitle);
        
        contentLayout->addWidget(createTextViewFrame("멀티라인 입력", "TextEdit-멀티라인", 120));
        
        // 구분선
        contentLayout->addWidget(createSeparator());
        
        // 로그 패널
        contentLayout->addWidget(createLogPanel(), 1);
        
        scrollArea->setWidget(contentWidget);
        mainLayout->addWidget(scrollArea);
        
        // 하단 버튼 바
        QHBoxLayout *buttonLayout = new QHBoxLayout;
        buttonLayout->setContentsMargins(10, 5, 10, 5);
        buttonLayout->addStretch();
        
        QPushButton *clearBtn = new QPushButton("로그 지우기");
        connect(clearBtn, &QPushButton::clicked, this, &TestWindow::onClearLog);
        buttonLayout->addWidget(clearBtn);
        
        mainLayout->addLayout(buttonLayout);
        
        // 포커스 변경 시그널 연결
        connect(qApp, &QApplication::focusChanged, this, &TestWindow::onFocusChanged);
    }
    
    QFrame *createSeparator() {
        QFrame *line = new QFrame;
        line->setFrameShape(QFrame::HLine);
        line->setFrameShadow(QFrame::Sunken);
        return line;
    }
    
    QGroupBox *createStatusPanel() {
        QGroupBox *group = new QGroupBox("입력기 상태");
        QGridLayout *grid = new QGridLayout(group);
        grid->setColumnStretch(1, 1);
        
        // Focus
        grid->addWidget(new QLabel("현재 포커스:"), 0, 0);
        focusLabel = new QLabel("(없음)");
        grid->addWidget(focusLabel, 0, 1);
        
        // 입력 모드
        grid->addWidget(new QLabel("입력 모드:"), 1, 0);
        statusLabel = new QLabel("(감지 중...)");
        grid->addWidget(statusLabel, 1, 1);
        
        // Preedit
        grid->addWidget(new QLabel("Preedit:"), 2, 0);
        preeditLabel = new QLabel("");
        grid->addWidget(preeditLabel, 2, 1);
        
        // Last Commit
        grid->addWidget(new QLabel("Last Commit:"), 3, 0);
        commitLabel = new QLabel("");
        grid->addWidget(commitLabel, 3, 1);
        
        return group;
    }
    
    QWidget *createEntryRow(const QString &labelText, const QString &widgetName, const QString &placeholder) {
        QWidget *row = new QWidget;
        QHBoxLayout *layout = new QHBoxLayout(row);
        layout->setContentsMargins(0, 0, 0, 0);
        
        QLabel *label = new QLabel(labelText);
        label->setFixedWidth(120);
        layout->addWidget(label);
        
        QLineEdit *entry = new QLineEdit;
        entry->setPlaceholderText(placeholder);
        entry->setProperty("widget-name", widgetName);
        connect(entry, &QLineEdit::textChanged, this, &TestWindow::onTextChanged);
        layout->addWidget(entry);
        
        return row;
    }
    
    QWidget *createPasswordEntryRow(const QString &labelText, const QString &widgetName) {
        QWidget *row = new QWidget;
        QHBoxLayout *layout = new QHBoxLayout(row);
        layout->setContentsMargins(0, 0, 0, 0);
        
        QLabel *label = new QLabel(labelText);
        label->setFixedWidth(120);
        layout->addWidget(label);
        
        QLineEdit *entry = new QLineEdit;
        entry->setEchoMode(QLineEdit::Password);
        entry->setProperty("widget-name", widgetName);
        connect(entry, &QLineEdit::textChanged, this, &TestWindow::onTextChanged);
        layout->addWidget(entry);
        
        return row;
    }
    
    QWidget *createSpinButtonRow(const QString &labelText, const QString &widgetName) {
        QWidget *row = new QWidget;
        QHBoxLayout *layout = new QHBoxLayout(row);
        layout->setContentsMargins(0, 0, 0, 0);
        
        QLabel *label = new QLabel(labelText);
        label->setFixedWidth(120);
        layout->addWidget(label);
        
        QSpinBox *spin = new QSpinBox;
        spin->setRange(0, 100);
        spin->setProperty("widget-name", widgetName);
        layout->addWidget(spin);
        layout->addStretch();
        
        return row;
    }
    
    QGroupBox *createTextViewFrame(const QString &title, const QString &widgetName, int height) {
        QGroupBox *group = new QGroupBox(title);
        QVBoxLayout *layout = new QVBoxLayout(group);
        
        QTextEdit *textEdit = new QTextEdit;
        textEdit->setMinimumHeight(height);
        textEdit->setProperty("widget-name", widgetName);
        layout->addWidget(textEdit);
        
        return group;
    }
    
    QGroupBox *createLogPanel() {
        QGroupBox *group = new QGroupBox("이벤트 로그");
        QVBoxLayout *layout = new QVBoxLayout(group);
        
        logTextEdit = new QTextEdit;
        logTextEdit->setReadOnly(true);
        logTextEdit->setFont(QFont("Monospace"));
        layout->addWidget(logTextEdit);
        
        return group;
    }
    
    void logMessage(const QString &message) {
        if (logTextEdit) {
            QString timestamp = QDateTime::currentDateTime().toString("[hh:mm:ss] ");
            logTextEdit->append(timestamp + message);
        }
    }
};

#include "main.moc"

int main(int argc, char *argv[]) {
    QApplication app(argc, argv);
    
    TestWindow window;
    window.show();
    
    return app.exec();
}
