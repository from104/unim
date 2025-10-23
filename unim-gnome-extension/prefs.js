import Adw from 'gi://Adw';
import Gtk from 'gi://Gtk';
import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

export default class UnimPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const page = new Adw.PreferencesPage();
        window.add(page);

        const group = new Adw.PreferencesGroup({
            title: 'General'
        });
        page.add(group);

        const row = new Adw.ActionRow({
            title: 'Show Notification',
            subtitle: 'Show a notification when text is converted'
        });
        group.add(row);

        const toggle = new Gtk.Switch({
            active: true,
            valign: Gtk.Align.CENTER
        });
        row.add_suffix(toggle);
    }
}