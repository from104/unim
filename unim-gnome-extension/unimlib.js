import Gio from 'gi://Gio';
import GLib from 'gi://GLib';

export default class UnimLib {
    constructor(libPath) {
        // libPath is likely something like /.../lib/libunim_core.so
        // We want the bin directory which is sibling to lib
        this._binPath = libPath.replace('/lib/libunim_core.so', '/bin/unim-cli');
    }

    /**
     * Initialize the library.
     * @returns {boolean} True if binary exists
     */
    init() {
        const file = Gio.File.new_for_path(this._binPath);
        if (file.query_exists(null)) {
            console.log(`[unim] CLI binary found at ${this._binPath}`);
            return true;
        } else {
            console.error(`[unim] CLI binary NOT found at ${this._binPath}`);
            return false;
        }
    }

    /**
     * Transform a string between keyboard layouts using the Rust CLI.
     * @param {string} input - The string to transform
     * @param {string} fromLayout - Current keyboard layout (e.g., 'ko_2bulstd', 'en_qwerty')
     * @param {string} toLayout - Target keyboard layout
     * @returns {Promise<string>} The transformed string
     */
    async transformString(input, fromLayout, toLayout) {
        if (!input) return input;

        try {
            let args = [this._binPath];
            
            const isToKorean = toLayout.startsWith('ko_');
            if (isToKorean) {
                args.push('--compose');
                const layoutMatch = toLayout.match(/ko_(.*)std/);
                args.push('-k', layoutMatch ? layoutMatch[1] : '2bul');
                
                const enLayoutMatch = fromLayout.match(/en_(.*)/);
                args.push('-e', enLayoutMatch ? enLayoutMatch[1] : 'qwerty');
            } else {
                args.push('--decompose');
                const layoutMatch = fromLayout.match(/ko_(.*)std/);
                args.push('-k', layoutMatch ? layoutMatch[1] : '2bul');
                
                const enLayoutMatch = toLayout.match(/en_(.*)/);
                args.push('-e', enLayoutMatch ? enLayoutMatch[1] : 'qwerty');
            }

            const proc = Gio.Subprocess.new(
                args,
                Gio.SubprocessFlags.STDIN_PIPE | Gio.SubprocessFlags.STDOUT_PIPE | Gio.SubprocessFlags.STDERR_PIPE
            );

            return new Promise((resolve) => {
                proc.communicate_utf8_async(input, null, (p, res) => {
                    try {
                        const result = p.communicate_utf8_finish(res);
                        // GJS 1.80 returns [success, stdout, stderr]
                        const [success, stdout, stderr] = result;
                        
                        if (success) {
                            resolve(stdout ? stdout.trim() : input);
                        } else {
                            console.warn(`[unim] CLI failed with stderr: ${stderr}`);
                            resolve(input);
                        }
                    } catch (e) {
                        console.error(`[unim] CLI communication failed: ${e.message}`);
                        resolve(input);
                    }
                });
            });
        } catch (e) {
            console.error(`[unim] transformString execution failed: ${e.message}`);
            return input;
        }
    }

    /**
     * Close the library. (NOP for CLI version)
     */
    close() {
        // Nothing to do
    }
}
