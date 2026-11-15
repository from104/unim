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
        // 유일한 예외(기현님 승인, 대상③ S0a): key_handler.js 의 탐침이 PRIMARY mimetype '개수'만 센다.
        // 아래 전용 검사가 그 한 곳의 형태(.length 만 사용)를 고정한다.
        if (f === 'key_handler.js' && pat === FORBIDDEN[1]) continue;
        eq(`${f}: 금지 패턴 '${pat}' 없음`, text.includes(pat), false);
    }
}
// get_mimetypes 예외의 형태 고정: 정확히 한 곳, `_primaryMimeCount` 안, 결과는 length 로만 쓴다(이름·내용 금지).
{
    const gm = FORBIDDEN[1];
    eq('key_handler.js: mimetype 조회는 정확히 한 곳', khSrc.split(gm).length - 1, 1);
    const body = bodyOf(khSrc, '_primaryMimeCount() {');
    eq('_primaryMimeCount 본문을 찾음', body !== null, true);
    eq('mimetype 조회는 _primaryMimeCount 안에만 있다', body !== null && body.includes(gm), true);
    eq('mimetype 결과는 개수(.length)만 반환, 이름·내용 사용 없음',
        body !== null && /return mimes \? mimes\.length : 0;/.test(body) &&
        !/mimes\[|mimes\.(map|join|forEach|find|includes|slice)/.test(body), true);
    eq('다른 파일은 get_mimetypes 를 쓰지 않는다',
        extFiles.filter((f) => f !== 'key_handler.js' && read(f).includes(gm)), []);
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
// ── 대상③ S0a·S0b: 빈 preedit 왕복 판정(순수 함수) + 배선 정적 검사 ────────────────
{
    const { isLibreOfficeWmClass, cacheStateName, decidePoke, POKE_TIMEOUT_MS } = sf;
    eq('왕복 타임아웃 250ms', POKE_TIMEOUT_MS, 250);
    // LO wm_class 판정 표
    const lo = [
        ['libreoffice-writer', true], ['libreoffice-calc', true], ['libreoffice-startcenter', true],
        ['LibreOffice', true], ['soffice', true], ['soffice.bin', true], ['SOFFICE', true],
        ['firefox', false], ['org.gnome.TextEditor', false], ['gedit', false], ['', false],
        ['my-libreoffice', false], [' libreoffice', false], ['office', false],
        [null, false], [undefined, false], [42, false],
    ];
    for (const [cls, want] of lo) eq(`LO wm_class: ${JSON.stringify(cls)}`, isLibreOfficeWmClass(cls), want);

    // 캐시 상태 이름
    const base = { fieldGen: 3, fieldFocusAtMs: 0, cache: null };
    const cache = (o = {}) => ({ text: 'x', cursor: 0, anchor: 0, gen: 3, atMs: 0, dirty: false, keySent: false, ...o });
    eq('cacheState: 캐시 없음 → none', cacheStateName(base), 'none');
    eq('cacheState: 다른 세대 → none', cacheStateName({ ...base, cache: cache({ gen: 2 }) }), 'none');
    eq('cacheState: dirty', cacheStateName({ ...base, cache: cache({ dirty: true }) }), 'dirty');
    eq('cacheState: keysent', cacheStateName({ ...base, cache: cache({ keySent: true }) }), 'keysent');
    eq('cacheState: dirty 가 keysent 보다 먼저', cacheStateName({ ...base, cache: cache({ dirty: true, keySent: true }) }), 'dirty');
    eq('cacheState: fresh', cacheStateName({ ...base, cache: cache() }), 'fresh');

    // 발동 조건 표
    const ok = { hasFocus: true, preeditEmpty: true, sensitive: false, popupVisible: false,
                 isLibreOffice: true, inFlight: false, cacheState: 'none' };
    eq('왕복: 모두 충족(캐시 none)', decidePoke(ok), { poke: true, reason: 'ok' });
    eq('왕복: 캐시 dirty 면 발동', decidePoke({ ...ok, cacheState: 'dirty' }).poke, true);
    eq('왕복: 캐시 keysent 면 발동', decidePoke({ ...ok, cacheState: 'keysent' }).poke, true);
    eq('왕복: 캐시 fresh 면 불필요', decidePoke({ ...ok, cacheState: 'fresh' }), { poke: false, reason: 'cache_fresh' });
    eq('왕복: 포커스 없음', decidePoke({ ...ok, hasFocus: false }), { poke: false, reason: 'nofocus' });
    eq('왕복: 조합 중(preedit 있음)', decidePoke({ ...ok, preeditEmpty: false }), { poke: false, reason: 'preedit' });
    eq('왕복: 비밀번호·PIN 칸', decidePoke({ ...ok, sensitive: true }), { poke: false, reason: 'sensitive' });
    eq('왕복: 팝업이 보임', decidePoke({ ...ok, popupVisible: true }), { poke: false, reason: 'popup' });
    eq('왕복: LO 가 아님', decidePoke({ ...ok, isLibreOffice: false }), { poke: false, reason: 'notlo' });
    eq('왕복: 이미 진행 중', decidePoke({ ...ok, inFlight: true }), { poke: false, reason: 'inflight' });
    eq('왕복: 여러 조건 동시 실패 시 앞선 이유', decidePoke({ ...ok, hasFocus: false, sensitive: true }).reason, 'nofocus');
}
// ── 대상③ Firefox: 한자키 직전 낡음 표시 + 보고 보류, 왕복 없음 (HANJA_WORD_SPEC §2.4b.6, v1.9) ──
{
    const { isFirefoxWmClass, isLibreOfficeWmClass, decidePoke, decideHanjaInvalidate, FIREFOX_HOLD_MS,
            shouldForwardSurrounding } = sf;
    eq('Firefox 보고 보류 500ms(R7 상한 450ms 보다 길다)', FIREFOX_HOLD_MS, 500);
    const ff = [
        ['firefox', true], ['firefox_firefox', true], ['Firefox', true], ['firefox-esr', true],
        ['org.mozilla.firefox', true], ['org.mozilla.Firefox', true],
        ['my-firefox', false], [' firefox', false], ['org.mozilla.thunderbird', false],
        ['libreoffice-writer', false], ['soffice', false], ['google-chrome', false], ['', false],
        [null, false], [undefined, false], [7, false],
    ];
    for (const [cls, want] of ff) eq(`Firefox wm_class: ${JSON.stringify(cls)}`, isFirefoxWmClass(cls), want);
    eq('LO 판정은 Firefox 를 LO 로 보지 않는다', isLibreOfficeWmClass('firefox_firefox'), false);
    eq('왕복: Firefox 는 대상이 아니다(빈 조합이 선택을 지운다 — 실측)',
        decidePoke({ hasFocus: true, preeditEmpty: true, sensitive: false, popupVisible: false,
                     isLibreOffice: false, isFirefox: true, inFlight: false, cacheState: 'none' }).reason, 'notlo');

    const inv = { hasFocus: true, preeditEmpty: true, sensitive: false, popupVisible: false, isFirefox: true };
    eq('낡음 표시: Firefox·조합 없음 → 표시', decideHanjaInvalidate(inv), true);
    eq('낡음 표시: Firefox 아님 → 안 함', decideHanjaInvalidate({ ...inv, isFirefox: false }), false);
    eq('낡음 표시: 조합 중(대상①) → 안 함', decideHanjaInvalidate({ ...inv, preeditEmpty: false }), false);
    eq('낡음 표시: 포커스 없음 → 안 함', decideHanjaInvalidate({ ...inv, hasFocus: false }), false);
    eq('낡음 표시: 비밀번호 칸 → 안 함', decideHanjaInvalidate({ ...inv, sensitive: true }), false);
    eq('낡음 표시: 팝업 → 안 함', decideHanjaInvalidate({ ...inv, popupVisible: true }), false);

    eq('보고 넘김: 보류 없음', shouldForwardSurrounding({ holdUntilMs: 0, nowMs: 100, pokeAwaitingReply: false }), true);
    eq('보고 넘김: 보류 중 → 안 넘김', shouldForwardSurrounding({ holdUntilMs: 600, nowMs: 100, pokeAwaitingReply: false }), false);
    eq('보고 넘김: 보류 중이어도 (LO) 왕복 응답은 넘김', shouldForwardSurrounding({ holdUntilMs: 600, nowMs: 100, pokeAwaitingReply: true }), true);
    eq('보고 넘김: 보류 기한 지남', shouldForwardSurrounding({ holdUntilMs: 600, nowMs: 600, pokeAwaitingReply: false }), true);

    // 배선
    const bh = bodyOf(khSrc, '_beforeHanjaProcessKey(keyval, before) {') ?? '';
    const iInv = bh.indexOf('shouldInvalidateBeforeHanja()');
    eq('한자키 직전: Firefox 낡음 표시는 KEY 힌트 + 보고 보류',
        /shouldInvalidateBeforeHanja\(\)\) \{\s*this\._sendHint\(HINT_KEY, keyval, 0\);\s*this\._inputMethod\.beginFirefoxHold\(\);/.test(bh), true);
    eq('한자키 직전: PRIMARY 비어 있음(kind 7) 다음, 왕복 판정 전에 낡음 표시',
        bh.indexOf('_maybeSendPrimaryEmpty()') < iInv && iInv < bh.indexOf('pokeSurroundingReport(before)'), true);
    const pk = bodyOf(imSrc, 'pokeSurroundingReport(before) {') ?? '';
    eq('왕복 판정에 Firefox 를 넘기지 않는다(LO 전용)', !pk.includes('isFirefox'), true);
    const nd = bodyOf(imSrc, '_noteDocumentChanged() {') ?? '';
    eq('문서 변경이 보류를 푼다', nd.includes('this._ffHoldUntilMs = 0;'), true);
    const ss = bodyOf(imSrc, 'vfunc_set_surrounding(text, cursor, anchor) {') ?? '';
    eq('보고 넘김은 shouldForwardSurrounding 로 판정', ss.includes('shouldForwardSurrounding(') &&
        /if \(forward && this\._dbusIME\?\.isConnected\)/.test(ss), true);
    eq('보고를 막아도 확장 캐시는 갱신한다(대조용)', ss.indexOf('factsOnSurrounding(') < ss.indexOf('shouldForwardSurrounding('), true);
    eq('포커스 변화가 보류를 푼다', (bodyOf(imSrc, 'vfunc_focus_in(_focus) {') ?? '').includes('this._ffHoldUntilMs = 0;') &&
        (bodyOf(imSrc, 'vfunc_focus_out() {') ?? '').includes('this._ffHoldUntilMs = 0;'), true);
    eq('보류 시작은 FIREFOX_HOLD_MS', (bodyOf(imSrc, 'beginFirefoxHold() {') ?? '').includes('+ FIREFOX_HOLD_MS'), true);
    const inb = bodyOf(imSrc, 'shouldInvalidateBeforeHanja() {') ?? '';
    eq('낡음 표시 판정은 decideHanjaInvalidate', inb.includes('decideHanjaInvalidate('), true);
    eq('Firefox 판정도 셸 모달이면 거짓', (bodyOf(imSrc, 'isFocusWindowFirefox() {') ?? '').includes('Main.modalCount > 0'), true);
    eq('hanja_key 탐침에 ff', bh.includes('ff=${st.ff}'), true);
}
{
    const { pokeCancelledByPreedit, isLibreOfficeWmClass, cacheStateName, decidePoke } = sf;
    // 배선 — develop 모드 게이트, 취소 지점, 소스 정리
    const poke = bodyOf(imSrc, 'pokeSurroundingReport(before) {');
    eq('pokeSurroundingReport 본문을 찾음', poke !== null, true);
    eq('pokeSurroundingReport: develop 게이트 없음(S2 — LO 한정은 decidePoke 가 맡는다)',
        poke !== null && !poke.includes('isUnimDevelop'), true);
    eq('pokeSurroundingReport: 일반 로그는 poke start / poke end why= 뿐',
        poke !== null && poke.includes("unimLog('IME', 'poke start')") &&
        (bodyOf(imSrc, '_endPoke(why) {') ?? '').includes("unimLog('IME', `poke end why=${why}`)") &&
        (bodyOf(imSrc, '_cancelPoke(why) {') ?? '').includes("unimLog('IME', `poke end why=${why}`)"), true);
    eq('pokeSurroundingReport: 발동 판정은 decidePoke', poke !== null && poke.includes('decidePoke('), true);
    eq('pokeSurroundingReport: 시작은 빈 문자열 preedit',
        poke !== null && poke.includes("set_preedit_text('', 0, 0, Clutter.PreeditResetMode.CLEAR)"), true);
    eq('pokeSurroundingReport: 250ms 타임아웃 소스를 건다(POKE_TIMEOUT_MS)',
        poke !== null && poke.includes('GLib.timeout_add(GLib.PRIORITY_DEFAULT, POKE_TIMEOUT_MS'), true);
    const rep = bodyOf(imSrc, '_onPokeReport() {');
    eq('보고 도착 뒤 null 복귀는 별도 주기(GLib.idle_add)에서 — 같은 주기에 보내지 않는다',
        rep !== null && rep.includes('GLib.idle_add(') && !rep.includes('set_preedit_text'), true);
    const end = bodyOf(imSrc, '_endPoke(why) {');
    eq('_endPoke: null 로 되돌린다', end !== null && end.includes('set_preedit_text(null, 0, 0, Clutter.PreeditResetMode.CLEAR)'), true);
    const cancel = bodyOf(imSrc, '_cancelPoke(why) {');
    eq('_cancelPoke: null 을 따로 보내지 않는다(실제 preedit 이 덮어쓴다)', cancel !== null && !cancel.includes('set_preedit_text'), true);
    const clr = bodyOf(imSrc, '_clearPokeSources(poke) {');
    eq('_clearPokeSources: timeout·idle 소스 모두 제거', clr !== null && (clr.match(/GLib\.source_remove\(/g) ?? []).length === 2, true);
    eq('destroyPoke 가 소스를 정리', (bodyOf(imSrc, 'destroyPoke() {') ?? '').includes('_clearPokeSources('), true);
    eq('extension.js: IME 끌 때 destroyPoke 호출', extSrc.includes('this._inputMethod.destroyPoke()'), true);
    eq('extension.js: PopupView 보임 조회를 IM 에 연결', extSrc.includes('setPopupVisibleQuery(() => this._popupView?.isVisible === true)'), true);
    const hooks = [
        ['vfunc_focus_out() {', "_cancelPoke('focus_out')"],
        ['vfunc_reset() {', "_cancelPoke('reset')"],
        ['commitText(text) {', "_cancelPoke('commit')"],
    ];
    for (const [m, call] of hooks) eq(`${m.replace(/\W+$/, '')}: 왕복 취소`, (bodyOf(imSrc, m) ?? '').includes(call), true);
    // updatePreedit: 빈 갱신은 왕복을 취소하지 않고(done 병합 방지), 비어 있지 않은 갱신만 취소한다
    for (const [txt, want] of [['', false], [null, false], [undefined, false], ['ㅎ', true], ['\u200B', true], ['한', true]])
        eq(`pokeCancelledByPreedit(${JSON.stringify(txt)})`, pokeCancelledByPreedit(txt), want);
    const upd = bodyOf(imSrc, 'updatePreedit(text) {') ?? '';
    eq('updatePreedit: 판정은 pokeCancelledByPreedit', upd.includes('pokeCancelledByPreedit('), true);
    eq('updatePreedit: 취소는 _poke 가 있을 때만 + preedit 사유', /if \(this\._poke\) \{[^]*?_cancelPoke\('preedit'\)/.test(upd), true);
    eq('updatePreedit: 빈 갱신은 취소 호출 앞에서 반환(null 미전송)',
        upd.indexOf('return;') > 0 && upd.indexOf('return;') < upd.indexOf("_cancelPoke('preedit')"), true);
    eq('updatePreedit: 빈 갱신 반환이 set_preedit_text 보다 앞', upd.indexOf('return;') < upd.indexOf('set_preedit_text'), true);
    eq('vfunc_set_surrounding: 보고를 데몬에 전달한 뒤 왕복 보고 처리',
        /setSurroundingText\(text \|\| '', cursor \|\| 0, anchor \|\| 0\);\s*\}\s*[^]*?this\._onPokeReport\(\);/.test(bodyOf(imSrc, 'vfunc_set_surrounding(text, cursor, anchor) {') ?? ''), true);
    // 한자키 직전 호출 3곳 + develop 게이트
    eq('한자키 processKey 직전 _beforeHanjaProcessKey 3곳', (khSrc.match(/this\._beforeHanjaProcessKey\(/g) ?? []).length, 3);
    const before = bodyOf(khSrc, '_beforeHanjaProcessKey(keyval, before) {');
    eq('_beforeHanjaProcessKey: 한자키일 때만(develop 게이트 없음 — S2·S5)',
        before !== null && /^\{\s*if \(!this\._hanjaKeysyms\.has\(keyval\)\) return;/.test(before), true);
    eq('_beforeHanjaProcessKey: hanja_key 탐침 후 왕복', before !== null &&
        before.indexOf("unimProbe('hanja_key'") > 0 && before.indexOf("unimProbe('hanja_key'") < before.indexOf('pokeSurroundingReport('), true);
    eq('hint_sent 탐침은 develop 게이트 안에서 버튼 상태를 읽는다',
        /if \(isUnimDevelop\(\)\) unimProbe\('hint_sent', `kind=\$\{kind\} btn=/.test(khSrc), true);
    eq('primary_owner 탐침은 develop 게이트 안', /if \(isUnimDevelop\(\)\) \{[^}]*?unimProbe\('primary_owner'/.test(khSrc), true);
    eq('reset 탐침(vfunc_reset·cursor-jump) 배선', imSrc.includes("unimProbe('reset'") &&
        imSrc.includes('this._probeReset(false)') && imSrc.includes('this._probeReset(true)'), true);
    for (const ev of ['hanja_key', 'primary_owner', 'poke_start', 'poke_report', 'poke_end', 'poke_skip'])
        eq(`탐침 사건 ${ev}`, (imSrc + khSrc).includes(`unimProbe('${ev}'`), true);
    eq('wm_class 이름을 로그에 보간하지 않는다', /unimProbe\([^;]*(wm_class|wmClass)/.test(imSrc + khSrc), false);
    // ── 한자키 직전 캐시 상태 스냅샷 (검증 수정: 한자키 자신이 keySent 를 세우기 전의 상태로 판정) ──
    {
        const { factsOnFocusIn, factsOnSurrounding, factsOnEngineKey, createSurroundingFacts } = sf;
        const surr = (f) => factsOnSurrounding(f, { text: 'x', cursor: 1, anchor: 1 }, { hasFocus: true, sensitive: false, nowMs: 10 });
        // vfunc_filter_key_event 와 같은 순서: 스냅샷(세기 전) → factsOnEngineKey → 키 핸들러(스냅샷 사용)
        const hanjaKey = (f) => ({ snapshot: cacheStateName(f), after: factsOnEngineKey(f) });
        const loCtx = { hasFocus: true, preeditEmpty: true, sensitive: false, popupVisible: false, isLibreOffice: true, inFlight: false };
        let f = factsOnFocusIn(createSurroundingFacts(), 0);
        f = surr(f);
        let h = hanjaKey(f);
        eq('신선한 보고 직후 한자키: 스냅샷 cache=fresh', h.snapshot, 'fresh');
        eq('  (같은 키 뒤 라이브 상태는 keysent — 라이브로 판정하면 안 되는 이유)', cacheStateName(h.after), 'keysent');
        eq('신선한 보고 직후 한자키: poke_skip why=cache_fresh', decidePoke({ ...loCtx, cacheState: h.snapshot }), { poke: false, reason: 'cache_fresh' });
        eq('  (라이브 상태로 판정하면 skip 이 발동하지 못한다)', decidePoke({ ...loCtx, cacheState: cacheStateName(h.after) }).poke, true);
        // 보고 → 화살표 키 → 한자키: keysent
        f = surr(f);
        f = factsOnEngineKey(f);                       // 화살표 키
        h = hanjaKey(f);
        eq('보고 → 화살표 → 한자키: 스냅샷 cache=keysent', h.snapshot, 'keysent');
        eq('보고 → 화살표 → 한자키: 왕복 발동', decidePoke({ ...loCtx, cacheState: h.snapshot }).poke, true);
        // 보고 없음 → none
        h = hanjaKey(factsOnFocusIn(createSurroundingFacts(), 0));
        eq('보고 없이 한자키: 스냅샷 cache=none, 왕복 발동', [h.snapshot, decidePoke({ ...loCtx, cacheState: h.snapshot }).poke], ['none', true]);
    }
    // 스냅샷 배선: vfunc 가 factsOnEngineKey 직전에 develop 게이트 안에서 기록, 핸들러·왕복이 사용, 공유 상태 불변
    {
        const vf = bodyOf(imSrc, 'vfunc_filter_key_event(event) {') ?? '';
        const iSnap = vf.indexOf('this._probeCacheBeforeKey = cacheStateName(this._facts)');
        const iFacts = vf.indexOf('this._facts = factsOnEngineKey(this._facts)');
        eq('vfunc: 키가 세어지기 전 스냅샷 기록(develop 게이트)',
            iSnap > 0 && iSnap < iFacts && /if \(isUnimDevelop\(\)\) this\._probeCacheBeforeKey = /.test(vf), true);
        eq('vfunc: factsOnEngineKey 호출은 그대로 한 번(공유 상태 불변)', (vf.match(/factsOnEngineKey\(/g) ?? []).length, 1);
        eq('probeHanjaKeyState 가 스냅샷을 사용', (bodyOf(imSrc, 'probeHanjaKeyState(before) {') ?? '').includes('_cacheStateForHanjaKey(before)'), true);
        eq('pokeSurroundingReport 가 스냅샷으로 판정', (bodyOf(imSrc, 'pokeSurroundingReport(before) {') ?? '').includes('cacheState = this._cacheStateForHanjaKey(before)'), true);
        eq('큐 항목이 스냅샷을 싣는다(cacheBefore)', khSrc.includes('cacheBefore: this._inputMethod.cacheStateBeforeKey'), true);
        eq('큐 처리는 근사(approx) 표시', khSrc.includes('approx: true'), true);
        eq('hanja_key 탐침이 snap= 출처를 찍는다', khSrc.includes('snap=${st.snap}'), true);
    }
}
// ── 대상③ S4: 앱 창 눌림 판정 ─────────────────────────────────────────────────
{
    const { decideAppPress, buttonFromMask, APP_PRESS_DEDUP_MS, BUTTON1_MASK, BUTTON2_MASK, BUTTON3_MASK } = sf;
    const { HINT_POINTER_APP } = mod;
    eq('버튼 마스크 상수(Clutter 와 같은 값)', [BUTTON1_MASK, BUTTON2_MASK, BUTTON3_MASK], [0x100, 0x200, 0x400]);
    eq('중복 거르기 창은 더블클릭 간격(121ms 실측)보다 짧다', APP_PRESS_DEDUP_MS < 121, true);
    for (const [m, b] of [[0x0, 0], [0x100, 1], [0x200, 2], [0x400, 3], [0x300, 1], [0x600, 2], [0x101, 1], [0x1, 0]])
        eq(`buttonFromMask(0x${m.toString(16)})`, buttonFromMask(m), b);
    const ok = { direct: false, btnMask: 0x100, overWin: true, popup: false, lastMask: 0 };
    eq('앱 눌림: 모두 충족', decideAppPress(ok), { send: true, reason: 'ok' });
    eq('앱 눌림: 실측 — btn=0x100 over_win=1 direct=0', decideAppPress({ ...ok, btnMask: 0x100 }).send, true);
    eq('앱 눌림: 가운데 버튼도 보낸다', decideAppPress({ ...ok, btnMask: 0x200 }).send, true);
    eq('앱 눌림: 보조 버튼도 판정은 통과(kind 8 은 낡음만 알린다)', decideAppPress({ ...ok, btnMask: 0x400 }).send, true);
    eq('앱 눌림: 직접 호출 제외', decideAppPress({ ...ok, direct: true }), { send: false, reason: 'direct' });
    eq('앱 눌림: 버튼 없음(키보드 유래 reset)', decideAppPress({ ...ok, btnMask: 0 }), { send: false, reason: 'nobtn' });
    eq('앱 눌림: 수정자 비트만 있고 버튼 없음', decideAppPress({ ...ok, btnMask: 0x1 }).reason, 'nobtn');
    eq('앱 눌림: 앱 창 밖(셸 UI 는 captured 가 센다)', decideAppPress({ ...ok, overWin: false }), { send: false, reason: 'notwin' });
    eq('앱 눌림: 팝업 클릭 제외', decideAppPress({ ...ok, popup: true }), { send: false, reason: 'popup' });
    eq('앱 눌림: 같은 눌림의 중복 reset', decideAppPress({ ...ok, lastMask: 0x100 }), { send: false, reason: 'dup' });
    eq('앱 눌림: 다른 버튼이면 새 눌림', decideAppPress({ ...ok, btnMask: 0x200, lastMask: 0x100 }).send, true);
    eq('앱 눌림: 창이 지나 lastMask=0 이면(더블클릭 둘째) 새 눌림', decideAppPress({ ...ok, lastMask: 0 }).send, true);
    eq('앱 눌림: 여러 조건 동시 실패 시 앞선 이유', decideAppPress({ ...ok, direct: true, btnMask: 0 }).reason, 'direct');

    // 시간 계산 포함 판정 (shouldReportAppPress)
    const { shouldReportAppPress, FACT_HINT_POINTER_APP, createSurroundingFacts, factsOnSurrounding, factsOnHintSent } = sf;
    const sk = { ...ok, nowMs: 1000, lastMs: -Infinity, lastMask: 0 };
    delete sk.lastMask; sk.lastMask = 0;
    eq('shouldReport: 첫 눌림', shouldReportAppPress(sk), { send: true, reason: 'ok' });
    eq('shouldReport: 같은 눌림 20ms 뒤 중복 reset', shouldReportAppPress({ ...sk, lastMs: 980, lastMask: 0x100 }).reason, 'dup');
    eq('shouldReport: 39ms 는 중복', shouldReportAppPress({ ...sk, lastMs: 961, lastMask: 0x100 }).reason, 'dup');
    eq('shouldReport: 40ms 는 새 눌림', shouldReportAppPress({ ...sk, lastMs: 960, lastMask: 0x100 }).send, true);
    eq('shouldReport: 더블클릭 둘째(121ms) 는 새 눌림', shouldReportAppPress({ ...sk, lastMs: 879, lastMask: 0x100 }).send, true);
    eq('shouldReport: 다른 버튼은 창 안이라도 새 눌림', shouldReportAppPress({ ...sk, btnMask: 0x200, lastMs: 990, lastMask: 0x100 }).send, true);
    eq('shouldReport: 앱 창 밖·팝업·직접은 시간과 무관하게 제외',
        [{ overWin: false }, { popup: true }, { direct: true }].map((o) => shouldReportAppPress({ ...sk, ...o }).send), [false, false, false]);

    // kind 8: 분류하지 않고, 스냅샷은 낡게 만든다
    eq('앱 눌림 kind 는 8, pointer_hints 와 같다', [HINT_POINTER_APP, FACT_HINT_POINTER_APP], [8, 8]);
    eq('앱 눌림 힌트는 스냅샷을 낡게 만든다(SUPERSEDING)', (() => {
        let f = createSurroundingFacts();
        f = factsOnSurrounding(f, { text: 'ab', cursor: 1, anchor: 1 }, { hasFocus: true, sensitive: false, nowMs: 0 });
        return factsOnHintSent(f, FACT_HINT_POINTER_APP).cache.dirty;
    })(), true);

    // 배선
    const np = bodyOf(imSrc, '_noteAppPress() {') ?? '';
    eq('_noteAppPress: 판정은 shouldReportAppPress, direct=false', np.includes('shouldReportAppPress(') && np.includes('direct: false'), true);
    eq('_noteAppPress: 셸 모달 중에는 앱 창 안으로 보지 않는다', np.includes('Main.modalCount === 0'), true);
    eq('_noteAppPress: 분류 정보를 넘기지 않는다', np.includes('this._appPressHandler()'), true);
    eq('_noteAppPress: 팝업은 reset 핸들러가 닫기 전에 묻는다', (bodyOf(imSrc, 'vfunc_reset() {') ?? '').indexOf('_noteAppPress()') <
        (bodyOf(imSrc, 'vfunc_reset() {') ?? '').indexOf('this._resetHandler'), true);
    eq('cursor-jump 직접 경로는 앱 눌림을 보내지 않는다', !(imSrc.slice(imSrc.indexOf('외부 cursor 점프 감지'), imSrc.indexOf('외부 cursor 점프 감지') + 1200)).includes('_noteAppPress'), true);
    const ra = bodyOf(khSrc, '_reportAppPress() {') ?? '';
    eq('_reportAppPress: kind 8 을 한 번 보낸다', ra.includes('this._sendHint(HINT_POINTER_APP)') && (ra.match(/_sendHint\(/g) ?? []).length === 1, true);
    eq('_reportAppPress: classifyPointer·_pointerState 를 건드리지 않는다', !ra.includes('classifyPointer') && !ra.includes('_pointerState'), true);
    eq('KeyHandler 가 앱 눌림 핸들러를 IM 에 연결', khSrc.includes('this._inputMethod.setAppPressHandler('), true);
}
// ── 대상③ S5: PRIMARY 비어 있음 판정 ───────────────────────────────────────────
{
    const { decidePrimaryEmpty, PRIMARY_EMPTY_MIN_MS, FACT_HINT_KEY, createSurroundingFacts, factsOnSurrounding, factsOnHintSent } = sf;
    const { HINT_PRIMARY_EMPTY } = mod;
    eq('PRIMARY_EMPTY 는 7', HINT_PRIMARY_EMPTY, 7);
    eq('최소 경과 100ms', PRIMARY_EMPTY_MIN_MS, 100);
    const ok = { mimes: 0, sinceOwnerMs: 840, ownerFocusWin: 7, focusWin: 7 };
    eq('PRIMARY 비어 있음: 모두 충족', decidePrimaryEmpty(ok), { send: true, reason: 'ok' });
    // 실측 사례 (2026-11-13)
    eq('실측: gedit·LO 선택 해제 클릭 뒤(mimes 0, since 840)', decidePrimaryEmpty({ ...ok, sinceOwnerMs: 840 }).send, true);
    eq('실측: TextEditor owner null 직후 mimes=0 claim 뒤 840ms', decidePrimaryEmpty({ ...ok, mimes: 0, sinceOwnerMs: 840 }).send, true);
    for (const n of [2, 10, 13]) eq(`실측: 드래그·더블클릭 선택 뒤 mimes=${n}`, decidePrimaryEmpty({ ...ok, mimes: n }), { send: false, reason: 'mimes' });
    eq('실측: 드래그 중 ~50ms 교대(null/claim)', decidePrimaryEmpty({ ...ok, sinceOwnerMs: 50 }), { send: false, reason: 'recent' });
    eq('경계: 99ms 는 아직', decidePrimaryEmpty({ ...ok, sinceOwnerMs: 99 }).reason, 'recent');
    eq('경계: 100ms 는 허용', decidePrimaryEmpty({ ...ok, sinceOwnerMs: 100 }).send, true);
    eq('mimetype 조회 실패(-1)는 비어 있음이 아니다', decidePrimaryEmpty({ ...ok, mimes: -1 }), { send: false, reason: 'mimes' });
    eq('mimetype 1개도 선택으로 본다', decidePrimaryEmpty({ ...ok, mimes: 1 }).send, false);
    eq('owner-changed 를 본 적 없음(-1)', decidePrimaryEmpty({ ...ok, sinceOwnerMs: -1 }), { send: false, reason: 'noowner' });
    eq('포커스 창을 모름', decidePrimaryEmpty({ ...ok, focusWin: null }), { send: false, reason: 'nowin' });
    eq('owner-changed 가 다른 창이 포커스를 가진 동안 일어남', decidePrimaryEmpty({ ...ok, ownerFocusWin: 3 }), { send: false, reason: 'otherwin' });
    eq('owner-changed 때 포커스 창을 몰랐음(null)', decidePrimaryEmpty({ ...ok, ownerFocusWin: null }).send, false);
    eq('민감 목적(비밀번호)에서는 보내지 않는다', decidePrimaryEmpty({ ...ok, sensitive: true }), { send: false, reason: 'sensitive' });
    eq('팝업 표시 중에는 보내지 않는다', decidePrimaryEmpty({ ...ok, popup: true }), { send: false, reason: 'popup' });
    eq('여러 조건 동시 실패 시 앞선 이유', decidePrimaryEmpty({ ...ok, sensitive: true, mimes: 5 }).reason, 'sensitive');
    eq('SUPERSEDING 이 아니다: PRIMARY_EMPTY 힌트는 캐시를 낡게 만들지 않는다', (() => {
        let f = createSurroundingFacts();
        f = factsOnSurrounding(f, { text: 'ab', cursor: 1, anchor: 1 }, { hasFocus: true, sensitive: false, nowMs: 0 });
        return factsOnHintSent(f, HINT_PRIMARY_EMPTY).cache.dirty;
    })(), false);
    eq('KEY 힌트는 여전히 캐시를 낡게 만든다(대조군)', (() => {
        let f = createSurroundingFacts();
        f = factsOnSurrounding(f, { text: 'ab', cursor: 1, anchor: 1 }, { hasFocus: true, sensitive: false, nowMs: 0 });
        return factsOnHintSent(f, FACT_HINT_KEY).cache.dirty;
    })(), true);

    // 배선
    const mp = bodyOf(khSrc, '_maybeSendPrimaryEmpty() {') ?? '';
    eq('_maybeSendPrimaryEmpty: 판정은 decidePrimaryEmpty, 보내는 건 HINT_PRIMARY_EMPTY', mp.includes('decidePrimaryEmpty(') && mp.includes('_sendHint(HINT_PRIMARY_EMPTY)'), true);
    eq('_maybeSendPrimaryEmpty: 민감 목적·팝업을 넘긴다', mp.includes('isSensitivePurpose()') && mp.includes('isPopupShown()'), true);
    const bh = bodyOf(khSrc, '_beforeHanjaProcessKey(keyval, before) {') ?? '';
    eq('_beforeHanjaProcessKey: 한자키만, PRIMARY_EMPTY 가 poke 보다 먼저',
        bh.includes('_hanjaKeysyms.has(keyval)') && bh.indexOf('_maybeSendPrimaryEmpty()') > 0 &&
        bh.indexOf('_maybeSendPrimaryEmpty()') < bh.indexOf('pokeSurroundingReport('), true);
    eq('_beforeHanjaProcessKey: poke 는 develop 게이트 밖', !/if \(isUnimDevelop\(\)\) \{[^}]*pokeSurroundingReport/.test(bh), true);
    eq('한자키 processKey 직전 세 곳이 모두 _beforeHanjaProcessKey 를 부른다',
        (khSrc.match(/this\._beforeHanjaProcessKey\(/g) ?? []).length, 3);
    const wo = bodyOf(khSrc, '_watchPrimaryOwner() {') ?? '';
    eq('_watchPrimaryOwner: 시각·포커스 창 id 를 모든 모드에서 기록', wo.includes('this._primaryOwnerAtMs =') && wo.includes('this._primaryOwnerFocusWin = this._focusWinId()'), true);
    const mc = bodyOf(khSrc, '_primaryMimeCount() {') ?? '';
    eq('PRIMARY 는 개수만 읽는다(get_mimetypes → length, 내용 읽기 API 없음)', mc.includes('.get_mimetypes(') && mc.includes('mimes.length') &&
        !/get_mimetype_content|transfer_async|\.get_content\(|read_async/.test(khSrc), true);
    eq('주석: 개수만 조회, 내용·이름은 읽지 않는다', khSrc.includes('mimetype **개수만** 조회하고 내용·이름은 읽지 않는다'), true);
    eq('SUPERSEDING_HINTS 에 PRIMARY_EMPTY(7) 없음', /SUPERSEDING_HINTS = new Set\(\[[^\]]*\]\)/.exec(read('surrounding_facts.js'))[0].includes('7'), false);
}
// 평문 로그 전수 (이 확장의 사용자 텍스트·키 로그는 길이/종류만)
eq('TypeFIX 완료 로그: replacement 평문 없음', extSrc.includes("replacement='${replacement}'"), false);
eq('UserDict 등록 로그: word 평문 없음', extSrc.includes("UserDict 등록: '${word}'"), false);
eq('vfunc_filter 로그: keyval·keycode 원시값 없음', /vfunc_filter: type=[^`]*keyval=\$\{keyval\}/.test(imSrc), false);

console.log(`surrounding-hints: ${pass} 통과, ${fail} 실패`);
process.exit(fail === 0 ? 0 : 1);
