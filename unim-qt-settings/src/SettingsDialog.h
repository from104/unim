/**
 * UNIM Qt6 Settings Dialog
 * 
 * 입력기 설정을 위한 Qt6 기반 다이얼로그
 */

#ifndef SETTINGS_DIALOG_H
#define SETTINGS_DIALOG_H

#include <QDialog>
#include <QComboBox>
#include <QCheckBox>
#include <QSlider>
#include <QLabel>
#include <QPushButton>

class SettingsDialog : public QDialog {
    Q_OBJECT

public:
    explicit SettingsDialog(QWidget *parent = nullptr);
    ~SettingsDialog() override = default;

private slots:
    void onHangulLayoutChanged(int index);
    void onLatinLayoutChanged(int index);
    void onAutoSwitchToggled(bool checked);
    void onThresholdChanged(int value);
    void onSave();
    void onCancel();

private:
    void setupUI();
    void loadConfig();
    bool saveConfig();
    void updateThresholdLabel();
    QString getConfigPath();

    // UI 위젯
    QComboBox *m_hangulLayoutCombo;
    QComboBox *m_latinLayoutCombo;
    QCheckBox *m_autoSwitchCheck;
    QSlider *m_thresholdSlider;
    QLabel *m_thresholdLabel;
    QPushButton *m_saveButton;
    QPushButton *m_cancelButton;

    // 설정 상태
    int m_hangulLayout;     // 0: 두벌식, 1: 세벌식390, 2: 세벌식391
    int m_latinLayout;      // 0: QWERTY, 1: Dvorak
    bool m_autoSwitchEnabled;
    double m_autoSwitchThreshold;
};

#endif // SETTINGS_DIALOG_H
