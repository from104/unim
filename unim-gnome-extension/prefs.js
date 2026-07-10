/**
 * UNIM GNOME Shell Extension — Preferences
 *
 * 단일 창구화(SSoT = config.yaml) 정책에 따라 GNOME Shell API에 직접
 * 의존하는 5개 설정만 노출한다. 자판·입력 모드·오타 교정 등 일반 설정은
 * `unim-settings-gtk`로 리다이렉트한다.
 */

import Gio from 'gi://Gio';
import Adw from 'gi://Adw';
import Gtk from 'gi://Gtk';
import GLib from 'gi://GLib';
import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';
import { unimLog, unimError } from './logging.js';

export default class UnimPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings();

        const page = new Adw.PreferencesPage({
            title: _('UNIM'),
            icon_name: 'input-keyboard-symbolic'
        });
        window.add(page);

        // ============================================
        // 일반 설정 리다이렉트
        // ============================================
        const generalGroup = new Adw.PreferencesGroup({
            title: _('일반 설정'),
            description: _('자판·입력 모드·오타 교정 등 일반 설정은 UNIM 설정 앱(unim-settings-gtk)에서 관리합니다.')
        });
        page.add(generalGroup);

        const launchRow = new Adw.ActionRow({
            title: _('UNIM 설정 앱 열기'),
            subtitle: _('자판, 모드, 단축키, 자동 교정 등 모든 일반 설정'),
            activatable: true
        });
        launchRow.add_suffix(new Gtk.Image({ icon_name: 'go-next-symbolic' }));
        launchRow.connect('activated', () => {
            try {
                Gio.Subprocess.new(
                    ['unim-settings-gtk'],
                    Gio.SubprocessFlags.NONE
                );
                unimLog('PREFS', 'unim-settings-gtk 실행');
                window.close();
            } catch (e) {
                unimError('PREFS', `unim-settings-gtk 실행 실패: ${e.message}`);
                try {
                    const toast = new Adw.Toast({
                        title: _('UNIM 설정 앱 실행 실패. 터미널에서 `unim-settings-gtk`를 직접 실행하세요.'),
                        timeout: 5
                    });
                    if (typeof window.add_toast === 'function') {
                        window.add_toast(toast);
                    }
                } catch (_toastErr) {
                    // Toast API 미지원 환경은 로그만
                }
            }
        });
        generalGroup.add(launchRow);

        // ============================================
        // 표시 (Shell API 의존)
        // ============================================
        const displayGroup = new Adw.PreferencesGroup({
            title: _('표시')
        });
        page.add(displayGroup);

        // 패널 클릭 동작 (왼쪽 클릭 한/영 전환 vs 메뉴)
        const clickOptions = new Gtk.StringList();
        clickOptions.append(_('왼쪽 클릭 = 한/영 전환'));
        clickOptions.append(_('왼쪽 클릭 = 메뉴 (GNOME 기본)'));
        const clickRow = new Adw.ComboRow({
            title: _('패널 클릭 동작'),
            subtitle: _('오른쪽 클릭은 항상 메뉴를 표시합니다.'),
            model: clickOptions,
        });
        const clickValues = ['toggle-mode', 'menu'];
        const currentClick = settings.get_string('panel-click-action');
        clickRow.set_selected(Math.max(0, clickValues.indexOf(currentClick)));
        clickRow.connect('notify::selected', () => {
            settings.set_string('panel-click-action', clickValues[clickRow.get_selected()]);
        });
        displayGroup.add(clickRow);

        this._addSwitch(displayGroup, settings, 'show-notification',
            _('변환 알림 표시'),
            _('오타 교정 시 알림 메시지 표시'));

        // ============================================
        // 실시간 입력기 (Wayland 전용)
        // ============================================
        const imeGroup = new Adw.PreferencesGroup({
            title: _('실시간 입력기'),
            description: _('Wayland 세션 전용. Clutter.InputMethod로 IBus를 대체합니다.')
        });
        page.add(imeGroup);

        const imeRow = this._addSwitch(imeGroup, settings, 'enable-ime',
            _('IME 모드 활성화'),
            _('Wayland 세션에서 한글 실시간 입력'));
        // Wayland가 아닐 때 비활성화
        const sessionType = GLib.getenv('XDG_SESSION_TYPE') || '';
        if (sessionType !== 'wayland') {
            imeRow.set_sensitive(false);
            imeRow.set_subtitle(_('Wayland 세션에서만 사용 가능합니다.'));
        }

        // ============================================
        // 변환 단축키 (Main.wm.addKeybinding)
        // ============================================
        const shortcutGroup = new Adw.PreferencesGroup({
            title: _('변환 단축키'),
            description: _('포커스된 단어를 순/역방향으로 변환 후 교체')
        });
        page.add(shortcutGroup);

        this._addShortcutRow(shortcutGroup, settings, 'shortcut-normal',
            _('영어 → 한글'), _('gksrmf → 한글'));
        this._addShortcutRow(shortcutGroup, settings, 'shortcut-normal-reverse',
            _('한글 → 영어'), _('ㅗ디ㅣㅐ → hello'));

        // ============================================
        // 사용자 사전 등록 단축키
        // ============================================
        const userDictGroup = new Adw.PreferencesGroup({
            title: _('사용자 사전 등록'),
            description: _('선택한 한글을 영문으로 변환해 역방향 교정 사용자 사전에 추가합니다.')
        });
        page.add(userDictGroup);

        this._addShortcutRow(userDictGroup, settings, 'shortcut-register-userdict',
            _('선택 영역 등록'), _('ㅎㅑㅅ → git (CLI 명령어 등)'));
    }

    // --------------------------------------------
    // Helper: SwitchRow (returns row for post-tweak)
    // --------------------------------------------
    _addSwitch(group, settings, key, title, subtitle) {
        const row = new Adw.ActionRow({ title, subtitle: subtitle || '' });
        group.add(row);

        const toggle = new Gtk.Switch({
            active: settings.get_boolean(key),
            valign: Gtk.Align.CENTER
        });
        settings.bind(key, toggle, 'active', Gio.SettingsBindFlags.DEFAULT);

        row.add_suffix(toggle);
        row.activatable_widget = toggle;
        return row;
    }

    // --------------------------------------------
    // Helper: Shortcut row (entry + reset button)
    // --------------------------------------------
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
}
