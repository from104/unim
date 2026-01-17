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
 * Input category (Hangul/Latin)
 */
typedef enum {
    UNIM_INPUT_CATEGORY_HANGUL = 0,
    UNIM_INPUT_CATEGORY_LATIN = 1,
} UnimInputCategory;

/**
 * Hangul keyboard layout
 */
typedef enum {
    UNIM_HANGUL_LAYOUT_DUBEOLSIK = 0,
    UNIM_HANGUL_LAYOUT_SEBEOLSIK_390 = 1,
    UNIM_HANGUL_LAYOUT_SEBEOLSIK_391 = 2,
} UnimHangulLayout;

/**
 * Latin keyboard layout
 */
typedef enum {
    UNIM_LATIN_LAYOUT_QWERTY = 0,
    UNIM_LATIN_LAYOUT_DVORAK = 1,
} UnimLatinLayout;

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
 * Sets the Hangul layout in the configuration.
 */
void unim_config_set_hangul_layout(UnimConfig *config, UnimHangulLayout layout);

/**
 * Sets the Latin layout in the configuration.
 */
void unim_config_set_latin_layout(UnimConfig *config, UnimLatinLayout layout);

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
 * Set the Hangul layout of the engine immediately.
 */
void unim_engine_set_hangul_layout(UnimEngine *engine, UnimHangulLayout layout);

/**
 * Set the Latin layout of the engine immediately.
 */
void unim_engine_set_latin_layout(UnimEngine *engine, UnimLatinLayout layout);

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
 * Gets the current Hangul layout from configuration.
 */
UnimHangulLayout unim_config_get_hangul_layout(const UnimConfig *config);

/**
 * Gets the current Latin layout from configuration.
 */
UnimLatinLayout unim_config_get_latin_layout(const UnimConfig *config);

#ifdef __cplusplus
}
#endif

#endif /* UNIM_CAPI_H */
