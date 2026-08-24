//! Column measurement, including the cases that break naive implementations.

use cellwidth::{char_width, width, Ambiguous, Clusters, Control, Width};

#[test]
fn ascii() {
    assert_eq!(width(""), 0);
    assert_eq!(width("hello"), 5);
    assert_eq!(width("  spaced  "), 10);
    for c in ' '..='~' {
        assert_eq!(char_width(c), 1, "{c:?}");
    }
}

#[test]
fn combining_marks_are_free() {
    assert_eq!(width("café"), 4); // precomposed
    assert_eq!(width("cafe\u{301}"), 4); // decomposed
    assert_eq!(width("a\u{300}\u{301}\u{302}"), 1);
    // Zalgo text still occupies one column per base character.
    assert_eq!(width("z\u{0301}\u{0489}\u{036F}\u{0353}"), 1);
}

#[test]
fn east_asian_is_double() {
    assert_eq!(width("日本語"), 6);
    assert_eq!(width("한국어"), 6);
    assert_eq!(width("中文abc"), 7);
    assert_eq!(width("\u{FF21}\u{FF22}"), 4); // fullwidth Latin
    assert_eq!(width("\u{FF61}\u{FF62}"), 2); // halfwidth katakana
}

#[test]
fn hangul_jamo_compose_into_one_syllable() {
    // Decomposed 한: choseong + jungseong + jongseong is still two columns.
    assert_eq!(width("\u{1112}\u{1161}\u{11AB}"), 2);
    assert_eq!(width("\u{D55C}"), 2); // precomposed, for comparison
}

#[test]
fn emoji() {
    assert_eq!(width("😀"), 2);
    assert_eq!(width("👍🏽"), 2, "skin tone modifier must not add a column");
    assert_eq!(width("👨‍👩‍👧‍👦"), 2, "ZWJ family is one glyph");
    assert_eq!(width("👩🏾‍🚀"), 2, "modifier plus ZWJ");
    assert_eq!(width("🇯🇵"), 2, "flag is a regional indicator pair");
    assert_eq!(width("🇯🇵🇺🇸"), 4, "two flags");
    assert_eq!(width("\u{1F1EF}"), 2, "a lone regional indicator");
}

#[test]
fn variation_selectors_pick_the_presentation() {
    assert_eq!(width("\u{2764}"), 1, "bare heart is a text dingbat");
    assert_eq!(
        width("\u{2764}\u{FE0F}"),
        2,
        "VS16 asks for emoji presentation"
    );
    assert_eq!(width("\u{231A}"), 2, "watch defaults to emoji");
    assert_eq!(
        width("\u{231A}\u{FE0E}"),
        1,
        "VS15 asks for text presentation"
    );
    assert_eq!(width("1\u{FE0F}\u{20E3}"), 2, "keycap");
}

#[test]
fn zero_width_and_format_characters() {
    assert_eq!(width("\u{200B}"), 0); // ZWSP
    assert_eq!(width("\u{200D}"), 0); // ZWJ
    assert_eq!(width("\u{FEFF}"), 0); // BOM
    assert_eq!(width("a\u{202E}b"), 2); // bidi override
}

#[test]
fn ansi_escapes_are_free() {
    assert_eq!(width("\x1b[31mred\x1b[0m"), 3);
    assert_eq!(width("\x1b[1;38;2;255;0;0mfancy\x1b[m"), 5);
    assert_eq!(width("\x1b]0;window title\x07visible"), 7, "OSC title");
    assert_eq!(
        width("\x1b]8;;https://example.com\x1b\\link\x1b]8;;\x1b\\"),
        4,
        "OSC 8 hyperlink: only the label is visible"
    );
    assert_eq!(width("\u{9b}31mred\u{9b}0m"), 3, "8-bit C1 CSI");
    assert_eq!(width("\x1b[31m日本\x1b[0m"), 4);

    let literal = Width::DEFAULT.ansi(false);
    assert!(
        literal.of("\x1b[31mred") > 3,
        "escapes counted as characters"
    );
}

#[test]
fn ambiguous_width_is_a_policy() {
    let narrow = Width::DEFAULT;
    let wide = Width::DEFAULT.ambiguous(Ambiguous::Wide);
    for s in ["±", "°", "§", "α", "→", "─"] {
        assert_eq!(narrow.of(s), 1, "{s} narrow");
        assert_eq!(wide.of(s), 2, "{s} wide");
    }
    // Unambiguous characters are unaffected by the policy.
    assert_eq!(wide.of("a"), 1);
    assert_eq!(wide.of("日"), 2);
}

#[test]
fn tabs_advance_to_the_next_stop() {
    let w = Width::DEFAULT;
    assert_eq!(w.of("\t"), 8);
    assert_eq!(w.of("a\t"), 8);
    assert_eq!(w.of("abcdefg\t"), 8);
    assert_eq!(w.of("abcdefgh\t"), 16);
    assert_eq!(w.of("\t\t"), 16);
    assert_eq!(Width::DEFAULT.tab_stop(4).of("a\tb"), 5);
    assert_eq!(Width::DEFAULT.tab_stop(0).of("a\tb"), 2);
}

#[test]
fn control_characters() {
    let w = Width::DEFAULT;
    assert_eq!(w.of("a\u{7}b"), 2, "BEL takes no space");
    assert_eq!(w.of("\u{0}"), 0);
    let caret = Width::DEFAULT.control(Control::Caret);
    assert_eq!(caret.of("a\u{7}b"), 4, "rendered as ^G");
}

/// The two cluster models, and the terminals each was measured against.
/// See `results/` for the recordings and `probe/` for the harness.
#[test]
fn cluster_models_match_the_terminals_they_were_measured_against() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
    // kitty and foot draw one glyph; VTE and Alacritty draw four emoji.
    assert_eq!(Width::MODERN.of(family), 2);
    assert_eq!(Width::LEGACY.of(family), 8);
    assert_eq!(
        Width::DEFAULT,
        Width::MODERN,
        "the default is the modern model"
    );

    let legacy = Width::LEGACY;
    assert_eq!(
        legacy.of("\u{1F44D}\u{1F3FD}"),
        4,
        "modifier counted separately"
    );
    assert_eq!(legacy.of("\u{2764}\u{FE0F}"), 1, "VS16 ignored");
    assert_eq!(legacy.of("\u{231A}\u{FE0E}"), 2, "VS15 ignored");
    assert_eq!(legacy.of("1\u{FE0F}\u{20E3}"), 1, "keycap not composed");
    assert_eq!(
        legacy.of("\u{1F1EF}"),
        1,
        "lone regional indicator is narrow"
    );
    assert_eq!(
        legacy.of("\u{1F1EF}\u{1F1F5}"),
        2,
        "a flag is two narrow letters"
    );

    // The models differ on non-emoji clusters too.
    assert_eq!(
        Width::MODERN.of("\u{0995}\u{09BE}"),
        1,
        "Bengali, as kitty draws it"
    );
    assert_eq!(legacy.of("\u{0995}\u{09BE}"), 2, "as VTE draws it");
    assert_eq!(Width::MODERN.of("\u{AC00}\u{302E}"), 2);
    assert_eq!(legacy.of("\u{AC00}\u{302E}"), 4);

    // Everyday text is identical under both, which is why the choice only
    // matters for the exotic cases.
    for t in [
        "hello",
        "\u{65E5}\u{672C}\u{8A9E}",
        "caf\u{E9}",
        "\u{1F600}",
    ] {
        assert_eq!(Width::MODERN.of(t), Width::LEGACY.of(t), "{t:?}");
    }
    assert_eq!(Width::DEFAULT.clusters(Clusters::CodePoints), Width::LEGACY);
}

#[test]
fn width_is_additive_over_clusters() {
    // Whatever the policy, measuring the parts must match measuring the whole,
    // as long as no tabs are involved.
    let policies = [
        Width::DEFAULT,
        Width::DEFAULT.ambiguous(Ambiguous::Wide),
        Width::LEGACY,
    ];
    let samples = [
        "日本語abc",
        "👨‍👩‍👧‍👦x🇯🇵",
        "café±°",
        "क्षि नमस्ते",
        "\x1b[31mred\x1b[0m",
    ];
    for w in policies {
        for s in samples {
            let sum: usize = cellwidth::graphemes(&cellwidth::strip_ansi(s))
                .map(|g| w.of_grapheme(g))
                .sum();
            assert_eq!(sum, w.of(s), "{s:?}");
        }
    }
}

/// Places where this crate deliberately disagrees with POSIX `wcwidth`.
///
/// These were found by diffing every code point against glibc, and each one is
/// a considered choice rather than an accident.
#[test]
fn deliberate_divergences_from_wcwidth() {
    // Unicode 16 reclassified a batch of symbols as Wide. glibc still has the
    // old data; we follow the current UCD.
    assert_eq!(char_width('\u{2630}'), 2, "trigram, Wide since Unicode 16");
    assert_eq!(char_width('\u{1D360}'), 2, "counting rod numeral");

    // Invisible format characters take no cells, though wcwidth gives them one.
    assert_eq!(char_width('\u{0600}'), 0, "Arabic number sign");
    assert_eq!(char_width('\u{00AD}'), 0, "soft hyphen");

    // Emoji modifiers recolour the preceding emoji instead of taking cells.
    assert_eq!(char_width('\u{1F3FB}'), 0);

    // ...but characters that merely *combine* while still being drawn do take
    // cells. Other_Grapheme_Extend is a segmentation property, not a display
    // one, and treating it as zero-width is a common source of bugs.
    assert_eq!(
        char_width('\u{09BE}'),
        1,
        "Bengali vowel sign AA is visible"
    );
    assert_eq!(char_width('\u{FF9E}'), 1, "halfwidth katakana voiced mark");
    assert_eq!(char_width('\u{302E}'), 2, "Hangul tone mark");
    assert_eq!(
        char_width('\u{3164}'),
        2,
        "Hangul filler: blank, but 2 columns"
    );
    // The *cluster* width then depends on the model, and both answers were
    // measured: VTE draws two columns, kitty draws one.
    assert_eq!(Width::LEGACY.of("\u{0995}\u{09BE}"), 2, "VTE");
    assert_eq!(Width::MODERN.of("\u{0995}\u{09BE}"), 1, "kitty");
}

/// Regressions found by the fuzzer.
#[test]
fn flag_with_a_combining_mark_is_still_one_flag() {
    // A regional indicator pair followed by a combining mark is one cluster,
    // and one flag glyph. Requiring the cluster to be exactly two characters
    // made this measure 4.
    assert_eq!(width("\u{1F1E6}\u{1F1E6}\u{0301}"), 2);
    assert_eq!(
        cellwidth::graphemes("\u{1F1E6}\u{1F1E6}\u{0301}").count(),
        1
    );
    assert_eq!(width("\u{1F1EF}\u{1F1F5}\u{FE0F}"), 2);
}

/// A chain of malformed escapes must not leave a dangling one behind.
#[test]
fn nested_dangling_escapes_are_all_trimmed() {
    for s in ["\u{1b}\u{1b}-", "\u{1b}\u{1b}\u{1b}", "a\u{1b}\u{1b}["] {
        assert_eq!(cellwidth::width(&cellwidth::cell(s, 12)), 12, "{s:?}");
    }
}

#[test]
fn default_policy_matches_the_constant() {
    assert_eq!(Width::default(), Width::DEFAULT);
    assert_eq!(Width::default().of("日本語"), 6);
}

#[test]
fn grapheme_iterator_exposes_its_remainder() {
    let mut it = cellwidth::graphemes("a日本");
    assert_eq!(it.remainder(), "a日本");
    assert_eq!(it.next(), Some("a"));
    assert_eq!(it.remainder(), "日本");
    it.next();
    it.next();
    assert_eq!(it.remainder(), "");
    assert_eq!(it.next(), None);
}

/// `char_width` is a public entry point in its own right, and its control and
/// tab paths are not reachable through `width` (which measures clusters).
#[test]
fn char_width_covers_controls_and_tabs() {
    assert_eq!(char_width('\t'), 8, "a tab from column 0");
    assert_eq!(Width::DEFAULT.tab_stop(4).of_char('\t'), 4);
    assert_eq!(Width::DEFAULT.tab_stop(0).of_char('\t'), 0);

    for c in ['\u{0}', '\u{7}', '\n', '\r', '\u{1f}'] {
        assert_eq!(char_width(c), 0, "C0 {c:?}");
    }
    assert_eq!(char_width('\u{7f}'), 0, "DEL");
    for c in ['\u{80}', '\u{9b}', '\u{9f}'] {
        assert_eq!(char_width(c), 0, "C1 {c:?}");
    }

    let caret = Width::DEFAULT.control(Control::Caret);
    assert_eq!(caret.of_char('\u{7}'), 2);
    assert_eq!(caret.of_char('\u{7f}'), 2);
    assert_eq!(caret.of_char('\u{9b}'), 2);

    // And the ordinary paths, for contrast.
    assert_eq!(char_width('a'), 1);
    assert_eq!(char_width('\u{301}'), 0);
    assert_eq!(char_width('\u{65e5}'), 2);
    assert_eq!(char_width('\u{b1}'), 1);
    assert_eq!(
        Width::DEFAULT.ambiguous(Ambiguous::Wide).of_char('\u{b1}'),
        2
    );
}

#[test]
fn legacy_model_expands_tabs_by_position() {
    use cellwidth::Width;
    // The per-code-point model skips segmentation, so its tab handling has its
    // own path; a tab still advances to the next stop, from wherever it sits.
    assert_eq!(Width::LEGACY.of("a\tb"), 9);
    assert_eq!(Width::LEGACY.of_at("\t", 4), 4);
    assert_eq!(Width::LEGACY.tab_stop(4).of("日\tb"), 5);
    assert_eq!(Width::LEGACY.tab_stop(0).of("日\tb"), 3);
    assert_eq!(Width::LEGACY.of("日\tb"), Width::DEFAULT.of("日\tb"));
}
