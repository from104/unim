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


        // 4. Keyboard Shortcuts Group
        const shortcutGroup = new Adw.PreferencesGroup({
            title: _('Keyboard Shortcuts'),
            description: _('Current keybindings for various conversion modes. These can be configured in the GNOME Settings or Extension Manager.')
        });
        page.add(shortcutGroup);

        this._addShortcutRow(
            shortcutGroup,
            settings,
            'shortcut-normal',
            _('English → Korean'),
            _('Converts English typed with Korean layout and pastes it.')
        );
        this._addShortcutRow(
            shortcutGroup,
            settings,
            'shortcut-normal-reverse',
            _('Korean → English'),
            _('Converts Korean typed with English layout and pastes it.')
        );
        this._addShortcutRow(
            shortcutGroup,
            settings,
            'shortcut-terminal',
            _('Terminal (E → K)'),
            _('Converts with backspaces before pasting, suitable for terminals.')
        );
        this._addShortcutRow(
            shortcutGroup,
            settings,
            'shortcut-terminal-reverse',
            _('Terminal (K → E)'),
            _('Converts with backspaces before pasting, suitable for terminals.')
        );
        this._addShortcutRow(
            shortcutGroup,
            settings,
            'shortcut-copy-only',
            _('Copy Only (E → K)'),
            _('Converts and copies to clipboard without pasting.')
        );
        this._addShortcutRow(
            shortcutGroup,
            settings,
            'shortcut-copy-only-reverse',
            _('Copy Only (K → E)'),
            _('Converts and copies to clipboard without pasting.')
        );
    }

    // Helper to add an editable shortcut row with a reset button
    _addShortcutRow(group, settings, key, title, subtitle) {
        const row = new Adw.ActionRow({
            title: title,
            subtitle: subtitle
        });
        group.add(row);

        const entry = new Gtk.Entry({
            text: settings.get_strv(key)[0] || '',
            valign: Gtk.Align.CENTER,
            hexpand: true
        });

        // Update settings when the entry changes
        entry.connect('changed', (e) => {
            const text = e.get_text();
            if (text) {
                // Simplified validation: just check if it's not empty
                // In a production extension, you might want to validate the combo string
                settings.set_strv(key, [text]);
            }
        });

        // Listen for external changes (like reset)
        settings.connect(`changed::${key}`, () => {
            const newVal = settings.get_strv(key)[0] || '';
            if (entry.get_text() !== newVal) {
                entry.set_text(newVal);
            }
        });

        row.add_suffix(entry);

        const resetButton = new Gtk.Button({
            icon_name: 'edit-clear-all-symbolic',
            tooltip_text: _('Reset to default'),
            valign: Gtk.Align.CENTER,
            css_classes: ['flat']
        });

        resetButton.connect('clicked', () => {
            settings.reset(key);
        });

        row.add_suffix(resetButton);
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
