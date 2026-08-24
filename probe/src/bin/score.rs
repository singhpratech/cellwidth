//! Score each `Width` preset against every recorded terminal.
//!
//!     cargo run -p cellwidth-probe --bin score -- results/*.tsv

use cellwidth::Width;
use cellwidth_probe::CASES;
use std::collections::BTreeMap;

fn main() {
    let presets: [(&str, Width); 3] = [
        ("DEFAULT", Width::DEFAULT),
        ("MODERN", Width::MODERN),
        ("LEGACY", Width::LEGACY),
    ];

    let mut recordings: Vec<(String, BTreeMap<String, usize>)> = Vec::new();
    for path in std::env::args().skip(1) {
        let Ok(text) = std::fs::read_to_string(&path) else {
            continue;
        };
        let mut name = path.clone();
        let mut vals = BTreeMap::new();
        for line in text.lines() {
            if let Some(rest) = line.strip_prefix("# terminal\t") {
                name = rest.trim().into();
            } else if !line.starts_with('#') && !line.starts_with("id\t") {
                let f: Vec<&str> = line.split('\t').collect();
                if f.len() >= 3 {
                    if let Ok(n) = f[2].parse() {
                        vals.insert(f[0].to_string(), n);
                    }
                }
            }
        }
        recordings.push((name, vals));
    }

    println!(
        "agreement with each recorded terminal, out of {} cases\n",
        CASES.len()
    );
    print!("{:<10}", "preset");
    for (name, _) in &recordings {
        print!("{:>16}", short(name));
    }
    println!("{:>10}", "total");
    println!("{}", "-".repeat(10 + 16 * recordings.len() + 10));

    for (pname, w) in presets {
        print!("{:<10}", pname);
        let mut total = 0;
        let mut possible = 0;
        for (_, vals) in &recordings {
            let mut hit = 0;
            let mut n = 0;
            for case in CASES {
                if let Some(&t) = vals.get(case.id) {
                    n += 1;
                    if w.of(case.text) == t {
                        hit += 1;
                    }
                }
            }
            total += hit;
            possible += n;
            print!("{:>16}", format!("{hit}/{n}"));
        }
        println!("{:>10}", format!("{total}/{possible}"));
    }

    println!("\nper-case, which preset reproduces which terminal exactly:");
    for (name, vals) in &recordings {
        let mut exact = Vec::new();
        for (pname, w) in presets {
            let matches_everywhere = CASES
                .iter()
                .all(|c| !matches!(vals.get(c.id), Some(&t) if w.of(c.text) != t));
            if matches_everywhere {
                exact.push(pname);
            }
        }
        let verdict = if exact.is_empty() {
            "none (see the matrix)".to_string()
        } else {
            exact.join(", ")
        };
        println!("  {:<28} {}", short(name), verdict);
    }
}

fn short(s: &str) -> String {
    s.chars().take(15).collect()
}
