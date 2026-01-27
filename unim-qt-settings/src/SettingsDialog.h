/**
 * UNIM Qt6 Settings Dialog
 *
 * 입력기 설정을 위한 Qt6 기반 다이얼로그
 * unim-capi를 사용하여 설정 관리
 */

#ifndef SETTINGS_DIALOG_H
#define SETTINGS_DIALOG_H

#include <QDialog>
#include <QComboBox>
#include <QCheckBox>
#include <QSlider>
#include <QLabel>
#include <QGroupBox>

/* UNIM C API */
extern "C" {
#include "unim.h"
}

class SettingsDialog : public QDialog {
    Q_OBJECT

public:
    explicit SettingsDialog(QWidget *parent = nullptr);
    ~SettingsDialog() override;

private slots:
    void onKoreanLayoutChanged(int index);
    void onEnglishLayoutChanged(int index);
    void onAutoSwitchToggled(bool checked);
    void onThresholdChanged(int value);

private:
    void setupUI();
    void loadConfig();
    bool saveConfig();
    void updateThresholdLabel();
    void loadStyleSheet();

    // 섹션 생성 메서드
    QGroupBox* createLayoutSection();
    QGroupBox* createAutoSwitchSection();

    // UI 위젯
    QComboBox *m_koreanLayoutCombo;
    QComboBox *m_englishLayoutCombo;
    QCheckBox *m_autoSwitchCheck;
    QSlider *m_thresholdSlider;
    QLabel *m_thresholdLabel;

    // UNIM 설정 객체
    UnimConfig *m_config;

    // 설정 상태
    int m_koreanLayout;     // 0: 두벌식, 1: 세벌식390, 2: 세벌식391
    int m_englishLayout;    // 0: QWERTY, 1: Dvorak
    bool m_autoSwitchEnabled;
    double m_autoSwitchThreshold;
};

#endif // SETTINGS_DIALOG_H
