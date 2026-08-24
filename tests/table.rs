//! The table builder. The invariant that matters is that every rendered line is
//! the same number of terminal columns wide, whatever the cells contain.

use cellwidth::{width, Align, Border, Sizing, Table};

/// Cells chosen to break naive layout: wide, zero-width, ZWJ, flags, colour.
const ROWS: &[[&str; 3]] = &[
    ["nagoya-01", "日本語ログ出力", "1,204"],
    ["berlin-7", "Übermäßig groß", "88"],
    ["crew", "👨‍👩‍👧‍👦 shared", "3"],
    ["tokyo", "🇯🇵 Tōkyō 東京", "17,900"],
    ["mumbai", "क्षि नमस्ते", "412"],
    ["plain", "café au lait", "0"],
    [
        "styled",
        "\x1b[31mred\x1b[0m and \x1b[1;34mblue\x1b[0m",
        "7",
    ],
    ["empty", "", ""],
];

fn table(border: Border) -> Table {
    let mut t = Table::new()
        .column("host")
        .column("label")
        .column_aligned("count", Align::Right)
        .border(border);
    for r in ROWS {
        t = t.row(*r);
    }
    t
}

fn line_widths(out: &str) -> Vec<usize> {
    out.lines().map(width).collect()
}

#[test]
fn every_line_is_the_same_width() {
    for border in [
        Border::Light,
        Border::Heavy,
        Border::Ascii,
        Border::Markdown,
    ] {
        let out = table(border).render(None);
        let w = line_widths(&out);
        assert!(!w.is_empty());
        assert!(
            w.windows(2).all(|p| p[0] == p[1]),
            "{border:?} produced ragged lines {w:?}\n{out}"
        );
    }
}

#[test]
fn narrowing_wraps_instead_of_overflowing() {
    for limit in 24..=70 {
        for border in [Border::Light, Border::Ascii, Border::None] {
            let out = table(border).render(Some(limit));
            let w = line_widths(&out);
            assert!(
                w.windows(2).all(|p| p[0] == p[1]),
                "ragged at limit {limit}, {border:?}:\n{out}"
            );
            assert!(
                w[0] <= limit,
                "limit {limit} exceeded ({}) with {border:?}:\n{out}",
                w[0]
            );
        }
    }
}

#[test]
fn a_limit_wider_than_needed_changes_nothing() {
    let natural = table(Border::Light).render(None);
    let wide = table(Border::Light).render(Some(500));
    assert_eq!(natural, wide);
}

#[test]
fn alignment() {
    let t = Table::new()
        .column_aligned("l", Align::Left)
        .column_aligned("c", Align::Center)
        .column_aligned("r", Align::Right)
        .with_header(false)
        .border(Border::None)
        .row(["a", "b", "c"])
        .row(["xxxxx", "yyyyy", "zzzzz"]);
    let out = t.render(None);
    let first = out.lines().next().unwrap();
    assert_eq!(first, "a        b        c");
}

#[test]
fn fixed_columns_keep_their_width_even_when_squeezed() {
    let t = Table::new()
        .column_with("id", Align::Left, Sizing::Fixed(6))
        .column("text")
        .row(["abcdefghij", "some fairly long label here"]);
    for limit in [20, 30, 60] {
        let out = t.render(Some(limit));
        // The fixed column is exactly six columns of content plus its padding.
        let first_row = out.lines().nth(3).unwrap();
        let cell = first_row.split('│').nth(1).unwrap();
        assert_eq!(width(cell), 8, "at limit {limit}: {first_row:?}");
    }
}

#[test]
fn colour_survives_layout() {
    let out = table(Border::Light).render(None);
    assert!(out.contains("\x1b[31m"), "colour was stripped");
    assert!(out.contains("\x1b[0m"), "reset was stripped");
    // And it still lines up, which is the whole point.
    let w = line_widths(&out);
    assert!(w.windows(2).all(|p| p[0] == p[1]));
}

#[test]
fn degenerate_tables() {
    assert_eq!(Table::new().render(None), "");
    let empty = Table::new().column("only").render(None);
    assert!(!empty.is_empty());
    assert!(line_widths(&empty).windows(2).all(|p| p[0] == p[1]));
    // More cells than columns are dropped; fewer are padded.
    let t = Table::new()
        .column("a")
        .column("b")
        .row(["1", "2", "3"])
        .row(["only"]);
    let out = t.render(None);
    assert!(line_widths(&out).windows(2).all(|p| p[0] == p[1]), "{out}");
}

#[test]
fn padding_changes_the_table_width_predictably() {
    let base = Table::new().column("ab").with_header(true).row(["cd"]);
    let w0 = width(base.clone().padding(0).render(None).lines().next().unwrap());
    let w1 = width(base.clone().padding(1).render(None).lines().next().unwrap());
    let w3 = width(base.padding(3).render(None).lines().next().unwrap());
    // One column: each unit of padding adds two columns, one per side.
    assert_eq!(w1, w0 + 2);
    assert_eq!(w3, w0 + 6);
}

#[test]
fn the_measurement_policy_is_configurable() {
    let family = "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}";
    let modern = Table::new().column("x").row([family]).render(None);
    let legacy = Table::new()
        .column("x")
        .row([family])
        .measured_by(cellwidth::Width::LEGACY)
        .render(None);
    // VTE draws that emoji in eight columns, kitty in two, so the table sized
    // for each is a different width.
    assert!(
        width(legacy.lines().next().unwrap()) > width(modern.lines().next().unwrap()),
        "legacy {legacy}\nmodern {modern}"
    );
    // Each is aligned when measured by the policy it was built for -- and only
    // then, which is the whole reason the policy exists.
    for (out, w) in [
        (&modern, cellwidth::Width::DEFAULT),
        (&legacy, cellwidth::Width::LEGACY),
    ] {
        let widths: Vec<usize> = out.lines().map(|l| w.of(l)).collect();
        assert!(widths.windows(2).all(|p| p[0] == p[1]), "{out}");
    }
}

#[test]
fn tabs_in_cells_are_expanded() {
    // A tab cannot be laid out in a grid: its width depends on where the cell
    // lands. Expanding it against the cell's own origin keeps the row aligned.
    let out = Table::new()
        .column("h")
        .row(["a\tb"])
        .row(["plain"])
        .render(None);
    assert!(!out.contains('\t'), "a raw tab survived: {out:?}");
    // Expanded, not deleted: the tab still separates a from b.
    assert!(
        out.contains("a       b"),
        "tab was dropped instead of expanded: {out}"
    );
    assert!(line_widths(&out).windows(2).all(|p| p[0] == p[1]), "{out}");
}

#[test]
fn control_characters_cannot_wreck_a_row() {
    // A malformed string sequence carrying a newline would split the row.
    // The C1 forms matter as much as the ESC-introduced ones: `\u{9d}` is an
    // OSC introducer whose UTF-8 bytes look like ordinary text.
    // The C1 forms matter as much as the ESC-introduced ones: U+009D is an OSC
    // introducer whose UTF-8 bytes look like ordinary text. And deleting a
    // control character can leave neighbours that form a *new* introducer.
    for cell in [
        "\u{1b}_\n\u{7}",
        "a\u{0}b",
        "x\u{7}y",
        "\u{1b}]0;t\ru\u{7}",
        "\u{9d}\n\u{9c}",
        "\u{9b}\r\u{9c}",
        "\u{90}q\n\u{9c}tail",
        "\u{1b}\u{0}]\n\u{9d}\u{9c}",
    ] {
        let out = Table::new().column("h").row([cell]).render(None);
        assert!(
            line_widths(&out).windows(2).all(|p| p[0] == p[1]),
            "ragged for {cell:?}:\n{out}"
        );
    }
}

#[test]
fn max_sizing_caps_a_column() {
    let long = "an extremely long label that would otherwise dominate";
    let capped = Table::new()
        .column_with("h", Align::Left, Sizing::Max(10))
        .row([long])
        .render(None);
    assert!(width(capped.lines().next().unwrap()) <= 14, "{capped}");
    let uncapped = Table::new().column("h").row([long]).render(None);
    assert!(width(uncapped.lines().next().unwrap()) > 30);
    assert!(line_widths(&capped).windows(2).all(|p| p[0] == p[1]));
}

#[test]
fn a_limit_too_small_to_honour_still_renders() {
    // Every column is already at its floor: the layout gives up rather than
    // looping, and stays internally consistent.
    for limit in 1..=12 {
        let out = table(Border::Light).render(Some(limit));
        let w = line_widths(&out);
        assert!(
            w.windows(2).all(|p| p[0] == p[1]),
            "ragged at impossible limit {limit}:\n{out}"
        );
    }
}

#[test]
fn a_cell_starting_with_a_combining_mark_stays_self_contained() {
    // The mark would otherwise attach to the padding space and cost the row a
    // column. Found by the table fuzzer.
    for cell in [
        "\u{0301}x",
        "\u{AAE9}",
        "\u{093F}a",
        "\u{200D}z",
        "\u{1161}",
    ] {
        let out = Table::new()
            .column("h")
            .row([cell])
            .row(["plain"])
            .render(None);
        let w = line_widths(&out);
        assert!(
            w.windows(2).all(|p| p[0] == p[1]),
            "ragged for {cell:?}:\n{out}"
        );
    }
    // The mark is shown on a dotted circle, Unicode's own convention for an
    // isolated one.
    for cell in ["\u{0301}x", "\u{200D}z", "\u{093F}a"] {
        let out = Table::new().column("h").row([cell]).render(None);
        assert!(
            out.contains('\u{25CC}'),
            "no dotted circle for {cell:?}: {out:?}"
        );
    }
    // Ordinary text is untouched.
    let plain = Table::new().column("h").row(["ok"]).render(None);
    assert!(!plain.contains('\u{25CC}'));
}
