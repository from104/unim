import './unimlib.js'; // This won't work because it's default export
// Let's just copy the logic or use dynamic import if gjs supports it in scripts
// Actually, let's just make a test script that imports it.

import UnimLib from './unimlib.js';
import GLib from 'gi://GLib';

const loop = new GLib.MainLoop(null, false);

async function test() {
    try {
        const lib = new UnimLib('/dummy/path/lib/libunim_core.so');
        lib._binPath = './target/release/unim-cli';
        
        console.log('Testing en -> ko: dkssud');
        const result = await lib.transformString('dkssud', 'en_qwerty', 'ko_2bulstd');
        console.log('Result:', result);
        
        console.log('Testing ko -> en: 안녕');
        const result2 = await lib.transformString('안녕', 'ko_2bulstd', 'en_qwerty');
        console.log('Result:', result2);
    } catch (e) {
        console.error(e);
    } finally {
        loop.quit();
    }
}

test();
loop.run();
