//! Dev tool: search for a small table input that renders ragged, then shrink it.
fn ragged(cells: &[String], limit: Option<usize>) -> bool {
    let mut t = cellwidth::Table::new().column("c0");
    for c in cells {
        t = t.row([c.as_str()]);
    }
    let out = t.render(limit);
    let w: Vec<usize> = out.lines().map(cellwidth::width).collect();
    !w.windows(2).all(|p| p[0] == p[1])
}

fn main() {
    const ALPHA: &[char] = &[
        'a', ' ', '\t', '\n', '\r', '\u{b}', '\u{c}', '\u{85}', '\u{2028}', '\u{1b}', '\u{7}',
        '\0', '\u{65e5}', '\u{947}', '-', '(', ')', '_', '/', '1', ',', ']', '[', '\\', 'P', '#',
        ';', 'z', '\u{9b}', '\u{9d}', '\u{9c}',
    ];
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    let mut rng = move || {
        seed ^= seed << 13;
        seed ^= seed >> 7;
        seed ^= seed << 17;
        seed
    };
    for _ in 0..300_000 {
        let n = (rng() % 16 + 1) as usize;
        let cell: String = (0..n)
            .map(|_| ALPHA[(rng() % ALPHA.len() as u64) as usize])
            .collect();
        let cells = vec![cell];
        for limit in [None, Some(8), Some(12), Some(20)] {
            if !ragged(&cells, limit) {
                continue;
            }
            let mut cur: Vec<char> = cells[0].chars().collect();
            let mut changed = true;
            while changed {
                changed = false;
                for i in 0..cur.len() {
                    let mut t = cur.clone();
                    t.remove(i);
                    if ragged(&[t.iter().collect()], limit) {
                        cur = t;
                        changed = true;
                        break;
                    }
                }
            }
            let m: String = cur.iter().collect();
            println!(
                "limit {limit:?}, cell {m:?} codepoints {:?}",
                m.chars().map(|c| c as u32).collect::<Vec<_>>()
            );
            println!("  wrap(cell, 4) = {:?}", cellwidth::wrap(&m, 4));
            let mut tb = cellwidth::Table::new().column("c0");
            tb = tb.row([m.as_str()]);
            println!("{}", tb.render(limit));
            return;
        }
    }
    println!("no failure found");
}
