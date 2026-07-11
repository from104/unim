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
import { unimLog, unimError } from './logging.js';

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
        this._nameWatcherId = 0;
        this._isKorean = true;
        this._connected = false;
        this._inputActive = false;

        // 확장 디렉토리 경로 (아이콘 로드용)
        this._extensionPath = extension.path;
        
        // 패널 버튼 - 박스로 구성 (아이콘)
        this._box = new St.BoxLayout({ style_class: 'panel-status-menu-box' });
        
        // 아이콘 위젯 생성
        this._icon = new St.Icon({
            style_class: 'unim-indicator-icon system-status-icon',
            y_align: Clutter.ActorAlign.CENTER
        });
        this._box.add_child(this._icon);
        
        this.add_child(this._box);
        
        // 초기 아이콘 설정
        this._updateIcon();
        
        // 팝업 메뉴 생성
        this._buildMenu();
        
        // DBus 서비스 감시 시작
        this._watchDbusService();
        
        unimLog('INDICATOR', ' Panel indicator initialized');
    }

    /**
     * 패널 아이콘 클릭 처리.
     * panel-click-action = 'toggle-mode' 이면 왼쪽 클릭으로 한/영 전환,
     * 오른쪽 클릭은 기본 동작(메뉴 표시)을 유지한다.
     * 'menu' 설정이면 GNOME 기본 동작(왼쪽 클릭도 메뉴).
     */
    vfunc_event(event) {
        if (event.type() === Clutter.EventType.BUTTON_PRESS) {
            const action = this._settings.get_string('panel-click-action');
            if (action === 'toggle-mode' && event.get_button() === Clutter.BUTTON_PRIMARY) {
                if (this._connected) {
                    this._setMode(!this._isKorean);
                }
                return Clutter.EVENT_STOP;
            }
        }
        return super.vfunc_event(event);
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
        
        // === UNIM 설정 앱 (unim-settings) ===
        const unimSettingsItem = new PopupMenu.PopupMenuItem('UNIM 설정 (Settings)...');
        unimSettingsItem.connect('activate', () => this._openUnimSettings());
        this.menu.addMenuItem(unimSettingsItem);

        // === GNOME 확장 설정 ===
        const extSettingsItem = new PopupMenu.PopupMenuItem('GNOME 확장 설정 (Extension)...');
        extSettingsItem.connect('activate', () => this._openExtensionSettings());
        this.menu.addMenuItem(extSettingsItem);
        
        // 초기 메뉴 상태 업데이트
        this._updateMenuItems();
    }
    
    _updateMenuItems() {
        // 헤더 업데이트
        if (!this._connected) {
            this._headerItem.label.set_text('⚠️ UNIM 데몬 연결 안됨');
        } else if (!this._inputActive) {
            this._headerItem.label.set_text('💤 입력 대기 중');
        } else {
            this._headerItem.label.set_text(this._isKorean ? '🇰🇷 한국어 입력 중' : '🔤 영어 입력 중');
        }
        
        // 체크마크 표시
        this._koreanItem.setOrnament(this._isKorean ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
        this._englishItem.setOrnament(!this._isKorean ? PopupMenu.Ornament.CHECK : PopupMenu.Ornament.NONE);
    }
    
    /**
     * 입력 활성 상태 설정 (포커스 있는 입력 필드가 있을 때)
     * @param {boolean} active
     */
    setInputActive(active) {
        if (this._inputActive === active) return;
        this._inputActive = active;
        this._updateIcon();
        this._updateMenuItems();
    }

    _updateIcon() {
        let iconName;
        let styleClass = 'unim-indicator-icon system-status-icon';

        if (this._connected && this._inputActive) {
            // 연결됨 + 입력 포커스 있음 → 한/영 상태 표시
            iconName = this._isKorean ? 'unim-korean' : 'unim-english';
            styleClass += this._isKorean ? ' hangul-mode' : ' english-mode';
        } else {
            // 연결 안됨 또는 입력 포커스 없음 → 비활성(금지) 상태
            iconName = 'unim-disabled';
            styleClass += ' disconnected-mode';
        }
        
        // 확장 디렉토리의 icons 폴더에서 SVG 로드
        const iconPath = GLib.build_filenamev([this._extensionPath, 'icons', `${iconName}.svg`]);
        const iconFile = Gio.File.new_for_path(iconPath);
        
        if (iconFile.query_exists(null)) {
            const gicon = Gio.FileIcon.new(iconFile);
            this._icon.set_gicon(gicon);
        } else {
            // 아이콘 파일이 없으면 시스템 아이콘 사용
            this._icon.set_icon_name(iconName);
        }
        
        this._icon.set_style_class_name(styleClass);
    }
    
    /**
     * DBus 서비스 감시 시작
     * 
     * Gio.DBus.watch_name()을 사용하여 unim-daemon 서비스의 등장/소멸을 감시합니다.
     * 데몬이 시작되면 자동으로 연결하고, 종료되면 UI를 업데이트합니다.
     */
    _watchDbusService() {
        this._nameWatcherId = Gio.DBus.watch_name(
            Gio.BusType.SESSION,
            UNIM_BUS_NAME,
            Gio.BusNameWatcherFlags.NONE,
            this._onServiceAppeared.bind(this),
            this._onServiceVanished.bind(this)
        );
        unimLog('INDICATOR', ' Started watching DBus service');
        
        // DBus Activation: 프록시 생성 시도로 데몬 자동 시작 트리거
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, 100, () => {
            this._tryActivateDaemon();
            return GLib.SOURCE_REMOVE;
        });
    }
    
    /**
     * DBus Activation을 통해 데몬 시작을 시도합니다.
     * 
     * 데몬이 실행 중이지 않으면 DBus 서비스 파일에 의해 자동으로 시작됩니다.
     */
    _tryActivateDaemon() {
        unimLog('INDICATOR', ' Attempting DBus activation...');
        try {
            Gio.DBusProxy.new_for_bus(
                Gio.BusType.SESSION,
                Gio.DBusProxyFlags.NONE,
                null,
                UNIM_BUS_NAME,
                UNIM_OBJECT_PATH,
                UNIM_INTERFACE,
                null,
                (source, result) => {
                    try {
                        const proxy = Gio.DBusProxy.new_for_bus_finish(result);
                        if (proxy) {
                            unimLog('INDICATOR', ' DBus activation successful');
                        }
                    } catch (e) {
                        unimLog('INDICATOR', ` DBus activation attempt: ${e.message}`);
                    }
                }
            );
        } catch (e) {
            unimLog('INDICATOR', ` DBus activation failed: ${e.message}`);
        }
    }
    
    /**
     * DBus 서비스 등장 시 호출
     */
    _onServiceAppeared(connection, name, nameOwner) {
        unimLog('INDICATOR', ` DBus service appeared: ${name} (owner: ${nameOwner})`);
        this._setupDbusProxy();
    }
    
    /**
     * DBus 서비스 소멸 시 호출
     */
    _onServiceVanished(connection, name) {
        unimLog('INDICATOR', ` DBus service vanished: ${name}`);
        
        // 기존 프록시 정리
        if (this._dbusProxy && this._dbusSignalId > 0) {
            this._dbusProxy.disconnect(this._dbusSignalId);
            this._dbusSignalId = 0;
        }
        this._dbusProxy = null;
        this._connected = false;
        
        // UI 업데이트 (연결 안됨 상태 표시)
        this._updateIcon();
        this._updateMenuItems();
    }
    
    /**
     * DBus 프록시 설정 및 시그널 연결
     */
    _setupDbusProxy() {
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
                unimLog('INDICATOR', ' DBus proxy connected');
                
                // 초기 상태 조회
                this._fetchInitialMode();
            }
        } catch (e) {
            unimLog('INDICATOR', ` DBus proxy setup failed: ${e.message}`);
            this._connected = false;
            this._updateIcon();
            this._updateMenuItems();
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
            unimLog('INDICATOR', ` GetGlobalMode failed: ${e.message}`);
        }
    }
    
    _onModeChanged(isKorean) {
        this._isKorean = isKorean;
        this._updateIcon();
        this._updateMenuItems();
        unimLog('INDICATOR', ` Mode changed: ${isKorean ? 'Korean' : 'English'}`);
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
            unimError('INDICATOR', ` SetGlobalMode failed: ${e.message}`);
        }
    }
    

    
    _openExtensionSettings() {
        try {
            const argv = ['gnome-extensions', 'prefs', this._extension.metadata.uuid];
            GLib.spawn_async(null, argv, null, GLib.SpawnFlags.SEARCH_PATH, null);
        } catch (e) {
            unimError('INDICATOR', ` Failed to open extension settings: ${e.message}`);
        }
    }

    /**
     * UNIM 메인 설정 앱(unim-settings) 실행.
     * GNOME 확장 설정창과는 별도로, config.yaml을 편집하는 단일 창구.
     */
    _openUnimSettings() {
        try {
            GLib.spawn_async(
                null,
                ['unim-settings'],
                null,
                GLib.SpawnFlags.SEARCH_PATH,
                null
            );
        } catch (e) {
            unimError('INDICATOR', ` Failed to launch unim-settings: ${e.message}`);
        }
    }
    
    destroy() {
        // DBus 이름 감시 정리
        if (this._nameWatcherId > 0) {
            Gio.DBus.unwatch_name(this._nameWatcherId);
            this._nameWatcherId = 0;
        }
        
        // DBus 프록시 정리
        if (this._dbusProxy && this._dbusSignalId > 0) {
            this._dbusProxy.disconnect(this._dbusSignalId);
            this._dbusSignalId = 0;
        }
        this._dbusProxy = null;
        
        super.destroy();
        unimLog('INDICATOR', ' Panel indicator destroyed');
    }
});
