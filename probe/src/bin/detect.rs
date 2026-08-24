//! Ask the terminal you are actually running in which width model it uses.
//!
//!     cargo run -p cellwidth-probe --bin detect
//!
//! The recordings in `results/` show that terminals fall into two camps, and no
//! table can tell them apart. This can: print a probe string, ask where the
//! cursor ended up, and pick the preset that matches. A CLI can do the same at
//! startup in about forty lines.

use std::fs::OpenOptions;
use std::io::{Read, Write};
use std::process::{Command, Stdio};

fn stty(args: &[&str]) {
    let _ = Command::new("stty")
        .arg("-F")
        .arg("/dev/tty")
        .args(args)
        .stdin(Stdio::null())
        .status();
}

/// Draw `text` at column 0 and report how many columns the cursor moved.
fn measure(tty: &mut std::fs::File, text: &str) -> Option<usize> {
    let _ = tty.write_all(b"\r\x1b[2K");
    let _ = tty.write_all(text.as_bytes());
    let _ = tty.write_all(b"\x1b[6n");
    let _ = tty.flush();
    let mut buf = Vec::new();
    let mut b = [0u8; 1];
    loop {
        match tty.read(&mut b) {
            Ok(0) | Err(_) => return None,
            Ok(_) => {
                buf.push(b[0]);
                if b[0] == b'R' || buf.len() > 64 {
                    break;
                }
            }
        }
    }
    let s = String::from_utf8_lossy(&buf);
    let (_, col) = s
        .trim_start_matches(['\x1b', '['])
        .trim_end_matches('R')
        .split_once(';')?;
    col.trim()
        .parse::<usize>()
        .ok()
        .map(|c| c.saturating_sub(1))
}

fn main() {
    let Ok(mut tty) = OpenOptions::new().read(true).write(true).open("/dev/tty") else {
        eprintln!("detect: not attached to a terminal");
        std::process::exit(2);
    };
    stty(&["raw", "-echo", "min", "0", "time", "10"]);

    let sanity = measure(&mut tty, "abc");
    let family = measure(
        &mut tty,
        "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
    );
    let vs16 = measure(&mut tty, "\u{2764}\u{FE0F}");
    let bengali = measure(&mut tty, "\u{0995}\u{09BE}");

    let _ = tty.write_all(b"\r\x1b[2K");
    let _ = tty.flush();
    stty(&["sane"]);

    if sanity != Some(3) {
        println!("detect: the terminal did not answer a cursor report ({sanity:?}).");
        println!("        Fall back to Width::DEFAULT.");
        return;
    }

    println!("probe results for this terminal:");
    println!("  ZWJ family emoji : {}", fmt(family));
    println!("  heart + VS16     : {}", fmt(vs16));
    println!("  Bengali KA + AA  : {}", fmt(bengali));

    let recommend = match (family, vs16) {
        (Some(2), _) => "Width::DEFAULT      // grapheme clusters are one glyph",
        (Some(n), _) if n > 2 => "Width::LEGACY       // every code point counts on its own",
        _ => "Width::DEFAULT      // could not tell; the default is the safer guess",
    };
    println!("\nrecommended: {recommend}");
    if family == Some(2) && bengali == Some(2) {
        println!("note: this terminal collapses emoji but not Indic clusters, so neither");
        println!("      preset matches it exactly. DEFAULT is closer.");
    }
}

fn fmt(v: Option<usize>) -> String {
    v.map_or_else(|| "no answer".into(), |n| format!("{n} columns"))
}
