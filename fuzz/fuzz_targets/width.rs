//! Measurement must terminate, never panic, and stay within sane bounds for
//! every possible input, under every policy.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (&str, bool, bool, u8, bool)| {
    let (s, amb, legacy, tab, ansi) = data;
    let w = cellwidth::Width::DEFAULT
        .ambiguous(if amb {
            cellwidth::Ambiguous::Wide
        } else {
            cellwidth::Ambiguous::Narrow
        })
        .clusters(if legacy {
            cellwidth::Clusters::CodePoints
        } else {
            cellwidth::Clusters::WholeGlyph
        })
        .tab_stop(tab as usize)
        .ansi(ansi);

    let total = w.of(s);

    // No character is wider than two cells, except a tab, which cannot exceed
    // one tab stop.
    let bound: usize = s
        .chars()
        .map(|c| if c == '\t' { tab as usize } else { 2 })
        .sum();
    assert!(total <= bound, "{s:?} measured {total}, bound {bound}");

    // Measuring the clusters must equal measuring the whole, tabs aside.
    if !s.contains('\t') && !ansi {
        let sum: usize = cellwidth::graphemes(s).map(|g| w.of_grapheme(g)).sum();
        assert_eq!(sum, total, "cluster sum disagrees for {s:?}");
    }
});
