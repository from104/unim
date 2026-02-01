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

/**
 * Korean keyboard layout
 */
typedef enum {
    UNIM_KOREAN_LAYOUT_DUBEOLSIK = 0,
    UNIM_KOREAN_LAYOUT_SEBEOLSIK_390 = 1,
    UNIM_KOREAN_LAYOUT_SEBEOLSIK_391 = 2,
    UNIM_KOREAN_LAYOUT_SEBEOLSIK_NOSHIFT = 3,
} UnimKoreanLayout;

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
 * Sets the Korean layout in the configuration.
 */
void unim_config_set_korean_layout(UnimConfig *config, UnimKoreanLayout layout);

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
 * Set the Korean layout of the engine immediately.
 */
void unim_engine_set_korean_layout(UnimEngine *engine, UnimKoreanLayout layout);

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
 * Gets the current Korean layout from configuration.
 */
UnimKoreanLayout unim_config_get_korean_layout(const UnimConfig *config);

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
 * Auto Switch Configuration
 * ============================================ */

/**
 * Gets the auto switch enabled state.
 */
bool unim_config_get_auto_switch_enabled(const UnimConfig *config);

/**
 * Sets the auto switch enabled state.
 */
void unim_config_set_auto_switch_enabled(UnimConfig *config, bool enabled);

/**
 * Gets the auto switch threshold (0.0 to 1.0).
 */
float unim_config_get_auto_switch_threshold(const UnimConfig *config);

/**
 * Sets the auto switch threshold (0.0 to 1.0).
 */
void unim_config_set_auto_switch_threshold(UnimConfig *config, float threshold);

/**
 * Gets the auto switch notification enabled state.
 */
bool unim_config_get_auto_switch_notification(const UnimConfig *config);

/**
 * Sets the auto switch notification enabled state.
 */
void unim_config_set_auto_switch_notification(UnimConfig *config, bool show);

/* ============================================
 * Layout Enumeration Helpers
 * ============================================ */

/**
 * Returns the number of supported Korean layouts.
 */
size_t unim_korean_layout_count(void);

/**
 * Returns the internal name of a Korean layout.
 */
UnimStr unim_korean_layout_name(UnimKoreanLayout layout);

/**
 * Returns the display name of a Korean layout (for UI).
 */
UnimStr unim_korean_layout_display_name(UnimKoreanLayout layout);

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
 * Returns the Korean layout at the specified index.
 * @param index Index (0 to unim_korean_layout_count()-1)
 * @return Korean layout enum value
 */
UnimKoreanLayout unim_korean_layout_at(size_t index);

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

#ifdef __cplusplus
}
#endif

#endif /* UNIM_CAPI_H */
