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
        const imeGroup = new Adw.PreferencesGroup({
            title: _('실시간 입력기 (IME)'),
            description: _('실시간 한글 입력기 활성화 (IBus 대체)')
        });
        inputPage.add(imeGroup);

        this._addToggle(
            imeGroup,
            settings,
            'enable-ime',
            _('IME 모드 활성화'),
            _('Clutter.InputMethod 기반 실시간 한글 입력 — 활성화 시 IBus를 대체합니다')
        );

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
                ['per_app', _('앱별 독립')],
                ['per_window', _('창별 독립')]
            ],
            true
        );

        // Note about config sync
        const noteGroup = new Adw.PreferencesGroup();
        inputPage.add(noteGroup);
        
        const noteRow = new Adw.ActionRow({
            title: _('ℹ️ 설정 동기화'),
            subtitle: _('레이아웃 변경 시 ~/.config/unim/config.yaml에 자동 저장됩니다.')
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

        // Normal Mode Shortcuts
        const normalGroup = new Adw.PreferencesGroup({
            title: _('일반 변환'),
            description: _('선택한 텍스트를 변환 후 붙여넣기')
        });
        typefixPage.add(normalGroup);

        this._addShortcutRow(normalGroup, settings, 'shortcut-normal',
            _('영어 → 한글'), _('gksrmf → 한글'));
        this._addShortcutRow(normalGroup, settings, 'shortcut-normal-reverse',
            _('한글 → 영어'), _('ㅗ디ㅣㅐ → hello'));

        // Terminal Mode Shortcuts
        const terminalGroup = new Adw.PreferencesGroup({
            title: _('터미널 변환'),
            description: _('Backspace 처리 후 붙여넣기 (터미널 호환)')
        });
        typefixPage.add(terminalGroup);

        this._addShortcutRow(terminalGroup, settings, 'shortcut-terminal',
            _('터미널 (E → K)'), null);
        this._addShortcutRow(terminalGroup, settings, 'shortcut-terminal-reverse',
            _('터미널 (K → E)'), null);

        // Copy Only Shortcuts
        const copyGroup = new Adw.PreferencesGroup({
            title: _('복사만'),
            description: _('변환 후 클립보드에 복사 (붙여넣기 안함)')
        });
        typefixPage.add(copyGroup);

        this._addShortcutRow(copyGroup, settings, 'shortcut-copy-only',
            _('복사 (E → K)'), null);
        this._addShortcutRow(copyGroup, settings, 'shortcut-copy-only-reverse',
            _('복사 (K → E)'), null);
    }

    // Sync layout settings to ~/.config/unim/config.yaml
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
            
            const modeSharingMap = {
                'global': 'Global',
                'per_app': 'PerApp',
                'per_window': 'PerWindow'
            };
            const modeSharing = modeSharingMap[settings.get_string('mode-sharing')] || 'Global';
            
            const yamlContent = `# UNIM Configuration (synced from GNOME Extension)
engine:
  default_category: ${initialMode}
  mode_sharing: ${modeSharing}
  korean:
    layout: ${koreanLayout}
    preedit_johab: false
    word_commit: false
  english:
    layout: ${englishLayout}
    preferred_direct: true
  auto_switch:
    enabled: false
    threshold: 0.8
    show_notification: true
`;
            
            GLib.file_set_contents(configPath, yamlContent);
            unimLog('PREFS', 'Config synced: ' + configPath);
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
        row.add_suffix(toggle);
        row.activatable_widget = toggle;
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
