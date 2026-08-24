//! Dev tool: every fully-qualified emoji sequence should be 2 columns.
use std::collections::BTreeMap;
fn main() {
    let data = include_str!("../tests/data/emoji-test.txt");
    let mut bad: BTreeMap<usize, (usize, Vec<String>)> = BTreeMap::new();
    let mut total = 0;
    for line in data.lines() {
        let Some((cps, rest)) = line.split_once(';') else {
            continue;
        };
        if !rest.trim_start().starts_with("fully-qualified") {
            continue;
        }
        let s: String = cps
            .split_whitespace()
            .filter_map(|h| u32::from_str_radix(h, 16).ok())
            .filter_map(char::from_u32)
            .collect();
        total += 1;
        let w = cellwidth::width(&s);
        if w != 2 {
            let e = bad.entry(w).or_insert((0, Vec::new()));
            e.0 += 1;
            if e.1.len() < 8 {
                e.1.push(format!("{s} {}", cps.trim()));
            }
        }
    }
    println!("fully-qualified emoji sequences: {total}");
    println!(
        "not 2 columns wide: {}",
        bad.values().map(|v| v.0).sum::<usize>()
    );
    for (w, (n, ex)) in &bad {
        println!("\n  width {w}: {n} sequences");
        for e in ex {
            println!("      {e}");
        }
    }
}
