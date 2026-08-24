//! Head-to-head throughput against the incumbent crates.
//!
//!     cargo run --release --manifest-path oracles/Cargo.toml --example bench
//!
//! Each cell is the best of seven rounds of 100,000 calls. The comparison is
//! not entirely fair to cellwidth: `unicode-width` does not segment into
//! clusters and does not parse escape sequences, so on emoji and ANSI input
//! it is doing strictly less work and returning a different (wrong for a
//! terminal) answer. `Width::LEGACY` is the like-for-like per-code-point
//! model. Numbers are printed as a Markdown table for the README.

use std::hint::black_box;
use std::time::Instant;

use cellwidth::Width;
use unicode_segmentation::UnicodeSegmentation;
use unicode_width::UnicodeWidthStr;

const ITERS: u32 = 100_000;
const ROUNDS: u32 = 7;

fn best_ns<F: FnMut() -> usize>(mut f: F) -> f64 {
    let mut best = f64::MAX;
    for _ in 0..ROUNDS {
        let t = Instant::now();
        for _ in 0..ITERS {
            black_box(f());
        }
        best = best.min(t.elapsed().as_secs_f64() / f64::from(ITERS) * 1e9);
    }
    best
}

fn main() {
    let inputs: [(&str, &str); 5] = [
        (
            "ascii",
            "the quick brown fox jumps over the lazy dog, repeatedly and at length",
        ),
        (
            "cjk",
            "日本語のテキストはこのように全角文字で構成されていますので幅の計算が必要です",
        ),
        (
            "emoji",
            "👨‍👩‍👧‍👦 crew 🇯🇵 flag 👍🏽 thumbs 🦊 fox ❤️ heart 🚀 rocket 🎉 party 🌍 world 🔥 fire",
        ),
        (
            "ansi",
            "\x1b[1;31mred\x1b[0m \x1b[32mgreen\x1b[0m \x1b[34mblue\x1b[0m \x1b[33myellow\x1b[0m plain tail text here",
        ),
        ("table row", "host-01 │ 日本語ログ │ \x1b[32mhealthy\x1b[0m │ 👍🏽 ok"),
    ];

    println!("width, ns per call (bytes in parentheses; best of {ROUNDS} × {ITERS})\n");
    println!(
        "| input | bytes | `cellwidth::width` | `Width::LEGACY` | `unicode-width` | answers |"
    );
    println!("|---|---:|---:|---:|---:|---|");
    for (name, s) in inputs {
        let ours = best_ns(|| cellwidth::width(black_box(s)));
        let legacy = best_ns(|| Width::LEGACY.of(black_box(s)));
        let theirs = best_ns(|| UnicodeWidthStr::width(black_box(s)));
        let answers = format!(
            "{} / {} / {}",
            cellwidth::width(s),
            Width::LEGACY.of(s),
            UnicodeWidthStr::width(s)
        );
        println!(
            "| {name} | {} | {ours:.0} ns | {legacy:.0} ns | {theirs:.0} ns | {answers} |",
            s.len()
        );
    }

    println!("\ngrapheme clusters, ns per call to count them\n");
    println!("| input | clusters | `cellwidth::graphemes` | `unicode-segmentation` |");
    println!("|---|---:|---:|---:|");
    for (name, s) in inputs {
        let ours = best_ns(|| cellwidth::graphemes(black_box(s)).count());
        let theirs = best_ns(|| black_box(s).graphemes(true).count());
        let n = cellwidth::graphemes(s).count();
        assert_eq!(n, s.graphemes(true).count());
        println!("| {name} | {n} | {ours:.0} ns | {theirs:.0} ns |");
    }
}
