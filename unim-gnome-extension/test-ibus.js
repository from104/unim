#!/usr/bin/gjs

/**
 * IBus 상태 읽기/설정 테스트 스크립트 (gi://IBus 사용)
 * 
 * 사용법:
 *   gjs -m test-ibus.js
 */

import GLib from 'gi://GLib';
import IBus from 'gi://IBus';

const LOOP = GLib.MainLoop.new(null, false);

function timeout(ms) {
    return new Promise(resolve => {
        GLib.timeout_add(GLib.PRIORITY_DEFAULT, ms, () => {
            resolve();
            return GLib.SOURCE_REMOVE;
        });
    });
}

class IBusTester {
    constructor() {
        this._bus = null;
    }

    async init() {
        console.log('Initializing IBus...');
        try {
            IBus.init();
            this._bus = new IBus.Bus();
            
            if (!this._bus.is_connected()) {
                console.log('Connecting to IBus...');
                await timeout(1000);
            }

            if (this._bus.is_connected()) {
                console.log('✓ IBus connected');
                return true;
            } else {
                console.error('✗ IBus not connected');
                return false;
            }
        } catch (e) {
            console.error(`Init failed: ${e.message}`);
            return false;
        }
    }

    /**
     * 현재 엔진 읽기
     */
    getCurrentEngine() {
        return new Promise((resolve) => {
            try {
                this._bus.get_global_engine_async(-1, null, (bus, res) => {
                    try {
                        const engineDesc = bus.get_global_engine_async_finish(res);
                        if (engineDesc) {
                            resolve(engineDesc.get_name());
                        } else {
                            resolve(null);
                        }
                    } catch (e) {
                        console.error(`get_global_engine failed: ${e.message}`);
                        resolve(null);
                    }
                });
            } catch (e) {
                resolve(null);
            }
        });
    }

    /**
     * 엔진 설정
     */
    setEngine(engineName) {
        return new Promise((resolve) => {
            try {
                this._bus.set_global_engine_async(engineName, -1, null, (bus, res) => {
                    try {
                        const success = bus.set_global_engine_async_finish(res);
                        resolve(success);
                    } catch (e) {
                        console.error(`set_global_engine failed: ${e.message}`);
                        resolve(false);
                    }
                });
            } catch (e) {
                resolve(false);
            }
        });
    }

    async runAllTests() {
        console.log('═══════════════════════════════════════════════════════');
        console.log('IBus 상태 읽기/설정 테스트 (GI)');
        console.log('═══════════════════════════════════════════════════════');

        if (!await this.init()) {
            LOOP.quit();
            return;
        }

        // 1. 현재 엔진 읽기
        console.log('\n=== Test 1: Get Current Engine ===');
        const originalEngine = await this.getCurrentEngine();
        console.log(`Current Engine: ${originalEngine}`);

        if (!originalEngine) {
            console.error('Could not get current engine.');
            LOOP.quit();
            return;
        }

        // 2. 토글 테스트
        console.log('\n=== Test 2: Toggle Test ===');
        
        // 한글 엔진인지 확인
        const isKorean = originalEngine.includes('hangul') || 
                        originalEngine.includes('korean') || 
                        originalEngine.includes('kr');
        
        let targetEngine = 'hangul';
        if (isKorean) {
            targetEngine = 'xkb:us::eng';
            console.log('Detected Korean input, switching to English...');
        } else {
            console.log('Detected non-Korean input, switching to Hangul...');
        }

        console.log(`Setting engine to: ${targetEngine}...`);
        let success = await this.setEngine(targetEngine);
        
        if (success) {
            console.log(`✓ Successfully requested set to ${targetEngine}`);
            
            // Wait and verify
            await timeout(500);
            const current = await this.getCurrentEngine();
            if (current === targetEngine) {
                console.log(`✓ Verified: Current engine is now ${current}`);
            } else {
                console.log(`⚠ Warning: Expected ${targetEngine} but got ${current}`);
            }

            // 5초 대기
            console.log('\n⏳ Waiting 5 seconds before restoring...');
            for (let i = 5; i > 0; i--) {
                console.log(`   ${i}...`);
                await timeout(1000);
            }

            // 복구
            console.log(`\n🔄 Restoring original engine: ${originalEngine}`);
            success = await this.setEngine(originalEngine);
            if (success) {
                await timeout(500);
                const restored = await this.getCurrentEngine();
                if (restored === originalEngine) {
                    console.log(`✓ Successfully restored to: ${restored}`);
                } else {
                    console.log(`⚠ Warning: Restore requested but engine is ${restored}`);
                }
            } else {
                console.log('✗ Restore request failed');
            }

        } else {
            console.log('✗ Failed to set engine');
        }

        console.log('\n═══════════════════════════════════════════════════════');
        console.log('테스트 완료');
        console.log('═══════════════════════════════════════════════════════');
        LOOP.quit();
    }
}

const tester = new IBusTester();
tester.runAllTests();
LOOP.run();
