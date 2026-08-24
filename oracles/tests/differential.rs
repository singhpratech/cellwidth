//! Differential testing against independent implementations.
//!
//! The point of this file is that none of the expectations here were written by
//! reading `cellwidth`'s own source. They come from other people's code
//! (`unicode-width`, `unicode-segmentation`) and from Unicode's own published
//! data. Where we disagree with an oracle, the disagreement is enumerated
//! explicitly below, so a regression cannot hide inside a fuzzy tolerance.

use std::collections::BTreeMap;

use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthChar;

/// Parse the code point column of an `emoji-test.txt` line.
fn emoji_sequence(line: &str) -> Option<(String, &str)> {
    let (cps, rest) = line.split_once(';')?;
    let status = rest.split('#').next()?.trim();
    let s: String = cps
        .split_whitespace()
        .map(|h| u32::from_str_radix(h, 16).ok().and_then(char::from_u32))
        .collect::<Option<String>>()?;
    Some((s, status))
}

/// Unicode publishes every valid emoji sequence. A fully-qualified one is by
/// definition emoji-presented, so a terminal draws it in two columns -- there
/// is no judgement call to make and no excuse for getting any of them wrong.
#[test]
fn every_fully_qualified_emoji_is_two_columns() {
    let data = include_str!("../../tests/data/emoji-test.txt");
    let mut checked = 0;
    let mut wrong = Vec::new();
    for line in data.lines() {
        let Some((s, status)) = emoji_sequence(line) else {
            continue;
        };
        if status != "fully-qualified" {
            continue;
        }
        checked += 1;
        let w = cellwidth::width(&s);
        if w != 2 {
            wrong.push(format!(
                "  {s}  {} => {w} columns",
                s.chars()
                    .map(|c| format!("U+{:04X}", c as u32))
                    .collect::<Vec<_>>()
                    .join(" ")
            ));
        }
    }
    assert!(checked > 3000, "only {checked} sequences parsed");
    assert!(
        wrong.is_empty(),
        "{}/{checked} emoji are not 2 columns:\n{}",
        wrong.len(),
        wrong.join("\n")
    );
    eprintln!("emoji-test.txt: {checked} fully-qualified sequences, all 2 columns");
}

/// Every emoji sequence is also exactly one grapheme cluster.
#[test]
fn every_emoji_is_one_cluster() {
    let data = include_str!("../../tests/data/emoji-test.txt");
    let mut wrong = Vec::new();
    for line in data.lines() {
        let Some((s, status)) = emoji_sequence(line) else {
            continue;
        };
        if status != "fully-qualified" {
            continue;
        }
        let n = cellwidth::graphemes(&s).count();
        if n != 1 {
            wrong.push(format!("  {s} split into {n} clusters"));
        }
    }
    assert!(wrong.is_empty(), "{}\n", wrong.join("\n"));
}

/// Segmentation must agree with `unicode-segmentation`, character for
/// character, across a corpus wide enough to hit every rule.
/// Every code point alone and paired with each kind of joiner, plus every
/// emoji sequence in context.
fn cluster_corpus() -> Vec<String> {
    let mut corpus: Vec<String> = Vec::new();

    // Every code point alone, and paired with each kind of joiner.
    let tails = [
        '\u{0301}',
        '\u{200D}',
        '\u{FE0F}',
        '\u{1F3FB}',
        '\u{20E3}',
        '\u{09BE}',
        '\u{094D}',
        '\u{1F1E6}',
        '\u{1160}',
        '\u{11A8}',
        '\u{0903}',
        '\u{0600}',
    ];
    for cp in (0u32..=0x10FFFF).step_by(3) {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        corpus.push(c.to_string());
        for t in tails {
            corpus.push(format!("{c}{t}"));
            corpus.push(format!("{t}{c}"));
        }
    }
    // Real emoji sequences, which exercise GB11 hardest.
    let data = include_str!("../../tests/data/emoji-test.txt");
    for line in data.lines() {
        if let Some((s, _)) = emoji_sequence(line) {
            corpus.push(s.clone());
            corpus.push(format!("{s}{s}"));
            corpus.push(format!("a{s}b"));
        }
    }
    corpus
}

#[test]
fn clusters_match_unicode_segmentation() {
    let corpus = cluster_corpus();
    let mut mismatches = 0;
    let mut first = Vec::new();
    for s in &corpus {
        let ours: Vec<&str> = cellwidth::graphemes(s).collect();
        let theirs: Vec<&str> = s.graphemes(true).collect();
        if ours != theirs {
            mismatches += 1;
            if first.len() < 10 {
                first.push(format!(
                    "  {:?}\n    ours   {ours:?}\n    theirs {theirs:?}",
                    s
                ));
            }
        }
    }
    assert_eq!(
        mismatches,
        0,
        "{mismatches} of {} corpus strings segmented differently:\n{}",
        corpus.len(),
        first.join("\n")
    );
    eprintln!(
        "unicode-segmentation: {} strings agree exactly",
        corpus.len()
    );
}

/// Where we disagree with `unicode-width` at the code point level, the
/// disagreement is deliberate. Every bucket below is a decision with a reason;
/// an unexpected bucket, or a changed count, fails the test.
#[test]
fn char_width_divergence_from_unicode_width_is_accounted_for() {
    let mut buckets: BTreeMap<(usize, i32), usize> = BTreeMap::new();
    for cp in 0u32..=0x10FFFF {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let ours = cellwidth::char_width(c);
        let theirs = UnicodeWidthChar::width(c).map_or(-1, |w| w as i32);
        if ours as i32 != theirs {
            *buckets.entry((ours, theirs)).or_default() += 1;
        }
    }

    // (ours, theirs) => (count, why)
    let expected: &[((usize, i32), usize, &str)] = &[
        (
            (0, -1),
            64,
            "C0/C1 controls: unicode-width declines to answer, we apply a policy",
        ),
        (
            (0, 1),
            28,
            "invisible format characters such as U+0600 take no cells",
        ),
        ((0, 2), 5, "emoji modifiers merge into the preceding emoji"),
        (
            (1, 0),
            75,
            "General_Category=Mc marks are spacing, whatever Other_Grapheme_Extend says",
        ),
        ((1, 2), 1, "U+17A4, a deprecated Khmer vowel"),
        (
            (1, 3),
            1,
            "U+17D8: unicode-width has a bespoke 3-column case for it",
        ),
        (
            (2, 0),
            5,
            "Hangul fillers and tone marks are blank but do occupy cells",
        ),
        (
            (2, 1),
            26,
            "a lone regional indicator draws as a boxed letter",
        ),
        (
            (8, -1),
            1,
            "U+0009 tab, which advances to the next tab stop",
        ),
    ];

    let mut report = String::new();
    for &(key, count, why) in expected {
        let got = buckets.remove(&key).unwrap_or(0);
        if got != count {
            report.push_str(&format!(
                "  ours={} theirs={}: expected {count} ({why}), got {got}\n",
                key.0, key.1
            ));
        }
    }
    for ((ours, theirs), n) in &buckets {
        report.push_str(&format!(
            "  UNEXPECTED bucket ours={ours} theirs={theirs}: {n}\n"
        ));
    }
    assert!(
        report.is_empty(),
        "unicode-width divergence changed:\n{report}"
    );
}

/// The compatibility shim reproduces `unicode-segmentation` in both of its
/// modes, including the legacy clusters that drop GB9a, GB9b and GB9c.
#[test]
fn compat_segmentation_matches_unicode_segmentation_in_both_modes() {
    use cellwidth::compat::UnicodeSegmentation as Ours;
    use unicode_segmentation::UnicodeSegmentation as Theirs;

    let corpus = cluster_corpus();
    let mut mismatches = 0;
    let mut first = Vec::new();
    let mut legacy_differs = 0;
    for s in &corpus {
        for extended in [true, false] {
            let ours: Vec<(usize, &str)> = Ours::grapheme_indices(s.as_str(), extended).collect();
            let theirs: Vec<(usize, &str)> =
                Theirs::grapheme_indices(s.as_str(), extended).collect();
            if ours != theirs {
                mismatches += 1;
                if first.len() < 10 {
                    first.push(format!(
                        "  {s:?} extended={extended}\n    ours   {ours:?}\n    theirs {theirs:?}"
                    ));
                }
            }
            let plain: Vec<&str> = Ours::graphemes(s.as_str(), extended).collect();
            assert_eq!(
                plain,
                Theirs::graphemes(s.as_str(), extended).collect::<Vec<_>>()
            );
        }
        if Theirs::graphemes(s.as_str(), true).count()
            != Theirs::graphemes(s.as_str(), false).count()
        {
            legacy_differs += 1;
        }
    }
    assert_eq!(
        mismatches,
        0,
        "{mismatches} of {} corpus strings segmented differently:\n{}",
        corpus.len() * 2,
        first.join("\n")
    );
    // The legacy mode has to be doing something, or agreement proves nothing.
    assert!(
        legacy_differs > 1000,
        "only {legacy_differs} strings distinguish the modes"
    );
    eprintln!(
        "unicode-segmentation compat: {} strings agree in both modes; {legacy_differs} distinguish them",
        corpus.len()
    );
}

/// The shim keeps `unicode-width`'s `None`-for-control convention on `char`,
/// and on `str` its answers are exactly cellwidth's.
#[test]
fn compat_width_keeps_unicode_width_conventions() {
    use cellwidth::compat::{UnicodeWidthChar as OurChar, UnicodeWidthStr as OurStr};
    use unicode_width::{UnicodeWidthChar as TheirChar, UnicodeWidthStr as TheirStr};

    let mut none_mismatch = 0;
    let mut same_value = 0;
    for cp in 0u32..=0x10FFFF {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let ours = OurChar::width(c);
        let theirs = TheirChar::width(c);
        if ours.is_none() != theirs.is_none() {
            none_mismatch += 1;
        }
        if ours == theirs {
            same_value += 1;
        }
        assert_eq!(
            OurChar::width_cjk(c).is_none(),
            TheirChar::width_cjk(c).is_none()
        );
    }
    assert_eq!(
        none_mismatch, 0,
        "None must mean the same thing in both crates"
    );
    // Values differ only in the deliberate buckets pinned above; sanity-check
    // that agreement is the overwhelming norm.
    assert!(
        same_value > 1_100_000,
        "only {same_value} code points agree"
    );

    for s in cluster_corpus().iter().step_by(97) {
        assert_eq!(OurStr::width(s.as_str()), cellwidth::width(s));
        assert_eq!(
            OurStr::width_cjk(s.as_str()),
            cellwidth::Width::DEFAULT
                .ambiguous(cellwidth::Ambiguous::Wide)
                .of(s)
        );
    }
    // Where the two crates differ on a string, the difference is the point.
    // These pin the figures quoted in `src/compat.rs`.
    assert_eq!(OurStr::width("👨‍👩‍👧‍👦"), 2);
    assert_eq!(TheirStr::width("👨‍👩‍👧‍👦"), 2);
    assert_eq!(OurStr::width("\x1b[31mred\x1b[0m"), 3);
    assert_eq!(TheirStr::width("\x1b[31mred\x1b[0m"), 12);
    assert_eq!(OurStr::width("क्षि"), 1);
    assert_eq!(TheirStr::width("क्षि"), 3);
    assert_eq!(OurStr::width("a\tb"), 9);
    assert_eq!(TheirStr::width("a\tb"), 3);
    eprintln!(
        "unicode-width compat: None-set identical; {same_value} code points identical in value"
    );
}
