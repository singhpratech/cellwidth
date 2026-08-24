//! Splitting must never invent, drop or reorder bytes.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|s: &str| {
    let clusters: String = cellwidth::graphemes(s).collect();
    assert_eq!(clusters, s, "graphemes lost data");
    assert!(cellwidth::graphemes(s).all(|g| !g.is_empty()));

    let pieces: String = cellwidth::pieces(s).map(|p| p.as_str()).collect();
    assert_eq!(pieces, s, "pieces lost data");

    // Stripping escapes joins clusters the escape kept apart, so the width can
    // move in either direction: a lone combining mark measures as its own
    // cluster, but merged into a base it takes the base's width. There is no
    // useful inequality to assert here, only that it does not panic.
    let stripped = cellwidth::strip_ansi(s);
    let _ = cellwidth::width(&stripped);
});
