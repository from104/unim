import Gio from 'gi://Gio';
import Adw from 'gi://Adw';
import Gtk from 'gi://Gtk';
import {ExtensionPreferences, gettext as _} from 'resource:///org/gnome/Shell/Extensions/js/extensions/prefs.js';

export default class UnimPreferences extends ExtensionPreferences {
    fillPreferencesWindow(window) {
        const settings = this.getSettings();
        const page = new Adw.PreferencesPage();
        window.add(page);

        // 1. General Settings Group
        const generalGroup = new Adw.PreferencesGroup({
            title: _('General Settings'),
            description: _('Basic configuration for the extension')
        });
        page.add(generalGroup);

        // Enable/Disable Extension
        this._addToggle(
            generalGroup,
            settings,
            'enable-extension',
            _('Enable Extension'),
            _('Turn the extension functionality on or off')
        );

        // Show Notification
        this._addToggle(
            generalGroup,
            settings,
            'show-notification',
            _('Show Notifications'),
            _('Show a notification when text is converted')
        );


        // 3. Keyboard Layout Settings Group
        const layoutGroup = new Adw.PreferencesGroup({
            title: _('Keyboard Layouts'),
            description: _('Select your keyboard layouts for accurate conversion')
        });
        page.add(layoutGroup);

        // Korean Layout Combo
        this._addCombo(
            layoutGroup,
            settings,
            'korean-layout',
            _('Korean Layout'),
            _('Select the Korean keyboard layout'),
            [
                ['2bul', _('2-Set Standard (두벌식 표준)')],
                ['390', _('3-Set 390 (세벌식 390)')],
                ['391', _('3-Set 391 (세벌식 391/최종)')]
            ]
        );

        // English Layout Combo
        this._addCombo(
            layoutGroup,
            settings,
            'english-layout',
            _('English Layout'),
            _('Select the English keyboard layout'),
            [
                ['qwerty', _('QWERTY')],
                ['dvorak', _('Dvorak')]
            ]
        );


        // 4. Manual Conversion Group
        const manualGroup = new Adw.PreferencesGroup({
            title: _('Manual Conversion'),
            description: _('Shortcuts for manual text conversion')
        });
        page.add(manualGroup);

        // Enable Manual Conversion
        this._addToggle(
            manualGroup,
            settings,
            'enable-manual-conversion',
            _('Enable Shortcut'),
            _('Allow manual conversion via keyboard shortcut')
        );

        // Shortcut Row
        const shortcutRow = new Adw.ActionRow({
            title: _('Conversion Shortcut'),
            subtitle: _('Shortcut key string (e.g., "<Super>k")')
        });
        manualGroup.add(shortcutRow);

        const shortcutEntry = new Gtk.Entry({
            placeholder_text: '<Super>k',
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


        // Auto Paste Toggle
        this._addToggle(
            manualGroup,
            settings,
            'auto-paste',
            _('Auto Paste'),
            _('Automatically paste converted text after copying to clipboard')
        );
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
