//! Build the matrix: what each real terminal did, next to what cellwidth says.
//!
//!     cargo run -p cellwidth-probe --bin compare -- results/*.tsv

use std::collections::BTreeMap;

use cellwidth_probe::{codepoints, CASES};

fn main() {
    let paths: Vec<String> = std::env::args().skip(1).collect();
    if paths.is_empty() {
        eprintln!("usage: compare <results.tsv>...");
        std::process::exit(2);
    }

    // terminal name -> case id -> measured columns
    let mut terms: Vec<(String, BTreeMap<String, String>)> = Vec::new();
    for path in &paths {
        let text = match std::fs::read_to_string(path) {
            Ok(t) => t,
            Err(e) => {
                eprintln!("skipping {path}: {e}");
                continue;
            }
        };
        let mut name = path.clone();
        let mut vals = BTreeMap::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("# terminal\t") {
                name = rest.trim().to_string();
                continue;
            }
            if line.starts_with('#') || line.starts_with("id\t") {
                continue;
            }
            let f: Vec<&str> = line.split('\t').collect();
            if f.len() >= 3 {
                vals.insert(f[0].to_string(), f[2].to_string());
            }
        }
        terms.push((name, vals));
    }

    let w_id = CASES.iter().map(|c| c.id.len()).max().unwrap_or(10).max(4);
    print!("{:<w$}  {:>3}", "case", "cw", w = w_id);
    for (name, _) in &terms {
        print!("  {:>10}", truncate(name, 10));
    }
    println!("   note");
    println!("{}", "-".repeat(w_id + 7 + 12 * terms.len() + 20));

    let mut disagreements = 0;
    let mut unanimous_against = Vec::new();

    for case in CASES {
        let ours = cellwidth::width(case.text);
        print!("{:<w$}  {:>3}", case.id, ours, w = w_id);
        let mut seen = Vec::new();
        for (_, vals) in &terms {
            let v = vals.get(case.id).cloned().unwrap_or_else(|| "?".into());
            let flag = match v.parse::<usize>() {
                Ok(n) if n == ours => " ",
                Ok(_) => "*",
                Err(_) => " ",
            };
            print!("  {:>9}{}", v, flag);
            if let Ok(n) = v.parse::<usize>() {
                seen.push(n);
            }
        }
        println!("   {}", case.note);

        if !seen.is_empty() {
            if seen.iter().any(|&n| n != ours) {
                disagreements += 1;
            }
            if seen.iter().all(|&n| n != ours) && seen.len() == terms.len() {
                unanimous_against.push((case.id, ours, seen[0], codepoints(case.text)));
            }
        }
    }

    println!();
    println!("cases where at least one terminal disagrees: {disagreements}");
    if unanimous_against.is_empty() {
        println!("cases where EVERY terminal disagrees with cellwidth: none");
    } else {
        println!(
            "cases where EVERY terminal disagrees with cellwidth: {}",
            unanimous_against.len()
        );
        for (id, ours, theirs, cps) in &unanimous_against {
            println!("  {id:<22} cellwidth={ours}  terminals={theirs}   {cps}");
        }
        println!("\nThese are the ones worth changing. The rest are genuine");
        println!("terminal-to-terminal disagreement and belong in a policy.");
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.chars().count() <= n {
        s.to_string()
    } else {
        s.chars().take(n - 1).chain("…".chars()).collect()
    }
}
