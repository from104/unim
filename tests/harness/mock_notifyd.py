#!/usr/bin/env python3
"""가짜 org.freedesktop.Notifications 서버 — L3 하네스용 (NOTIFY_SPEC §6 P2)

세션 버스에 `org.freedesktop.Notifications` 이름을 잡고, 받은 `Notify` 호출을
JSONL 로 남긴다. L3(Xvfb)에는 셸·알림 데몬이 없고 데몬이 fdo 로 직접 호출하는
경로(D3)만 시험하므로 이 20여 줄이면 충분하다. 하네스(harness.py)가 로그를 읽어
호출 횟수·body·hints 를 단언한다.

  UNIM_MOCK_NOTIFY_LOG=/path/notify.jsonl  mock_notifyd.py

의존: python3-gi(Gio) 만. 없으면 비정상 종료 코드 2 로 끝나고, 하네스는 알림 단언을
건너뛴다(경고 출력) — 코드 회귀가 아닌 환경 부족이다.
표준 출력은 쓰지 않는다(dbus-run-session 파이프 행 방지, functional-test.sh 주석 참조).
"""

import json
import os
import sys
import time

try:
    import gi
    gi.require_version("Gio", "2.0")
    from gi.repository import Gio, GLib
except Exception:           # python3-gi 부재
    sys.exit(2)

LOG = os.environ.get("UNIM_MOCK_NOTIFY_LOG", "")
if not LOG:
    sys.exit(2)

XML = """
<node>
  <interface name="org.freedesktop.Notifications">
    <method name="GetCapabilities"><arg direction="out" type="as"/></method>
    <method name="Notify">
      <arg direction="in" type="s"/><arg direction="in" type="u"/>
      <arg direction="in" type="s"/><arg direction="in" type="s"/>
      <arg direction="in" type="s"/><arg direction="in" type="as"/>
      <arg direction="in" type="a{sv}"/><arg direction="in" type="i"/>
      <arg direction="out" type="u"/>
    </method>
    <method name="CloseNotification"><arg direction="in" type="u"/></method>
    <method name="GetServerInformation">
      <arg direction="out" type="s"/><arg direction="out" type="s"/>
      <arg direction="out" type="s"/><arg direction="out" type="s"/>
    </method>
  </interface>
</node>
"""

_next_id = [0]


def _append(rec: dict) -> None:
    with open(LOG, "a", encoding="utf-8") as f:
        f.write(json.dumps(rec, ensure_ascii=False, default=str) + "\n")
        f.flush()
        os.fsync(f.fileno())


def on_call(conn, sender, path, iface, method, params, inv):
    if method == "Notify":
        (app_name, replaces_id, app_icon, summary, body,
         actions, hints, expire) = params.unpack()
        if replaces_id:
            nid = replaces_id
        else:
            _next_id[0] += 1
            nid = _next_id[0]
        _append({"t": time.time(), "id": nid, "app_name": app_name,
                 "replaces_id": replaces_id, "app_icon": app_icon,
                 "summary": summary, "body": body, "actions": list(actions),
                 "hints": hints, "expire_timeout": expire})
        inv.return_value(GLib.Variant("(u)", (nid,)))
    elif method == "GetCapabilities":
        inv.return_value(GLib.Variant("(as)", (["body", "persistence"],)))
    elif method == "GetServerInformation":
        inv.return_value(GLib.Variant("(ssss)", ("unim-mock", "unim", "0", "1.2")))
    else:
        inv.return_value(None)


node = Gio.DBusNodeInfo.new_for_xml(XML)


def on_bus(conn, name):
    conn.register_object("/org/freedesktop/Notifications",
                         node.interfaces[0], on_call, None, None)


open(LOG, "a").close()      # 하네스가 파일 존재로 준비를 안다
Gio.bus_own_name(Gio.BusType.SESSION, "org.freedesktop.Notifications",
                 Gio.BusNameOwnerFlags.NONE, on_bus, None, lambda *_: sys.exit(3))
GLib.MainLoop().run()
