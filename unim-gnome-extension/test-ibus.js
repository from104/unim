#!/usr/bin/gjs

/**
 * IBus 상태 읽기/설정 테스트 스크립트
 * 
 * 사용법:
 *   gjs -m test-ibus.js
 * 
 * 이 스크립트는 extension.js의 InputMethodManager와 동일한 로직을 테스트합니다.
 */

import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const IBUS_SERVICE = 'org.freedesktop.IBus';
const IBUS_PATH = '/org/freedesktop/IBus';
const IBUS_IFACE = 'org.freedesktop.IBus';
const DBUS_PROPS_IFACE = 'org.freedesktop.DBus.Properties';

// byte array decode helper
const ByteArray = imports.byteArray;

class IBusTester {
    constructor() {
    }

    /**
     * IBus 서비스가 실행 중인지 확인
     */
    checkIBusService() {
        try {
            const reply = Gio.DBus.session.call_sync(
                'org.freedesktop.DBus',
                '/org/freedesktop/DBus',
                'org.freedesktop.DBus',
                'NameHasOwner',
                new GLib.Variant('(s)', [IBUS_SERVICE]),
                new GLib.VariantType('(b)'),
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );
            const [hasOwner] = reply.deep_unpack();
            return hasOwner;
        } catch (e) {
            console.error(`[test] Failed to check IBus service: ${e.message}`);
            return false;
        }
    }

    /**
     * IBus 객체가 존재하는지 확인 (Introspect 시도)
     */
    checkIBusObject() {
        try {
            Gio.DBus.session.call_sync(
                IBUS_SERVICE,
                IBUS_PATH,
                'org.freedesktop.DBus.Introspectable',
                'Introspect',
                null,
                new GLib.VariantType('(s)'),
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );
            return true;
        } catch (e) {
            console.warn(`[test] IBus object check failed: ${e.message}`);
            return false;
        }
    }

    /**
     * 현재 엔진 읽기 (extension.js와 동일한 로직)
     */
    getCurrentEngine() {
        console.log('\n=== Test 1: Get Current Engine ===');
        
        // Method 1: D-Bus Properties.Get
        try {
            console.log('Trying D-Bus Properties.Get...');
            const reply = Gio.DBus.session.call_sync(
                IBUS_SERVICE,
                IBUS_PATH,
                DBUS_PROPS_IFACE,
                'Get',
                new GLib.Variant('(ss)', [IBUS_IFACE, 'GlobalEngine']),
                new GLib.VariantType('(v)'),
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );

            const [variant] = reply.deep_unpack();
            const [name, displayName] = variant.deep_unpack();
            console.log(`✓ Success! Engine: ${name}, Display: ${displayName}`);
            return name;
        } catch (e) {
            console.log(`✗ Failed: ${e.message}`);
        }

        // Method 2: GetGlobalEngine 메서드 (일부 IBus 버전)
        try {
            console.log('Trying GetGlobalEngine method...');
            const reply = Gio.DBus.session.call_sync(
                IBUS_SERVICE,
                IBUS_PATH,
                IBUS_IFACE,
                'GetGlobalEngine',
                null,
                new GLib.VariantType('(ss)'),
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );
            const [name, displayName] = reply.deep_unpack();
            console.log(`✓ Success! Engine: ${name}, Display: ${displayName}`);
            return name;
        } catch (e) {
            console.log(`✗ Failed: ${e.message}`);
        }

        // Method 3: ibus engine 명령어 (fallback)
        if (GLib.find_program_in_path('ibus')) {
            try {
                console.log('Trying ibus engine command...');
                const [ok, out] = GLib.spawn_command_line_sync('ibus engine');
                if (ok && out) {
                    const text = ByteArray.toString(out).trim();
                    if (text.length > 0) {
                        console.log(`✓ Success! Engine: ${text}`);
                        return text;
                    }
                }
            } catch (e) {
                console.log(`✗ Failed: ${e.message}`);
            }
        }

        console.log('✗ All methods failed');
        return null;
    }

    /**
     * 사용 가능한 엔진 목록 조회
     */
    listEngines() {
        console.log('\n=== Test 2: List Available Engines ===');
        try {
            // IBus는 ListEngines 메서드를 제공할 수 있음
            const reply = Gio.DBus.session.call_sync(
                IBUS_SERVICE,
                IBUS_PATH,
                IBUS_IFACE,
                'ListEngines',
                null,
                new GLib.VariantType('(a(ssss))'),
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );
            const [engines] = reply.deep_unpack();
            console.log(`✓ Found ${engines.length} engines:`);
            engines.forEach(([name, longName, description, language], idx) => {
                console.log(`  [${idx}] ${name} - ${longName} (${language})`);
            });
            return engines;
        } catch (e) {
            console.log(`✗ ListEngines failed: ${e.message}`);
            console.log('  Trying alternative method...');
            
            // 대안: ibus list-engine 명령어
            if (GLib.find_program_in_path('ibus')) {
                try {
                    const [ok, out] = GLib.spawn_command_line_sync('ibus list-engine');
                    if (ok && out) {
                        const text = ByteArray.toString(out).trim();
                        console.log('Available engines (from ibus list-engine):');
                        console.log(text);
                        return text;
                    }
                } catch (e2) {
                    console.log(`✗ ibus list-engine also failed: ${e2.message}`);
                }
            }
            return null;
        }
    }

    /**
     * 엔진 설정 테스트
     */
    testSetEngine(engineName) {
        console.log(`\n=== Test 3: Set Engine to "${engineName}" ===`);
        try {
            Gio.DBus.session.call_sync(
                IBUS_SERVICE,
                IBUS_PATH,
                IBUS_IFACE,
                'SetGlobalEngine',
                new GLib.Variant('(s)', [engineName]),
                null,
                Gio.DBusCallFlags.NONE,
                -1,
                null
            );
            console.log(`✓ Successfully set engine to: ${engineName}`);
            
            // 확인: 다시 읽어보기 (약간의 지연 후)
            GLib.usleep(200000); // 200ms
            const current = this.getCurrentEngine();
            if (current === engineName) {
                console.log(`✓ Verified: Current engine is now ${current}`);
                return true;
            } else {
                console.log(`⚠ Warning: Set to ${engineName} but current is ${current}`);
                return false;
            }
        } catch (e) {
            console.log(`✗ Failed: ${e.message}`);
            // Fallback: ibus engine command
            if (GLib.find_program_in_path('ibus')) {
                try {
                    GLib.spawn_command_line_async(`ibus engine ${engineName}`);
                    console.log(`✓ Set engine via ibus command: ${engineName}`);
                    GLib.usleep(200000);
                    return true;
                } catch (e2) {
                    console.log(`✗ ibus command also failed: ${e2.message}`);
                }
            }
            return false;
        }
    }

    /**
     * 한영 토글 테스트
     */
    testToggle() {
        console.log('\n=== Test 4: Toggle Test ===');
        
        const current = this.getCurrentEngine();
        if (!current) {
            console.log('✗ Cannot get current engine, aborting toggle test');
            return false;
        }

        console.log(`Current engine: ${current}`);
        
        // 한글 엔진인지 확인
        const isKorean = current.includes('hangul') || 
                        current.includes('korean') || 
                        current.includes('kr');
        
        let targetEngine;
        if (isKorean) {
            targetEngine = 'xkb:us::eng';
            console.log('Detected Korean input, switching to English...');
        } else {
            // 한글 엔진 찾기 (hangul이 포함된 엔진)
            targetEngine = 'hangul';  // 기본값
            console.log('Detected non-Korean input, switching to Hangul...');
        }

        const success = this.testSetEngine(targetEngine);
        
        if (success) {
            // 다시 원래대로 돌리기
            console.log(`\nRestoring original engine: ${current}`);
            this.testSetEngine(current);
        }
        
        return success;
    }

    /**
     * 전체 테스트 실행
     */
    runAllTests() {
        console.log('═══════════════════════════════════════════════════════');
        console.log('IBus 상태 읽기/설정 테스트');
        console.log('═══════════════════════════════════════════════════════');

        // 0. 서비스 확인
        console.log('\n=== Test 0: IBus Service Check ===');
        const hasService = this.checkIBusService();
        console.log(`IBus service running: ${hasService ? '✓ Yes' : '✗ No'}`);
        
        if (!hasService) {
            console.log('\n⚠ IBus service is not running. Please start IBus first.');
            console.log('  Try: ibus-daemon -rd');
            return;
        }

        const hasObject = this.checkIBusObject();
        console.log(`IBus object accessible: ${hasObject ? '✓ Yes' : '✗ No'}`);

        // 1. 현재 엔진 읽기
        const current = this.getCurrentEngine();

        // 2. 엔진 목록 조회
        this.listEngines();

        // 3. 엔진 설정 테스트 (현재 엔진으로 다시 설정)
        if (current) {
            this.testSetEngine(current);
        }

        // 4. 토글 테스트
        this.testToggle();

        console.log('\n═══════════════════════════════════════════════════════');
        console.log('테스트 완료');
        console.log('═══════════════════════════════════════════════════════');
    }
}

// 실행
try {
    const tester = new IBusTester();
    tester.runAllTests();
} catch (e) {
    console.error(`[test] Fatal error: ${e.message}`);
    if (e.stack) console.error(e.stack);
    imports.system.exit(1);
}

