//! Conformance against the official UAX #29 grapheme break test suite.
//!
//! `tests/data/GraphemeBreakTest.txt` ships with the Unicode Character
//! Database. Each line is a sequence of code points separated by `÷` (break)
//! or `×` (no break), which is exactly what `graphemes` has to reproduce.

use cellwidth::graphemes;

/// Parse one test line into the input string and its expected clusters.
fn parse(line: &str) -> Option<(String, Vec<String>)> {
    let body = line.split('#').next()?.trim();
    if body.is_empty() {
        return None;
    }
    let mut input = String::new();
    let mut expected: Vec<String> = Vec::new();
    let mut current = String::new();
    for token in body.split_whitespace() {
        match token {
            "÷" => {
                if !current.is_empty() {
                    expected.push(std::mem::take(&mut current));
                }
            }
            "×" => {}
            hex => {
                let cp = u32::from_str_radix(hex, 16).ok()?;
                let c = char::from_u32(cp)?;
                input.push(c);
                current.push(c);
            }
        }
    }
    if !current.is_empty() {
        expected.push(current);
    }
    Some((input, expected))
}

#[test]
fn grapheme_break_test() {
    let data = include_str!("data/GraphemeBreakTest.txt");
    let mut checked = 0;
    let mut failures = Vec::new();

    for (n, line) in data.lines().enumerate() {
        let Some((input, expected)) = parse(line) else {
            continue;
        };
        checked += 1;
        let got: Vec<String> = graphemes(&input).map(String::from).collect();
        if got != expected {
            let fmt = |v: &Vec<String>| -> String {
                v.iter()
                    .map(|g| {
                        g.chars()
                            .map(|c| format!("{:04X}", c as u32))
                            .collect::<Vec<_>>()
                            .join(" ")
                    })
                    .collect::<Vec<_>>()
                    .join(" ÷ ")
            };
            failures.push(format!(
                "line {}:\n  expected {}\n  got      {}\n  {}",
                n + 1,
                fmt(&expected),
                fmt(&got),
                line.split('#').nth(1).unwrap_or("").trim()
            ));
        }
    }

    assert!(
        checked > 500,
        "only {checked} cases parsed; data file broken?"
    );
    assert!(
        failures.is_empty(),
        "{}/{} UAX #29 cases failed:\n\n{}",
        failures.len(),
        checked,
        failures.join("\n\n")
    );
    eprintln!("UAX #29: {checked} cases passed");
}

/// Splitting and rejoining must be lossless for arbitrary text.
#[test]
fn graphemes_round_trip() {
    let samples = [
        "",
        "plain ascii",
        "e\u{301}cole",
        "🇯🇵🇺🇸🇬🇧",
        "👨‍👩‍👧‍👦👍🏽",
        "क्षि नमस्ते",
        "한국어 조합",
        "\r\n\r\n",
        "a\u{200D}b",
        "\u{1F1E6}",
        "mixed 日本語 with 👩🏾‍🚀 and \u{fe0f}",
    ];
    for s in samples {
        let joined: String = graphemes(s).collect();
        assert_eq!(joined, s, "round trip failed for {s:?}");
        assert!(
            graphemes(s).all(|g| !g.is_empty()),
            "empty cluster in {s:?}"
        );
    }
}
