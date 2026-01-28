/**
 * UNIM Qt6 Settings Dialog - Implementation (Adwaita Redesign)
 */

#include "SettingsDialog.h"

#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QFrame>
#include <QMessageBox>
#include <QFont>
#include <QFile>
#include <QCoreApplication>
#include <QDir>

SettingsDialog::SettingsDialog(QWidget *parent)
    : QDialog(parent)
    , m_config(nullptr)
    , m_koreanLayout(0)
    , m_englishLayout(0)
    , m_autoSwitchEnabled(false)
    , m_autoSwitchThreshold(0.7)
{
    setWindowTitle(tr("UNIM Settings"));
    setMinimumWidth(480);
    loadConfig();
    loadStyleSheet();
    setupUI();
    
    // 창 크기 자동 조절
    adjustSize();
    setFixedWidth(width());
}

SettingsDialog::~SettingsDialog() {
    if (m_config) {
        unim_config_delete(m_config);
        m_config = nullptr;
    }
}

void SettingsDialog::loadStyleSheet() {
    QStringList paths = {
        QCoreApplication::applicationDirPath() + "/../src/style.qss",
        QCoreApplication::applicationDirPath() + "/style.qss",
        ":/style.qss"
    };

    for (const QString &path : paths) {
        QFile file(path);
        if (file.open(QIODevice::ReadOnly | QIODevice::Text)) {
            setStyleSheet(file.readAll());
            file.close();
            return;
        }
    }
}

void SettingsDialog::loadConfig() {
    m_config = unim_config_load();
    if (!m_config) {
        m_config = unim_config_default();
    }
    m_koreanLayout = static_cast<int>(unim_config_get_korean_layout(m_config));
    m_englishLayout = static_cast<int>(unim_config_get_english_layout(m_config));
    m_autoSwitchEnabled = unim_config_get_auto_switch_enabled(m_config);
    m_autoSwitchThreshold = static_cast<double>(unim_config_get_auto_switch_threshold(m_config));
}

bool SettingsDialog::saveConfig() {
    if (!m_config) return false;
    unim_config_set_korean_layout(m_config, static_cast<UnimKoreanLayout>(m_koreanLayout));
    unim_config_set_english_layout(m_config, static_cast<UnimEnglishLayout>(m_englishLayout));
    unim_config_set_auto_switch_enabled(m_config, m_autoSwitchEnabled);
    unim_config_set_auto_switch_threshold(m_config, static_cast<float>(m_autoSwitchThreshold));
    return unim_config_save(m_config);
}

// 행(Row) 생성 유틸리티
QWidget* createRow(const QString &labelText, QWidget *control) {
    auto *row = new QWidget();
    row->setObjectName("settingsRow");
    auto *layout = new QHBoxLayout(row);
    layout->setContentsMargins(16, 8, 16, 8);
    
    auto *label = new QLabel(labelText);
    layout->addWidget(label);
    layout->addStretch();
    layout->addWidget(control);
    
    return row;
}

void SettingsDialog::setupUI() {
    auto *mainLayout = new QVBoxLayout(this);
    mainLayout->setContentsMargins(24, 8, 24, 32);
    mainLayout->setSpacing(0);

    // --- Keyboard Layout Section ---
    auto *layoutTitle = new QLabel(tr("Keyboard Layout"));
    layoutTitle->setObjectName("sectionTitle");
    mainLayout->addWidget(layoutTitle);

    auto *layoutCard = new QFrame();
    layoutCard->setObjectName("settingsCard");
    auto *layoutCardLayout = new QVBoxLayout(layoutCard);
    layoutCardLayout->setContentsMargins(0, 4, 0, 4);
    layoutCardLayout->setSpacing(0);

    // Korean Row - 동적 생성
    m_koreanLayoutCombo = new QComboBox();
    size_t koreanCount = unim_korean_layout_count();
    for (size_t i = 0; i < koreanCount; i++) {
        UnimStr name = unim_korean_layout_display_name(unim_korean_layout_at(i));
        m_koreanLayoutCombo->addItem(QString::fromUtf8(reinterpret_cast<const char*>(name.ptr), static_cast<int>(name.len)));
    }
    m_koreanLayoutCombo->setCurrentIndex(m_koreanLayout);
    connect(m_koreanLayoutCombo, &QComboBox::currentIndexChanged, this, &SettingsDialog::onKoreanLayoutChanged);
    layoutCardLayout->addWidget(createRow(tr("Korean Layout"), m_koreanLayoutCombo));


    // English Row - 동적 생성
    m_englishLayoutCombo = new QComboBox();
    size_t englishCount = unim_english_layout_count();
    for (size_t i = 0; i < englishCount; i++) {
        UnimStr name = unim_english_layout_display_name(unim_english_layout_at(i));
        m_englishLayoutCombo->addItem(QString::fromUtf8(reinterpret_cast<const char*>(name.ptr), static_cast<int>(name.len)));
    }
    m_englishLayoutCombo->setCurrentIndex(m_englishLayout);
    connect(m_englishLayoutCombo, &QComboBox::currentIndexChanged, this, &SettingsDialog::onEnglishLayoutChanged);
    layoutCardLayout->addWidget(createRow(tr("English Layout"), m_englishLayoutCombo));

    mainLayout->addWidget(layoutCard);

    // --- Auto Switch Section ---
    auto *switchTitle = new QLabel(tr("Auto Switch"));
    switchTitle->setObjectName("sectionTitle");
    mainLayout->addWidget(switchTitle);

    auto *switchCard = new QFrame();
    switchCard->setObjectName("settingsCard");
    auto *switchCardLayout = new QVBoxLayout(switchCard);
    switchCardLayout->setContentsMargins(0, 4, 0, 4);
    switchCardLayout->setSpacing(0);

    // Enable Row
    m_autoSwitchCheck = new QCheckBox();
    m_autoSwitchCheck->setChecked(m_autoSwitchEnabled);
    m_autoSwitchCheck->setCursor(Qt::PointingHandCursor);
    connect(m_autoSwitchCheck, &QCheckBox::toggled, this, &SettingsDialog::onAutoSwitchToggled);
    switchCardLayout->addWidget(createRow(tr("Enable Auto Switch"), m_autoSwitchCheck));


    // Threshold Row
    auto *thresholdContainer = new QWidget();
    auto *thresholdLayout = new QHBoxLayout(thresholdContainer);
    thresholdLayout->setContentsMargins(0, 0, 0, 0);
    thresholdLayout->setSpacing(12);

    m_thresholdSlider = new QSlider(Qt::Horizontal);
    m_thresholdSlider->setRange(0, 100);
    m_thresholdSlider->setValue(static_cast<int>(m_autoSwitchThreshold * 100));
    m_thresholdSlider->setEnabled(m_autoSwitchEnabled);
    m_thresholdSlider->setMinimumWidth(120);
    connect(m_thresholdSlider, &QSlider::valueChanged, this, &SettingsDialog::onThresholdChanged);
    thresholdLayout->addWidget(m_thresholdSlider);

    m_thresholdLabel = new QLabel();
    m_thresholdLabel->setObjectName("thresholdLabel");
    m_thresholdLabel->setEnabled(m_autoSwitchEnabled);
    updateThresholdLabel();
    thresholdLayout->addWidget(m_thresholdLabel);

    switchCardLayout->addWidget(createRow(tr("Threshold"), thresholdContainer));

    mainLayout->addWidget(switchCard);
    mainLayout->addStretch();
}

void SettingsDialog::updateThresholdLabel() {
    m_thresholdLabel->setText(QString("%1%").arg(static_cast<int>(m_autoSwitchThreshold * 100)));
}

void SettingsDialog::onKoreanLayoutChanged(int index) {
    m_koreanLayout = index;
    saveConfig();
}

void SettingsDialog::onEnglishLayoutChanged(int index) {
    m_englishLayout = index;
    saveConfig();
}

void SettingsDialog::onAutoSwitchToggled(bool checked) {
    m_autoSwitchEnabled = checked;
    m_thresholdSlider->setEnabled(checked);
    m_thresholdLabel->setEnabled(checked);
    saveConfig();
}

void SettingsDialog::onThresholdChanged(int value) {
    m_autoSwitchThreshold = value / 100.0;
    updateThresholdLabel();
    saveConfig();
}
