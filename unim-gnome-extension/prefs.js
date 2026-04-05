import Gio from 'gi://Gio';
import Adw from 'gi://Adw';
import Gtk from 'gi://Gtk';
import GLib from 'gi://GLib';
import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';
import { unimLog, unimError } from './logging.js';

export default class UnimPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings();
        
        // ============================================
        // Page 1: Input Method Settings
        // ============================================
        const inputPage = new Adw.PreferencesPage({
            title: _('입력기'),
            icon_name: 'input-keyboard-symbolic'
        });
        window.add(inputPage);

        // Panel Indicator Settings
        const indicatorGroup = new Adw.PreferencesGroup({
            title: _('패널 표시'),
            description: _('GNOME Shell 상단 패널에 한/영 상태 표시')
        });
        inputPage.add(indicatorGroup);

        this._addToggle(
            indicatorGroup,
            settings,
            'show-panel-indicator',
            _('패널 인디케이터 표시'),
            _('상단 패널에 한/영 상태 아이콘 표시')
        );

        // IME Mode Settings
        const isWayland = GLib.getenv('XDG_SESSION_TYPE') === 'wayland';

        const imeGroup = new Adw.PreferencesGroup({
            title: _('실시간 입력기 (IME)'),
            description: isWayland
                ? _('실시간 한글 입력기 활성화 (IBus 대체)')
                : _('⚠️ Wayland 세션 전용 기능입니다. X11에서는 GTK/Qt IM 모듈을 사용하세요.')
        });
        inputPage.add(imeGroup);

        const imeRow = new Adw.ActionRow({
            title: _('IME 모드 활성화'),
            subtitle: _('Clutter.InputMethod 기반 실시간 한글 입력 — 활성화 시 IBus를 대체합니다'),
            sensitive: isWayland
        });
        imeGroup.add(imeRow);

        const imeToggle = new Gtk.Switch({
            active: settings.get_boolean('enable-ime'),
            valign: Gtk.Align.CENTER,
            sensitive: isWayland
        });

        if (isWayland) {
            settings.bind('enable-ime', imeToggle, 'active', Gio.SettingsBindFlags.DEFAULT);
        }
        imeRow.add_suffix(imeToggle);
        imeRow.activatable_widget = isWayland ? imeToggle : null;

        // Keyboard Layout Settings
        const layoutGroup = new Adw.PreferencesGroup({
            title: _('키보드 레이아웃'),
            description: _('한글/영어 키보드 배열 설정 (변경 시 자동 저장)')
        });
        inputPage.add(layoutGroup);

        this._addCombo(
            layoutGroup,
            settings,
            'korean-layout',
            _('한글 레이아웃'),
            _('한글 키보드 배열'),
            [
                ['2bul', _('두벌식 표준')],
                ['3bul390', _('세벌식 390')],
                ['3bul391', _('세벌식 최종')],
                ['3bul_noshift', _('세벌식 순아래')]
            ],
            true
        );

        this._addCombo(
            layoutGroup,
            settings,
            'english-layout',
            _('영어 레이아웃'),
            _('영어 키보드 배열'),
            [
                ['qwerty', _('QWERTY')],
                ['dvorak', _('Dvorak')],
                ['colemak', _('Colemak')],
                ['colemak_dh', _('Colemak-DH')],
                ['workman', _('Workman')]
            ],
            true
        );

        this._addCombo(
            layoutGroup,
            settings,
            'initial-mode',
            _('초기 입력 모드'),
            _('세션 시작 시 기본 입력 모드'),
            [
                ['Korean', _('한글')],
                ['English', _('영문')]
            ],
            true
        );

        this._addCombo(
            layoutGroup,
            settings,
            'mode-sharing',
            _('모드 공유 방식'),
            _('앱 간 한/영 상태 공유 방식'),
            [
                ['global', _('전역 공유')],
                ['per_app', _('앱별 독립')]
            ],
            true
        );

        // Popup Mode
        const popupGroup = new Adw.PreferencesGroup({
            title: _('팝업 설정'),
            description: _('한자/특수문자 팝업 표시 방식')
        });
        inputPage.add(popupGroup);

        this._addCombo(
            popupGroup,
            settings,
            'popup-mode',
            _('팝업 표시 방식'),
            _('한자/특수문자 팝업 렌더링 방식'),
            [
                ['Standalone', _('독립형 (GUI)')],
                ['Embedded', _('내장형 (프론트엔드)')]
            ],
            true
        );

        // Note about config sync
        const noteGroup = new Adw.PreferencesGroup();
        inputPage.add(noteGroup);

        const noteRow = new Adw.ActionRow({
            title: _('ℹ️ 설정 동기화'),
            subtitle: _('설정 변경 시 ~/.config/unim/config.yaml에 자동 저장됩니다.')
        });
        noteGroup.add(noteRow);

        // ============================================
        // Page 2: TypeFix (Manual Han/Eng Typo Conversion)
        // ============================================
        const typefixPage = new Adw.PreferencesPage({
            title: _('오타 변환'),
            icon_name: 'preferences-desktop-keyboard-shortcuts-symbolic'
        });
        window.add(typefixPage);

        // ===== AutoTypeFix Section =====
        const autoGroup = new Adw.PreferencesGroup({
            title: _('자동 오타 교정 (AutoTypeFix)'),
            description: _('입력 중 한/영 오타를 실시간으로 감지하여 자동 교정합니다')
        });
        typefixPage.add(autoGroup);

        this._addToggle(autoGroup, settings, 'auto-typefix-enabled',
            _('자동 오타 교정 사용'), _('키스트로크 기반 실시간 한영 오타 감지 및 교정'));

        this._addToggle(autoGroup, settings, 'auto-typefix-direction-a',
            _('영→한 교정'), _('영어 모드에서 한글을 치려고 한 경우 자동 교정'));

        this._addToggle(autoGroup, settings, 'auto-typefix-direction-b',
            _('한→영 교정'), _('한글 모드에서 영어를 치려고 한 경우 자동 교정'));

        this._addSpinRow(autoGroup, settings, 'auto-typefix-kor-threshold',
            _('한글 음절 임계값'), _('영→한 교정 트리거에 필요한 완성 음절 수'), 2, 5, 1);

        this._addSpinRow(autoGroup, settings, 'auto-typefix-eng-min-length',
            _('영문 단어 최소 길이'), _('한→영 교정 트리거에 필요한 영문 단어 길이'), 5, 10, 1);

        this._addSpinRow(autoGroup, settings, 'auto-typefix-time-window',
            _('시간 윈도우 (ms)'), _('이 시간 내의 연속 키스트로크만 검사'), 500, 5000, 100);

        // ===== Manual TypeFix Section =====
        // Usage Guide
        const guideGroup = new Adw.PreferencesGroup({
            title: _('수동 한영 오타 변환'),
            description: _('한글 자판으로 영어를 입력했거나, 영어 자판으로 한글을 입력했을 때 변환합니다. 사용법: 잘못 입력한 텍스트를 선택(드래그) → 단축키 입력 → 변환된 텍스트로 자동 교체')
        });
        typefixPage.add(guideGroup);

        this._addToggle(
            guideGroup,
            settings,
            'show-notification',
            _('변환 알림 표시'),
            _('텍스트 변환 시 알림 표시')
        );

        // Shortcut Settings
        const shortcutGroup = new Adw.PreferencesGroup({
            title: _('변환 단축키'),
            description: _('선택한 텍스트를 변환 후 자동 교체')
        });
        typefixPage.add(shortcutGroup);

        this._addShortcutRow(shortcutGroup, settings, 'shortcut-normal',
            _('영어 → 한글'), _('gksrmf → 한글'));
        this._addShortcutRow(shortcutGroup, settings, 'shortcut-normal-reverse',
            _('한글 → 영어'), _('ㅗ디ㅣㅐ → hello'));
    }

    // Sync layout settings to ~/.config/unim/config.yaml
    // Reads existing config first, updates only changed fields (preserves toggle_keys, hanja_keys, etc.)
    _syncToConfigFile(settings) {
        try {
            const configDir = GLib.build_filenamev([GLib.get_home_dir(), '.config', 'unim']);
            const configPath = GLib.build_filenamev([configDir, 'config.yaml']);
            
            GLib.mkdir_with_parents(configDir, 0o755);
            
            const koreanLayoutMap = {
                '2bul': 'Dubeolsik',
                '3bul390': 'Sebeolsik390',
                '3bul391': 'Sebeolsik391',
                '3bul_noshift': 'SebeolsikNoShift'
            };
            
            const englishLayoutMap = {
                'qwerty': 'Qwerty',
                'dvorak': 'Dvorak',
                'colemak': 'Colemak',
                'colemak_dh': 'ColemakDh',
                'workman': 'Workman'
            };
            
            const koreanLayout = koreanLayoutMap[settings.get_string('korean-layout')] || 'Dubeolsik';
            const englishLayout = englishLayoutMap[settings.get_string('english-layout')] || 'Qwerty';
            const initialMode = settings.get_string('initial-mode') || 'English';
            const popupMode = settings.get_string('popup-mode') || 'Standalone';

            const modeSharingMap = {
                'global': 'Global',
                'per_app': 'PerApp'
            };
            const modeSharing = modeSharingMap[settings.get_string('mode-sharing')] || 'Global';
            
            // Read existing config to preserve fields not managed by GNOME Extension
            let existingContent = '';
            try {
                const [ok, contents] = GLib.file_get_contents(configPath);
                if (ok) {
                    existingContent = new TextDecoder().decode(contents);
                }
            } catch (_) {
                // File doesn't exist yet — will create from template
            }
            
            let yamlContent;
            if (existingContent.length > 0) {
                // Update only the fields we manage, preserving everything else
                const replacements = [
                    [/^(\s*default_category:\s*).*$/m, `$1${initialMode}`],
                    [/^(\s*mode_sharing:\s*).*$/m, `$1${modeSharing}`],
                    [/^(\s*layout:\s*)(?:Dubeolsik|Sebeolsik390|Sebeolsik391|SebeolsikNoShift)\s*$/m,
                        `$1${koreanLayout}`],
                    [/^(\s*layout:\s*)(?:Qwerty|Dvorak|Colemak|ColemakDh|Workman)\s*$/m,
                        `$1${englishLayout}`],
                    [/^(\s*popup_mode:\s*).*$/m, `$1${popupMode}`],
                ];
                
                yamlContent = existingContent;
                for (const [pattern, replacement] of replacements) {
                    yamlContent = yamlContent.replace(pattern, replacement);
                }
                // popup_mode 행이 없으면 engine: 블록 끝에 추가
                if (!/^\s*popup_mode:/m.test(yamlContent)) {
                    yamlContent = yamlContent.replace(
                        /^(engine:.*$)/m,
                        `$1\n  popup_mode: ${popupMode}`
                    );
                }
            } else {
                // No existing config — create default template
                yamlContent = `# UNIM Configuration
engine:
  default_category: ${initialMode}
  mode_sharing: ${modeSharing}
  korean:
    layout: ${koreanLayout}
  english:
    layout: ${englishLayout}
`;
            }
            
            GLib.file_set_contents(configPath, yamlContent);
            unimLog('PREFS', 'Config synced (merge): ' + configPath);
            return true;
        } catch (e) {
            unimError('PREFS', 'Sync failed: ' + e.message);
            return false;
        }
    }

    // Helper: Shortcut row
    _addShortcutRow(group, settings, key, title, subtitle) {
        const row = new Adw.ActionRow({ title, subtitle: subtitle || '' });
        group.add(row);

        const entry = new Gtk.Entry({
            text: settings.get_strv(key)[0] || '',
            valign: Gtk.Align.CENTER,
            width_chars: 20
        });

        entry.connect('changed', (e) => {
            const text = e.get_text();
            if (text) settings.set_strv(key, [text]);
        });

        settings.connect(`changed::${key}`, () => {
            const newVal = settings.get_strv(key)[0] || '';
            if (entry.get_text() !== newVal) entry.set_text(newVal);
        });

        row.add_suffix(entry);

        const resetBtn = new Gtk.Button({
            icon_name: 'edit-undo-symbolic',
            tooltip_text: _('기본값 복원'),
            valign: Gtk.Align.CENTER,
            css_classes: ['flat']
        });
        resetBtn.connect('clicked', () => settings.reset(key));
        row.add_suffix(resetBtn);
    }

    // Helper: Toggle switch
    _addToggle(group, settings, key, title, subtitle) {
        const row = new Adw.ActionRow({ title, subtitle });
        group.add(row);

        const toggle = new Gtk.Switch({
            active: settings.get_boolean(key),
            valign: Gtk.Align.CENTER
        });

        settings.bind(key, toggle, 'active', Gio.SettingsBindFlags.DEFAULT);

        // auto-typefix 관련 키는 config.yaml에도 동기화
        if (key.startsWith('auto-typefix-')) {
            toggle.connect('notify::active', () => {
                this._syncAutoTypeFixToConfig(settings);
            });
        }

        row.add_suffix(toggle);
        row.activatable_widget = toggle;
    }

    // Helper: Spin row (integer setting)
    _addSpinRow(group, settings, key, title, subtitle, min, max, step) {
        const row = new Adw.ActionRow({ title, subtitle });
        group.add(row);

        const adj = new Gtk.Adjustment({ lower: min, upper: max, step_increment: step, value: settings.get_uint(key) });
        const spin = new Gtk.SpinButton({ adjustment: adj, valign: Gtk.Align.CENTER, width_chars: 6 });

        settings.bind(key, spin, 'value', Gio.SettingsBindFlags.DEFAULT);

        // config.yaml 동기화
        spin.connect('value-changed', () => {
            this._syncAutoTypeFixToConfig(settings);
        });

        row.add_suffix(spin);
    }

    // AutoTypeFix 설정을 config.yaml에 동기화
    _syncAutoTypeFixToConfig(settings) {
        try {
            const configDir = GLib.build_filenamev([GLib.get_home_dir(), '.config', 'unim']);
            const configPath = GLib.build_filenamev([configDir, 'config.yaml']);

            GLib.mkdir_with_parents(configDir, 0o755);

            let content = '';
            try {
                const [ok, c] = GLib.file_get_contents(configPath);
                if (ok) content = new TextDecoder().decode(c);
            } catch (_) {}

            const atf = {
                enabled: settings.get_boolean('auto-typefix-enabled'),
                time_window_ms: settings.get_uint('auto-typefix-time-window'),
                kor_syllable_threshold: settings.get_uint('auto-typefix-kor-threshold'),
                eng_word_min_length: settings.get_uint('auto-typefix-eng-min-length'),
                direction_a: settings.get_boolean('auto-typefix-direction-a'),
                direction_b: settings.get_boolean('auto-typefix-direction-b'),
            };

            // auto_typefix 블록이 있으면 교체, 없으면 추가
            const atfYaml = `  auto_typefix:\n    enabled: ${atf.enabled}\n    time_window_ms: ${atf.time_window_ms}\n    kor_syllable_threshold: ${atf.kor_syllable_threshold}\n    eng_word_min_length: ${atf.eng_word_min_length}\n    direction_a: ${atf.direction_a}\n    direction_b: ${atf.direction_b}`;

            if (/^\s*auto_typefix:/m.test(content)) {
                // 기존 블록 교체 (auto_typefix: 부터 다음 비-들여쓰기 행까지)
                content = content.replace(
                    /^(\s*auto_typefix:)\n(?:\s{4,}\S.*\n)*/m,
                    atfYaml + '\n'
                );
            } else if (/^engine:/m.test(content)) {
                // engine: 블록 끝에 추가
                content = content.replace(/^(engine:.*$)/m, `$1\n${atfYaml}`);
            }

            GLib.file_set_contents(configPath, content);
            unimLog('PREFS', 'AutoTypeFix config synced');
        } catch (e) {
            unimError('PREFS', 'AutoTypeFix sync failed: ' + e.message);
        }
    }

    // Helper: Combo dropdown with optional config sync
    _addCombo(group, settings, key, title, subtitle, options, syncToConfig = false) {
        const row = new Adw.ActionRow({ title, subtitle });
        group.add(row);

        const model = new Gtk.StringList();
        options.forEach(([id, label]) => model.append(label));

        const combo = new Gtk.DropDown({ model, valign: Gtk.Align.CENTER });

        const currentId = settings.get_string(key);
        const index = options.findIndex(([id]) => id === currentId);
        if (index !== -1) combo.set_selected(index);

        combo.connect('notify::selected', () => {
            const idx = combo.get_selected();
            if (idx !== Gtk.INVALID_LIST_POSITION) {
                const [id] = options[idx];
                settings.set_string(key, id);
                
                if (syncToConfig) {
                    this._syncToConfigFile(settings);
                }
            }
        });

        row.add_suffix(combo);
    }
}
