//! Raw C-ABI shim so the browser demo runs the real crate.
//!
//! No wasm-bindgen: cellwidth has no dependencies and this keeps it that way.
//! Strings cross the boundary as (ptr, len) pairs packed into a u64.

use cellwidth::{cell, graphemes, truncate, wrap, Ambiguous, Clusters, Width};

/// ptr in the high 32 bits, len in the low 32.
fn pack(s: String) -> u64 {
    let boxed = s.into_boxed_str();
    let len = boxed.len() as u64;
    let ptr = Box::into_raw(boxed) as *mut u8 as u64;
    (ptr << 32) | len
}

fn borrow<'a>(ptr: *const u8, len: usize) -> &'a str {
    if ptr.is_null() || len == 0 {
        return "";
    }
    let bytes = unsafe { core::slice::from_raw_parts(ptr, len) };
    core::str::from_utf8(bytes).unwrap_or("")
}

fn preset(mode: u32) -> Width {
    match mode {
        1 => Width::LEGACY,
        2 => Width::DEFAULT.ambiguous(Ambiguous::Wide),
        _ => Width::DEFAULT,
    }
}

/// Allocate `len` bytes for JS to write UTF-8 into.
#[no_mangle]
pub extern "C" fn cw_alloc(len: usize) -> *mut u8 {
    let mut v = Vec::<u8>::with_capacity(len);
    let p = v.as_mut_ptr();
    core::mem::forget(v);
    p
}

/// Release a buffer handed out by `cw_alloc`.
#[no_mangle]
pub extern "C" fn cw_free(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        unsafe { drop(Vec::from_raw_parts(ptr, 0, len)) }
    }
}

/// Release a string returned packed from one of the string calls.
#[no_mangle]
pub extern "C" fn cw_free_str(ptr: *mut u8, len: usize) {
    if !ptr.is_null() && len > 0 {
        unsafe {
            let s = core::slice::from_raw_parts_mut(ptr, len);
            drop(Box::from_raw(s as *mut [u8] as *mut str));
        }
    }
}

/// Display width in terminal cells.
#[no_mangle]
pub extern "C" fn cw_width(ptr: *const u8, len: usize, mode: u32) -> u32 {
    preset(mode).of(borrow(ptr, len)) as u32
}

/// Byte length of `str::len`, for the comparison panel.
#[no_mangle]
pub extern "C" fn cw_bytes(ptr: *const u8, len: usize) -> u32 {
    borrow(ptr, len).len() as u32
}

/// `chars().count()`, for the comparison panel.
#[no_mangle]
pub extern "C" fn cw_chars(ptr: *const u8, len: usize) -> u32 {
    borrow(ptr, len).chars().count() as u32
}

/// Number of extended grapheme clusters (UAX #29).
#[no_mangle]
pub extern "C" fn cw_graphemes(ptr: *const u8, len: usize) -> u32 {
    graphemes(borrow(ptr, len)).count() as u32
}

/// Longest prefix fitting `cols`, as a packed string.
#[no_mangle]
pub extern "C" fn cw_truncate(ptr: *const u8, len: usize, cols: u32) -> u64 {
    pack(truncate(borrow(ptr, len), cols as usize).to_string())
}

/// Exactly `cols` columns: truncate with an ellipsis or pad with spaces.
#[no_mangle]
pub extern "C" fn cw_cell(ptr: *const u8, len: usize, cols: u32) -> u64 {
    pack(cell(borrow(ptr, len), cols as usize).into_owned())
}

/// Wrap to `cols`, newline separated.
#[no_mangle]
pub extern "C" fn cw_wrap(ptr: *const u8, len: usize, cols: u32) -> u64 {
    pack(wrap(borrow(ptr, len), cols as usize).join("\n"))
}

/// Grapheme clusters joined by U+001F, so JS can split them exactly.
#[no_mangle]
pub extern "C" fn cw_split(ptr: *const u8, len: usize) -> u64 {
    let v: Vec<&str> = graphemes(borrow(ptr, len)).collect();
    pack(v.join("\u{1f}"))
}

/// Per-cluster widths, joined by a comma.
#[no_mangle]
pub extern "C" fn cw_cluster_widths(ptr: *const u8, len: usize, mode: u32) -> u64 {
    let w = preset(mode);
    let v: Vec<String> = graphemes(borrow(ptr, len))
        .map(|g| w.of(g).to_string())
        .collect();
    pack(v.join(","))
}

/// Width counting every code point separately, as wcwidth does.
#[no_mangle]
pub extern "C" fn cw_codepoint_width(ptr: *const u8, len: usize) -> u32 {
    Width::DEFAULT.clusters(Clusters::CodePoints).of(borrow(ptr, len)) as u32
}

/// The Unicode version the tables were generated from, packed.
#[no_mangle]
pub extern "C" fn cw_unicode_version() -> u64 {
    pack(cellwidth::UNICODE_VERSION.to_string())
}
