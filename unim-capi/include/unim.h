/* UNIM Input Method C API - Auto-generated header for external modules */
#ifndef UNIM_CAPI_H
#define UNIM_CAPI_H

#include <stdint.h>
#include <stdbool.h>
#include <stddef.h>

#ifdef __cplusplus
extern "C" {
#endif

/* API Version */
#define UNIM_API_VERSION 1

/* ============================================
 * Type Definitions
 * ============================================ */

/**
 * Input category (Korean/English)
 */
typedef enum {
    UNIM_INPUT_CATEGORY_KOREAN = 0,
    UNIM_INPUT_CATEGORY_ENGLISH = 1,
} UnimInputCategory;

/*
 * Note (Phase 8): UnimKoreanLayout enum was removed. Korean layout is now a
 * profile-name string (e.g., "ko_2bulstd", "ko_3bul390", "ko_3bul_qwerty") that
 * can match either a built-in profile or a user profile under
 * `~/.config/unim/layouts/<name>.json`. Legacy enum names ("Dubeolsik" etc.)
 * and short aliases ("2bul", "390") are auto-promoted to the canonical name.
 *
 * The corresponding setter/getter functions now take/return C strings; see
 * `unim_config_set_korean_layout` / `unim_config_get_korean_layout` below.
 */

/**
 * English keyboard layout
 */
typedef enum {
    UNIM_ENGLISH_LAYOUT_QWERTY = 0,
    UNIM_ENGLISH_LAYOUT_DVORAK = 1,
    UNIM_ENGLISH_LAYOUT_COLEMAK = 2,
    UNIM_ENGLISH_LAYOUT_COLEMAK_DH = 3,
    UNIM_ENGLISH_LAYOUT_WORKMAN = 4,
} UnimEnglishLayout;

/**
 * Mode sharing mode (Global/PerApp)
 */
typedef enum {
    UNIM_MODE_SHARING_GLOBAL = 0,   /**< All apps share the same input mode */
    UNIM_MODE_SHARING_PER_APP = 1,  /**< Each app maintains its own input mode */
} UnimModeSharingMode;

/**
 * Modifier key state
 */
typedef struct {
    bool shift;
    bool control;
    bool alt;
    bool super_key;
    bool caps_lock;
    bool num_lock;
} UnimModifierState;

/**
 * Input result
 */
typedef struct {
    bool consumed;
    bool preedit_changed;
    bool commit_changed;
} UnimInputResult;

/**
 * UTF-8 string reference (valid while engine is alive)
 */
typedef struct {
    const uint8_t *ptr;
    size_t len;
} UnimStr;

/* Opaque types */
typedef struct Config UnimConfig;
typedef struct InputEngine UnimEngine;

/* ============================================
 * API Version
 * ============================================ */

/**
 * Returns the API version.
 */
size_t unim_api_version(void);

/* ============================================
 * Configuration Management
 * ============================================ */

/**
 * Loads configuration from the default path.
 * Returns default config if file not found or parse error.
 * 
 * @return Config pointer. Must be freed with unim_config_delete().
 */
UnimConfig *unim_config_load(void);

/**
 * Creates a default configuration.
 * 
 * @return Config pointer. Must be freed with unim_config_delete().
 */
UnimConfig *unim_config_default(void);

/**
 * Ensures the config file exists and is valid.
 * 
 * Creates default config file if missing or invalid.
 * 
 * @return true if config file is valid, false on error.
 */
bool unim_config_ensure_file(void);

/**
 * Frees a Config object.
 * 
 * @param config Config pointer created by unim_config_load or unim_config_default.
 */
void unim_config_delete(UnimConfig *config);

/**
 * Checks if the config file has been modified and needs reload.
 * 
 * @param config Config pointer.
 * @return true if reload is needed.
 */
bool unim_config_needs_reload(const UnimConfig *config);

/**
 * Reloads the config from file if it has been modified.
 * 
 * @param config Config pointer (will be updated in place).
 * @return true if reload was successful, false if no change or failed.
 */
bool unim_config_reload(UnimConfig *config);

/**
 * Sets the Korean layout profile name in the configuration.
 *
 * @param layout Null-terminated UTF-8 profile name (e.g., "ko_2bulstd" or a user profile).
 *               Legacy enum names and short aliases are auto-normalized.
 * @return true on success; false if layout is NULL, invalid UTF-8, or empty.
 */
bool unim_config_set_korean_layout(UnimConfig *config, const char *layout);

/**
 * Sets the English layout in the configuration.
 */
void unim_config_set_english_layout(UnimConfig *config, UnimEnglishLayout layout);

/* ============================================
 * Engine Lifecycle
 * ============================================ */

/**
 * Creates a new InputEngine.
 * 
 * @param config Configuration reference.
 * @return Engine pointer. Must be freed with unim_engine_delete().
 */
UnimEngine *unim_engine_new(const UnimConfig *config);

/**
 * Frees an InputEngine object.
 * 
 * @param engine Engine pointer created by unim_engine_new.
 */
void unim_engine_delete(UnimEngine *engine);

/* ============================================
 * Input Processing
 * ============================================ */

/**
 * Processes a key press.
 * 
 * @param engine Engine reference.
 * @param config Config reference.
 * @param hardware_code Hardware keycode (evdev).
 * @param state Modifier key state.
 * @return Input result.
 */
UnimInputResult unim_engine_press_key(
    UnimEngine *engine,
    const UnimConfig *config,
    uint16_t hardware_code,
    UnimModifierState state
);

/**
 * Returns the commit string.
 * Valid while engine is alive and until next key press.
 * 
 * @param engine Engine reference.
 * @return UTF-8 string reference.
 */
UnimStr unim_engine_commit_str(const UnimEngine *engine);

/**
 * Returns the preedit string.
 * Valid while engine is alive and until next key press.
 * 
 * @param engine Engine reference.
 * @return UTF-8 string reference.
 */
UnimStr unim_engine_preedit_str(const UnimEngine *engine);

/* ============================================
 * State Management
 * ============================================ */

/**
 * Sets the input category.
 */
void unim_engine_set_input_category(UnimEngine *engine, UnimInputCategory category);

/**
 * Gets the current input category.
 */
UnimInputCategory unim_engine_get_input_category(const UnimEngine *engine);

/**
 * Set the Korean layout profile of the engine immediately.
 *
 * @param layout Null-terminated UTF-8 profile name. See `unim_config_set_korean_layout`.
 * @return true on success; false on NULL/invalid/empty input.
 */
bool unim_engine_set_korean_layout(UnimEngine *engine, const char *layout);

/**
 * Set the English layout of the engine immediately.
 */
void unim_engine_set_english_layout(UnimEngine *engine, UnimEnglishLayout layout);

/**
 * Resets the engine state.
 */
void unim_engine_reset(UnimEngine *engine);

/**
 * Clears the commit buffer.
 */
void unim_engine_clear_commit(UnimEngine *engine);

/**
 * Flushes preedit to commit.
 */
void unim_engine_clear_preedit(UnimEngine *engine);

/**
 * Removes preedit without committing.
 */
void unim_engine_remove_preedit(UnimEngine *engine);

/**
 * Checks if currently composing.
 */
bool unim_engine_is_composing(const UnimEngine *engine);

/**
 * Checks ready state (for frontend compatibility).
 */
bool unim_engine_check_ready(const UnimEngine *engine);

UnimInputResult unim_engine_end_ready(UnimEngine *engine);

/* ============================================
 * Configuration Getters
 * ============================================ */

/**
 * Gets the current Korean layout profile name from configuration.
 *
 * Returned UnimStr references a Rust-owned String; copy the content if you need to keep it.
 */
UnimStr unim_config_get_korean_layout(const UnimConfig *config);

/**
 * Gets the current English layout from configuration.
 */
UnimEnglishLayout unim_config_get_english_layout(const UnimConfig *config);

/**
 * Gets the default (initial) input category.
 *
 * @param config Config pointer.
 * @return Input category (KOREAN=0, ENGLISH=1).
 */
UnimInputCategory unim_config_get_default_category(const UnimConfig *config);

/**
 * Sets the default (initial) input category.
 *
 * @param config Config pointer.
 * @param category Input category (KOREAN=0, ENGLISH=1).
 */
void unim_config_set_default_category(UnimConfig *config, UnimInputCategory category);

/**
 * Saves the configuration to the default path.
 *
 * @param config Config pointer.
 * @return true if save was successful, false on error.
 */
bool unim_config_save(const UnimConfig *config);

/* ============================================
 * Mode Sharing Configuration
 * ============================================ */

/**
 * Gets the mode sharing mode.
 *
 * @return GLOBAL=0, PER_APP=1
 */
UnimModeSharingMode unim_config_get_mode_sharing(const UnimConfig *config);

/**
 * Sets the mode sharing mode.
 */
void unim_config_set_mode_sharing(UnimConfig *config, UnimModeSharingMode mode);

/**
 * Returns the number of supported mode sharing modes.
 */
size_t unim_mode_sharing_count(void);

/**
 * Returns the display name of a mode sharing mode (for UI).
 */
UnimStr unim_mode_sharing_display_name(UnimModeSharingMode mode);

/**
 * Returns the mode sharing mode at the specified index.
 */
UnimModeSharingMode unim_mode_sharing_at(size_t index);

/* ============================================
 * Layout Enumeration Helpers
 * ============================================ */

/**
 * Returns the number of supported Korean layouts.
 */
size_t unim_korean_layout_count(void);

/**
 * Returns the canonical profile name of a built-in Korean layout by index.
 * Out-of-range index returns an empty UnimStr.
 */
UnimStr unim_korean_layout_name(size_t index);

/**
 * Returns the display name of a built-in Korean layout by index (for UI).
 * Returns an empty UnimStr for out-of-range or non-built-in profiles.
 */
UnimStr unim_korean_layout_display_name(size_t index);

/**
 * Returns the number of supported English layouts.
 */
size_t unim_english_layout_count(void);

/**
 * Returns the internal name of an English layout.
 */
UnimStr unim_english_layout_name(UnimEnglishLayout layout);

/**
 * Returns the display name of an English layout (for UI).
 */
UnimStr unim_english_layout_display_name(UnimEnglishLayout layout);

/**
 * Legacy alias — identical to unim_korean_layout_name(index).
 * Kept for ABI continuity after the Phase 8 enum removal.
 * @param index Index (0 to unim_korean_layout_count()-1)
 * @return Canonical profile name as UnimStr (empty if out of range)
 */
UnimStr unim_korean_layout_at(size_t index);

/**
 * Returns the English layout at the specified index.
 * @param index Index (0 to unim_english_layout_count()-1)
 * @return English layout enum value
 */
UnimEnglishLayout unim_english_layout_at(size_t index);

/* ============================================
 * Status File Management
 * ============================================ */

/**
 * Gets the current input mode from status file.
 *
 * @return 0 = English, 1 = Korean, -1 = error
 */
int32_t unim_status_get(void);

/**
 * Sets the input mode to status file.
 *
 * @param category 0 = English, 1 = Korean
 * @return true if successful, false on error.
 */
bool unim_status_set(int32_t category);

/* ============================================
 * Popup State Management
 * ============================================ */

/* Opaque popup state type */
typedef struct PopupState UnimPopupState;

/* PopupKey constants */
#define UNIM_POPUP_KEY_NUMBER_1   1
#define UNIM_POPUP_KEY_NUMBER_9   9
#define UNIM_POPUP_KEY_LETTER_0  10  /* Q */
#define UNIM_POPUP_KEY_LETTER_8  18  /* O */
#define UNIM_POPUP_KEY_UP        20
#define UNIM_POPUP_KEY_DOWN      21
#define UNIM_POPUP_KEY_LEFT      22
#define UNIM_POPUP_KEY_RIGHT     23
#define UNIM_POPUP_KEY_ENTER     24
#define UNIM_POPUP_KEY_ESCAPE    25
#define UNIM_POPUP_KEY_TAB       26
#define UNIM_POPUP_KEY_SHIFT_TAB 27
#define UNIM_POPUP_KEY_PAGE_UP   28
#define UNIM_POPUP_KEY_PAGE_DOWN 29
#define UNIM_POPUP_KEY_SPACE     30
#define UNIM_POPUP_KEY_BACKSPACE 31
#define UNIM_POPUP_KEY_MODIFIER  32
#define UNIM_POPUP_KEY_OTHER     33

/* PopupKeyResult kind constants */
#define UNIM_POPUP_RESULT_SELECT      0
#define UNIM_POPUP_RESULT_CANCEL      1
#define UNIM_POPUP_RESULT_UPDATED     2
#define UNIM_POPUP_RESULT_CONSUMED    3
#define UNIM_POPUP_RESULT_NOT_HANDLED 4

/**
 * Popup key result returned by unim_popup_handle_key().
 */
typedef struct {
    int32_t kind;           /**< Result kind (UNIM_POPUP_RESULT_*) */
    int32_t selected_index; /**< Selected index for SELECT, -1 otherwise */
} UnimPopupKeyResult;

/**
 * Converts GDK keyval to PopupKey constant.
 */
uint32_t unim_popup_key_from_gdk(uint32_t gdk_keyval);

/**
 * Converts Qt key to PopupKey constant.
 */
uint32_t unim_popup_key_from_qt(int32_t qt_key);

/**
 * Creates a new hanja popup state.
 *
 * @param target_ptr Target string pointer (UTF-8)
 * @param target_len Target string length
 * @param hanja_ptrs Array of hanja string pointers
 * @param hanja_lens Array of hanja string lengths
 * @param meaning_ptrs Array of meaning string pointers
 * @param meaning_lens Array of meaning string lengths
 * @param count Number of candidates
 * @return PopupState pointer. Must be freed with unim_popup_free().
 */
UnimPopupState *unim_popup_new_hanja(
    const uint8_t *target_ptr, size_t target_len,
    const uint8_t **hanja_ptrs, const size_t *hanja_lens,
    const uint8_t **meaning_ptrs, const size_t *meaning_lens,
    size_t count
);

/**
 * Creates a new special character popup state.
 *
 * @param target_ptr Target string pointer (UTF-8)
 * @param target_len Target string length
 * @param char_ptrs Array of character string pointers
 * @param char_lens Array of character string lengths
 * @param count Number of characters
 * @param top_row_ptr Top row label string pointer (UTF-8)
 * @param top_row_len Top row label string length
 * @return PopupState pointer. Must be freed with unim_popup_free().
 */
UnimPopupState *unim_popup_new_special(
    const uint8_t *target_ptr, size_t target_len,
    const uint8_t **char_ptrs, const size_t *char_lens,
    size_t count,
    const uint8_t *top_row_ptr, size_t top_row_len
);

/**
 * Frees a PopupState.
 */
void unim_popup_free(UnimPopupState *state);

/**
 * Processes a key event in the popup.
 *
 * @param state PopupState pointer
 * @param popup_key PopupKey constant (UNIM_POPUP_KEY_*)
 * @return Key result with kind and optional selected_index
 */
UnimPopupKeyResult unim_popup_handle_key(UnimPopupState *state, uint32_t popup_key);

/**
 * Processes a mouse click in the popup.
 */
UnimPopupKeyResult unim_popup_handle_click(UnimPopupState *state, int32_t row, int32_t col);

/* Popup state queries */
int32_t unim_popup_get_kind(const UnimPopupState *state);          /**< 0=Hanja, 1=Special */
int32_t unim_popup_get_rows(const UnimPopupState *state);
int32_t unim_popup_get_cols(const UnimPopupState *state);
int32_t unim_popup_get_sel_row(const UnimPopupState *state);
int32_t unim_popup_get_sel_col(const UnimPopupState *state);
int32_t unim_popup_get_current_page(const UnimPopupState *state);
int32_t unim_popup_get_total_pages(const UnimPopupState *state);
int32_t unim_popup_get_total_items(const UnimPopupState *state);
bool    unim_popup_cell_exists(const UnimPopupState *state, int32_t row, int32_t col);
UnimStr unim_popup_get_cell_text(const UnimPopupState *state, int32_t row, int32_t col);
UnimStr unim_popup_get_item(const UnimPopupState *state, int32_t index);
UnimStr unim_popup_get_meaning(const UnimPopupState *state, int32_t index);
int32_t unim_popup_selected_index(const UnimPopupState *state);
UnimStr unim_popup_get_target(const UnimPopupState *state);
UnimStr unim_popup_get_top_row(const UnimPopupState *state);

#ifdef __cplusplus
}
#endif

#endif /* UNIM_CAPI_H */
