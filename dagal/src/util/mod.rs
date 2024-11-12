use std::ffi::{c_char, CStr, CString};

pub use align::*;
pub use free_list_allocator::FreeList;
pub use slot_map::*;

pub mod align;
mod format;
pub mod free_list_allocator;
pub mod queue_allocator;
/// Utility functions commonly used
pub mod slot_map;
pub mod tests;
mod traits;

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

fn to_u8_slice<T: Sized>(data: &[T]) -> &[u8] {
    unsafe { std::slice::from_raw_parts(data.as_ptr() as *const u8, std::mem::size_of_val(data)) }
}
