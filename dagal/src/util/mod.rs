use std::ffi::{c_char, CStr, CString};

/// Utility functions commonly used
pub mod deletion_stack;
pub use deletion_stack::DeletionStack;
pub mod slot_map;
pub mod tests;

pub fn convert_raw_c_ptrs_to_cstring(raw_pointers: &'static [*const c_char]) -> Vec<CString> {
    raw_pointers.iter().map(|&p| wrap_c_str(p)).collect()
}

/// Thanks phobos
/// https://github.com/NotAPenguin0/phobos-rs/blob/2a1e539611bb3ede5c2d7978300353630c7c553b/src/util/string.rs#L7
#[allow(clippy::not_unsafe_ptr_arg_deref)]
pub fn wrap_c_str(c: *const c_char) -> CString {
    return if c.is_null() {
        CString::new("").unwrap()
    } else {
        unsafe { CString::new(CStr::from_ptr(c).to_bytes()).unwrap() }
    };
}
