//! The compatibility shims give the same answers as the native API and keep
//! the conventions of the crates they replace.

use cellwidth::compat::{UnicodeSegmentation, UnicodeWidthChar, UnicodeWidthStr};
use cellwidth::{char_width, graphemes, width, Ambiguous, Width};

const CASES: &[&str] = &[
    "",
    "hello",
    "café",
    "日本語",
    "👨‍👩‍👧‍👦",
    "🇯🇵 Tōkyō 東京",
    "\x1b[31mred\x1b[0m",
    "°C ± α",
    "a\tb",
    "\r\n",
    "क्षि",
    "\u{0600}12",
    "क\u{0903}",
];

#[test]
fn str_width_is_the_free_function() {
    let cjk = Width::DEFAULT.ambiguous(Ambiguous::Wide);
    for s in CASES {
        assert_eq!(s.width(), width(s), "{s:?}");
        assert_eq!(s.width_cjk(), cjk.of(s), "{s:?}");
    }
    assert_eq!("°C".width(), 2);
    assert_eq!("°C".width_cjk(), 3);
    assert_eq!("👨‍👩‍👧‍👦".width(), 2);
    assert_eq!("\x1b[31mred\x1b[0m".width(), 3);
}

#[test]
fn char_width_is_none_exactly_for_control_characters() {
    let cjk = Width::DEFAULT.ambiguous(Ambiguous::Wide);
    // Every code point natively; a prime-strided sample under Miri, where a
    // full sweep would take longer than the rest of the suite combined.
    let step = if cfg!(miri) { 997 } else { 1 };
    for cp in (0u32..=0x10FFFF).step_by(step) {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let expect = if c.is_control() {
            None
        } else {
            Some(char_width(c))
        };
        assert_eq!(c.width(), expect, "U+{cp:04X}");
        let expect_cjk = if c.is_control() {
            None
        } else {
            Some(cjk.of_char(c))
        };
        assert_eq!(c.width_cjk(), expect_cjk, "U+{cp:04X}");
    }
    assert_eq!('\x1b'.width(), None);
    assert_eq!('\t'.width(), None);
    assert_eq!('\u{85}'.width(), None);
    assert_eq!('a'.width(), Some(1));
    assert_eq!('日'.width(), Some(2));
    assert_eq!('\u{301}'.width(), Some(0));
    assert_eq!('°'.width(), Some(1));
    assert_eq!('°'.width_cjk(), Some(2));
}

#[test]
fn extended_graphemes_are_the_free_function() {
    for s in CASES {
        let shim: Vec<&str> = s.graphemes(true).collect();
        let native: Vec<&str> = graphemes(s).collect();
        assert_eq!(shim, native, "{s:?}");
    }
}

#[test]
fn legacy_graphemes_drop_gb9a_gb9b_gb9c() {
    // GB9a: a spacing mark joins in extended clusters only.
    let bengali: Vec<&str> = "क\u{0903}".graphemes(false).collect();
    assert_eq!(bengali, ["क", "\u{0903}"]);
    assert_eq!("क\u{0903}".graphemes(true).count(), 1);

    // GB9b: a prepended character joins in extended clusters only.
    let prepend: Vec<&str> = "\u{0600}12".graphemes(false).collect();
    assert_eq!(prepend, ["\u{0600}", "1", "2"]);
    assert_eq!("\u{0600}12".graphemes(true).count(), 2);

    // GB9c: an Indic conjunct is one extended cluster, two legacy ones.
    let conjunct: Vec<&str> = "क्ष".graphemes(false).collect();
    assert_eq!(conjunct, ["क्", "ष"]);
    assert_eq!("क्ष".graphemes(true).count(), 1);

    // Everything else is unchanged: GB9 (extend, ZWJ), GB11, GB12, Hangul.
    for s in ["café", "👨‍👩‍👧‍👦", "🇯🇵🇯🇵", "각", "\r\n", "👍🏽", ""]
    {
        let legacy: Vec<&str> = s.graphemes(false).collect();
        let extended: Vec<&str> = s.graphemes(true).collect();
        assert_eq!(legacy, extended, "{s:?}");
    }
}

#[test]
fn grapheme_indices_report_byte_offsets() {
    let got: Vec<(usize, &str)> = "a👍🏽b".grapheme_indices(true).collect();
    assert_eq!(got, [(0, "a"), (1, "👍🏽"), (9, "b")]);

    let legacy: Vec<(usize, &str)> = "क\u{0903}".grapheme_indices(false).collect();
    assert_eq!(legacy, [(0, "क"), (3, "\u{0903}")]);

    for s in CASES {
        for (at, g) in s.grapheme_indices(true) {
            assert_eq!(&s[at..at + g.len()], g, "{s:?}");
        }
        let joined: String = s.grapheme_indices(true).map(|(_, g)| g).collect();
        assert_eq!(&joined, s);
    }
}

#[test]
fn grapheme_indices_iterator_contract() {
    let mut it = "ab".grapheme_indices(true);
    assert_eq!(it.size_hint(), (1, Some(2)));
    assert_eq!(it.next(), Some((0, "a")));
    let copy = it.clone();
    assert_eq!(it.next(), Some((1, "b")));
    assert_eq!(it.next(), None);
    assert_eq!(it.next(), None, "fused");
    assert_eq!(copy.collect::<Vec<_>>(), [(1, "b")]);
    assert_eq!("".grapheme_indices(true).size_hint(), (0, Some(0)));
    assert_eq!("".grapheme_indices(false).next(), None);
}
