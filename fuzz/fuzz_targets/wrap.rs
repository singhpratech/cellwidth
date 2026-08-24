//! Wrapping must respect the budget and preserve the text.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (&str, u8)| {
    let (s, w) = data;
    let w = (w as usize).max(1);
    let lines = cellwidth::wrap(s, w);

    for line in &lines {
        // A line may exceed the budget only when one indivisible cluster is
        // wider than the whole line. Zero-width clusters ride along for free
        // and do not count towards that.
        if cellwidth::width(line) > w {
            let stripped = cellwidth::strip_ansi(line);
            let visible = cellwidth::graphemes(&stripped)
                .filter(|g| cellwidth::width(g) > 0)
                .count();
            assert!(
                visible <= 1,
                "wrap({s:?}, {w}) line {line:?} is {} columns from {visible} clusters",
                cellwidth::width(line)
            );
        }
    }
    // Every visible character survives, in order. Wrapping collapses runs of
    // whitespace, so whitespace is excluded from the comparison. Inputs that
    // contain an escape introducer (ESC or a C1 control) are skipped: wrapping
    // may reopen styling on each line, and collapsing whitespace next to a
    // malformed escape legitimately changes where that escape ends.
    let has_escape = s
        .chars()
        .any(|c| c == '\u{1b}' || ('\u{80}'..='\u{9f}').contains(&c));
    if !has_escape {
        let visible = |t: &str| -> String { t.chars().filter(|c| !c.is_whitespace()).collect() };
        let joined: String = lines.concat();
        assert_eq!(visible(&joined), visible(s), "wrap({s:?}, {w}) changed the text");
    }
});
