//! Dev tool: search for a small `wrap` input that overflows its budget, then
//! shrink it by deleting characters while the failure survives.
fn bad(s: &str, w: usize) -> bool {
    cellwidth::wrap(s, w).iter().any(|line| {
        cellwidth::width(line) > w && {
            let plain = cellwidth::strip_ansi(line);
            cellwidth::graphemes(&plain)
                .filter(|g| cellwidth::width(g) > 0)
                .count()
                > 1
        }
    })
}

fn main() {
    const ALPHA: &[char] = &[
        'a', ' ', '\t', '\n', '\u{1b}', '\u{7}', '\0', '\u{65e5}', '\u{947}', ';', ']', 't', '_',
        '^', '+', '&', '/', ':', '$', '4', 'E', '(', ')', '-',
    ];
    let mut seed = 0x2545_F491_4F6C_DD1Du64;
    let mut rng = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };

    for _ in 0..120_000 {
        let n = (rng() % 140 + 2) as usize;
        let s: String = (0..n)
            .map(|_| ALPHA[(rng() % ALPHA.len() as u64) as usize])
            .collect();
        for w in 1..=40usize {
            if !bad(&s, w) {
                continue;
            }
            // Shrink: drop any character that is not needed to keep it failing.
            let mut cur: Vec<char> = s.chars().collect();
            let mut changed = true;
            while changed {
                changed = false;
                for i in 0..cur.len() {
                    let mut t = cur.clone();
                    t.remove(i);
                    let joined: String = t.iter().collect();
                    if bad(&joined, w) {
                        cur = t;
                        changed = true;
                        break;
                    }
                }
            }
            let m: String = cur.iter().collect();
            println!("width {w}, input {m:?}");
            println!(
                "  codepoints: {:?}",
                m.chars().map(|c| c as u32).collect::<Vec<_>>()
            );
            println!("  lines: {:?}", cellwidth::wrap(&m, w));
            for line in cellwidth::wrap(&m, w) {
                println!("    {:?} = {} columns", line, cellwidth::width(&line));
            }
            return;
        }
    }
    println!("no failure found");
}
