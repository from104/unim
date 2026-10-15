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

console.log(`surrounding-hints: ${pass} 통과, ${fail} 실패`);
process.exit(fail === 0 ? 0 : 1);
