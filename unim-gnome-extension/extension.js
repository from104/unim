import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';

export default class UnimAutocorrectExtension extends Extension {
    enable() {
        console.log(`[${this.uuid}] enabling`);
    }

    disable() {
        console.log(`[${this.uuid}] disabling`);
    }
} 