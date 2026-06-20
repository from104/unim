# IMM32 ë¯¸ì—°ê²° ì§„ë‹¨ â€” ì‹¤ì¸¡ ì¦ê±° (ìë™ ìˆ˜ì§‘)

## 1. .ime íŒŒì¼ ì¡´ì¬/í¬ê¸° (System32=x64, SysWOW64=x86)
-rwxr-xr-x 1 USER 197609 7653376 Jun 17 10:51 /c/Windows/System32/unim_imm32.ime
-rwxr-xr-x 1 USER 197609 7475712 Jun 17 10:51 /c/Windows/SysWOW64/unim_imm32.ime

## 2. ì„¤ì¹˜ ë””ë ‰í„°ë¦¬
total 22438
-rw-r--r-- 1 USER 197609      542 May 20 02:43 register-tsf.bat
-rwxr-xr-x 1 USER 197609  8722944 Jun 17 10:49 unim_tsf.dll
-rwxr-xr-x 1 USER 197609   350208 Jun 17 10:51 unim-popup-win.exe
-rwxr-xr-x 1 USER 197609 26434560 Jun 17 10:51 unim-tsf-settings.exe
-rw-r--r-- 1 USER 197609      391 May 20 02:43 unregister-tsf.bat

## 3. Keyboard Layouts\E0200412 (IME ë“±ë¡)

HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200412
    Ime File    REG_SZ    unim_imm32.ime
    Layout File    REG_SZ    KBDA1.DLL
    Layout Text    REG_SZ    Korean Input Method (UNIM)
    Layout Display Name    REG_SZ    @%SystemRoot%\system32\unim_imm32.ime,-1
    Layout Id    REG_SZ    0412


## 3b. ëª¨ë“  í•œêµ­ì–´(0412) Keyboard Layouts í‚¤ (MS IME ë¹„êµìš©)
HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\00000412
HKEY_LOCAL_MACHINE\SYSTEM\CurrentControlSet\Control\Keyboard Layouts\E0200412

## 4. ì‚¬ìš©ì ì…ë ¥ê¸° ëª©ë¡ (Preload / Substitutes)

HKEY_CURRENT_USER\Keyboard Layout\Preload
    1    REG_SZ    00000412

--- Substitutes ---


## 5. CTF Assemblies (HKL<->TIP ë§¤í•‘)
¿À·ù: ¿À·ù: ½Ã½ºÅÛÀÌ ÁöÁ¤µÈ ·¹Áö½ºÆ®¸® Å° ¶Ç´Â °ªÀ» Ã£À» ¼ö ¾ø½À´Ï´Ù.

## 6. unim TSF TIP CLSID ë“±ë¡ ì—¬ë¶€

HKEY_LOCAL_MACHINE\SOFTWARE\Classes\CLSID\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}
    (±âº»°ª)    REG_SZ    UNIM Korean IME

HKEY_LOCAL_MACHINE\SOFTWARE\Classes\CLSID\{A1B2C3D4-E5F6-7890-ABCD-EF1234567890}\InProcServer32
    (±âº»°ª)    REG_SZ    C:\Program Files\UNIM\unim_tsf.dll
    ThreadingModel    REG_SZ    Apartment


## 7. KakaoTalk / Hwp ì‹¤í–‰/ë¹„íŠ¸ìˆ˜/ë¡œë“œëœ ëª¨ë“ˆ

ÀÌ¹ÌÁö ÀÌ¸§                    PID ¼¼¼Ç ÀÌ¸§              ¼¼¼Ç#  ¸Ş¸ğ¸® »ç¿ë
========================= ======== ================ =========== ============
KakaoTalk.exe                23544 Console                    1    124,396 K
Á¤º¸: ½ÇÇà ÁßÀÎ ÀÛ¾÷ Áß ÁöÁ¤µÈ Á¶°Ç¿¡ ÀÏÄ¡ÇÏ´Â ÀÛ¾÷ÀÌ ¾ø½À´Ï´Ù.
--- unim_imm32.ime ë¡œë“œëœ í”„ë¡œì„¸ìŠ¤ ---
Á¤º¸: ½ÇÇà ÁßÀÎ ÀÛ¾÷ Áß ÁöÁ¤µÈ Á¶°Ç¿¡ ÀÏÄ¡ÇÏ´Â ÀÛ¾÷ÀÌ ¾ø½À´Ï´Ù.
--- unim_tsf.dll ë¡œë“œëœ í”„ë¡œì„¸ìŠ¤ ---

ÀÌ¹ÌÁö ÀÌ¸§                    PID ¸ğµâ                                        
========================= ======== ============================================
explorer.exe                 10432 unim_tsf.dll                                
SearchHost.exe               13424 unim_tsf.dll                                
msedgewebview2.exe           14380 unim_tsf.dll                                
msrdc.exe                    15296 unim_tsf.dll                                
Cloudflare WARP.exe          17160 unim_tsf.dll                                
msedgewebview2.exe           17940 unim_tsf.dll                                
RaiDrive.Mount.exe           19252 unim_tsf.dll                                
chrome.exe                   19244 unim_tsf.dll                                
msedgewebview2.exe           18516 unim_tsf.dll                                
MSPCManager.exe              22252 unim_tsf.dll                                
msedgewebview2.exe           21716 unim_tsf.dll                                
msedgewebview2.exe           16368 unim_tsf.dll                                
unim-tsf-settings.exe        23452 unim_tsf.dll                                
wezterm-gui.exe               8416 unim_tsf.dll                                
wezterm-gui.exe              24820 unim_tsf.dll                                
Notepad.exe                  18928 unim_tsf.dll                                

## 8. KakaoTalk ì„¤ì¹˜ ê²½ë¡œ ì¶”ì •
-rwxr-xr-x 1 USER 197609 27523688 Jun 11 18:29 /c/Program Files (x86)/Kakao/KakaoTalk/KakaoTalk.exe

## 9. ë¡œê·¸ íŒŒì¼ (TEMP)
-rw-r--r-- 1 USER 197609   3394 Jun 17 12:42 /tmp/unim-popup-win.log
-rw-r--r-- 1 USER 197609 647215 Jun 17 12:59 /tmp/unim-tsf.log
-rw-r--r-- 1 USER 197609   3394 Jun 17 12:42 /c/Users/USER/AppData/Local/Temp/unim-popup-win.log
-rw-r--r-- 1 USER 197609 647215 Jun 17 12:59 /c/Users/USER/AppData/Local/Temp/unim-tsf.log

## 10. build-msi.bat ì„œëª… ë‹¨ê³„ ì¡´ì¬ ì—¬ë¶€
(ì—†ìœ¼ë©´ ë¯¸ì„œëª…)
