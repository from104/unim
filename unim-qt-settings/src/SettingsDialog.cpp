/**
 * UNIM Qt6 Settings Dialog - Implementation
 *
 * 입력기 설정을 위한 Qt6 기반 다이얼로그 구현
 * unim-capi를 사용하여 설정 관리
 */

#include "SettingsDialog.h"

#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGridLayout>
#include <QGroupBox>
#include <QMessageBox>
#include <QFont>

SettingsDialog::SettingsDialog(QWidget *parent)
    : QDialog(parent)
    , m_config(nullptr)
    , m_koreanLayout(0)
    , m_englishLayout(0)
    , m_autoSwitchEnabled(false)
    , m_autoSwitchThreshold(0.7)
{
    // API 버전 확인
    if (unim_api_version() != UNIM_API_VERSION) {
        qWarning("UNIM API version mismatch: expected %d, got %zu",
                 UNIM_API_VERSION, unim_api_version());
    }

    setWindowTitle(tr("UNIM Settings"));
    setFixedSize(420, 320);

    loadConfig();
    setupUI();
}

SettingsDialog::~SettingsDialog() {
    if (m_config) {
        unim_config_delete(m_config);
        m_config = nullptr;
    }
}

void SettingsDialog::loadConfig() {
    // C API로 설정 로드
    m_config = unim_config_load();
    if (!m_config) {
        qWarning("Failed to load config, using defaults");
        m_config = unim_config_default();
        return;
    }

    // C API로 값 읽기
    m_koreanLayout = static_cast<int>(unim_config_get_korean_layout(m_config));
    m_englishLayout = static_cast<int>(unim_config_get_english_layout(m_config));
    m_autoSwitchEnabled = unim_config_get_auto_switch_enabled(m_config);
    m_autoSwitchThreshold = static_cast<double>(unim_config_get_auto_switch_threshold(m_config));
}

bool SettingsDialog::saveConfig() {
    if (!m_config) {
        qWarning("No config object");
        return false;
    }

    // C API로 값 설정
    unim_config_set_korean_layout(m_config, static_cast<UnimKoreanLayout>(m_koreanLayout));
    unim_config_set_english_layout(m_config, static_cast<UnimEnglishLayout>(m_englishLayout));
    unim_config_set_auto_switch_enabled(m_config, m_autoSwitchEnabled);
    unim_config_set_auto_switch_threshold(m_config, static_cast<float>(m_autoSwitchThreshold));

    // C API로 저장
    return unim_config_save(m_config);
}

void SettingsDialog::setupUI() {
    auto *mainLayout = new QVBoxLayout(this);
    mainLayout->setSpacing(12);
    mainLayout->setContentsMargins(20, 20, 20, 20);

    auto *titleLabel = new QLabel(tr("UNIM Settings"));
    QFont titleFont = titleLabel->font();
    titleFont.setPointSize(14);
    titleFont.setBold(true);
    titleLabel->setFont(titleFont);
    titleLabel->setAlignment(Qt::AlignCenter);
    mainLayout->addWidget(titleLabel);

    auto *settingsGroup = new QGroupBox(tr("Keyboard Layout"));
    auto *gridLayout = new QGridLayout(settingsGroup);
    gridLayout->setSpacing(10);

    int row = 0;

    gridLayout->addWidget(new QLabel(tr("Korean Layout:")), row, 0, Qt::AlignRight);
    m_koreanLayoutCombo = new QComboBox();
    m_koreanLayoutCombo->addItem(tr("2-bul Standard"));
    m_koreanLayoutCombo->addItem(tr("3-bul 390"));
    m_koreanLayoutCombo->addItem(tr("3-bul Final"));
    m_koreanLayoutCombo->setCurrentIndex(m_koreanLayout);
    connect(m_koreanLayoutCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &SettingsDialog::onKoreanLayoutChanged);
    gridLayout->addWidget(m_koreanLayoutCombo, row, 1);
    row++;

    gridLayout->addWidget(new QLabel(tr("English Layout:")), row, 0, Qt::AlignRight);
    m_englishLayoutCombo = new QComboBox();
    m_englishLayoutCombo->addItem("QWERTY");
    m_englishLayoutCombo->addItem("Dvorak");
    m_englishLayoutCombo->setCurrentIndex(m_englishLayout);
    connect(m_englishLayoutCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &SettingsDialog::onEnglishLayoutChanged);
    gridLayout->addWidget(m_englishLayoutCombo, row, 1);
    row++;

    mainLayout->addWidget(settingsGroup);

    auto *autoSwitchGroup = new QGroupBox(tr("Auto Switch"));
    auto *autoLayout = new QGridLayout(autoSwitchGroup);
    autoLayout->setSpacing(10);

    row = 0;

    m_autoSwitchCheck = new QCheckBox(tr("Enable Auto Switch"));
    m_autoSwitchCheck->setChecked(m_autoSwitchEnabled);
    connect(m_autoSwitchCheck, &QCheckBox::toggled,
            this, &SettingsDialog::onAutoSwitchToggled);
    autoLayout->addWidget(m_autoSwitchCheck, row, 0, 1, 2);
    row++;

    autoLayout->addWidget(new QLabel(tr("Threshold:")), row, 0, Qt::AlignRight);
    auto *thresholdLayout = new QHBoxLayout();
    m_thresholdSlider = new QSlider(Qt::Horizontal);
    m_thresholdSlider->setRange(0, 100);
    m_thresholdSlider->setValue(static_cast<int>(m_autoSwitchThreshold * 100));
    m_thresholdSlider->setEnabled(m_autoSwitchEnabled);
    connect(m_thresholdSlider, &QSlider::valueChanged,
            this, &SettingsDialog::onThresholdChanged);
    thresholdLayout->addWidget(m_thresholdSlider);

    m_thresholdLabel = new QLabel();
    m_thresholdLabel->setMinimumWidth(45);
    updateThresholdLabel();
    thresholdLayout->addWidget(m_thresholdLabel);

    autoLayout->addLayout(thresholdLayout, row, 1);

    mainLayout->addWidget(autoSwitchGroup);

    mainLayout->addStretch();

    auto *buttonLayout = new QHBoxLayout();
    buttonLayout->addStretch();

    m_cancelButton = new QPushButton(tr("Cancel"));
    connect(m_cancelButton, &QPushButton::clicked, this, &SettingsDialog::onCancel);
    buttonLayout->addWidget(m_cancelButton);

    m_saveButton = new QPushButton(tr("Save"));
    m_saveButton->setDefault(true);
    connect(m_saveButton, &QPushButton::clicked, this, &SettingsDialog::onSave);
    buttonLayout->addWidget(m_saveButton);

    mainLayout->addLayout(buttonLayout);
}

void SettingsDialog::updateThresholdLabel() {
    m_thresholdLabel->setText(QString("%1%").arg(static_cast<int>(m_autoSwitchThreshold * 100)));
}

void SettingsDialog::onKoreanLayoutChanged(int index) {
    m_koreanLayout = index;
}

void SettingsDialog::onEnglishLayoutChanged(int index) {
    m_englishLayout = index;
}

void SettingsDialog::onAutoSwitchToggled(bool checked) {
    m_autoSwitchEnabled = checked;
    m_thresholdSlider->setEnabled(checked);
}

void SettingsDialog::onThresholdChanged(int value) {
    m_autoSwitchThreshold = value / 100.0;
    updateThresholdLabel();
}

void SettingsDialog::onSave() {
    if (saveConfig()) {
        QMessageBox::information(this, tr("Notice"), tr("Settings saved successfully."));
        accept();
    } else {
        QMessageBox::critical(this, tr("Error"), tr("Failed to save settings."));
    }
}

void SettingsDialog::onCancel() {
    reject();
}
