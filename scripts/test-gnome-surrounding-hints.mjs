#!/usr/bin/env node
/*
 * GNOME 확장 대상③ 거부 신호(pointer_hints.js) 단위 시험 + 확장 소스 정적 검사.
 *
 * 순수 JS 모듈이라 gjs·GNOME 없이 node 로 돈다. 확장 디렉터리는 .js 가 ESM 이지만
 * package.json 이 없어 node 버전에 따라 모듈 판별이 갈리므로, 소스를 읽어 data: URL
 * 로 import 한다(어느 node 에서도 동일). 확장에는 시험 파일을 두지 않는다 —
 * 패키징(zip·install)에 섞이지 않게 scripts/ 에 둔다.
 *
 * 사용법: node scripts/test-gnome-surrounding-hints.mjs
 * 종료 코드: 0 = 전부 통과, 1 = 실패 있음
 */

import { readFileSync, readdirSync } from 'node:fs';
import { fileURLToPath } from 'node:url';
import { dirname, join } from 'node:path';

const here = dirname(fileURLToPath(import.meta.url));
const extDir = join(here, '..', 'unim-gnome-extension');
const read = (name) => readFileSync(join(extDir, name), 'utf8');

const mod = await import('data:text/javascript;base64,' + Buffer.from(read('pointer_hints.js')).toString('base64'));
const {
    createPointerState, classifyPointer, hintKeyLogName,
    MULTI_CLICK_MS, DRAG_THRESHOLD_PX,
    HINT_KEY, HINT_POINTER_PLAIN, HINT_POINTER_SELECT, HINT_PRIMARY_CHANGED, HINT_SELECTION_UNKNOWN,
} = mod;

let pass = 0;
let fail = 0;

function eq(name, actual, expected) {
    const a = JSON.stringify(actual);
    const e = JSON.stringify(expected);
    if (a === e) {
        pass++;
    } else {
        fail++;
        console.error(`FAIL ${name}\n   기대 ${e}\n   실제 ${a}`);
    }
}

// ── 상수(데몬 계약과 같은 값) ────────────────────────────────────────────
eq('상수: kind 값', [HINT_KEY, HINT_POINTER_PLAIN, HINT_POINTER_SELECT, HINT_PRIMARY_CHANGED, HINT_SELECTION_UNKNOWN],
    [1, 2, 3, 4, 5]);
eq('상수: 더블클릭 500ms · 드래그 4px', [MULTI_CLICK_MS, DRAG_THRESHOLD_PX], [500, 4]);

// 사건 열을 순서대로 먹여 힌트 열을 돌려준다.
function run(events, state = createPointerState()) {
    const hints = [];
    for (const ev of events) {
        const r = classifyPointer(state, ev);
        state = r.state;
        hints.push(r.hint);
    }
    return { hints, state };
}
const press = (nowMs, o = {}) => ({ type: 'press', button: 1, shift: false, nowMs, x: 100, y: 100, ...o });
const motion = (x, y) => ({ type: 'motion', x, y });
const release = (button = 1) => ({ type: 'release', button });

// ── 클릭 ─────────────────────────────────────────────────────────────────
eq('단일 클릭 → PLAIN', run([press(1000)]).hints, [2]);
eq('단일 클릭의 뗌은 보고 없음', run([press(1000), release()]).hints, [2, 0]);
eq('Shift+클릭 → SELECT', run([press(1000, { shift: true })]).hints, [3]);
eq('500ms 안의 재눌림(더블클릭) → SELECT', run([press(1000), release(), press(1300)]).hints, [2, 0, 3]);
eq('세 번째 눌림도 SELECT', run([press(1000), press(1200), press(1400)]).hints, [2, 3, 3]);
eq('정확히 500ms 간격은 단일 클릭(초과 아님)', run([press(1000), press(1500)]).hints, [2, 2]);
eq('499ms 간격은 더블클릭', run([press(1000), press(1499)]).hints, [2, 3]);
eq('500ms 넘긴 재눌림 → PLAIN', run([press(1000), press(1600)]).hints, [2, 2]);

// ── 버튼 ─────────────────────────────────────────────────────────────────
eq('보조 버튼(3) 무시', run([press(1000, { button: 3 })]).hints, [0]);
eq('보조 버튼은 더블클릭 판정에 끼지 않는다', run([press(1000), press(1100, { button: 3 }), press(1900)]).hints, [2, 0, 2]);
eq('가운데 버튼(2) → PLAIN', run([press(1000, { button: 2 })]).hints, [2]);
eq('가운데 버튼은 더블클릭 판정에 끼지 않는다', run([press(1000, { button: 2 }), press(1200)]).hints, [2, 2]);
eq('가운데 버튼 뗌은 보고 없음', run([release(2)]).hints, [0]);

// ── 드래그 ───────────────────────────────────────────────────────────────
eq('눌리지 않은 모션은 무시', run([motion(500, 500)]).hints, [0]);
eq('4px 이하 이동은 드래그 아님', run([press(1000), motion(104, 100)]).hints, [2, 0]);
eq('4px 초과 이동 → SELECT', run([press(1000), motion(105, 100)]).hints, [2, 3]);
eq('대각 이동도 거리로 판정(3,3 → 4.24px)', run([press(1000), motion(103, 103)]).hints, [2, 3]);
eq('드래그는 눌림당 1회만', run([press(1000), motion(110, 100), motion(150, 100), motion(200, 100)]).hints, [2, 3, 0, 0]);
eq('뗀 뒤 모션은 무시', run([press(1000), release(), motion(300, 300)]).hints, [2, 0, 0]);
eq('다음 눌림에서 드래그 보고 재무장', run([press(1000), motion(200, 100), release(), press(2000), motion(300, 100)]).hints,
    [2, 3, 0, 2, 3]);
eq('Shift+클릭 뒤 드래그도 눌림당 1회 보고', run([press(1000, { shift: true }), motion(200, 100)]).hints, [3, 3]);
eq('보조 버튼 뗌은 주 버튼 눌림 상태를 풀지 않는다',
    run([press(1000), release(3), motion(200, 100)]).hints, [2, 0, 3]);

// ── 터치 ─────────────────────────────────────────────────────────────────
eq('터치 → SELECT', run([{ type: 'touch' }]).hints, [3]);

// ── 알 수 없는 사건 ──────────────────────────────────────────────────────
eq('알 수 없는 사건 종류는 무시', run([{ type: 'scroll' }]).hints, [0]);

// ── 불변성 ───────────────────────────────────────────────────────────────
{
    const s0 = createPointerState();
    classifyPointer(s0, press(1000));
    eq('classifyPointer 는 입력 상태를 바꾸지 않는다', s0, createPointerState());
}

// ── 키 로그 이름(문자 키는 가린다) ───────────────────────────────────────
eq('로그 이름: 탐색 키', [0xff51, 0xff52, 0xff53, 0xff54, 0xff50, 0xff57, 0xff55, 0xff56].map(hintKeyLogName),
    ['left', 'up', 'right', 'down', 'home', 'end', 'pgup', 'pgdn']);
eq('로그 이름: 키패드 탐색 키', [0xff96, 0xff98, 0xff95, 0xff9c].map(hintKeyLogName), ['left', 'right', 'home', 'end']);
eq('로그 이름: 편집 키', [0xff0d, 0xff09, 0xff08, 0xffff, 0xff1b].map(hintKeyLogName),
    ['enter', 'tab', 'backspace', 'delete', 'escape']);
eq('로그 이름: 문자 키는 char', [0x61, 0x41, 0xea3, 0x3b1, 0x1000041].map(hintKeyLogName),
    ['char', 'char', 'char', 'char', 'char']);

// ── 정적 검사(소스 문자열) — 회귀 방지 ─────────────────────────────────────
const extSrc = read('extension.js');
const khSrc = read('key_handler.js');
const dbusSrc = read('dbus_ime.js');
const imSrc = read('unim_input_method.js');

function bodyOf(source, marker) {
    const i = source.indexOf(marker);
    if (i < 0) return null;
    const open = source.indexOf('{', i);
    let depth = 0;
    for (let k = open; k < source.length; k++) {
        if (source[k] === '{') depth++;
        else if (source[k] === '}' && --depth === 0) return source.slice(open, k + 1);
    }
    return null;
}

// 1) 이모지 팝업 표시 경로는 preedit 을 건드리면 안 된다(GTK4 가 preedit-start 에서 선택을
//    지운다, ce32aed). onShow* 본문에 updatePreedit 호출이 없어야 한다.
for (const marker of ['onShowEmoji:', 'onShowHanja:', 'onShowSpecial:']) {
    const body = bodyOf(extSrc, marker);
    eq(`${marker} 본문을 찾음`, body !== null, true);
    eq(`${marker} 안에 updatePreedit 호출 없음`, body !== null && /updatePreedit\s*\(/.test(body), false);
}
// ZWSP 로 preedit 을 세우는 코드 금지: updatePreedit 의 인자에 ZWSP 상수·리터럴이 없어야 한다.
eq('updatePreedit(ZWSP) 호출 없음', /updatePreedit\s*\([^)]*(ZWSP|\\u200[bB])/.test(extSrc), false);

// 2) 진단 — 어느 사본이 로드됐는지(버전·경로)가 남아야 한다.
eq('enable() 이 version·path 를 로그', /version=\$\{this\.metadata/.test(extSrc) && /path=\$\{this\.path/.test(extSrc), true);

// 3) 클립보드·PRIMARY 내용 읽기 금지(HANJA_WORD_SPEC v1.6 — 읽기도 금지). 패턴 문자열이 이 파일 자체에
//    걸리지 않게 조각을 이어 만든다.
const FORBIDDEN = [
    ['St', 'Clipboard'].join('.'),
    'get_' + 'mimetypes(',
    'primary' + '_selection',
    'gi://' + 'St',
];
const extFiles = readdirSync(extDir).filter((f) => f.endsWith('.js'));
for (const f of extFiles) {
    // prefs.js 는 별도 프로세스(Adw)이고 St 를 쓰지 않는다 — 전 파일에 같은 규칙을 적용한다.
    const text = read(f);
    for (const pat of FORBIDDEN.slice(0, 3)) {
        eq(`${f}: 금지 패턴 '${pat}' 없음`, text.includes(pat), false);
    }
}
eq('key_handler.js 에 get_text 호출 없음(PRIMARY 내용 읽기 금지)', /\.get_text\s*\(/.test(khSrc), false);
eq('key_handler.js 가 St 를 import 하지 않는다', khSrc.includes(FORBIDDEN[3]), false);
eq('옛 게이트 로그 제거', khSrc.includes('PRIMARY 게이트:'), false);
eq('옛 키보드 선택 추적 제거', /_trackKbdKey|_kbdSel|_watchImFocus|_probePrimaryThenProcess/.test(khSrc), false);
eq('옛 RELEASE 지연 큐잉 제거', /setKeyReleaseHandler|_deferRelease/.test(khSrc + imSrc), false);

// 4) 새 배선
eq('owner-changed 구독', khSrc.includes("'owner-changed'"), true);
eq('SELECTION_PRIMARY 만 PRIMARY_CHANGED 로 보고',
    /selectionType === Meta\.SelectionType\.SELECTION_PRIMARY/.test(khSrc) && khSrc.includes('HINT_PRIMARY_CHANGED'), true);
eq('stage captured-event 포인터 보고(_reportPointer)', khSrc.includes('this._reportPointer(event)'), true);
{
    const body = bodyOf(khSrc, '_bypassCombo(keyval, evdevKeycode, taggedState, isAutoEnglishTrigger) {');
    eq('_bypassCombo 본문을 찾음', body !== null, true);
    eq('_bypassCombo 가 KEY 힌트를 보낸다(_sendHint(HINT_KEY …))',
        body !== null && /_sendHint\(HINT_KEY,\s*keyval,\s*taggedState\)/.test(body), true);
    // 순서: 로컬 커밋 → KEY 힌트 → Reset/processKeyAsync
    const iHint = body ? body.indexOf('_sendHint(HINT_KEY') : -1;
    const iReset = body ? body.indexOf('resetAsync()') : -1;
    const iAsync = body ? body.indexOf('processKeyAsync(') : -1;
    const iCommit = body ? body.indexOf('commitText(preedit)') : -1;
    eq('힌트는 로컬 커밋 뒤', iCommit >= 0 && iHint > iCommit, true);
    eq('힌트는 resetAsync·processKeyAsync 앞', iHint >= 0 && iHint < iReset && iHint < iAsync, true);
}
{
    const body = bodyOf(khSrc, '_sendHint(kind');
    eq('_sendHint 는 비동기 송신(surroundingHintAsync)만 쓴다',
        body !== null && body.includes('surroundingHintAsync(') && !/\.surroundingHint\(/.test(body), true);
}
eq('힌트 로그에 keyval 평문을 남기지 않는다(hintKeyLogName 경유)',
    khSrc.includes('hintKeyLogName(keyval)') && !/unimLog\([^;]*key=\$\{keyval\}/.test(khSrc), true);

// 5) 능력 선언 · 확정 게이트
// 5a) 능력 비트 순수 함수(capabilities.js) — 실측 스위치(GSettings surrounding-word-replace)
{
    const cap = await import('data:text/javascript;base64,' + Buffer.from(read('capabilities.js')).toString('base64'));
    eq('능력 상수: REPLACE_SURROUNDING=0x1 · POINTER_HINTS=0x4',
        [cap.CAP_REPLACE_SURROUNDING, cap.CAP_POINTER_HINTS], [0x1, 0x4]);
    eq('스위치 키 이름', cap.SURROUNDING_WORD_REPLACE_KEY, 'surrounding-word-replace');
    eq('꺼짐(false) → 0x4', cap.computeClientCapabilities(false), 0x4);
    eq('켜짐(true) → 0x5', cap.computeClientCapabilities(true), 0x5);
    eq('미지정(undefined) → 0x4', cap.computeClientCapabilities(undefined), 0x4);
    eq('null → 0x4', cap.computeClientCapabilities(null), 0x4);
    eq('truthy 비불리언(1·"true") → 0x4(정확히 true 만 켠다)',
        [cap.computeClientCapabilities(1), cap.computeClientCapabilities('true')], [0x4, 0x4]);
    eq('SELECTION_RELIABLE(0x2) 는 어느 경우도 켜지 않는다',
        [cap.computeClientCapabilities(true), cap.computeClientCapabilities(false)].some((c) => (c & 0x2) !== 0), false);
    eq('대상③ 허용 조건 (caps&1)&&(caps&6): 켜짐=참 · 꺼짐=거짓',
        [0x5, 0x4].map((c) => Boolean((c & 1) && (c & 6))), [true, false]);
}

// 5b) 배선 — 하드코딩 상수 제거, 설정 키로 실시간 반영
eq('하드코딩 SURROUNDING_WORD_REPLACE_ENABLED 상수는 더 이상 없다',
    /SURROUNDING_WORD_REPLACE_ENABLED/.test(dbusSrc) || /SURROUNDING_WORD_REPLACE_ENABLED/.test(extSrc), false);
eq('dbus_ime.js 가 computeClientCapabilities 를 import',
    /import \{ computeClientCapabilities \} from '\.\/capabilities\.js'/.test(dbusSrc), true);
eq('dbus_ime.js 스위치 기본값 false', /this\._surroundingWordReplace = false;/.test(dbusSrc), true);
eq('focusIn 이 능력을 선언(현재 스위치 값 사용)',
    /focusIn\(windowId\) \{[\s\S]*?setClientCapabilitiesAsync\(computeClientCapabilities\(this\._surroundingWordReplace\)\)/.test(dbusSrc), true);
{
    const body = bodyOf(dbusSrc, 'setSurroundingWordReplace(enabled) {');
    eq('setSurroundingWordReplace 본문을 찾음', body !== null, true);
    eq('값이 같으면 재선언하지 않는다(조기 반환)', body !== null && /next === this\._surroundingWordReplace\) return false/.test(body), true);
    eq('값이 바뀌면 현재 컨텍스트에 즉시 재선언',
        body !== null && /setClientCapabilitiesAsync\(computeClientCapabilities\(next\)\)/.test(body), true);
}
eq('extension.js: 연결 전에 초기 값을 주입', /new UnimDbusIME\(\);[\s\S]*?setSurroundingWordReplace\(this\._readSurroundingWordReplace\(\)\)/.test(extSrc), true);
eq('extension.js: 키 변경을 _connectSettingChanged 로 구독(disable 에서 해제되는 목록)',
    /_connectSettingChanged\(SURROUNDING_WORD_REPLACE_KEY,/.test(extSrc), true);
eq('extension.js: disable() 이 _settingsChangedIds 전부 disconnect',
    /for \(const id of this\._settingsChangedIds\) \{\s*this\._settings\.disconnect\(id\);/.test(extSrc), true);
{
    const body = bodyOf(extSrc, '_readSurroundingWordReplace() {');
    eq('_readSurroundingWordReplace 본문을 찾음', body !== null, true);
    eq('키가 없는 옛 스키마는 has_key 로 거르고 false', body !== null && /has_key\(SURROUNDING_WORD_REPLACE_KEY\)\) return false/.test(body), true);
    eq('읽기 실패도 false 로 폴백', body !== null && /catch \(e\)[\s\S]*return false/.test(body), true);
}
{
    const xml = read('schemas/org.gnome.shell.extensions.unim.gschema.xml');
    const m = xml.match(/<key type="b" name="surrounding-word-replace">([\s\S]*?)<\/key>/);
    eq('gschema 에 surrounding-word-replace(boolean) 키', m !== null, true);
    eq('gschema 기본값 false', m !== null && /<default>false<\/default>/.test(m[1]), true);
    eq('gschema 설명에 실측 전용·켜지 말 것 경고',
        m !== null && m[1].includes('HANJA_WORD_SPEC §6.5 (10) 실측용 — 실측 전에는 켜지 말 것'), true);
    eq('prefs UI 에 노출하지 않는다', read('prefs.js').includes('surrounding-word-replace'), false);
}
eq('SELECTION_RELIABLE(0x2) 미선언', /CAP_SELECTION_RELIABLE/.test(dbusSrc) || /CAP_SELECTION_RELIABLE/.test(read('capabilities.js')), false);
eq('ReplaceSurroundingText 글로벌 구독 + 자기 context 만 dispatch',
    dbusSrc.includes("'ReplaceSurroundingText',\n            null,") && /path === this\._contextPath/.test(dbusSrc), true);
eq('ReplaceSurroundingText 구독을 재구독 전에도 해제(_teardownDaemonSession)',
    (dbusSrc.match(/signal_unsubscribe\(this\._replaceSignalId\)/g) ?? []).length >= 2, true);
{
    const body = bodyOf(extSrc, 'onReplaceSurrounding:');
    eq('onReplaceSurrounding 본문을 찾음', body !== null, true);
    eq('onReplaceSurrounding 에 _hasFocus 게이트', body !== null && /!this\._inputMethod\?\._hasFocus/.test(body), true);
    eq('onReplaceSurrounding 은 replaceSurrounding 호출', body !== null && body.includes('replaceSurrounding('), true);
}
eq('replaceSurrounding: delete_surrounding(-a, a+b) 뒤 commit',
    /delete_surrounding\(-deleteBefore, deleteBefore \+ deleteAfter\)/.test(imSrc), true);
eq('vfunc_focus_out 이 surrounding 캐시를 새 객체로 초기화',
    /vfunc_focus_out\(\) \{[\s\S]*?this\._lastSurrounding = \{ text: '', cursor: 0, anchor: 0 \};/.test(imSrc), true);
eq('vfunc_set_surrounding 은 Mutter 값을 그대로 전달(가짜 anchor 없음)',
    /setSurroundingText\(text \|\| '', cursor \|\| 0, anchor \|\| 0\)/.test(imSrc), true);

// 6) 평문 로그 금지(HANJA_WORD_SPEC §2.8) — 확정 문자열을 로그에 보간하지 않는다.
eq('dbus_ime.js: AutoTypefixApply/CommitText 평문 로그 없음',
    /unimLog\('DBUS_IME', `(?:AutoTypefixApply|CommitText|UpdatePreeditText|ReplaceSurroundingText)[^`]*'\$\{/.test(dbusSrc), false);
eq('extension.js: AutoTypeFix/CommitText 평문 로그 없음',
    /unimLog\('EXT', `(?:AutoTypeFix|CommitText|ReplaceSurrounding)[^`]*'\$\{/.test(extSrc), false);

// ── surrounding_facts.js — 필드 세대·재송신 판정·확정 대조 (통합 계획 P3-2·P3-3) ──────────
const sf = await import('data:text/javascript;base64,' + Buffer.from(read('surrounding_facts.js')).toString('base64'));
{
    const {
        FACT_HINT_KEY, FACT_HINT_POINTER_PLAIN, FACT_HINT_POINTER_SELECT, HINT_FIELD_FOCUS, RESEND_MAX_AGE_MS,
        FIELD_FOCUS_RECENT_MS,
        createSurroundingFacts, factsOnFocusIn: focusInAt, factsOnFocusOut, factsOnSurrounding,
        factsOnHintSent, factsOnDocumentChanged, factsOnEngineKey, factsOnSensitive,
        decideResend, verifyReplaceExpectation,
    } = sf;
    const T0 = 10_000;
    const factsOnFocusIn = (f, t = T0) => focusInAt(f, t);   // 기본: 스냅샷과 같은 시각에 포커스
    const snap = { text: '국어교육을 한다', cursor: 3, anchor: 3 };
    const ctx = (over = {}) => ({ hasFocus: true, sensitive: false, nowMs: T0 + 50, ...over });
    const recv = (f, over = {}) => factsOnSurrounding(f, snap, { hasFocus: true, sensitive: false, nowMs: T0, ...over });

    // 상수·계약
    eq('facts: kind 값이 pointer_hints 와 같다', [FACT_HINT_KEY, FACT_HINT_POINTER_PLAIN, FACT_HINT_POINTER_SELECT],
        [HINT_KEY, HINT_POINTER_PLAIN, HINT_POINTER_SELECT]);
    eq('facts: FIELD_FOCUS 는 6', HINT_FIELD_FOCUS, 6);
    eq('facts: 필드 포커스 최근성·재송신 나이 500ms', [FIELD_FOCUS_RECENT_MS, RESEND_MAX_AGE_MS], [500, 500]);

    // 순서 A: focus_in → set_surrounding (정상 순서; FIELD_FOCUS 시점엔 캐시가 없다)
    {
        let f = createSurroundingFacts();
        f = factsOnFocusIn(f);
        eq('순서A: focus_in 시점에는 캐시 없음 → nocache', decideResend(f, ctx()), { resend: false, reason: 'nocache' });
        f = recv(f);
        // 창 FocusIn 이 뒤늦게 온 경우 — 같은 세대·힌트 없음 → 재송신
        eq('순서A: 그 뒤 창 FocusIn 직후 → 재송신', decideResend(f, ctx()), { resend: true, reason: 'ok' });
    }
    // 순서 B: set_surrounding → focus_in (포커스 전 도착분이 새 필드 몫)
    {
        let f = createSurroundingFacts();
        f = recv(f, { hasFocus: false });
        eq('순서B: 포커스 전 도착분은 다음 세대로 매긴다', f.cache.gen, 1);
        f = factsOnFocusIn(f);
        eq('순서B: focus_in 후 세대가 맞아떨어진다', [f.fieldGen, f.cache.gen], [1, 1]);
        eq('순서B: FIELD_FOCUS 직후 → 재송신', decideResend(f, ctx()), { resend: true, reason: 'ok' });
    }
    // 포커스 아웃은 캐시를 비운다 — 이전 필드 본문이 다음 필드로 새지 않는다
    {
        let f = factsOnFocusIn(createSurroundingFacts());
        f = recv(f);
        f = factsOnFocusOut(f);
        eq('focus_out 이 캐시를 비운다', f.cache, null);
        f = factsOnFocusIn(f);
        eq('focus_out 뒤 새 필드, 캐시 없으면 재송신 안 함', decideResend(f, ctx()).reason, 'nocache');
    }
    // 세대 불일치: 이전 필드에서 받은 캐시는 새 필드 FocusIn 에서 재송신하지 않는다
    {
        let f = factsOnFocusIn(createSurroundingFacts());
        f = recv(f);                       // gen 1 필드에서 받음
        f = factsOnFocusIn(f);             // focus_out 없이 필드가 바뀜(방어) → 세대 2
        eq('세대 불일치 → gen_mismatch', decideResend(f, ctx()), { resend: false, reason: 'gen_mismatch' });
    }
    // 거절 이유 우선순위
    {
        let f = factsOnFocusIn(createSurroundingFacts());
        f = recv(f);
        eq('nofocus 가 최우선', decideResend(f, ctx({ hasFocus: false, sensitive: true })).reason, 'nofocus');
        eq('비밀번호 칸 → sensitive', decideResend(f, ctx({ sensitive: true })), { resend: false, reason: 'sensitive' });
        eq('포커스가 500ms 보다 오래 → focus_old', decideResend(f, ctx({ nowMs: T0 + FIELD_FOCUS_RECENT_MS + 1 })).reason, 'focus_old');
        eq('경계(정확히 500ms)는 허용', decideResend(f, ctx({ nowMs: T0 + FIELD_FOCUS_RECENT_MS })).resend, true);
        // 포커스는 최근인데 캐시만 묵은 경우(캐시 수신이 포커스보다 훨씬 앞섬) → expired
        const g = recv(factsOnFocusIn(createSurroundingFacts(), T0 + 400));
        eq('묵은 캐시 → expired', decideResend(g, ctx({ nowMs: T0 + RESEND_MAX_AGE_MS + 1 })).reason, 'expired');
    }
    // 비밀번호 칸에서 받은 스냅샷은 아예 보관하지 않는다
    {
        let f = factsOnFocusIn(createSurroundingFacts());
        f = recv(f, { sensitive: true });
        eq('sensitive 수신 → 캐시 보관 안 함', f.cache, null);
        eq('sensitive 수신 후 재송신 안 함', decideResend(f, ctx()).reason, 'nocache');
    }
    // KEY·포인터 힌트·커밋 뒤에는 재송신 금지
    for (const [name, kind] of [['KEY', 1], ['POINTER_PLAIN', 2], ['POINTER_SELECT', 3]]) {
        let f = factsOnFocusIn(createSurroundingFacts());
        f = recv(f);
        f = factsOnHintSent(f, kind);
        eq(`${name} 힌트 뒤 → dirty`, decideResend(f, ctx()), { resend: false, reason: 'dirty' });
    }
    for (const [name, kind] of [['PRIMARY_CHANGED', 4], ['SELECTION_UNKNOWN', 5], ['FIELD_FOCUS', 6]]) {
        let f = factsOnFocusIn(createSurroundingFacts());
        f = recv(f);
        f = factsOnHintSent(f, kind);
        eq(`${name} 힌트는 재송신을 막지 않는다`, decideResend(f, ctx()).resend, true);
    }
    {
        let f = factsOnFocusIn(createSurroundingFacts());
        f = recv(f);
        f = factsOnDocumentChanged(f);
        eq('확장이 커밋한 뒤 → dirty', decideResend(f, ctx()).reason, 'dirty');
        f = recv(f);   // 새 스냅샷이 오면 다시 신선
        eq('새 스냅샷이 오면 dirty 해제', decideResend(f, ctx({ nowMs: T0 + 10 })).resend, true);
    }
    // 힌트 먼저 → 스냅샷 나중: 스냅샷이 힌트보다 새것이므로 재송신 가능
    {
        let f = factsOnFocusIn(createSurroundingFacts());
        f = factsOnHintSent(f, 1);
        f = recv(f);
        eq('힌트 뒤에 받은 스냅샷은 신선', decideResend(f, ctx()).resend, true);
    }
    // 불변성
    {
        const f0 = factsOnFocusIn(createSurroundingFacts());
        const f1 = recv(f0);
        const f2 = factsOnHintSent(f1, 1);
        eq('상태는 불변 갱신(입력 객체를 바꾸지 않는다)', [f0.cache, f1.cache.dirty, f2.cache.dirty], [null, false, true]);
    }

    // (a) 창 전환: 이전 창 필드의 캐시가 새 창 FocusIn 뒤로 새지 않는다
    {
        // A 필드: focus_in(T0) + 스냅샷. B 창 FocusIn 이 A 의 focus_out 보다 먼저 온다(notify 선행).
        let f = recv(factsOnFocusIn(createSurroundingFacts(), T0));
        eq('창 전환: 이전 창 필드 포커스가 오래됐으면 재송신 안 함',
            decideResend(f, ctx({ nowMs: T0 + 5_000 })), { resend: false, reason: 'focus_old' });
        // A 의 focus_out 이후에는 포커스 자체가 없다
        const out = factsOnFocusOut(f);
        eq('focus_out 뒤 포커스 시각 없음 → nofocus 아니면 focus_old/nocache', out.fieldFocusAtMs, null);
        eq('focus_out 뒤 재송신 안 함(캐시 비움)', decideResend(out, ctx()).resend, false);
        // 캐시가 남아 있어도 focus_out 뒤 focus 시각이 null 이면 막힌다(방어)
        const odd = { ...f, fieldFocusAtMs: null };
        eq('필드 포커스 시각이 null → focus_old', decideResend(odd, ctx()).reason, 'focus_old');
        // 새 창 B: focus_in(B) 직후 창 FocusIn → 허용
        let b = recv(factsOnFocusIn(factsOnFocusOut(f), T0 + 6_000), { nowMs: T0 + 6_005 });
        eq('새 창 focus_in 직후 창 FocusIn → 재송신', decideResend(b, ctx({ nowMs: T0 + 6_050 })).resend, true);
    }
    // (b) 엔진으로 간 키·확장 자신의 커밋은 재송신을 막는다 (확정 대조는 막지 않는다)
    {
        let f = recv(factsOnFocusIn(createSurroundingFacts()));
        const k = factsOnEngineKey(f);
        eq('엔진 키 뒤 → key_sent', decideResend(k, ctx()), { resend: false, reason: 'key_sent' });
        const hz = factsOnEngineKey(factsOnSurrounding(factsOnFocusIn(createSurroundingFacts()),
            { text: '가나다 국어교육을 한다', cursor: 6, anchor: 6 }, { hasFocus: true, sensitive: false, nowMs: T0 }));
        eq('엔진 키는 확정 대조를 막지 않는다(한자키 자체가 이 경로)',
            [hz.cache.keySent, verifyReplaceExpectation(hz.cache, 2, 2, '국어교육').ok], [true, true]);
        eq('캐시 없을 때 엔진 키 기록은 무시', factsOnEngineKey(factsOnFocusIn(createSurroundingFacts())).cache, null);
        eq('새 스냅샷이 오면 key_sent 해제', decideResend(recv(k, { nowMs: T0 + 20 }), ctx({ nowMs: T0 + 30 })).resend, true);
    }
    // 확정 대조: dirty 이면 드롭 (P3 검증 반영)
    {
        const f = recv(factsOnFocusIn(createSurroundingFacts()));
        const c = { text: '가나다 국어교육을 한다', cursor: 6, anchor: 6, gen: 1, atMs: 0, dirty: false, keySent: false };
        eq('dirty 아님 → 일치', verifyReplaceExpectation(c, 2, 2, '국어교육').ok, true);
        eq('dirty → verify_mismatch', verifyReplaceExpectation({ ...c, dirty: true }, 2, 2, '국어교육').reason, 'verify_mismatch');
        eq('KEY 힌트 뒤 캐시는 대조 불가', verifyReplaceExpectation(factsOnHintSent(f, 1).cache, 0, 0, 'x').ok, false);
        eq('확장 커밋 뒤 캐시는 대조 불가', verifyReplaceExpectation(factsOnDocumentChanged(f).cache, 2, 2, '국어교육').ok, false);
        eq('keySent 만으로는 대조 가능(키 하나가 한자키)', verifyReplaceExpectation({ ...c, keySent: true }, 2, 2, '국어교육').ok, true);
    }
    // (3) 목적이 focus_in 뒤 비밀번호로 바뀌면 캐시를 비운다
    {
        let f = recv(factsOnFocusIn(createSurroundingFacts()));
        f = factsOnSensitive(f);
        eq('늦게 온 비밀번호 purpose → 캐시 비움', f.cache, null);
        eq('캐시 비운 뒤 재송신·대조 불가', [decideResend(f, ctx()).reason, verifyReplaceExpectation(f.cache, 0, 0, 'x').ok], ['nocache', false]);
        const e = factsOnFocusIn(createSurroundingFacts());
        eq('캐시 없을 때 factsOnSensitive 는 그대로', factsOnSensitive(e), e);
    }

    // 확정 대조
    {
        const c = { text: '가나다 국어교육을 한다', cursor: 6, anchor: 6, gen: 1, atMs: 0, dirty: false };
        // 코드포인트: 가0 나1 다2 ' '3 국4 어5 | 교6 육7 을8
        eq('대조: a=2,b=2 → 국어교육 일치', verifyReplaceExpectation(c, 2, 2, '국어교육'),
            { ok: true, reason: 'ok', cacheLen: 12, expectLen: 4 });
        eq('대조: 내용이 다르면 verify_mismatch', verifyReplaceExpectation(c, 2, 2, '국어교욱').reason, 'verify_mismatch');
        eq('대조: 길이(a+b)가 expect 와 다르면 불일치', verifyReplaceExpectation(c, 2, 1, '국어교육').ok, false);
        eq('대조: 범위가 앞으로 벗어나면 불일치', verifyReplaceExpectation(c, 7, 0, '가나다 국어교').ok, false);
        eq('대조: 범위가 뒤로 벗어나면 불일치', verifyReplaceExpectation(c, 0, 99, '교육을 한다').ok, false);
        eq('대조: 캐시 없음 → fail-closed', verifyReplaceExpectation(null, 2, 2, '국어교육'),
            { ok: false, reason: 'verify_mismatch', cacheLen: 0, expectLen: 4 });
        eq('대조: expect 가 비면 불일치', verifyReplaceExpectation(c, 0, 0, '').ok, false);
        eq('대조: expect 가 문자열이 아니면 불일치', verifyReplaceExpectation(c, 2, 2, undefined).ok, false);
        eq('대조: 음수·소수 a,b 는 불일치', [verifyReplaceExpectation(c, -1, 5, '어교육').ok,
            verifyReplaceExpectation(c, 1.5, 2.5, '국어교육').ok], [false, false]);
        // 서로게이트 쌍(BMP 밖) — 코드포인트 단위로 센다
        const e = { text: 'a𠮷b', cursor: 2, anchor: 2, gen: 1, atMs: 0, dirty: false };   // a0 𠮷1 b2
        eq('대조: BMP 밖 문자도 코드포인트 1자', verifyReplaceExpectation(e, 1, 1, '𠮷b').ok, true);
        eq('대조: UTF-16 길이로 세면 안 된다', verifyReplaceExpectation(e, 1, 1, '𠮷b').expectLen, 2);
        // 대조는 캐시를 바꾸지 않는다
        eq('대조 후 캐시 불변', [c.text, c.cursor, c.dirty], ['가나다 국어교육을 한다', 6, false]);
    }
}

// ── surrounding_facts 배선 정적 검사 (P3-1~P3-3, 탐침) ────────────────────────
eq('surrounding_facts.js 는 import 가 없다(data: URL 로 읽는 순수 모듈)', /^\s*import\s/m.test(read('surrounding_facts.js')), false);
eq('surrounding_facts.js 는 gi 를 쓰지 않는다', /gi:\/\//.test(read('surrounding_facts.js')), false);
{
    const body = bodyOf(imSrc, 'vfunc_focus_in(_focus) {');
    eq('vfunc_focus_in 본문을 찾음', body !== null, true);
    eq('vfunc_focus_in: 세대 증가(factsOnFocusIn)', body !== null && body.includes('factsOnFocusIn('), true);
    eq('vfunc_focus_in: FIELD_FOCUS 를 비동기 힌트로 송신',
        body !== null && body.includes('surroundingHintAsync(HINT_FIELD_FOCUS, 0, 0)') && !/\.surroundingHint\(/.test(body), true);
    eq('vfunc_focus_in: 힌트 송신 뒤 재송신 호출',
        body !== null && body.indexOf('surroundingHintAsync(HINT_FIELD_FOCUS') < body.indexOf("resendSurroundingFacts('field_focus')"), true);
    eq('vfunc_focus_in: 중간 return 없음(TTL 폐기가 FIELD_FOCUS 를 삼키지 않는다)', body !== null && /\breturn\b/.test(body), false);
}
{
    const body = bodyOf(imSrc, 'resendSurroundingFacts(trigger) {');
    eq('resendSurroundingFacts 본문을 찾음', body !== null, true);
    eq('재송신: decideResend 판정 + 비동기 송신, 동기 setSurroundingText 금지',
        body !== null && body.includes('decideResend(') && body.includes('setSurroundingTextAsync(') &&
        !/\.setSurroundingText\(/.test(body), true);
    eq('재송신: 비밀번호 판정에 isSensitivePurpose 사용', body !== null && body.includes('isSensitivePurpose()'), true);
    eq('재송신 로그는 길이만', body !== null && /surrounding 재송신: trigger=\$\{trigger\}, len=\$\{Array\.from\(c\.text\)\.length\}/.test(body), true);
}
eq('focus_in 시각을 facts 에 기록(factsOnFocusIn(…, 시각))', /factsOnFocusIn\(this\._facts, GLib\.get_monotonic_time\(\) \/ 1000\)/.test(imSrc), true);
eq('엔진으로 가는 키가 factsOnEngineKey 를 부른다(KEY_PRESS 경로, 핸들러 호출 앞)',
    /factsOnEngineKey\(this\._facts\);[\s\S]*?if \(this\._keyHandler\) \{/.test(imSrc), true);
eq('expectSelfBackspaces 가 문서 변경으로 기록', /expectSelfBackspaces\(count\) \{[\s\S]*?_noteDocumentChanged\(\)/.test(imSrc), true);
eq('_applyContentPurpose 가 비밀번호 purpose 에서 캐시를 비운다',
    /_applyContentPurpose\(\) \{[\s\S]*?isSensitivePurpose\(\)\) this\._facts = factsOnSensitive\(this\._facts\)/.test(imSrc), true);
// 엔진이 소비하지 않아 앱으로 가는 키는 캐시를 dirty 로 한다 (소비된 키는 아니다)
{
    eq('noteKeyPassedToApp 이 _noteDocumentChanged 를 부른다', /noteKeyPassedToApp\(\) \{\s*this\._noteDocumentChanged\(\);/.test(imSrc), true);
    const hot = bodyOf(khSrc, '_handleVfuncKey(keyval, keycode, state, event) {');
    const queued = bodyOf(khSrc, '_processQueuedEntry(entry) {');
    eq('_processQueuedEntry 본문을 찾음', queued !== null, true);
    if (queued) {
        eq('큐 처리: 미소비 키(consumed=false)에서 dirty', /if \(!consumed\) this\._inputMethod\.noteKeyPassedToApp\(\);\s*this\._inputMethod\.notify_key_event\(entry\.event, consumed\)/.test(queued), true);
        eq('큐 처리: !result 폴백에서 dirty', /if \(!result\) \{\s*this\._inputMethod\.noteKeyPassedToApp\(\);/.test(queued), true);
        eq('큐 처리: 연결 끊김 폴백에서 dirty', /!this\._dbusIME\.isConnected\) \{\s*this\._inputMethod\.noteKeyPassedToApp\(\);/.test(queued), true);
        eq('큐 처리: 소비 키 경로에는 무조건 dirty 가 없다', (queued.match(/noteKeyPassedToApp/g) ?? []).length, 3);
    }
    if (hot) {
        eq('핫패스: !result 에서 dirty 뒤 return false', /if \(!result\) \{[^}]*?noteKeyPassedToApp\(\);\s*this\._drainKeyQueue\(\);\s*return false;/.test(hot), true);
        eq('핫패스: 소비·커밋 없는 키(return false)에서 dirty', /noteKeyPassedToApp\(\);\s*this\._drainKeyQueue\(\);\s*return false;\s*\}\s*\/\/ 커밋이 딸린/.test(hot), true);
        eq('핫패스: 커밋이 딸린 미소비 키에서만 dirty', /if \(!consumed\) this\._inputMethod\.noteKeyPassedToApp\(\);\s*this\._inputMethod\.notify_key_event\(event, consumed\)/.test(hot), true);
    }
}
eq('focus_out 이 필드 상태를 비운다(factsOnFocusOut)',
    /vfunc_focus_out\(\) \{[\s\S]*?this\._facts = factsOnFocusOut\(this\._facts\);/.test(imSrc), true);
eq('set_surrounding 이 필드 세대 캐시를 갱신(비밀번호 칸은 보관 안 함)',
    /vfunc_set_surrounding\([^)]*\) \{[\s\S]*?factsOnSurrounding\([\s\S]*?sensitive: this\.isSensitivePurpose\(\)/.test(imSrc), true);
eq('_sendHint 가 힌트 송신을 재송신 판정에 알린다(noteHintSent)', khSrc.includes('noteHintSent?.(kind)'), true);
eq('창 FocusIn 직후 재송신(extension.js)',
    /this\._dbusIME\.focusIn\(windowId\);[\s\S]*?resendSurroundingFacts\('win_focus_in'\)/.test(extSrc), true);
{
    const body = bodyOf(extSrc, 'onReplaceSurrounding:');
    eq('onReplaceSurrounding: 캐시 대조(verifyReplaceExpectation) 후 실행',
        body !== null && body.includes('verifyReplaceExpectation(') &&
        body.indexOf('verifyReplaceExpectation(') < body.indexOf('replaceSurrounding('), true);
    eq('onReplaceSurrounding: expect 를 무시하지 않는다(_expectText 아님)', body !== null && !/\b_expectText\b/.test(body), true);
    eq('onReplaceSurrounding: 드롭 로그는 reason 코드와 길이만',
        body !== null && body.includes('reason=${v.reason}') && body.includes('expectLen=${v.expectLen}') &&
        !/expectText\}/.test(body), true);
}
// 탐침: UNIM_DEVELOP=1 일 때만, 내용 없이 종류·시각만 (P0-3)
{
    const logSrc = read('logging.js');
    eq('unimProbe 는 develop 모드에서만 동작', /export function unimProbe[\s\S]*?if \(!isDevelopMode\(\)\) return;/.test(logSrc), true);
    eq('탐침 형식: PROBE seq= t= ev=', logSrc.includes('PROBE seq=${_probeSeq} t=${t} ev=${event}'), true);
    eq('captured-event 포인터 탐침(_probePointerEvent)', khSrc.includes('this._probePointerEvent(event)'), true);
    const probeEvents = ['focus_in', 'focus_out', 'set_surrounding', 'field_focus_sent', 'resend'];
    for (const ev of probeEvents) eq(`탐침 사건 ${ev}`, imSrc.includes(`unimProbe('${ev}'`), true);
    for (const ev of ['win_focus_notify', 'win_focus_in_sent']) eq(`탐침 사건 ${ev}`, extSrc.includes(`unimProbe('${ev}'`), true);
    eq('탐침 사건 captured_ptr / hint_sent', khSrc.includes("unimProbe('captured_ptr'") && khSrc.includes("unimProbe('hint_sent'"), true);
    eq('탐침이 문서 내용을 보간하지 않는다',
        /unimProbe\([^;]*\$\{[^}]*(text|Text|preedit|commit)\b[^}]*\}/
            .test((imSrc + extSrc + khSrc).replace(/Array\.from\([^)]*\)\.length/g, 'LEN')), false);
    eq('탐침이 버튼 마스크를 읽는다(global.get_pointer)', imSrc.includes('global.get_pointer()') && khSrc.includes('global.get_pointer()'), true);
}
// 평문 로그 전수 (이 확장의 사용자 텍스트·키 로그는 길이/종류만)
eq('TypeFIX 완료 로그: replacement 평문 없음', extSrc.includes("replacement='${replacement}'"), false);
eq('UserDict 등록 로그: word 평문 없음', extSrc.includes("UserDict 등록: '${word}'"), false);
eq('vfunc_filter 로그: keyval·keycode 원시값 없음', /vfunc_filter: type=[^`]*keyval=\$\{keyval\}/.test(imSrc), false);

console.log(`surrounding-hints: ${pass} 통과, ${fail} 실패`);
process.exit(fail === 0 ? 0 : 1);
