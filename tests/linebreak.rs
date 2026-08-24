//! Conformance against Unicode's own line breaking test suite (UAX #14).

use cellwidth::{line_breaks, Break};

/// Parse a test line into the input string and the set of break offsets.
fn parse(line: &str) -> Option<(String, Vec<usize>)> {
    let body = line.split('#').next()?.trim();
    if body.is_empty() {
        return None;
    }
    let mut input = String::new();
    let mut breaks = Vec::new();
    for token in body.split_whitespace() {
        match token {
            "\u{00F7}" => breaks.push(input.len()),
            "\u{00D7}" => {}
            hex => input.push(char::from_u32(u32::from_str_radix(hex, 16).ok()?)?),
        }
    }
    // The leading × or ÷ at offset 0 is rule LB2 and is never a break we emit.
    breaks.retain(|&b| b != 0);
    Some((input, breaks))
}

#[test]
fn line_break_test() {
    let data = include_str!("data/LineBreakTest.txt");
    let mut checked = 0;
    let mut failures = Vec::new();

    for (n, line) in data.lines().enumerate() {
        let Some((input, expected)) = parse(line) else {
            continue;
        };
        checked += 1;
        let got: Vec<usize> = line_breaks(&input).map(|(at, _)| at).collect();
        if got != expected {
            if failures.len() < 25 {
                let cps: Vec<String> = input.chars().map(|c| format!("{:04X}", c as u32)).collect();
                failures.push(format!(
                    "line {}: {}\n    expected breaks at {expected:?}\n    got           {got:?}",
                    n + 1,
                    cps.join(" ")
                ));
            } else {
                failures.push(String::new());
            }
        }
    }

    assert!(checked > 19_000, "only {checked} cases parsed");
    let n = failures.len();
    assert!(
        failures.is_empty(),
        "{n}/{checked} UAX #14 cases failed ({:.2}%):\n\n{}",
        100.0 * n as f64 / checked as f64,
        failures
            .iter()
            .filter(|f| !f.is_empty())
            .cloned()
            .collect::<Vec<_>>()
            .join("\n\n")
    );
    eprintln!("UAX #14: {checked} cases passed");
}

/// Mandatory breaks are the ones the text itself demands.
#[test]
fn mandatory_versus_allowed() {
    let b: Vec<_> = line_breaks("a\nb").collect();
    assert_eq!(b, [(2, Break::Mandatory), (3, Break::Mandatory)]);
    let b: Vec<_> = line_breaks("a b").collect();
    assert_eq!(b, [(2, Break::Allowed), (3, Break::Mandatory)]);
}
