//! Ask the terminal we are running inside how wide it actually drew a string.
//!
//! For each case: park the cursor at column 0, print the text, send `CSI 6n`
//! (Device Status Report) and read back `CSI row;col R`. The column the cursor
//! ended up in is the width the terminal really used -- not what any table says
//! it should be.
//!
//!     cargo run -p cellwidth-probe --bin probe -- results/kitty.tsv
//!
//! Must be run attached to a terminal. Writes TSV to the given path, because
//! stdout belongs to the terminal under test.

use std::fs::{File, OpenOptions};
use std::io::{Read, Write};
use std::process::{Command, Stdio};

use cellwidth_probe::{codepoints, CASES};

/// Put the tty into raw mode with a read timeout, so a terminal that never
/// answers a DSR costs us two seconds rather than hanging forever.
fn stty(args: &[&str]) -> bool {
    Command::new("stty")
        .arg("-F")
        .arg("/dev/tty")
        .args(args)
        .stdin(Stdio::null())
        .status()
        .map(|s| s.success())
        .unwrap_or(false)
}

/// Read the reply to a DSR query: bytes up to and including the final `R`.
fn read_report(tty: &mut File) -> Option<(usize, usize)> {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    loop {
        match tty.read(&mut byte) {
            Ok(0) => return None, // timed out
            Ok(_) => {
                buf.push(byte[0]);
                if byte[0] == b'R' {
                    break;
                }
                if buf.len() > 64 {
                    return None;
                }
            }
            Err(_) => return None,
        }
    }
    // CSI row ; col R
    let s = String::from_utf8_lossy(&buf);
    let body = s.trim_start_matches(['\x1b', '[']).trim_end_matches('R');
    let (row, col) = body.split_once(';')?;
    Some((row.trim().parse().ok()?, col.trim().parse().ok()?))
}

/// Ask the terminal to name itself (XTVERSION). Not every terminal answers.
fn terminal_name(tty: &mut File) -> String {
    let _ = tty.write_all(b"\x1b[>q");
    let _ = tty.flush();
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    // The reply is DCS > | text ST; stop at the ST backslash or on timeout.
    while let Ok(n) = tty.read(&mut byte) {
        if n == 0 {
            break;
        }
        buf.push(byte[0]);
        if byte[0] == b'\\' || buf.len() > 128 {
            break;
        }
    }
    let s = String::from_utf8_lossy(&buf);
    let cleaned: String = s
        .trim_start_matches(['\x1b', 'P', '>', '|'])
        .trim_end_matches(['\x1b', '\\'])
        .chars()
        .filter(|c| !c.is_control())
        .collect();
    if cleaned.is_empty() {
        std::env::var("TERM").unwrap_or_else(|_| "unknown".into())
    } else {
        cleaned
    }
}

fn main() {
    let out_path = std::env::args().nth(1).unwrap_or_else(|| {
        eprintln!("usage: probe <output.tsv> [label]");
        std::process::exit(2);
    });
    // Not every terminal answers XTVERSION; a label can be supplied instead.
    let label = std::env::args().nth(2);

    let mut tty = match OpenOptions::new().read(true).write(true).open("/dev/tty") {
        Ok(f) => f,
        Err(e) => {
            eprintln!("probe: cannot open /dev/tty: {e}");
            std::process::exit(2);
        }
    };

    if !stty(&["raw", "-echo", "min", "0", "time", "20"]) {
        eprintln!("probe: could not set raw mode");
        std::process::exit(2);
    }

    let name = match &label {
        Some(l) => {
            // Still drain any XTVERSION reply so it cannot pollute the first
            // cursor report.
            let _ = terminal_name(&mut tty);
            l.clone()
        }
        None => terminal_name(&mut tty),
    };
    let mut rows = Vec::new();
    let mut failed_sanity = Vec::new();

    for case in CASES {
        // Clear the line and start at column 1 so the reported column is the
        // width and nothing else.
        let _ = tty.write_all(b"\r\x1b[2K");
        let _ = tty.write_all(case.text.as_bytes());
        let _ = tty.write_all(b"\x1b[6n");
        let _ = tty.flush();

        let measured = match read_report(&mut tty) {
            // Columns are 1-based, so the cursor sits one past what was drawn.
            Some((_, col)) if col >= 1 => Some(col - 1),
            _ => None,
        };

        // If the harness cannot measure plain ASCII, nothing else it says is
        // worth reading.
        if case.id == "ascii" && measured != Some(3) {
            failed_sanity.push(format!("ascii measured {measured:?}, expected 3"));
        }

        rows.push((case, measured));
    }

    let _ = tty.write_all(b"\r\x1b[2K");
    let _ = tty.flush();
    stty(&["sane"]);

    if !failed_sanity.is_empty() {
        eprintln!("probe: harness sanity check failed: {failed_sanity:?}");
        std::process::exit(3);
    }

    let mut out = File::create(&out_path).expect("create output");
    writeln!(out, "# terminal\t{name}").unwrap();
    writeln!(out, "# TERM\t{}", std::env::var("TERM").unwrap_or_default()).unwrap();
    writeln!(out, "id\tcodepoints\tterminal\tcellwidth\tnote").unwrap();
    for (case, measured) in rows {
        writeln!(
            out,
            "{}\t{}\t{}\t{}\t{}",
            case.id,
            codepoints(case.text),
            measured
                .map(|w| w.to_string())
                .unwrap_or_else(|| "-".into()),
            cellwidth::width(case.text),
            case.note
        )
        .unwrap();
    }
    eprintln!("probe: wrote {out_path} ({name})");
}
