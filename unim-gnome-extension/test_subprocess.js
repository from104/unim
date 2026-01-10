import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

const loop = new GLib.MainLoop(null, false);

const proc = new Gio.Subprocess({
    argv: ['echo', 'hello'],
    flags: Gio.SubprocessFlags.STDOUT_PIPE,
});

proc.communicate_utf8_async('test input', null, (p, res) => {
    try {
        const result = p.communicate_utf8_finish(res);
        console.log('Result length:', result.length);
        console.log('Result[0] (success):', result[0]);
        console.log('Result[1] (stdout):', result[1]);
        console.log('Result[2] (stderr):', result[2]);
    } catch (e) {
        console.error('Error:', e.message);
    }
    loop.quit();
});

loop.run();
