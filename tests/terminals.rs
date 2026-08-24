//! Replay what real terminals actually did.
//!
//! `tests/data/terminals/*.tsv` are recordings made by `probe/`, which prints a
//! string into a live terminal, asks for the cursor position with `CSI 6n`, and
//! writes down the column it landed in. Nothing here is an opinion about how
//! wide something ought to be: it is what four terminals measurably drew.
//!
//! Regenerate with `probe/drivers/run_all.sh`.

use cellwidth::Width;

struct Recording {
    terminal: String,
    /// (case id, text, columns the terminal used)
    rows: Vec<(String, String, usize)>,
}

fn parse(tsv: &str) -> Recording {
    let mut terminal = String::from("unknown");
    let mut rows = Vec::new();
    for line in tsv.lines() {
        if let Some(rest) = line.strip_prefix("# terminal\t") {
            terminal = rest.trim().to_string();
            continue;
        }
        if line.starts_with('#') || line.starts_with("id\t") || line.is_empty() {
            continue;
        }
        let f: Vec<&str> = line.split('\t').collect();
        assert!(f.len() >= 3, "malformed row: {line:?}");
        // Rebuild the text from the code point column, so the recordings are
        // self-describing and the test needs no shared case list.
        let text: String = f[1]
            .split_whitespace()
            .map(|h| char::from_u32(u32::from_str_radix(h, 16).expect("hex")).expect("scalar"))
            .collect();
        if let Ok(cols) = f[2].parse::<usize>() {
            rows.push((f[0].to_string(), text, cols));
        }
    }
    rows.sort();
    Recording { terminal, rows }
}

fn recordings() -> Vec<Recording> {
    [
        include_str!("data/terminals/vte.tsv"),
        include_str!("data/terminals/kitty.tsv"),
        include_str!("data/terminals/alacritty.tsv"),
        include_str!("data/terminals/wezterm.tsv"),
    ]
    .iter()
    .map(|t| parse(t))
    .collect()
}

/// The harness must be able to measure plain text, or none of its other
/// numbers mean anything.
#[test]
fn recordings_are_sane() {
    for r in recordings() {
        assert!(
            r.rows.len() >= 30,
            "{}: only {} rows",
            r.terminal,
            r.rows.len()
        );
        for (id, expect) in [("ascii", 3), ("cjk", 6), ("accent-precomp", 4)] {
            let (_, _, got) = r
                .rows
                .iter()
                .find(|(i, _, _)| i == id)
                .unwrap_or_else(|| panic!("{}: missing case {id}", r.terminal));
            assert_eq!(*got, expect, "{}: {id} measured {got}", r.terminal);
        }
    }
}

/// `Width::DEFAULT` is not a guess: it reproduces kitty exactly.
#[test]
fn default_reproduces_kitty_exactly() {
    let kitty = parse(include_str!("data/terminals/kitty.tsv"));
    let mut wrong = Vec::new();
    for (id, text, cols) in &kitty.rows {
        let ours = Width::DEFAULT.of(text);
        if ours != *cols {
            wrong.push(format!("  {id}: kitty drew {cols}, cellwidth says {ours}"));
        }
    }
    assert!(
        wrong.is_empty(),
        "{} diverged from {}:\n{}",
        wrong.len(),
        kitty.terminal,
        wrong.join("\n")
    );
}

/// Agreement with each recorded terminal, pinned. A change here is not
/// necessarily a bug, but it is always a decision that needs justifying: it
/// means cellwidth now matches real terminals differently than when these
/// numbers were measured.
#[test]
fn agreement_with_real_terminals_is_pinned() {
    // (terminal, DEFAULT hits, LEGACY hits, of how many)
    let expected = [
        ("VTE(7600)", 17, 29, 32),
        ("kitty(0.48.2)", 32, 20, 32),
        ("alacritty 0.17.0", 21, 27, 32),
        ("WezTerm 20240203-110809-5046fc22", 24, 23, 32),
    ];

    let mut report = String::new();
    for r in recordings() {
        let hits = |w: Width| r.rows.iter().filter(|(_, t, c)| w.of(t) == *c).count();
        let (d, l, n) = (hits(Width::DEFAULT), hits(Width::LEGACY), r.rows.len());
        match expected.iter().find(|(name, ..)| *name == r.terminal) {
            Some(&(_, ed, el, en)) if (d, l, n) == (ed, el, en) => {}
            Some(&(name, ed, el, en)) => report.push_str(&format!(
                "  {name}: expected DEFAULT {ed}/{en} LEGACY {el}/{en}, got DEFAULT {d}/{n} LEGACY {l}/{n}\n"
            )),
            None => report.push_str(&format!("  unrecognised recording: {}\n", r.terminal)),
        }
    }
    assert!(report.is_empty(), "terminal agreement changed:\n{report}");
}

/// Whatever a terminal does, cellwidth should be able to say it. A case that
/// no preset can reproduce is a gap in the model, not a quirk.
#[test]
fn every_recorded_measurement_is_reachable_by_some_preset() {
    let presets = [
        ("DEFAULT", Width::DEFAULT),
        ("LEGACY", Width::LEGACY),
        (
            "DEFAULT+ambiguous-wide",
            Width::DEFAULT.ambiguous(cellwidth::Ambiguous::Wide),
        ),
        (
            "LEGACY+ambiguous-wide",
            Width::LEGACY.ambiguous(cellwidth::Ambiguous::Wide),
        ),
    ];
    let mut unreachable = Vec::new();
    for r in recordings() {
        for (id, text, cols) in &r.rows {
            if !presets.iter().any(|(_, w)| w.of(text) == *cols) {
                let opts: Vec<String> = presets
                    .iter()
                    .map(|(n, w)| format!("{n}={}", w.of(text)))
                    .collect();
                unreachable.push(format!(
                    "  {:<20} {id:<20} drew {cols}, presets gave {}",
                    r.terminal,
                    opts.join(" ")
                ));
            }
        }
    }
    // Known and understood: three cases where a terminal's own Unicode data or
    // rendering differs from anything a width table can express.
    let allowed = [
        "trigram",
        "counting-rod",
        "soft-hyphen",
        "hangul-filler",
        "arabic-number-sign",
        "devanagari-ksi",
    ];
    let real: Vec<&String> = unreachable
        .iter()
        .filter(|l| !allowed.iter().any(|a| l.contains(a)))
        .collect();
    assert!(
        real.is_empty(),
        "{} recorded measurements no preset can produce:\n{}",
        real.len(),
        real.iter()
            .map(|s| s.as_str())
            .collect::<Vec<_>>()
            .join("\n")
    );
}
