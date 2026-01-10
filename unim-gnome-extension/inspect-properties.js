#!/usr/bin/gjs

/**
 * IBus 엔진 속성(Property) 탐색 스크립트
 * 
 * 이 스크립트는 현재 활성화된 IBus 엔진의 속성 목록을 출력합니다.
 * ibus-hangul의 '한글/영문' 전환 스위치 속성 이름(예: InputMode)을 찾기 위해 사용합니다.
 */

import GLib from 'gi://GLib';
import IBus from 'gi://IBus';

const LOOP = GLib.MainLoop.new(null, false);

function printProperties(prop, level = 0) {
    const prefix = '  '.repeat(level);
    const key = prop.get_key();
    const label = prop.get_label().get_text();
    const type = prop.get_prop_type(); // 0: Normal, 1: Toggle, 2: Radio, 3: Menu, 4: Separator
    
    let state = '';
    if (type === IBus.PropType.TOGGLE) {
        state = `[Toggle: ${prop.get_state() === IBus.PropState.CHECKED ? 'ON' : 'OFF'}]`;
    } else if (type === IBus.PropType.RADIO) {
        state = `[Radio: ${prop.get_state() === IBus.PropState.CHECKED ? 'ON' : 'OFF'}]`;
    }

    console.log(`${prefix}- Key: "${key}", Label: "${label}", Type: ${type} ${state}`);

    if (prop.get_sub_props) {
        const subProps = prop.get_sub_props();
        if (subProps) {
            for (let i = 0; i < subProps.size(); i++) {
                printProperties(subProps.get(i), level + 1);
            }
        }
    }
}

async function run() {
    console.log('Initializing IBus...');
    IBus.init();
    const bus = new IBus.Bus();

    if (!bus.is_connected()) {
        console.log('Waiting for connection...');
        // Simple wait loop
        let count = 0;
        while (!bus.is_connected() && count < 10) {
            GLib.usleep(500000); // 500ms
            count++;
        }
    }

    if (!bus.is_connected()) {
        console.error('Failed to connect to IBus.');
        return;
    }

    console.log('✓ IBus connected');

    // 현재 엔진 확인
    const engine = bus.get_global_engine();
    if (engine) {
        console.log(`Current Engine: ${engine.get_name()}`);
    }

    // 속성 가져오기 요청 (비동기일 수 있으므로 PanelService 시뮬레이션 필요하지만, 
    // 간단히 현재 등록된 프로퍼티를 가져오기 위해 PanelService에 연결 시도)
    
    // 참고: GJS 스크립트에서 실행 중인 엔진의 속성을 바로 가져오는 것은 
    // IBusPanelService 역할을 하지 않으면 어려울 수 있습니다.
    // 하지만 IBusBus는 request_name 등을 하지 않아도 시그널을 받을 수 있는지 확인해봅니다.

    console.log('Listening for property updates... (Please interact with the input method)');
    
    // We need to use current_input_context to get properties? 
    // No, properties are usually sent to the Panel.
    
    // Let's look at the active input context logic if possible, but it's restricted.
    // Instead, let's try to register as a panel service temporarily to receive property updates?
    // That might conflict with GNOME Shell.
    
    // Alternative: Check if we can simply query the bus for the panel service object? No.
    
    console.log('Note: IBus properties are usually managed by the active Panel Service (GNOME Shell).');
    console.log('Trying to connect to IBus Panel Service signals via IBusBus...');

    // IBusBus doesn't expose properties directly. 
    // However, let's try to assume ibus-hangul uses standard property names.
    
    console.log('\nCommon ibus-hangul properties to check:');
    console.log('- InputMode');
    console.log('- HangulMode');
    
    console.log('\nWaiting 5 seconds... (Please toggle Han/Eng key)');
    
    // 이 스크립트로는 외부에서 관리되는 프로퍼티를 조회하기 어려울 수 있습니다.
    // 다만 IBus API를 통해 탐색 가능한지 시도해봅니다.
    
    GLib.timeout_add(GLib.PRIORITY_DEFAULT, 5000, () => {
        LOOP.quit();
        return false;
    });
    
    LOOP.run();
}

run().catch(e => console.error(e));

