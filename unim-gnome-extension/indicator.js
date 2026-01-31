/**
 * UNIM Status Indicator for GNOME Shell Panel
 * 
 * 상태 표시 패널 버튼 - DBus를 통해 한/영 상태를 실시간으로 표시합니다.
 * 기존 unim-indicator의 트레이 메뉴 기능을 GNOME Shell 네이티브로 구현합니다.
 */

import GLib from 'gi://GLib';
import Gio from 'gi://Gio';
import GObject from 'gi://GObject';
import St from 'gi://St';
import Clutter from 'gi://Clutter';
import * as PanelMenu from 'resource:///org/gnome/shell/ui/panelMenu.js';
import * as PopupMenu from 'resource:///org/gnome/shell/ui/popupMenu.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

/** DBus 서비스 정보 */
const UNIM_BUS_NAME = 'org.atit.unim.InputMethod';
const UNIM_OBJECT_PATH = '/org/atit/unim/InputMethod';
const UNIM_INTERFACE = 'org.atit.unim.InputMethod';

/**
 * UNIM Panel Indicator
 * 
 * 상단 패널에 표시되는 한/영 상태 인디케이터
 */
export const UnimIndicator = GObject.registerClass(
class UnimIndicator extends PanelMenu.Button {
    _init(extension) {
        super._init(0.0, 'UNIM Indicator', false);
        
        this._extension = extension;
        this._settings = extension.getSettings();
        this._dbusProxy = null;
        this._dbusSignalId = 0;
        this._isKorean = true;
        this._connected = false;
        
        // 패널 버튼 - 박스로 구성 (아이콘 + 레이블)
        this._box = new St.BoxLayout({ style_class: 'panel-status-menu-box' });
        
        this._label = new St.Label({
            text: '한',
            y_align: Clutter.ActorAlign.CENTER,
            style_class: 'unim-indicator-label hangul-mode'
        });
        this._box.add_child(this._label);
        
        this.add_child(this._box);
        
        // 팝업 메뉴 생성
        this._buildMenu();
        
        // DBus 연결
        this._connectDbus();
        
        console.log('[unim-indicator] Panel indicator initialized');
    }
    
    _buildMenu() {
        // === 상태 표시 헤더 ===
        this._headerItem = new PopupMenu.PopupMenuItem('', { reactive: false });
        this._headerItem.label.set_style('font-weight: bold; font-size: 1.1em;');
        this.menu.addMenuItem(this._headerItem);
        
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        
        // === 입력 모드 선택 ===
        this._koreanItem = new PopupMenu.PopupMenuItem('한국어 모드 (Korean)');
        this._koreanItem.connect('activate', () => this._setMode(true));
        this.menu.addMenuItem(this._koreanItem);
        
        this._englishItem = new PopupMenu.PopupMenuItem('영어 모드 (English)');
        this._englishItem.connect('activate', () => this._setMode(false));
        this.menu.addMenuItem(this._englishItem);
        
        this.menu.addMenuItem(new PopupMenu.PopupSeparatorMenuItem());
        
        // === 설정 메뉴 ===
        const settingsItem = new PopupMenu.PopupMenuItem('설정 (Settings)...');
        settingsItem.connect('activate', () => this._openSettings());
        this.menu.addMenuItem(settingsItem);
        
        // === Extension 설정 ===
        const extSettingsItem = new PopupMenu.PopupMenuItem('Extension 설정...');
        extSettingsItem.connect('activate', () => this._openExtensionSettings());
        this.menu.addMenuItem(extSettingsItem);
        
        // 초기 메뉴 상태 업데이트
        this._updateMenuItems();
    }
    
    _updateMenuItems() {
        // 헤더 업데이트
        if (this._connected) {
            this._headerItem.label.set_text(this._isKorean ? '🇰🇷 한국어 입력 중' : '🔤 영어 입력 중');
        } else {
            this._headerItem.label.set_text('⚠️ UNIM 데몬 연결 안됨');
        }
        
        // 체크마크 표시
        this._koreanItem.setOrnament(this._isKorean ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
        this._englishItem.setOrnament(!this._isKorean ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
    }
    
    _updateLabel() {
        if (this._connected) {
            this._label.set_text(this._isKorean ? '한' : 'A');
            this._label.remove_style_class_name(this._isKorean ? 'english-mode' : 'hangul-mode');
            this._label.add_style_class_name(this._isKorean ? 'hangul-mode' : 'english-mode');
        } else {
            this._label.set_text('?');
            this._label.remove_style_class_name('hangul-mode');
            this._label.remove_style_class_name('english-mode');
            this._label.add_style_class_name('disconnected-mode');
        }
    }
    
    async _connectDbus() {
        try {
            this._dbusProxy = Gio.DBusProxy.new_for_bus_sync(
                Gio.BusType.SESSION,
                Gio.DBusProxyFlags.NONE,
                null,
                UNIM_BUS_NAME,
                UNIM_OBJECT_PATH,
                UNIM_INTERFACE,
                null
            );
            
            if (this._dbusProxy) {
                // 시그널 연결
                this._dbusSignalId = this._dbusProxy.connect('g-signal', 
                    (proxy, senderName, signalName, parameters) => {
                        if (signalName === 'GlobalModeChanged') {
                            const [isKorean] = parameters.deep_unpack();
                            this._onModeChanged(isKorean);
                        }
                    }
                );
                
                this._connected = true;
                console.log('[unim-indicator] DBus connected');
                
                // 초기 상태 조회
                this._fetchInitialMode();
            }
        } catch (e) {
            console.log(`[unim-indicator] DBus connection failed: ${e.message}`);
            this._connected = false;
            this._updateLabel();
            this._updateMenuItems();
            
            // 재시도 (5초 후)
            GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, 5, () => {
                if (!this._connected) {
                    console.log('[unim-indicator] Retrying DBus connection...');
                    this._connectDbus();
                }
                return GLib.SOURCE_REMOVE;
            });
        }
    }
    
    _fetchInitialMode() {
        try {
            const result = this._dbusProxy.call_sync(
                'GetGlobalMode',
                null,
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );
            
            if (result) {
                const [isKorean] = result.deep_unpack();
                this._onModeChanged(isKorean);
            }
        } catch (e) {
            console.log(`[unim-indicator] GetGlobalMode failed: ${e.message}`);
        }
    }
    
    _onModeChanged(isKorean) {
        this._isKorean = isKorean;
        this._updateLabel();
        this._updateMenuItems();
        console.log(`[unim-indicator] Mode changed: ${isKorean ? 'Korean' : 'English'}`);
    }
    
    _setMode(isKorean) {
        if (!this._dbusProxy || !this._connected) {
            Main.notify('UNIM', 'UNIM 데몬이 실행 중이지 않습니다.');
            return;
        }
        
        try {
            this._dbusProxy.call_sync(
                'SetGlobalMode',
                new GLib.Variant('(b)', [isKorean]),
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );
        } catch (e) {
            console.error(`[unim-indicator] SetGlobalMode failed: ${e.message}`);
        }
    }
    
    _openSettings() {
        // unim-gtk-settings 또는 unim-qt-settings 실행
        const settingsApps = [
            'unim-gtk-settings',
            'unim-qt-settings',
            '/usr/bin/unim-gtk-settings',
            '/usr/local/bin/unim-gtk-settings'
        ];
        
        for (const app of settingsApps) {
            try {
                GLib.spawn_command_line_async(app);
                return;
            } catch (e) {
                // 다음 시도
            }
        }
        
        Main.notify('UNIM', '설정 도구를 찾을 수 없습니다.');
    }
    
    _openExtensionSettings() {
        try {
            const argv = ['gnome-extensions', 'prefs', this._extension.metadata.uuid];
            GLib.spawn_async(null, argv, null, GLib.SpawnFlags.SEARCH_PATH, null);
        } catch (e) {
            console.error(`[unim-indicator] Failed to open extension settings: ${e.message}`);
        }
    }
    
    destroy() {
        if (this._dbusProxy && this._dbusSignalId > 0) {
            this._dbusProxy.disconnect(this._dbusSignalId);
            this._dbusSignalId = 0;
        }
        this._dbusProxy = null;
        
        super.destroy();
        console.log('[unim-indicator] Panel indicator destroyed');
    }
});
