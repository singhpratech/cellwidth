//! Rough throughput numbers.
//!
//!     cargo run --release --example bench

use std::hint::black_box;
use std::time::Instant;

fn bench(name: &str, input: &str, iters: u32) {
    // Warm up, then time.
    for _ in 0..iters / 10 {
        black_box(cellwidth::width(black_box(input)));
    }
    let t = Instant::now();
    for _ in 0..iters {
        black_box(cellwidth::width(black_box(input)));
    }
    let el = t.elapsed();
    let per = el.as_secs_f64() / f64::from(iters);
    let mb = (input.len() as f64 * f64::from(iters)) / el.as_secs_f64() / 1e6;
    println!(
        "  {name:<28} {:>8.0} ns/call  {:>7.0} MB/s  ({} bytes, width {})",
        per * 1e9,
        mb,
        input.len(),
        cellwidth::width(input)
    );
}

fn main() {
    let ascii = "the quick brown fox jumps over the lazy dog, repeatedly and at length";
    let cjk = "日本語のテキストはこのように全角文字で構成されていますので幅の計算が必要です";
    let emoji = "👨‍👩‍👧‍👦 crew 🇯🇵 flag 👍🏽 thumbs 🦊 fox ❤️ heart 🚀 rocket 🎉 party 🌍 world 🔥 fire";
    let ansi = "\x1b[1;31mred\x1b[0m \x1b[32mgreen\x1b[0m \x1b[34mblue\x1b[0m \x1b[33myellow\x1b[0m plain tail text here";
    let mixed = "host-01 │ 日本語ログ │ \x1b[32mhealthy\x1b[0m │ 👍🏽 ok";

    println!("width():");
    bench("ascii", ascii, 200_000);
    bench("cjk", cjk, 200_000);
    bench("emoji", emoji, 200_000);
    bench("ansi", ansi, 200_000);
    bench("mixed table row", mixed, 200_000);

    let iters = 200_000u32;
    let t = Instant::now();
    for _ in 0..iters {
        black_box(cellwidth::cell(black_box(mixed), 40));
    }
    println!(
        "\ncell() on a table row:       {:>8.0} ns/call (allocates)",
        t.elapsed().as_secs_f64() / f64::from(iters) * 1e9
    );

    let t = Instant::now();
    for _ in 0..iters {
        black_box(cellwidth::truncate(black_box(mixed), 20));
    }
    println!(
        "truncate() on a table row:   {:>8.0} ns/call (no allocation)",
        t.elapsed().as_secs_f64() / f64::from(iters) * 1e9
    );
}
