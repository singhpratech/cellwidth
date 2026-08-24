//! Dev tool: string-level differential against unicode-width, which is the
//! fair comparison since both crates measure by grapheme cluster.
//!     cargo run --release --example diff_strings
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthStr;

/// One cluster per generated string: base character plus plausible modifiers.
fn corpus() -> Vec<String> {
    let mut out = Vec::new();
    // Every code point on its own.
    for cp in 0u32..=0x10FFFF {
        if let Some(c) = char::from_u32(cp) {
            if c.is_control() {
                continue; // measured by policy, not by table
            }
            out.push(c.to_string());
        }
    }
    // Every code point followed by each interesting combining thing.
    let tails = [
        '\u{0301}',
        '\u{200D}',
        '\u{FE0F}',
        '\u{FE0E}',
        '\u{1F3FB}',
        '\u{20E3}',
        '\u{09BE}',
        '\u{0BBE}',
        '\u{094D}',
    ];
    for cp in (0x20u32..=0x10FFFF).step_by(7) {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        if c.is_control() {
            continue;
        }
        for t in tails {
            out.push(format!("{c}{t}"));
        }
    }
    out
}

fn main() {
    let mut buckets: BTreeMap<(usize, usize), (usize, Vec<String>)> = BTreeMap::new();
    let strings = corpus();
    for s in &strings {
        let ours = cellwidth::width(s);
        let theirs = UnicodeWidthStr::width(s.as_str());
        if ours == theirs {
            continue;
        }
        let e = buckets.entry((ours, theirs)).or_insert((0, Vec::new()));
        e.0 += 1;
        if e.1.len() < 5 {
            e.1.push(
                s.chars()
                    .map(|c| format!("U+{:04X}", c as u32))
                    .collect::<Vec<_>>()
                    .join("+"),
            );
        }
    }
    let total: usize = buckets.values().map(|v| v.0).sum();
    println!(
        "string-level: {total} disagreements out of {} strings ({:.4}%)",
        strings.len(),
        100.0 * total as f64 / strings.len() as f64
    );
    for ((ours, theirs), (n, ex)) in &buckets {
        println!("  {n:7}  ours={ours} theirs={theirs}   {}", ex.join("  "));
    }
}
