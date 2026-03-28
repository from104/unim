#!/usr/bin/env python3
"""UNIM IBus 호환 레이어 테스트

Session bus의 org.freedesktop.IBus 서비스에 연결하여
CreateInputContext, ProcessKeyEvent, FocusIn/Out 등을 테스트한다.

Usage:
    python3 tests/test_ibus_compat.py
"""

import sys
import dbus

IBUS_SERVICE = "org.freedesktop.IBus"
IBUS_PATH = "/org/freedesktop/IBus"
IBUS_IFACE = "org.freedesktop.IBus"
IBUS_IC_IFACE = "org.freedesktop.IBus.InputContext"

# IBus 포털
PORTAL_SERVICE = "org.freedesktop.portal.IBus"
PORTAL_PATH = "/org/freedesktop/IBus/Portal"
PORTAL_IFACE = "org.freedesktop.IBus.Portal"

# 한글 keysym
# GDK keysym
KEY_A = 0x061      # 'a'
KEY_R = 0x072      # 'r' (두벌식 ㅎ)
KEY_K = 0x06b      # 'k' (두벌식 ㅏ)
KEY_S = 0x073      # 's' (두벌식 ㄴ)
KEY_HANGUL = 0xff31 # Hangul toggle
KEY_ESC = 0xff1b
KEY_RETURN = 0xff0d
KEY_SPACE = 0x020

# X11 keycode (= evdev + 8) — IBus는 X11 keycode를 사용
XK_A = 38          # evdev 30 + 8
XK_R = 27          # evdev 19 + 8
XK_K = 45          # evdev 37 + 8
XK_S = 39          # evdev 31 + 8
XK_HANGUL = 130    # evdev 122 + 8
XK_ESC = 9         # evdev 1 + 8

PASS = "\033[92m✅ PASS\033[0m"
FAIL = "\033[91m❌ FAIL\033[0m"


def test_result(name, ok, detail=""):
    status = PASS if ok else FAIL
    msg = f"  {status} {name}"
    if detail:
        msg += f" — {detail}"
    print(msg)
    return ok


def main():
    bus = dbus.SessionBus()
    passed = 0
    failed = 0
    total = 0

    print("=" * 60)
    print("UNIM IBus 호환 레이어 테스트")
    print("=" * 60)

    # --- 1. 서비스 존재 확인 ---
    print("\n[1] 서비스 확인")
    total += 1
    try:
        obj = bus.get_object(IBUS_SERVICE, IBUS_PATH)
        ibus = dbus.Interface(obj, IBUS_IFACE)
        if test_result("org.freedesktop.IBus 서비스", True):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("org.freedesktop.IBus 서비스", False, str(e))
        failed += 1
        print("\n서비스가 없습니다. unim-daemon이 실행 중인지 확인하세요.")
        sys.exit(1)

    # --- 2. 포털 서비스 확인 ---
    total += 1
    try:
        portal_obj = bus.get_object(PORTAL_SERVICE, PORTAL_PATH)
        dbus.Interface(portal_obj, PORTAL_IFACE)
        if test_result("org.freedesktop.portal.IBus 포털", True):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("org.freedesktop.portal.IBus 포털", False, str(e))
        failed += 1

    # --- 3. CreateInputContext ---
    print("\n[2] InputContext 생성")
    total += 1
    try:
        ctx_path = ibus.CreateInputContext("test-ibus-compat")
        if test_result("CreateInputContext", True, f"path={ctx_path}"):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("CreateInputContext", False, str(e))
        failed += 1
        print("\nCreateInputContext 실패. 이후 테스트 불가.")
        sys.exit(1)

    # InputContext 프록시
    ic_obj = bus.get_object(IBUS_SERVICE, ctx_path)
    ic = dbus.Interface(ic_obj, IBUS_IC_IFACE)

    # --- 4. FocusIn ---
    print("\n[3] FocusIn/Out")
    total += 1
    try:
        ic.FocusIn()
        if test_result("FocusIn", True):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("FocusIn", False, str(e))
        failed += 1

    # --- 5. SetCapabilities ---
    total += 1
    try:
        ic.SetCapabilities(dbus.UInt32(0x1F))  # PREEDIT|AUX|LOOKUP|FOCUS|PROPERTY
        if test_result("SetCapabilities", True):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("SetCapabilities", False, str(e))
        failed += 1

    # --- 6. SetCursorLocation ---
    total += 1
    try:
        ic.SetCursorLocation(dbus.Int32(100), dbus.Int32(200), dbus.Int32(10), dbus.Int32(20))
        if test_result("SetCursorLocation", True):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("SetCursorLocation", False, str(e))
        failed += 1

    # --- 7. ProcessKeyEvent (영문) ---
    print("\n[4] ProcessKeyEvent")
    total += 1
    try:
        consumed = ic.ProcessKeyEvent(dbus.UInt32(KEY_A), dbus.UInt32(XK_A), dbus.UInt32(0))
        if test_result("ProcessKeyEvent (영문 'a')", True, f"consumed={consumed}"):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("ProcessKeyEvent (영문 'a')", False, str(e))
        failed += 1

    # --- 8. ProcessKeyEvent (한/영 전환) ---
    total += 1
    try:
        consumed = ic.ProcessKeyEvent(dbus.UInt32(KEY_HANGUL), dbus.UInt32(XK_HANGUL), dbus.UInt32(0))
        if test_result("ProcessKeyEvent (한/영 전환)", True, f"consumed={consumed}"):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("ProcessKeyEvent (한/영 전환)", False, str(e))
        failed += 1

    # --- 9. ProcessKeyEvent (한글 ㅎ → 'r' key) ---
    total += 1
    try:
        consumed = ic.ProcessKeyEvent(dbus.UInt32(KEY_R), dbus.UInt32(XK_R), dbus.UInt32(0))
        detail = f"consumed={consumed}"
        if consumed:
            detail += " (한글 조합 시작)"
        if test_result("ProcessKeyEvent (한글 'ㅎ')", consumed, detail):
            passed += 1
        else:
            failed += 1
    except dbus.exceptions.DBusException as e:
        test_result("ProcessKeyEvent (한글 'ㅎ')", False, str(e))
        failed += 1

    # --- 10. ProcessKeyEvent (한글 ㅏ → 'k' key) ---
    total += 1
    try:
        consumed = ic.ProcessKeyEvent(dbus.UInt32(KEY_K), dbus.UInt32(XK_K), dbus.UInt32(0))
        if test_result("ProcessKeyEvent (한글 'ㅏ' → '하')", consumed, f"consumed={consumed}"):
            passed += 1
        else:
            failed += 1
    except dbus.exceptions.DBusException as e:
        test_result("ProcessKeyEvent (한글 'ㅏ' → '하')", False, str(e))
        failed += 1

    # --- 11. ProcessKeyEvent (한글 ㄴ → 's' key → '한' 커밋) ---
    total += 1
    try:
        consumed = ic.ProcessKeyEvent(dbus.UInt32(KEY_S), dbus.UInt32(XK_S), dbus.UInt32(0))
        if test_result("ProcessKeyEvent (한글 'ㄴ' → '한')", consumed, f"consumed={consumed}"):
            passed += 1
        else:
            failed += 1
    except dbus.exceptions.DBusException as e:
        test_result("ProcessKeyEvent (한글 'ㄴ' → '한')", False, str(e))
        failed += 1

    # --- 12. Reset ---
    print("\n[5] Reset/Destroy")
    total += 1
    try:
        ic.Reset()
        if test_result("Reset", True):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("Reset", False, str(e))
        failed += 1

    # --- 13. FocusOut ---
    total += 1
    try:
        ic.FocusOut()
        if test_result("FocusOut", True):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("FocusOut", False, str(e))
        failed += 1

    # --- 14. IsEnabled ---
    total += 1
    try:
        enabled = ic.IsEnabled()
        if test_result("IsEnabled", True, f"enabled={enabled}"):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("IsEnabled", False, str(e))
        failed += 1

    # --- 15. GetEngine ---
    total += 1
    try:
        engine = ic.GetEngine()
        if test_result("GetEngine", True, f"type={type(engine).__name__}"):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("GetEngine", False, str(e))
        failed += 1

    # --- 16. Destroy ---
    total += 1
    try:
        ic.Destroy()
        if test_result("Destroy", True):
            passed += 1
    except dbus.exceptions.DBusException as e:
        test_result("Destroy", False, str(e))
        failed += 1

    # --- 17. Destroy 후 호출 실패 확인 ---
    total += 1
    try:
        ic.ProcessKeyEvent(dbus.UInt32(KEY_A), dbus.UInt32(XK_A), dbus.UInt32(0))
        test_result("Destroy 후 호출 거부", False, "예외 없이 성공 (잔여 객체)")
        failed += 1
    except dbus.exceptions.DBusException:
        if test_result("Destroy 후 호출 거부", True, "UnknownObject 정상"):
            passed += 1

    # --- 18. Portal CreateInputContext ---
    print("\n[6] Portal")
    total += 1
    try:
        portal_obj = bus.get_object(PORTAL_SERVICE, PORTAL_PATH)
        portal = dbus.Interface(portal_obj, PORTAL_IFACE)
        portal_ctx = portal.CreateInputContext("test-portal")
        if test_result("Portal CreateInputContext", True, f"path={portal_ctx}"):
            passed += 1
        # 정리
        portal_ic = dbus.Interface(bus.get_object(PORTAL_SERVICE, portal_ctx), IBUS_IC_IFACE)
        portal_ic.Destroy()
    except dbus.exceptions.DBusException as e:
        test_result("Portal CreateInputContext", False, str(e))
        failed += 1

    # --- 결과 ---
    print("\n" + "=" * 60)
    print(f"결과: {passed}/{total} 통과, {failed} 실패")
    print("=" * 60)

    sys.exit(0 if failed == 0 else 1)


if __name__ == "__main__":
    main()
