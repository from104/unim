#include <stdio.h>
#include <stdlib.h>
#include <unim.h>

void print_state(UnimEngine *engine) {
    UnimStr preedit = unim_engine_preedit_str(engine);
    UnimStr commit = unim_engine_commit_str(engine);
    
    printf("Preedit: '%.*s', Commit: '%.*s'\n", (int)preedit.len, preedit.ptr, (int)commit.len, commit.ptr);
}

int main() {
    printf("UNIM C-API Dynamic Layout Test Start\n");

    UnimConfig *config = unim_config_default();
    UnimEngine *engine = unim_engine_new(config);
    UnimModifierState no_mod = {false, false, false, false, false, false};

    // 1. Test 2-bul (Default)
    printf("\n--- Testing 2-bul (Should be '한') ---\n");
    // 'g'(34), 'k'(37), 's'(31)
    unim_engine_press_key(engine, config, 34, no_mod);
    unim_engine_press_key(engine, config, 37, no_mod);
    unim_engine_press_key(engine, config, 31, no_mod);
    print_state(engine);
    unim_engine_reset(engine);

    // 2. Test 3-bul 390
    printf("\n--- Testing 3-bul 390 (Should be '한') ---\n");
    unim_engine_set_hangul_layout(engine, UNIM_HANGUL_LAYOUT_SEBEOLSIK_390);
    // ㅎ(M:50), ㅏ(F:33), ㄴ(S:31)
    printf("Pressing 'M'(50), 'F'(33), 'S'(31)...\n");
    unim_engine_press_key(engine, config, 50, no_mod); // ㅎ
    unim_engine_press_key(engine, config, 33, no_mod); // ㅏ
    unim_engine_press_key(engine, config, 31, no_mod); // ㄴ
    print_state(engine);

    unim_engine_delete(engine);
    unim_config_delete(config);

    printf("\nUNIM C-API Test Finished\n");
    return 0;
}
