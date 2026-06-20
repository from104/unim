---
name: unim-tsf-fix
description: UNIM Korean TSF IME v0.3.0 packaging bug and the registry fix that makes it activatable
metadata: 
  node_type: memory
  type: project
  originSessionId: 33679f01-20bd-4153-9f14-b29aae550c59
---

UNIM v0.3.0 is installed at `C:\Program Files\UNIM\unim_tsf.dll` (x64 only). Its `register-tsf.bat` runs `regsvr32` but the DLL's `DllRegisterServer` is **incomplete** — it does not register TSF Categories or the required LanguageProfile values, so Windows TSF refuses to enumerate it as a keyboard input method.

Symptoms: After `regsvr32`, UNIM does not appear in Settings → Language → Korean → Keyboards "Add" list, and `Set-WinUserLanguageList` silently rejects the TIP.

**Why:** Vendor packaging defect in v0.3.0 — only writes minimal TIP/LanguageProfile keys; misses `Category\Category\{GUID}\{CLSID}` and `Category\Item\{CLSID}\{GUID}` subkeys plus several LanguageProfile values (`SubstituteLayout`, `Enable`, `IconFile`, `IconIndex`, `Display`).

**How to apply:** Required manual fixes (admin):
- TIP CLSID: `{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}`
- Profile GUID: `{B2C3D4E5-F6A7-8901-BCDE-F12345678901}`
1. Add missing values under `HKLM\SOFTWARE\Microsoft\CTF\TIP\{TIP}\LanguageProfile\0x00000412\{PROF}`: `SubstituteLayout`=0x412, `Enable`=1, `IconFile`, `IconIndex`, `Display`
2. Add 8 standard keyboard TIP categories under `HKLM\SOFTWARE\Microsoft\CTF\TIP\{TIP}\Category` (both `Category\{CAT}\{TIP}` and `Item\{TIP}\{CAT}`). Required minimum: `{34745C63-B2F0-4784-8B67-5E12C8701A31}` (GUID_TFCAT_TIP_KEYBOARD). Full set: 046B8C80, 13A016DF, 25504FB4, 34745C63, 364215D9, 49D2F9CE, 49D2F9CF, CCF05DD7.
3. Then `Set-WinUserLanguageList` with `0412:{TIP}{PROF}` added to ko `InputMethodTips` works.

Helper scripts left at `C:\Users\USER\AppData\Local\Temp\unim_fix.bat` and `unim_categories.bat`.

If user reinstalls/upgrades UNIM, these manual fixes may be wiped — re-apply or check if vendor shipped a corrected `DllRegisterServer`.
