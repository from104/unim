# Capture OutputDebugString from UNIM TSF DLL (no DebugView needed).
# Run this, then activate UNIM in Notepad; [UNIM-TSF] lines appear here.
#
# Run as ADMIN (global debug buffer needs elevation).
#   Set-ExecutionPolicy -Scope Process Bypass -Force
#   .\scripts\unim-capture-debug.ps1
# Ctrl+C to stop.

$cs = @"
using System;
using System.Text;
using System.Threading;
using System.Runtime.InteropServices;

public static class DbgCap {
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr CreateEventW(IntPtr a, bool manual, bool init, string name);
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr CreateFileMappingW(IntPtr h, IntPtr a, uint prot, uint hi, uint lo, string name);
    [DllImport("kernel32.dll", SetLastError=true)] static extern IntPtr MapViewOfFile(IntPtr h, uint access, uint hi, uint lo, UIntPtr size);
    [DllImport("kernel32.dll")] static extern bool SetEvent(IntPtr h);
    [DllImport("kernel32.dll")] static extern uint WaitForSingleObject(IntPtr h, uint ms);

    public static void Run() {
        IntPtr bufReady = CreateEventW(IntPtr.Zero, false, false, "DBWIN_BUFFER_READY");
        IntPtr dataReady = CreateEventW(IntPtr.Zero, false, false, "DBWIN_DATA_READY");
        IntPtr map = CreateFileMappingW((IntPtr)(-1), IntPtr.Zero, 0x04 /*PAGE_READWRITE*/, 0, 4096, "DBWIN_BUFFER");
        IntPtr buf = MapViewOfFile(map, 0x0004 /*FILE_MAP_READ*/, 0, 0, (UIntPtr)4096);
        if (bufReady == IntPtr.Zero || dataReady == IntPtr.Zero || map == IntPtr.Zero || buf == IntPtr.Zero) {
            Console.WriteLine("Failed to create DBWIN objects (is another debugger/DebugView running? close it).");
            return;
        }
        Console.WriteLine("Capturing OutputDebugString... (activate UNIM in Notepad; Ctrl+C to stop)");
        Console.WriteLine("Only [UNIM-TSF] lines are shown.");
        while (true) {
            SetEvent(bufReady);
            uint w = WaitForSingleObject(dataReady, 1000);
            if (w != 0) continue; // timeout, loop
            // buffer: [4-byte PID][ansi string]
            int pid = Marshal.ReadInt32(buf);
            string msg = Marshal.PtrToStringAnsi((IntPtr)((long)buf + 4));
            if (msg != null && msg.Contains("UNIM-TSF")) {
                Console.WriteLine("[pid " + pid + "] " + msg.TrimEnd());
            }
        }
    }
}
"@

try {
    Add-Type -TypeDefinition $cs -Language CSharp
} catch {
    Write-Host "compile error: $_" -ForegroundColor Red
    exit 1
}

[DbgCap]::Run()
