/**
 * UNIM Qt6 Settings Dialog - Implementation
 * 
 * 입력기 설정을 위한 Qt6 기반 다이얼로그 구현
 */

#include "SettingsDialog.h"

#include <QVBoxLayout>
#include <QHBoxLayout>
#include <QGridLayout>
#include <QGroupBox>
#include <QFile>
#include <QDir>
#include <QTextStream>
#include <QStandardPaths>
#include <QMessageBox>
#include <QFont>

SettingsDialog::SettingsDialog(QWidget *parent)
    : QDialog(parent)
    , m_hangulLayout(0)
    , m_latinLayout(0)
    , m_autoSwitchEnabled(false)
    , m_autoSwitchThreshold(0.7)
{
    setWindowTitle(tr("UNIM Settings"));
    setFixedSize(420, 320);
    
    loadConfig();
    setupUI();
}

QString SettingsDialog::getConfigPath() {
    QString configDir = QStandardPaths::writableLocation(QStandardPaths::ConfigLocation);
    return configDir + "/unim/config.yaml";
}

void SettingsDialog::loadConfig() {
    QString path = getConfigPath();
    QFile file(path);
    
    if (!file.open(QIODevice::ReadOnly | QIODevice::Text)) {
        return;
    }
    
    QTextStream in(&file);
    while (!in.atEnd()) {
        QString line = in.readLine().trimmed();
        
        if (line.startsWith("layout:")) {
            QString value = line.mid(7).trimmed();
            if (value == "Dubeolsik") m_hangulLayout = 0;
            else if (value == "Sebeolsik390") m_hangulLayout = 1;
            else if (value == "Sebeolsik391") m_hangulLayout = 2;
        }
        else if (line.startsWith("enabled:")) {
            QString value = line.mid(8).trimmed();
            m_autoSwitchEnabled = (value == "true");
        }
        else if (line.startsWith("threshold:")) {
            QString value = line.mid(10).trimmed();
            m_autoSwitchThreshold = value.toDouble();
        }
    }
    
    file.close();
}

bool SettingsDialog::saveConfig() {
    QString path = getConfigPath();
    QDir dir = QFileInfo(path).dir();
    
    if (!dir.exists()) {
        dir.mkpath(".");
    }
    
    QFile file(path);
    if (!file.open(QIODevice::WriteOnly | QIODevice::Text)) {
        QMessageBox::critical(this, tr("Error"), tr("Could not save settings file."));
        return false;
    }
    
    QString hangulName;
    switch (m_hangulLayout) {
        case 1: hangulName = "Sebeolsik390"; break;
        case 2: hangulName = "Sebeolsik391"; break;
        default: hangulName = "Dubeolsik"; break;
    }
    
    QString latinName;
    switch (m_latinLayout) {
        case 1: latinName = "Dvorak"; break;
        default: latinName = "Qwerty"; break;
    }
    
    QTextStream out(&file);
    out << "engine:\n";
    out << "  default_category: Hangul\n";
    out << "  hangul:\n";
    out << "    layout: " << hangulName << "\n";
    out << "    preedit_johab: false\n";
    out << "    word_commit: false\n";
    out << "  latin:\n";
    out << "    layout: " << latinName << "\n";
    out << "    preferred_direct: true\n";
    out << "  auto_switch:\n";
    out << "    enabled: " << (m_autoSwitchEnabled ? "true" : "false") << "\n";
    out << QString("    threshold: %1\n").arg(m_autoSwitchThreshold, 0, 'f', 2);
    out << "    show_notification: false\n";
    
    file.close();
    return true;
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
    m_hangulLayoutCombo = new QComboBox();
    m_hangulLayoutCombo->addItem(tr("2-bul Standard"));
    m_hangulLayoutCombo->addItem(tr("3-bul 390"));
    m_hangulLayoutCombo->addItem(tr("3-bul Final"));
    m_hangulLayoutCombo->setCurrentIndex(m_hangulLayout);
    connect(m_hangulLayoutCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &SettingsDialog::onHangulLayoutChanged);
    gridLayout->addWidget(m_hangulLayoutCombo, row, 1);
    row++;
    
    gridLayout->addWidget(new QLabel(tr("English Layout:")), row, 0, Qt::AlignRight);
    m_latinLayoutCombo = new QComboBox();
    m_latinLayoutCombo->addItem("QWERTY");
    m_latinLayoutCombo->addItem("Dvorak");
    m_latinLayoutCombo->setCurrentIndex(m_latinLayout);
    connect(m_latinLayoutCombo, QOverload<int>::of(&QComboBox::currentIndexChanged),
            this, &SettingsDialog::onLatinLayoutChanged);
    gridLayout->addWidget(m_latinLayoutCombo, row, 1);
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

void SettingsDialog::onHangulLayoutChanged(int index) {
    m_hangulLayout = index;
}

void SettingsDialog::onLatinLayoutChanged(int index) {
    m_latinLayout = index;
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
    }
}

void SettingsDialog::onCancel() {
    reject();
}
