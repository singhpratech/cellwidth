//! Dev tool: classify every disagreement with the incumbent crates.
//!     cargo run --release --example diff_oracles
use std::collections::BTreeMap;
use unicode_width::UnicodeWidthChar;

fn main() {
    let mut buckets: BTreeMap<(usize, i32), (usize, Vec<u32>)> = BTreeMap::new();
    for cp in 0u32..=0x10FFFF {
        let Some(c) = char::from_u32(cp) else {
            continue;
        };
        let ours = cellwidth::char_width(c);
        let theirs = match UnicodeWidthChar::width(c) {
            Some(w) => w as i32,
            None => -1,
        };
        if ours as i32 == theirs {
            continue;
        }
        let e = buckets.entry((ours, theirs)).or_insert((0, Vec::new()));
        e.0 += 1;
        if e.1.len() < 6 {
            e.1.push(cp);
        }
    }
    let total: usize = buckets.values().map(|v| v.0).sum();
    println!(
        "char_width vs unicode-width {}: {total} disagreements",
        env!("CARGO_PKG_VERSION")
    );
    for ((ours, theirs), (n, ex)) in &buckets {
        let names: Vec<String> = ex.iter().map(|c| format!("U+{c:04X}")).collect();
        println!("  {n:7}  ours={ours} theirs={theirs}   {}", names.join(" "));
    }
}
