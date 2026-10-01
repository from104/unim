#include <stdio.h>
#include <stdlib.h>
#include <unim.h>

// UnimInputResult 는 bool 5개 고정 레이아웃(ABI). Rust 쪽 size_of 단언과 짝.
_Static_assert(sizeof(UnimInputResult) == 5, "UnimInputResult layout changed");

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
    // 일반 칸에서는 한/영 전환 차단 플래그가 서지 않는다.
    printf("toggle blocked: %s\n", unim_engine_last_toggle_blocked(engine) ? "yes" : "no");
    unim_engine_reset(engine);

    // 2. Test 3-bul 390
    // Phase 8: layout is now a profile-name string (the KoreanLayout enum was
    // removed). Pass "ko_3bul390" instead of the old UNIM_HANGUL_LAYOUT_* enum.
    printf("\n--- Testing 3-bul 390 (Should be '한') ---\n");
    unim_engine_set_korean_layout(engine, "ko_3bul390");
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
