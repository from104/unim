import Gio from 'gi://Gio';
import Adw from 'gi://Adw';
import Gtk from 'gi://Gtk';
import {ExtensionPreferences} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

export default class UnimPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings();
        const page = new Adw.PreferencesPage();
        window.add(page);

        // 1. General Settings Group
        const generalGroup = new Adw.PreferencesGroup({
            title: 'General Settings',
            description: 'Basic configuration for the extension'
        });
        page.add(generalGroup);

        // Enable/Disable Extension
        this._addToggle(
            generalGroup,
            settings,
            'enable-extension',
            'Enable Extension',
            'Turn the extension functionality on or off'
        );

        // Show Panel Indicator
        this._addToggle(
            generalGroup,
            settings,
            'show-indicator',
            'Show Panel Indicator',
            'Show the input method status icon in the system panel'
        );

        // Show Notification
        this._addToggle(
            generalGroup,
            settings,
            'show-notification',
            'Show Notifications',
            'Show a notification when text is converted'
        );


        // 2. Automation Settings Group
        const automationGroup = new Adw.PreferencesGroup({
            title: 'Automation',
            description: 'Configure automatic conversion behavior'
        });
        page.add(automationGroup);

        // Enable Automatic Conversion
        this._addToggle(
            automationGroup,
            settings,
            'enable-automatic-conversion',
            'Automatic Conversion',
            'Automatically detect and fix Korean/English typos'
        );


        // 3. Keyboard Layout Settings Group
        const layoutGroup = new Adw.PreferencesGroup({
            title: 'Keyboard Layouts',
            description: 'Select your keyboard layouts for accurate conversion'
        });
        page.add(layoutGroup);

        // Korean Layout Combo
        this._addCombo(
            layoutGroup,
            settings,
            'korean-layout',
            'Korean Layout',
            'Select the Korean keyboard layout',
            [
                ['2bul', '2-Set Standard (두벌식 표준)'],
                ['390', '3-Set 390 (세벌식 390)'],
                ['391', '3-Set 391 (세벌식 391/최종)']
            ]
        );

        // English Layout Combo
        this._addCombo(
            layoutGroup,
            settings,
            'english-layout',
            'English Layout',
            'Select the English keyboard layout',
            [
                ['qwerty', 'QWERTY'],
                ['dvorak', 'Dvorak']
            ]
        );


        // 4. Manual Conversion Group
        const manualGroup = new Adw.PreferencesGroup({
            title: 'Manual Conversion',
            description: 'Shortcuts for manual text conversion'
        });
        page.add(manualGroup);

        // Enable Manual Conversion
        this._addToggle(
            manualGroup,
            settings,
            'enable-manual-conversion',
            'Enable Shortcut',
            'Allow manual conversion via keyboard shortcut'
        );

        // Shortcut Row (Note: Shortcut UI handling is complex in GTK4/Adw, using simple entry for now or ActionRow)
        // For simplicity and stability in Adw 1, we use an ActionRow with a simple label indicating it's handled via GNOME Settings for now, 
        // or a simple Entry if we want to store the string. The schema has 'manual-conversion-shortcut' as string.
        
        const shortcutRow = new Adw.ActionRow({
            title: 'Conversion Shortcut',
            subtitle: 'Shortcut key string (e.g., "<Super>i")'
        });
        manualGroup.add(shortcutRow);

        const shortcutEntry = new Gtk.Entry({
            placeholder_text: '<Super>i',
            text: settings.get_strv('manual-conversion-shortcut')[0] || '',
            valign: Gtk.Align.CENTER,
            hexpand: true
        });

        shortcutEntry.connect('changed', (entry) => {
            const text = entry.get_text();
            if (text) {
                settings.set_strv('manual-conversion-shortcut', [text]);
            }
        });

        shortcutRow.add_suffix(shortcutEntry);
    }

    // Helper to add a switch row
    _addToggle(group, settings, key, title, subtitle) {
        const row = new Adw.ActionRow({
            title: title,
            subtitle: subtitle
        });
        group.add(row);

        const toggle = new Gtk.Switch({
            active: settings.get_boolean(key),
            valign: Gtk.Align.CENTER
        });

        settings.bind(key, toggle, 'active', Gio.SettingsBindFlags.DEFAULT);
        row.add_suffix(toggle);
    }

    // Helper to add a combo row
    _addCombo(group, settings, key, title, subtitle, options) {
        const row = new Adw.ActionRow({
            title: title,
            subtitle: subtitle
        });
        group.add(row);

        const model = new Gtk.StringList();
        options.forEach(([id, label]) => model.append(label));

        const combo = new Gtk.DropDown({
            model: model,
            valign: Gtk.Align.CENTER
        });

        // Bind initial value
        const currentId = settings.get_string(key);
        const index = options.findIndex(([id]) => id === currentId);
        if (index !== -1) {
            combo.set_selected(index);
        }

        // Save on change
        combo.connect('notify::selected', () => {
            const selectedIdx = combo.get_selected();
            if (selectedIdx !== Gtk.INVALID_LIST_POSITION) {
                const [id] = options[selectedIdx];
                settings.set_string(key, id);
            }
        });

        row.add_suffix(combo);
    }
}
