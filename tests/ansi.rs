//! Escape sequence recognition.

use cellwidth::{pieces, strip_ansi, Piece};

fn split(s: &str) -> Vec<Piece<'_>> {
    pieces(s).collect()
}

#[test]
fn pieces_reproduce_the_input() {
    let samples = [
        "",
        "plain",
        "\x1b[31m",
        "\x1b[38;2;1;2;3mrgb\x1b[0m",
        "\x1b]0;title\x07body",
        "\x1b]8;;https://example.com\x1b\\text\x1b]8;;\x1b\\",
        "\x1bPtmux;\x1b\\passthrough",
        "\x1b7save\x1b8restore",
        "日本\x1b[Klanguage",
        "\x1b",
        "\x1b[",
        "\x1b[31",
    ];
    for s in samples {
        let joined: String = pieces(s).map(|p| p.as_str()).collect();
        assert_eq!(joined, s, "lossy split of {s:?}");
    }
}

#[test]
fn csi_boundaries() {
    assert_eq!(
        split("\x1b[1;31mx"),
        [Piece::Escape("\x1b[1;31m"), Piece::Text("x")]
    );
    assert_eq!(
        split("\x1b[Kx"),
        [Piece::Escape("\x1b[K"), Piece::Text("x")]
    );
    assert_eq!(
        split("\x1b[?25lx"),
        [Piece::Escape("\x1b[?25l"), Piece::Text("x")],
        "private parameter bytes belong to the sequence"
    );
}

#[test]
fn string_sequences_run_to_their_terminator() {
    assert_eq!(
        split("\x1b]0;t\x07after"),
        [Piece::Escape("\x1b]0;t\x07"), Piece::Text("after")],
        "BEL terminates OSC"
    );
    assert_eq!(
        split("\x1b]0;t\x1b\\after"),
        [Piece::Escape("\x1b]0;t\x1b\\"), Piece::Text("after")],
        "ST terminates OSC"
    );
    assert_eq!(
        split("\x1bPq#0;2\x1b\\after"),
        [Piece::Escape("\x1bPq#0;2\x1b\\"), Piece::Text("after")],
        "DCS, as used by sixel"
    );
}

#[test]
fn hyperlinks_leave_only_the_label() {
    let link = "\x1b]8;;https://example.com\x1b\\click here\x1b]8;;\x1b\\";
    assert_eq!(strip_ansi(link), "click here");
}

#[test]
fn eight_bit_c1_controls() {
    assert_eq!(strip_ansi("\u{9b}31mred\u{9b}0m"), "red");
    assert_eq!(strip_ansi("\u{9d}0;title\u{9c}body"), "body");
    // A bare U+00C2-prefixed character that is not a C1 control is just text.
    assert_eq!(strip_ansi("Ââ"), "Ââ");
}

#[test]
fn unterminated_sequences_swallow_the_rest() {
    // This is what a terminal does with them, so it is what measuring should
    // assume too.
    assert_eq!(strip_ansi("text\x1b[31"), "text");
    assert_eq!(strip_ansi("text\x1b]8;;never-closed"), "text");
}

#[test]
fn strip_borrows_when_there_is_nothing_to_do() {
    assert!(matches!(
        strip_ansi("plain 日本語"),
        std::borrow::Cow::Borrowed(_)
    ));
    assert!(matches!(strip_ansi("\x1b[0m"), std::borrow::Cow::Owned(_)));
}

/// Only CSI sequences carry SGR. Found by `fuzz/fuzz_targets/wrap.rs`, which
/// caught `wrap` injecting a reset into text that had no styling.
#[test]
fn non_csi_sequences_ending_in_m_are_not_colour_codes() {
    // A C1 SOS string that happens to end in `m`.
    let s = "\u{98}\u{1}\u{0}m";
    let lines = cellwidth::wrap(s, 95);
    assert_eq!(lines.concat(), s, "wrap added or dropped bytes");
    // An OSC that ends in `m` likewise.
    let osc = "\x1b]0;custom\x07plain";
    assert!(!cellwidth::wrap(osc, 40).concat().ends_with("\x1b[0m"));
    // A genuine SGR still gets closed.
    assert!(cellwidth::wrap("\x1b[31mred text here", 5)[0].ends_with("\x1b[0m"));
}

#[test]
fn piece_accessors() {
    let p: Vec<Piece> = pieces("a\x1b[0m").collect();
    assert!(!p[0].is_escape() && p[0].as_str() == "a");
    assert!(p[1].is_escape() && p[1].as_str() == "\x1b[0m");
}

#[test]
fn escapes_with_intermediate_bytes() {
    // CSI with an intermediate byte: set cursor style.
    assert_eq!(strip_ansi("\x1b[1 qx"), "x");
    // nF sequence with several intermediates: select character set.
    assert_eq!(strip_ansi("\x1b$)Cx"), "x");
    assert_eq!(strip_ansi("\x1b(Bx"), "x");
    // Two-byte Fp form.
    assert_eq!(strip_ansi("\x1b7x\x1b8"), "x");
}

#[test]
fn c1_sgr_is_recognised_for_style_tracking() {
    // An 8-bit CSI carrying colour must close like a 7-bit one.
    let out = cellwidth::Width::DEFAULT.truncate_ellipsis("\u{9b}31mred text", 5, "…");
    assert!(out.ends_with("\x1b[0m"), "{out:?}");
}

#[test]
fn escapes_can_be_measured_literally() {
    let literal = cellwidth::Width::DEFAULT.ansi(false);
    // The ESC is still a zero-column control character; the rest is text, so
    // three columns buys "[31".
    assert_eq!(literal.of("\x1b[31m"), 4);
    assert_eq!(literal.truncate("\x1b[31mred", 3), "\x1b[31");
    // The escape is just text now, so it wraps and hard-breaks like any word.
    assert_eq!(literal.wrap("\x1b[31m red", 3), ["\x1b[31", "m", "red"]);
    // And no reset is ever added, because nothing is recognised as styling.
    assert!(!literal
        .wrap("\x1b[31mred here", 4)
        .concat()
        .ends_with("\x1b[0m"));
}
