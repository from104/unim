#!/usr/bin/env gjs
/*
 * GNOME 확장 API 대조 검사기
 *
 * 확장의 JS 는 컴파일 검사를 받지 않는다. 그래서 GNOME Shell 이 API 를 하나
 * 없애면 빌드도 테스트도 전부 통과한 채 **사용자 기계에서만** 터진다.
 * 2026-08-23 에 정확히 그 일이 났다 — 셸 50 이 `Meta.is_wayland_compositor()`
 * 를 없앴는데, 확장은 "활성화됨" 으로 표시되면서 한 글자도 입력되지 않았다.
 *
 * 이 검사기는 확장 소스에서 GI 네임스페이스 심볼(`Meta.X`, `Clutter.Y`, …)을
 * 모두 뽑아 **지금 이 기계에 깔린** GNOME introspection 에 실재하는지 확인한다.
 * 즉 "이 셸에서 돌 것인가" 를 빌드 시점에 답한다.
 *
 * 한계 — 이 기계의 셸 버전만 검사한다. 지원 범위 전체(45~50)를 덮으려면
 * 각 버전의 기계나 컨테이너에서 돌려야 한다. CI 매트릭스가 그 몫이다.
 *
 * 사용법: scripts/check-gnome-api.js [확장디렉터리]
 * 종료 코드: 0 = 이상 없음, 1 = 없는 심볼 발견, 2 = 검사 자체가 불가
 */

const GLib = imports.gi.GLib;
const Gio  = imports.gi.Gio;

const EXT_DIR = ARGV[0] || 'unim-gnome-extension';

// 확장이 실제로 import 하는 GI 네임스페이스만 검사한다. Adw·Gtk 는 prefs.js
// 전용이라 셸 프로세스가 아닌 별도 프로세스에서 도는데, 그쪽은 일반 GTK 앱과
// 같은 규칙을 따르므로 여기서 같이 본다.
const NAMESPACES = ['Meta', 'Clutter', 'St', 'Shell', 'Cogl', 'Mtk', 'Adw', 'Gtk', 'Pango'];

// GNOME Shell 이 이미 없앴거나 없앨 예정이라, 직접 부르면 안 되는 심볼.
// 여기 오르면 "존재하더라도" 실패로 본다 — 옛 셸에서만 존재하는 API 를
// 무심코 되살리는 걸 막는다. 값은 대체 수단 안내다.
const FORBIDDEN = {
    'Meta.is_wayland_compositor':
        '셸 50 에서 제거됨 — extension.js 의 isWaylandCompositor() 를 쓸 것 ' +
        '(MetaContext.get_wayland_compositor() 우선, 옛 셸로 폴백)',
};

function listJsFiles(dir) {
    const out = [];
    const d = Gio.File.new_for_path(dir);
    const en = d.enumerate_children('standard::name,standard::type',
                                    Gio.FileQueryInfoFlags.NONE, null);
    let info;
    while ((info = en.next_file(null)) !== null) {
        const name = info.get_name();
        const path = GLib.build_filenamev([dir, name]);
        if (info.get_file_type() === Gio.FileType.DIRECTORY) {
            // schemas·locale·icons 안에는 검사할 JS 가 없다.
            if (!['schemas', 'locale', 'icons', 'po'].includes(name))
                out.push(...listJsFiles(path));
        } else if (name.endsWith('.js')) {
            out.push(path);
        }
    }
    return out;
}

function readFile(path) {
    const [ok, bytes] = GLib.file_get_contents(path);
    if (!ok)
        throw new Error(`읽지 못함: ${path}`);
    return new TextDecoder('utf-8').decode(bytes);
}

/**
 * 줄에서 주석을 걷어낸다. 옛 API 를 **설명하는** 주석은 정당하므로 검사
 * 대상이 아니다 — 오늘 없앤 API 를 왜 없앴는지 적어 둔 주석이 내일 경고로
 * 돌아오면, 주석을 안 쓰게 되지 검사기가 나아지지 않는다.
 *
 * 문자열 리터럴 안의 `//` 까지 가려내지는 않는다. 그 오탐은 GI 심볼 모양과
 * 겹칠 때만 나는데, 그런 문자열은 실제로 본 적이 없다.
 */
function stripComment(line) {
    const t = line.trimStart();
    if (t.startsWith('//') || t.startsWith('*') || t.startsWith('/*'))
        return '';
    const i = line.indexOf('//');
    return i === -1 ? line : line.slice(0, i);
}

/**
 * `Namespace.Symbol` 참조를 걷어 { 'Meta.Window': ['파일:줄', …] } 로 돌려준다.
 * 줄 끝에 `api-check: allow` 를 달면 그 줄은 통과한다 — 폴백 안에서 옛 API 의
 * **존재를 확인하는** 코드가 여기 해당한다. 그건 부르는 게 아니라 묻는 것이다.
 */
function collectSymbols(files) {
    const found = new Map();
    const re = new RegExp(`\\b(${NAMESPACES.join('|')})\\.([A-Za-z_][A-Za-z0-9_]*)`, 'g');
    for (const path of files) {
        const lines = readFile(path).split('\n');
        lines.forEach((line, i) => {
            if (line.includes('api-check: allow'))
                return;
            const code = stripComment(line);
            let m;
            re.lastIndex = 0;
            while ((m = re.exec(code)) !== null) {
                const key = `${m[1]}.${m[2]}`;
                if (!found.has(key))
                    found.set(key, []);
                found.get(key).push(`${path}:${i + 1}`);
            }
        });
    }
    return found;
}

function loadNamespace(ns) {
    try {
        return imports.gi[ns];
    } catch (e) {
        return null;
    }
}

// ── 검사 ────────────────────────────────────────────────────────────────────

let files;
try {
    files = listJsFiles(EXT_DIR);
} catch (e) {
    printerr(`확장 디렉터리를 읽지 못했다: ${EXT_DIR} — ${e.message}`);
    imports.system.exit(2);
}
if (files.length === 0) {
    printerr(`${EXT_DIR} 에 JS 파일이 없다`);
    imports.system.exit(2);
}

const symbols = collectSymbols(files);
const loaded = new Map();
const skipped = [];
for (const ns of NAMESPACES) {
    const mod = loadNamespace(ns);
    if (mod)
        loaded.set(ns, mod);
    else if ([...symbols.keys()].some(k => k.startsWith(`${ns}.`)))
        skipped.push(ns);
}

const missing = [];
const forbidden = [];
for (const [key, sites] of symbols) {
    const [ns, sym] = key.split('.');
    if (FORBIDDEN[key]) {
        forbidden.push([key, sites, FORBIDDEN[key]]);
        continue;
    }
    const mod = loaded.get(ns);
    if (!mod)
        continue;                       // 네임스페이스 자체를 못 얻었으면 판단 보류
    // GI 모듈의 프로퍼티 접근은 그 자리에서 타입을 해석하는데, gjs 가 표현하지
    // 못하는 타입이면 조회 자체가 예외를 던진다. 그건 "없다" 가 아니라
    // "판정 못 한다" 이므로 통과시킨다 — 여기서 잡고 싶은 건 삭제된 API 다.
    let present;
    try {
        present = mod[sym] !== undefined;
    } catch (e) {
        continue;
    }
    if (!present)
        missing.push([key, sites]);
}

const shellVersion = (() => {
    try {
        const [, out] = GLib.spawn_command_line_sync('gnome-shell --version');
        return new TextDecoder().decode(out).trim();
    } catch (e) {
        return '알 수 없음';
    }
})();

print(`대조 기준: ${shellVersion}`);
print(`검사한 파일 ${files.length}개 · 심볼 ${symbols.size}종`);

if (skipped.length)
    print(`⚠️  introspection 을 못 얻어 건너뛴 네임스페이스: ${skipped.join(', ')}`);

for (const [key, sites, why] of forbidden) {
    printerr(`❌ 쓰면 안 되는 API: ${key}`);
    printerr(`   ${why}`);
    for (const s of sites)
        printerr(`   ${s}`);
}

for (const [key, sites] of missing) {
    printerr(`❌ 이 셸에 없는 심볼: ${key}`);
    for (const s of sites)
        printerr(`   ${s}`);
}

if (missing.length || forbidden.length) {
    printerr('');
    printerr(`실패 — 없는 심볼 ${missing.length}건, 금지 API ${forbidden.length}건.`);
    printerr('직접 부르지 말고 기능 탐지(feature detection)로 감싸라 —');
    printerr('extension.js 의 isWaylandCompositor() 가 본보기다.');
    imports.system.exit(1);
}

print('✅ 확장이 쓰는 GI 심볼이 이 셸에 모두 있다.');
