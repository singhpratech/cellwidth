//! `truncate` must always return a prefix that fits, and must never stop
//! earlier than it has to.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (&str, u16)| {
    let (s, max) = data;
    let max = max as usize;
    let got = cellwidth::truncate(s, max);

    assert!(s.starts_with(got), "not a prefix");
    assert!(cellwidth::width(got) <= max, "over budget");

    // If anything was dropped, taking one more cluster must overflow --
    // otherwise we truncated too eagerly.
    if got.len() < s.len() {
        let rest = &s[got.len()..];
        if let Some(next) = cellwidth::graphemes(rest).next() {
            let bumped = &s[..got.len() + next.len()];
            assert!(
                cellwidth::width(bumped) > max || next.chars().all(|c| c == '\u{1b}'),
                "could have fitted more of {s:?} at {max}"
            );
        }
    }
    // Truncating to the full width is the identity.
    assert_eq!(cellwidth::truncate(s, cellwidth::width(s)), s);
});
