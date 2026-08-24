//! Whatever the cells contain and however narrow the terminal, every line of a
//! rendered table must be the same number of columns wide.
#![no_main]
use libfuzzer_sys::fuzz_target;

fuzz_target!(|data: (Vec<Vec<String>>, u8, u8, u8)| {
    let (rows, ncols, limit, style) = data;
    let ncols = (ncols % 5) as usize + 1;
    if rows.len() > 12 {
        return;
    }
    let border = match style % 5 {
        0 => cellwidth::Border::Light,
        1 => cellwidth::Border::Heavy,
        2 => cellwidth::Border::Ascii,
        3 => cellwidth::Border::Markdown,
        _ => cellwidth::Border::None,
    };
    let mut t = cellwidth::Table::new().border(border);
    for i in 0..ncols {
        t = t.column(format!("c{i}"));
    }
    for r in rows {
        if r.iter().any(|c| c.len() > 200) {
            return;
        }
        t = t.row(r);
    }
    let limit = if limit == 0 { None } else { Some(limit as usize) };
    let out = t.render(limit);
    let widths: Vec<usize> = out.lines().map(cellwidth::width).collect();
    if let Some(&first) = widths.first() {
        assert!(
            widths.iter().all(|&w| w == first),
            "ragged table (limit {limit:?}, {border:?}): {widths:?}\n{out}"
        );
    }
});
