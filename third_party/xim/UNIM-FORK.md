# xim 0.5.0 — UNIM 포크

> **이 포크는 일시적이다. 상류가 고쳐지면 지운다.**

`third_party/xim` 은 crates.io 의 `xim 0.5.0` **원본 그대로**에 최소 패치 두 건을
얹은 것이다. 루트 `Cargo.toml` 의 `[patch.crates-io]` 로 물려 있다.

## 상류 복귀 (되돌리는 법)

루트 `Cargo.toml` 에서 이 세 줄을 지우면 끝난다.

```toml
[patch.crates-io]
xim = { path = "third_party/xim" }
```

그다음 `cargo build` 하면 `Cargo.lock` 의 `xim` 항목에 `source = "registry+..."`
가 돌아온다(포크가 물려 있는 동안에는 그 줄이 없다 — 포크 적용 여부를 이걸로
판별한다). `third_party/` 디렉토리도 함께 지우면 흔적이 남지 않는다.

## 왜 포크했나

`xim 0.5.0` 은 **`XIM_PREEDIT_START` 를 동기 요청으로 다루지 않는다.**

XIM 명세상 `XIM_PREEDIT_START` 는 클라이언트가
`XIM_PREEDIT_START_REPLY` 로 답해야 하는 동기 요청이고, 서버는 그 답을 받은 뒤에
`XIM_PREEDIT_DRAW` 를 보내야 한다. 그런데 원본은 응답 처리부가 통째로

```rust
// Ignore start reply
Request::PreeditStartReply { .. } => {}
```

이고, `preedit_draw()` 는 `PreeditStart` 를 보낸 **직후** 곧바로 `PreeditDraw` 를
이어 보낸다. 그래서 ON-THE-SPOT(`PREEDIT_CALLBACKS`) 클라이언트가 핸드셰이크
도중 도착한 `PreeditDraw` 를 버린다.

동기 키 전달(`set_event_mask` 의 두 번째 인자)을 켜면 여기에 더해
`XIM_SYNC_REPLY` 까지 핸드셰이크보다 먼저 나가서 클라이언트가 `BadProtocol` 로
응답한다.

## 델타

`unim-patches/0001-preedit-start-handshake.patch` 에 그대로 들어 있다. 원본
2파일만 건드리며, 모든 변경 지점에 `UNIM patch:` 주석이 달려 있다.

| 파일 | 내용 |
|---|---|
| `src/server/connection.rs` | `InputContext` 에 `preedit_start_pending` · `pending_preedit` · `pending_sync_reply` 추가. `PreeditStartReply` 수신 시 보류분을 흘려보내고, 미뤄 둔 `SyncReply` 를 그때 발사 |
| `src/server.rs` | ① `preedit_draw()` 가 `PreeditStart` 를 보낸 뒤 응답 전까지 `PreeditDraw` 를 보내지 않고 보류 ② `preedit_clear_keep_session()` 신설 — preedit 내용만 비우고 `PreeditDone` 은 보내지 않는다 |

동작은 그대로 하위 호환이다 — `PreeditStart` 를 쓰지 않는 OVER-THE-SPOT
(`PREEDIT_POSITION`) 경로는 이 코드에 닿지 않는다.

`preedit_clear_keep_session()` 이 필요한 이유: ON-THE-SPOT 클라이언트는 한 키를
처리하다 `Commit` 을 만나면 뒤에 온 메시지를 버리므로 조합 종료 시의 "비움" 도
`Commit` 앞에 보내야 하는데, 그때 `PreeditDone` 까지 앞세우면 일부 클라이언트가
세션을 닫는다. 상류 API 에 없는 기능이라 추가했다(순수 추가라 기존 동작에 영향
없음).

## 상류에 낼 때

패치는 그대로 PR 로 낼 수 있게 최소·독립적으로 유지한다. 새 버전이 나오면:

1. 새 원본을 `third_party/.xim-pristine` 에 받고
2. `unim-patches/*.patch` 를 `patch -p1` 로 재적용
3. `make xim-fork-diff` 로 원본 대비 델타가 이 문서의 표와 일치하는지 확인

## 현재 상태 (2026-08-07)

"ON-THE-SPOT commit 직후 preedit 누락" 은 **해결됐다.** 실제 원인은 ON-THE-SPOT
클라이언트가 한 키를 처리하다 `Commit` 을 만나면 그 뒤 메시지를 버리는 것이었고,
`unim-frontends/xim/src/handler.rs` 의 `commit_then_preedit()` 에서 preedit 을
commit 보다 먼저 보내도록 바꿔 고쳤다(`IME_BEHAVIOR.md` §8.1 예외).

동기 이벤트 마스크(`set_event_mask` 두 번째 인자)를 1 로 올리는 길도 검토했으나,
클라이언트가 `SetIcValues` 직후 `BadProtocol` 을 내서 **채택하지 않았다.** 지금
코드는 비동기(0) 그대로다.

검증 장비 — 사람 손 없이 돌아간다:

- `tests/unim-test-xim` — 순수 Xlib ON-THE-SPOT. stdout 에 ms 단위 콜백 로그
- `tests/unim-test-gtk3` — `GDK_BACKEND=x11 GTK_IM_MODULE=xim` 로 띄우면
  Obsidian 과 같은 GTK XIM 경로. 판정은 `import -window` 스크린샷
- `xterm` — OVER-THE-SPOT 회귀 감시. `xterm -e "cat > /tmp/out"` 로 확정 문자열만
  뽑아 비교
- 키 주입은 `xdotool key`(XTEST). XWayland 클라이언트에는 먹는다

## 계측 (2026-08-26, 이슈 C Phase 0)

`src/x11rb.rs` 의 `send_req_impl` · `X11rbServer::handle_xim_protocol` ·
`X11rbClient::handle_xim_protocol` 에 `UNIM_XIM_TRACE` 환경변수 게이트 `eprintln!`
계측을 추가했다(기본 완전 무음, 동작 변경 없음) — 송신 분기(direct-ClientMessage
vs Property)·opcode·atom·PropMode, XIM_ERROR 전문, PREEDIT_DRAW/FORWARD_EVENT/COMMIT
의 property 경로 완주 여부를 opcode 별로 구분해 찍는다. 패치 파일은 만들지 않았다
(동작 불변 로깅이라 PR 대상 델타가 아님). 상류 복귀 시 그대로 버려도 무방.

## 결정 (2026-08-26, 이슈 C 종결) — CALLBACKS 를 광고하지 않는다

Phase 0 계측으로 웨지 메커니즘이 확정됐다: GTK3 im-xim + libX11 은 서버가 보낸
`PreeditStart`/`PreeditDraw` 콜백 요청이 트리거하는 재진입 `XSetICValues` 중첩에서
응답 경합으로 영구 웨지된다. XIM 프로토콜에는 요청-응답 시퀀스 번호가 없다 —
libX11 `imDefIc.c` 의 `_XimSetICValuesCheck` 는 imid/icid/opcode 만으로 매치하고,
그 중첩 안에서 도는 `_XimRead` 의 `_CheckCMEvent`(`imTrX.c:477`) 는 내용을 보지
않는다. 불일치한 응답 패킷은 `imTransR.c:243,302` 에서 `BadProtocol` 로 반사·
파괴되고 클라이언트는 그대로 멈춘다.

서버측 순서 조정으로는 회피되지 않음이 실측으로 반증됐다 — `UNIM_XIM_FIRE_AND_FORGET`
(PreeditStart 응답 대기 없이 즉시 Draw) 와 동기 이벤트 마스크 협상 둘 다 웨지를
없애지 못했다(후자는 오히려 `BadProtocol` 을 유발해 `UNIM-FORK.md` 위쪽 "현재
상태" 절에 이미 기록돼 있다). 유일한 서버측 회피는 **CALLBACKS 스타일을 애초에
협상 목록에 올리지 않는 것**이다.

GTK3 의 `ALLOWED_MASK`(`gtkimcontextxim.c:183-184`)에는 `PREEDIT_POSITION` 이 없어
CALLBACKS 를 빼면 GTK3 는 `PREEDIT_NOTHING` 으로 협상해 온다. GTK 는 협상된
스타일과 무관하게 spot 좌표를 `XSetICValues` 로 계속 보내므로(`gtkimcontextxim.c:
771-796`) 서버가 그 좌표로 자체 `PeWindow` 를 그릴 수 있다 — NOTHING 클라이언트도
preedit 을 잃지 않는다. 4차 실험에서 `ㄹㄹㄹ␣한␣` 전 키 통과·PeWindow 정상·웨지
0으로 실측 검증됐다.

`unim-frontends/xim/src/handler.rs` 의 `input_styles()` 는 이제
`[PREEDIT_NOTHING|STATUS_NOTHING, PREEDIT_POSITION|STATUS_NOTHING]` 두 개만
광고한다. 이 크레이트(`src/server.rs`) 자체는 CALLBACKS 협상 여부를 모른다 —
`preedit_start_pending`/`pending_preedit` 게이트는 CALLBACKS 로 명시 협상된 IC
(예: `tests/unim-test-xim` 처럼 협상 없이 `CreateIc` 로 그 스타일을 직접 요청하는
클라이언트)가 여전히 `PreeditStart`/`PreeditDraw` 콜백 경로를 타는 한 계속
필요하므로 그대로 둔다. 어떤 IC 에 그 경로를 태울지 고르는 스타일 게이트는
호출부인 `handler.rs` 의 `preedit()`·`commit_then_preedit()`·`clear_preedit()` 세 곳의 책임이다.
