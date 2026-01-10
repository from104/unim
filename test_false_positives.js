import UnimLib from './unim-gnome-extension/unimlib.js';
import GLib from 'gi://GLib';

const loop = new GLib.MainLoop(null, false);

async function test() {
    try {
        const lib = new UnimLib('/dummy/path/lib/libunim_core.so');
        lib._binPath = './target/release/unim-cli';
        
        const words = ['dkssud', 'hello', 'apple', 'gksrmf'];
        for (const word of words) {
            console.log(`Testing en -> ko: ${word}`);
            const result = await lib.transformString(word, 'en_qwerty', 'ko_2bulstd');
            console.log(`Result: ${result}`);
        }
    } catch (e) {
        console.error(e);
    } finally {
        loop.quit();
    }
}

test();
loop.run();
