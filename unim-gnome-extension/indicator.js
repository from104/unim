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

        // === 오프라인 사용자 매뉴얼 ===
        const helpItem = new PopupMenu.PopupMenuItem('도움말 (Help)');
        helpItem.connect('activate', () => this._openHelp());
        this.menu.addMenuItem(helpItem);

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
    
    /**
     * OS UI 언어가 한국어인지 — 로케일 환경변수로 판정.
     * `LC_ALL` → `LC_MESSAGES` → `LANG` 순서로 첫 비어있지 않은 값을 채택하고,
     * `ko` 프리픽스면 한국어로 본다.
     *
     * ⚠️ Rust `unim_gui_common::help::ui_language_is_korean()` 과 **동일 계약**이다.
     * 한쪽만 고치면 같은 데스크톱에서 트레이와 확장이 서로 다른 언어의 매뉴얼을 연다.
     */
    _uiLanguageIsKorean() {
        for (const key of ['LC_ALL', 'LC_MESSAGES', 'LANG']) {
            const val = GLib.getenv(key);
            if (val) return val.startsWith('ko');
        }
        return false;
    }

    /**
     * 매뉴얼 HTML 후보 디렉터리 (우선순위 순).
     *
     * ⚠️ Rust `unim_gui_common::help::help_dir_candidates()` 의 순서를 복제한다.
     * 확장은 JS 라 해석기를 공유할 수 없으므로 **순서가 계약**이며, 한쪽만 바뀌면
     * 조용히 다른 파일(예: /usr 에 남은 구버전)이 열린다. 대응:
     *   Rust ①② `UNIM_DATADIR`(컴파일 타임 주입) — JS 에는 대응 항목 없음(생략)
     *   Rust ③   /usr/share/unim/help
     *   Rust ④   /usr/local/share/unim/help
     *   Rust ⑤   개발 폴백 — Rust 는 실행 파일, 여기서는 확장 디렉터리의 조상에서 help/
     */
    _helpDirCandidates() {
        const dirs = ['/usr/share/unim/help', '/usr/local/share/unim/help'];
        let dir = this._extension?.path ?? null;
        while (dir && dir !== '/') {
            dirs.push(GLib.build_filenamev([dir, 'help']));
            dir = GLib.path_get_dirname(dir);
        }
        return dirs;
    }

    /**
     * 오프라인 사용자 매뉴얼을 기본 브라우저로 연다.
     * 파일이 존재하는 첫 후보를 채택하고, 실패는 조용히 삼키지 않는다 —
     * 눌렀는데 아무 일도 안 일어나는 것이 사용자에게 가장 나쁜 결과다.
     */
    _openHelp() {
        const korean = this._uiLanguageIsKorean();
        // 파일명은 Makefile·MSI·도움말 생성기와 공유하는 고정 계약.
        const name = korean ? 'unim-help-ko.html' : 'unim-help-en.html';
        const path = this._helpDirCandidates()
            .map(d => GLib.build_filenamev([d, name]))
            .find(p => GLib.file_test(p, GLib.FileTest.IS_REGULAR));

        if (!path) {
            unimError('INDICATOR', ` Help file not found: ${name}`);
            Main.notify('UNIM', korean
                ? '도움말 파일을 찾지 못했습니다. unim-common 패키지가 설치되어 있는지 확인해 주세요.'
                : 'Could not find the help file. Check that the unim-common package is installed.');
            return;
        }

        const uri = Gio.File.new_for_path(path).get_uri();
        try {
            if (Gio.AppInfo.launch_default_for_uri(uri, null)) return;
            unimError('INDICATOR', ' launch_default_for_uri returned false; falling back to xdg-open');
        } catch (e) {
            unimError('INDICATOR', ` launch_default_for_uri failed: ${e.message}`);
        }

        // 기본 핸들러가 없거나 실패한 환경(포터블·최소 설치)을 위한 폴백.
        try {
            GLib.spawn_async(null, ['xdg-open', path], null, GLib.SpawnFlags.SEARCH_PATH, null);
        } catch (e) {
            unimError('INDICATOR', ` Failed to open help: ${e.message}`);
            Main.notify('UNIM', korean
                ? '도움말을 열지 못했습니다.'
                : 'Could not open the help file.');
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
