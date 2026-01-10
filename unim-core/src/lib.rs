use std::ffi::{CStr, CString};
use std::os::raw::c_char;

use unim::keystroke;

pub fn unim_core_main() {
    println!("Hello from unim-core");
}

/// A simple function to be called from C/JavaScript
#[no_mangle]
pub extern "C" fn unim_add(a: i32, b: i32) -> i32 {
    a + b
}

/// Transforms a string from one keyboard layout to another.
///
/// # Safety
///
/// - `input_ptr` must be a valid, null-terminated C string.
/// - The caller is responsible for freeing the returned string using `free_string`.
#[no_mangle]
pub unsafe extern "C" fn transform_string(
    input_ptr: *const c_char,
    from_layout: *const c_char,
    to_layout: *const c_char,
) -> *mut c_char {
    if input_ptr.is_null() || from_layout.is_null() || to_layout.is_null() {
        return std::ptr::null_mut();
    }

    let input_cstr = CStr::from_ptr(input_ptr);
    let from_layout_cstr = CStr::from_ptr(from_layout);
    let to_layout_cstr = CStr::from_ptr(to_layout);

    let input = input_cstr.to_str().unwrap_or("");
    let from = from_layout_cstr.to_str().unwrap_or("en_qwerty");
    let to = to_layout_cstr.to_str().unwrap_or("ko_2bulstd");

    let keystrokes = keystroke::string_to_keystrokes(input, from);
    let result_string = keystroke::keystrokes_to_string(&keystrokes, to);

    CString::new(result_string).unwrap_or_default().into_raw()
}

/// Frees the memory of a string returned by `transform_string`.
///
/// # Safety
///
/// - `ptr` must be a pointer returned by `transform_string`.
#[no_mangle]
pub unsafe extern "C" fn free_string(ptr: *mut c_char) {
    if !ptr.is_null() {
        let _ = CString::from_raw(ptr);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        unim_core_main();
    }
}
