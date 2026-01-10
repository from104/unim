try {
    console.log('Testing globalThis.imports.ctypes...');
    const ctypes = globalThis.imports.ctypes;
    console.log('Success!', ctypes);
} catch (e) {
    console.log('Failed globalThis.imports.ctypes:', e.message);
}

try {
    console.log('Testing import { ctypes } from "gi://Gjs"...');
    // We can't use dynamic import for this easily in a script
} catch (e) {}
