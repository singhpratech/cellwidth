//! `cell` must produce exactly the requested number of columns. This is the
//! property every table layout depends on.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (&str, u8)| {
    let (s, w) = data;
    let w = w as usize;
    if w == 0 {
        return;
    }
    let out = cellwidth::cell(s, w);
    assert_eq!(
        cellwidth::width(&out),
        w,
        "cell({s:?}, {w}) = {out:?} is {} columns",
        cellwidth::width(&out)
    );
});
