//! Truncating, padding and wrapping.

use cellwidth::{cell, graphemes, pad_end, pad_start, strip_ansi, truncate, width, wrap, Width};

/// A corpus that exercises every hard case at once.
const CORPUS: &[&str] = &[
    "",
    "a",
    "hello world",
    "日本語テキスト",
    "café au lait",
    "cafe\u{301} au lait",
    "👨‍👩‍👧‍👦 family time",
    "🇯🇵🇺🇸🇬🇧 flags",
    "👍🏽 thumbs",
    "\u{2764}\u{FE0F} heart",
    "\x1b[31mred\x1b[0m and \x1b[1;34mblue\x1b[0m",
    "\x1b[32m日本語\x1b[0m mixed",
    "\x1b]8;;https://example.com\x1b\\a link\x1b]8;;\x1b\\",
    "क्षि नमस्ते",
    "한국어 조합 테스트",
    "one-very-long-unbreakable-token-that-exceeds-any-sane-column-budget",
];

#[test]
fn truncate_never_exceeds_the_budget() {
    for s in CORPUS {
        for max in 0..=30 {
            let got = truncate(s, max);
            assert!(
                width(got) <= max,
                "{s:?} truncated to {max} gave width {}",
                width(got)
            );
            assert!(s.starts_with(got), "{got:?} is not a prefix of {s:?}");
        }
    }
}

#[test]
fn truncate_takes_as_much_as_fits() {
    for s in CORPUS {
        for max in 0..=30 {
            let got = truncate(s, max);
            if got.len() == s.len() {
                continue;
            }
            // Adding the next cluster would have to overflow, otherwise we cut
            // too early.
            let rest = &s[got.len()..];
            let next = graphemes(rest).next().unwrap_or("");
            let bumped = &s[..got.len() + next.len()];
            assert!(
                width(bumped) > max,
                "{s:?} at {max}: could have fitted {bumped:?}"
            );
        }
    }
}

#[test]
fn truncate_keeps_clusters_whole() {
    assert_eq!(
        truncate("日本語", 1),
        "",
        "half a wide character is not an option"
    );
    assert_eq!(truncate("日本語", 2), "日");
    assert_eq!(truncate("日本語", 3), "日");
    assert_eq!(truncate("cafe\u{301}", 4), "cafe\u{301}");
    assert_eq!(truncate("👨‍👩‍👧‍👦x", 2), "👨‍👩‍👧‍👦", "never split a ZWJ sequence");
    assert_eq!(truncate("🇯🇵🇺🇸", 3), "🇯🇵", "never split a flag");
}

#[test]
fn truncate_never_splits_an_escape() {
    let s = "\x1b[31mred\x1b[0m";
    for max in 0..=6 {
        let got = truncate(s, max);
        // Every escape in the output must be complete: stripping colour codes
        // from the prefix must leave only the letters of "red".
        let plain = strip_ansi(got);
        assert!(
            "red".starts_with(plain.as_ref()),
            "{got:?} contains a mangled escape"
        );
        assert!(!got.contains("\x1b[3") || got.contains("\x1b[31m"));
    }
}

#[test]
fn ellipsis_marks_the_cut() {
    let w = Width::DEFAULT;
    assert_eq!(w.truncate_ellipsis("hello world", 8, "…"), "hello w…");
    assert_eq!(w.truncate_ellipsis("hello", 8, "…"), "hello");
    assert_eq!(w.truncate_ellipsis("日本語テキスト", 7, "…"), "日本語…");
    assert_eq!(w.truncate_ellipsis("hello world", 5, "..."), "he...");
    // No room for the marker at all.
    assert_eq!(w.truncate_ellipsis("hello", 0, "…"), "");
}

#[test]
fn cut_styled_text_closes_the_style() {
    let out = Width::DEFAULT.truncate_ellipsis("\x1b[31mred text\x1b[0m", 5, "…");
    assert!(out.ends_with("\x1b[0m"), "{out:?} leaves the terminal red");
    assert_eq!(width(&out), 5);
    // Nothing to close when the cut lands outside any styling.
    let plain = Width::DEFAULT.truncate_ellipsis("plain text", 5, "…");
    assert_eq!(plain, "plai…");
}

#[test]
fn cell_is_always_exactly_the_requested_width() {
    for s in CORPUS {
        for w in 1..=30 {
            let c = cell(s, w);
            assert_eq!(width(&c), w, "cell({s:?}, {w}) = {c:?}");
        }
    }
}

#[test]
fn padding() {
    assert_eq!(pad_end("hi", 5), "hi   ");
    assert_eq!(pad_start("hi", 5), "   hi");
    assert_eq!(Width::DEFAULT.center("hi", 6), "  hi  ");
    assert_eq!(
        Width::DEFAULT.center("hi", 7),
        "  hi   ",
        "odd gap leans left"
    );
    assert_eq!(
        pad_end("日本", 6),
        "日本  ",
        "padding counts columns, not chars"
    );
    assert_eq!(pad_end("too long", 3), "too long", "never truncates");
    assert_eq!(pad_end("\x1b[31mred\x1b[0m", 5), "\x1b[31mred\x1b[0m  ");
}

#[test]
fn wrapping_respects_the_budget() {
    for s in CORPUS {
        for w in 2..=20 {
            for line in wrap(s, w) {
                let single_cluster = graphemes(&strip_ansi(&line)).count() <= 1;
                assert!(
                    width(&line) <= w || single_cluster,
                    "wrap({s:?}, {w}) produced a {}-column line {line:?}",
                    width(&line)
                );
            }
        }
    }
}

#[test]
fn wrapping_handles_newlines_at_the_edges() {
    // A trailing newline ends the last line rather than starting an empty one.
    assert_eq!(wrap("a\n", 10), ["a"]);
    // A deliberate blank line survives.
    assert_eq!(wrap("a\n\nb", 10), ["a", "", "b"]);
    assert_eq!(wrap("\na", 10), ["", "a"]);
    // Empty input still yields one empty line, so callers can print it.
    assert_eq!(wrap("", 10), [""]);
    assert_eq!(wrap("   ", 10), [""]);
}

#[test]
fn wrapping_handles_every_line_separator() {
    // CR LF is one break, not two, and none of the separators survive into the
    // line they terminate.
    assert_eq!(wrap("a\r\nb", 10), ["a", "b"]);
    assert_eq!(wrap("a\rb", 10), ["a", "b"]);
    assert_eq!(wrap("a\u{2028}b", 10), ["a", "b"]);
    assert_eq!(wrap("a\u{85}b", 10), ["a", "b"]);
    assert_eq!(wrap("a\u{b}b", 10), ["a", "b"]);
    assert_eq!(wrap("a\r\n\r\nb", 10), ["a", "", "b"]);
    for out in [wrap("a\r\nb", 10), wrap("a\rb", 10)] {
        assert!(out.iter().all(|l| !l.contains(['\r', '\n'])), "{out:?}");
    }
}

#[test]
fn wrapping_keeps_the_words() {
    assert_eq!(wrap("the quick brown fox", 10), ["the quick", "brown fox"]);
    assert_eq!(wrap("one two three", 40), ["one two three"]);
    assert_eq!(wrap("hard\nbreak", 40), ["hard", "break"]);
    // A word longer than the line is broken by column, not dropped.
    let lines = wrap("supercalifragilistic", 7);
    assert_eq!(lines.concat(), "supercalifragilistic");
    assert!(lines.iter().all(|l| width(l) <= 7));
}

#[test]
fn wrapping_reopens_styles_on_each_line() {
    let lines = wrap("\x1b[31mred words here now\x1b[0m", 10);
    assert!(lines.len() > 1);
    for line in &lines {
        assert!(line.starts_with("\x1b[31m"), "{line:?} lost its colour");
        assert!(line.ends_with("\x1b[0m"), "{line:?} leaks its colour");
    }
}

#[test]
fn degenerate_widths() {
    assert_eq!(truncate("anything", 0), "");
    assert_eq!(wrap("anything", 0), Vec::<String>::new());
    assert_eq!(cell("", 3), "   ");
    // A cluster wider than the cell still cannot be halved.
    assert_eq!(width(&cell("日", 1)), 1);
}

/// Regressions found by the fuzzer. Each one is a real defect that the
/// hand-written tests above missed.
mod fuzz_regressions {
    use super::*;

    /// An unterminated escape swallows whatever follows it, so naive padding
    /// produced a cell that measured zero columns instead of the width asked
    /// for. Found by `fuzz/fuzz_targets/cell.rs`.
    #[test]
    fn dangling_escape_cannot_eat_the_padding() {
        for input in ["\x1b", "\x1b[", "\x1b[31", "text\x1b[", "\x1b]8;;http://x"] {
            for w in 1..=20 {
                assert_eq!(
                    width(&cell(input, w)),
                    w,
                    "cell({input:?}, {w}) = {:?}",
                    cell(input, w)
                );
            }
            assert!(width(&pad_end(input, 10)) >= 10, "pad_end({input:?})");
        }
    }

    /// A final line made only of zero-width characters was dropped, losing
    /// text. Found by `fuzz/fuzz_targets/wrap.rs`.
    #[test]
    fn wrap_keeps_a_final_zero_width_line() {
        let s = "\u{1F3FD}\u{0}\u{0}\u{0}";
        let joined: String = wrap(s, 1).concat();
        assert!(
            joined.contains('\u{0}'),
            "wrap dropped the NUL run: {joined:?}"
        );
        assert_eq!(joined.matches('\u{0}').count(), 3);
    }

    /// Removing escapes can merge clusters the escape held apart, which is
    /// legitimate: an escape between two halves of a ZWJ sequence really does
    /// break it in a terminal too.
    #[test]
    fn stripping_escapes_may_narrow_but_never_widen() {
        let split = "\u{1F468}\u{200D}\u{1F469}\u{200D}\x1b[\u{1F467}\u{200D}\u{1F466}";
        assert_eq!(width(split), 4, "the escape breaks the ZWJ sequence");
        assert_eq!(width(&strip_ansi(split)), 2, "without it, one family glyph");
    }
}
