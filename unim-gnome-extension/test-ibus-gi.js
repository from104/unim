#!/usr/bin/gjs

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

async function run() {
    console.log('Initializing IBus...');
    IBus.init();

    const bus = new IBus.Bus();
    
    if (!bus.is_connected()) await timeout(1000);

    if (!bus.is_connected()) {
        console.error('✗ IBus not connected');
        LOOP.quit();
        return;
    }
    
    console.log('✓ IBus connected');

    // Test Get Async
    console.log('Testing get_global_engine_async...');
    try {
        bus.get_global_engine_async(-1, null, (bus, res) => {
            try {
                const engine = bus.get_global_engine_async_finish(res);
                if (engine) {
                    console.log(`Current Engine: ${engine.get_name()}`);
                } else {
                    console.log('Current Engine: null');
                }
            } catch (e) {
                console.error(`get finish failed: ${e.message}`);
            }
        });
    } catch (e) {
        console.error(`get call failed: ${e.message}`);
    }
    
    await timeout(1000);

    // Test Set Async
    const target = 'xkb:us::eng';
    console.log(`Testing set_global_engine_async to ${target}...`);
    
    try {
        bus.set_global_engine_async(target, -1, null, (bus, res) => {
            try {
                const success = bus.set_global_engine_async_finish(res);
                console.log(`Set Result: ${success}`);
            } catch (e) {
                console.error(`set finish failed: ${e.message}`);
            }
        });
    } catch (e) {
        console.error(`set call failed: ${e.message}`);
    }

    await timeout(1000);
    
    // Check result
    // We can also use sync get if async crashes? 
    // But let's stick to async for now.
    
    // Restore to hangul
    console.log('Restoring to hangul...');
    try {
        bus.set_global_engine_async('hangul', -1, null, (bus, res) => {
            try {
                const success = bus.set_global_engine_async_finish(res);
                console.log(`Restore Result: ${success}`);
            } catch (e) {
                console.error(`restore finish failed: ${e.message}`);
            }
        });
    } catch (e) {
        console.error(`restore call failed: ${e.message}`);
    }

    await timeout(1000);
    LOOP.quit();
}

run().catch(e => console.error(e));
LOOP.run();
