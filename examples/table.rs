//! The problem this crate exists for: a table whose columns line up no matter
//! what is in them.
//!
//!     cargo run --example table

use cellwidth::{Align, Border, Sizing, Table, Width};

fn main() {
    let t = Table::new()
        .column("host")
        .column_with("label", Align::Left, Sizing::Max(22))
        .column_aligned("rps", Align::Right)
        .column_aligned("status", Align::Center)
        .row([
            "nagoya-prod-01",
            "日本語ログ出力",
            "1,204",
            "\x1b[32mhealthy\x1b[0m",
        ])
        .row([
            "berlin-edge-7",
            "Übermäßig groß",
            "88",
            "\x1b[33mdegraded\x1b[0m",
        ])
        .row([
            "crew-alpha",
            "👨‍👩‍👧‍👦 shared account",
            "3",
            "\x1b[32mhealthy\x1b[0m",
        ])
        .row([
            "tokyo-cdn",
            "🇯🇵 Tōkyō 東京 edge",
            "17,900",
            "\x1b[31moffline\x1b[0m",
        ])
        .row(["mumbai-2", "क्षि नमस्ते सर्वर", "412", "\x1b[32mhealthy\x1b[0m"]);

    println!("{}\n", t.render(None));

    println!("Squeezed into 46 columns, cells wrap rather than vanish:");
    println!("{}\n", t.clone().render(Some(46)));

    println!("Markdown, for pasting into an issue:");
    println!("{}\n", t.clone().border(Border::Markdown).render(None));

    println!("Borderless, for piping into something else:");
    println!("{}\n", t.clone().border(Border::None).render(None));

    println!("Line breaking finds the places a reader would accept:");
    for (text, w) in [
        ("The quick brown fox jumps over the lazy dog", 18),
        ("日本語のテキストは空白なしで折り返せます", 12),
        ("total cost 1,200 yen (approximately)", 14),
    ] {
        println!("  ┌{}┐", "─".repeat(w));
        for line in Width::DEFAULT.wrap(text, w) {
            println!("  │{}│", cellwidth::cell(&line, w));
        }
        println!("  └{}┘", "─".repeat(w));
    }
}
