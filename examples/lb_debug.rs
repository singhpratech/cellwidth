//! Dev tool: group UAX #14 failures by the rule Unicode applied.
//!     cargo run --release --example lb_debug -- /path/to/annotated/LineBreakTest.txt
use std::collections::BTreeMap;

fn main() {
    let path = std::env::args()
        .nth(1)
        .expect("path to annotated test file");
    let data = std::fs::read_to_string(path).expect("read");
    let mut by_rule: BTreeMap<String, (usize, Vec<String>)> = BTreeMap::new();
    let mut total = 0;
    let mut failed = 0;

    for line in data.lines() {
        let Some((body, comment)) = line.split_once('#') else {
            continue;
        };
        let body = body.trim();
        if body.is_empty() {
            continue;
        }
        // Rebuild the string and the expected break offsets, remembering which
        // rule the annotation credits at each decision point.
        let mut input = String::new();
        let mut expected = Vec::new();
        let mut ok = true;
        for tok in body.split_whitespace() {
            match tok {
                "\u{F7}" => expected.push(input.len()),
                "\u{D7}" => {}
                hex => match u32::from_str_radix(hex, 16).ok().and_then(char::from_u32) {
                    Some(c) => input.push(c),
                    None => ok = false,
                },
            }
        }
        if !ok {
            continue;
        }
        expected.retain(|&b| b != 0);
        total += 1;
        let got: Vec<usize> = cellwidth::line_breaks(&input).map(|(a, _)| a).collect();
        if got == expected {
            continue;
        }
        failed += 1;

        // The annotation lists rules in order: [x.y] before each decision.
        let rules: Vec<&str> = comment
            .split('[')
            .skip(1)
            .filter_map(|s| s.split(']').next())
            .collect();
        // Blame the first offset where we disagree.
        let mut idx = 0usize;
        let mut off = 0usize;
        let mut blame = "?".to_string();
        for (i, c) in input.char_indices() {
            if i == 0 {
                continue;
            }
            let want = expected.contains(&i);
            let have = got.contains(&i);
            idx += 1;
            off = i;
            if want != have {
                blame = format!(
                    "{}  ({})",
                    rules.get(idx).copied().unwrap_or("?"),
                    if want {
                        "expected BREAK"
                    } else {
                        "expected NO break"
                    }
                );
                break;
            }
            let _ = c;
        }
        let e = by_rule.entry(blame).or_insert((0, Vec::new()));
        e.0 += 1;
        if e.1.len() < 2 {
            e.1.push(format!(
                "    at byte {off}: {}",
                input
                    .chars()
                    .map(|c| format!("{:04X} ", c as u32))
                    .collect::<String>()
            ));
        }
    }

    println!("{failed}/{total} failing\n");
    let mut v: Vec<_> = by_rule.into_iter().collect();
    v.sort_by_key(|(_, (n, _))| std::cmp::Reverse(*n));
    for (rule, (n, ex)) in v {
        println!("  {n:5}  rule {rule}");
        for e in ex {
            println!("{e}");
        }
    }
}
