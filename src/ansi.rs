//! Recognising ANSI/VT escape sequences so they can be skipped when measuring
//! and kept intact when cutting.
//!
//! Escapes occupy zero columns but plenty of bytes. Code that measures or
//! truncates without accounting for them prints misaligned tables at best, and
//! at worst slices a sequence in half and leaves the terminal stuck in a colour
//! it never gets out of.

#[cfg(feature = "alloc")]
use alloc::{borrow::Cow, string::String};

const ESC: u8 = 0x1B;
const BEL: u8 = 0x07;

/// A run of the input that is either printable text or one escape sequence.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Piece<'a> {
    /// Printable text. Contains no escape sequences.
    Text(&'a str),
    /// One complete escape sequence, occupying zero columns.
    Escape(&'a str),
}

impl<'a> Piece<'a> {
    /// The underlying slice, whichever kind this is.
    pub fn as_str(&self) -> &'a str {
        match *self {
            Piece::Text(s) | Piece::Escape(s) => s,
        }
    }

    /// Whether this piece is an escape sequence.
    pub fn is_escape(&self) -> bool {
        matches!(self, Piece::Escape(_))
    }
}

/// Iterator splitting a string into text runs and escape sequences.
///
/// Created by [`pieces`](crate::pieces). Concatenating every piece reproduces
/// the input exactly, so this is a usable base for your own layout code.
#[derive(Clone)]
pub struct Pieces<'a> {
    s: &'a str,
    pos: usize,
}

impl<'a> Pieces<'a> {
    pub(crate) fn new(s: &'a str) -> Self {
        Pieces { s, pos: 0 }
    }
}

impl<'a> Iterator for Pieces<'a> {
    type Item = Piece<'a>;

    fn next(&mut self) -> Option<Piece<'a>> {
        let bytes = self.s.as_bytes();
        if self.pos >= bytes.len() {
            return None;
        }
        let start = self.pos;
        if let Some(len) = escape_len(bytes, start) {
            self.pos = start + len;
            return Some(Piece::Escape(&self.s[start..self.pos]));
        }
        // Scan for the next introducer byte rather than testing every position.
        // ESC and 0xC2 are never UTF-8 continuation bytes, so a hit is always on
        // a character boundary and the search cannot land mid-character.
        let mut i = start;
        loop {
            while i < bytes.len() && bytes[i] != ESC && bytes[i] != 0xC2 {
                i += 1;
            }
            if i >= bytes.len() || escape_len(bytes, i).is_some() {
                break;
            }
            i += 1; // a 0xC2 that begins an ordinary character, not a C1 control
        }
        self.pos = i;
        Some(Piece::Text(&self.s[start..i]))
    }
}

impl core::iter::FusedIterator for Pieces<'_> {}

/// Length in bytes of the escape sequence starting at `i`, if one starts there.
///
/// Handles 7-bit `ESC`-introduced sequences and their 8-bit C1 equivalents,
/// which arrive in a `&str` as U+0080..U+009F (`0xC2 0x8x`). An unterminated
/// sequence swallows the rest of the input, which is what a terminal does too.
pub(crate) fn escape_len(b: &[u8], i: usize) -> Option<usize> {
    match *b.get(i)? {
        ESC => match b.get(i + 1) {
            Some(b'[') => Some(csi_len(b, i, i + 2)),
            // OSC, DCS, SOS, PM, APC: string sequences ended by ST or BEL.
            Some(b']' | b'P' | b'X' | b'^' | b'_') => Some(string_len(b, i, i + 2)),
            // nF sequences: intermediate bytes then a final byte.
            Some(&c) if (0x20..=0x2F).contains(&c) => {
                let mut j = i + 1;
                while matches!(b.get(j), Some(&c) if (0x20..=0x2F).contains(&c)) {
                    j += 1;
                }
                if matches!(b.get(j), Some(&c) if (0x30..=0x7E).contains(&c)) {
                    j += 1;
                }
                Some(j - i)
            }
            // Fp/Fe/Fs: a single final byte, e.g. ESC 7, ESC M.
            Some(&c) if (0x30..=0x7E).contains(&c) => Some(2),
            // A stray or trailing ESC.
            _ => Some(1),
        },
        // C1 controls, UTF-8 encoded as two bytes.
        0xC2 => match b.get(i + 1) {
            Some(0x9B) => Some(csi_len(b, i, i + 2)),
            Some(0x9D | 0x90 | 0x98 | 0x9E | 0x9F) => Some(string_len(b, i, i + 2)),
            _ => None,
        },
        _ => None,
    }
}

/// CSI: parameter bytes, intermediate bytes, then one final byte.
fn csi_len(b: &[u8], start: usize, mut j: usize) -> usize {
    while matches!(b.get(j), Some(&c) if (0x30..=0x3F).contains(&c)) {
        j += 1;
    }
    while matches!(b.get(j), Some(&c) if (0x20..=0x2F).contains(&c)) {
        j += 1;
    }
    if matches!(b.get(j), Some(&c) if (0x40..=0x7E).contains(&c)) {
        j += 1;
    }
    j - start
}

/// A string sequence, terminated by BEL, `ESC \` or C1 ST (U+009C).
fn string_len(b: &[u8], start: usize, mut j: usize) -> usize {
    while j < b.len() {
        match b[j] {
            BEL => return j + 1 - start,
            ESC if b.get(j + 1) == Some(&b'\\') => return j + 2 - start,
            0xC2 if b.get(j + 1) == Some(&0x9C) => return j + 2 - start,
            _ => j += 1,
        }
    }
    j - start
}

/// Parameters of an SGR sequence (`CSI ... m`), or `None` if it is not one.
#[cfg(feature = "alloc")]
fn sgr_params(e: &str) -> Option<&str> {
    let b = e.as_bytes();
    if b.last() != Some(&b'm') {
        return None;
    }
    // Only CSI sequences carry SGR. An OSC, DCS or SOS string that happens to
    // end in `m` is not a colour code, and treating it as one made `wrap`
    // inject resets into text that had no styling at all.
    let head = match (b.first(), b.get(1)) {
        (Some(&ESC), Some(b'[')) => 2,
        (Some(&0xC2), Some(&0x9B)) => 2,
        _ => return None,
    };
    Some(&e[head..e.len() - 1])
}

/// Whether the string leaves SGR styling switched on at its end.
///
/// Used to decide whether a cut needs a trailing reset so the colour does not
/// bleed into whatever gets printed next.
#[cfg(feature = "alloc")]
pub(crate) fn leaves_style_open(s: &str) -> bool {
    let mut open = false;
    for piece in Pieces::new(s) {
        if let Piece::Escape(e) = piece {
            if let Some(params) = sgr_params(e) {
                // `CSI m` and `CSI 0 m` (and `0;0`) turn everything off.
                let is_reset = params.is_empty()
                    || params
                        .split(';')
                        .all(|p| p.is_empty() || p.bytes().all(|c| c == b'0'));
                open = !is_reset;
            }
        }
    }
    open
}

/// The SGR reset sequence, appended after a cut that leaves styling open.
#[cfg(feature = "alloc")]
pub(crate) const RESET: &str = "\x1b[0m";

/// Remove every escape sequence, leaving only printable text.
///
/// Borrows when there is nothing to strip.
///
/// ```
/// # use cellwidth::strip_ansi;
/// assert_eq!(strip_ansi("\x1b[31mred\x1b[0m"), "red");
/// assert_eq!(strip_ansi("plain"), "plain");
/// ```
#[cfg(feature = "alloc")]
pub fn strip_ansi(s: &str) -> Cow<'_, str> {
    if !has_escape(s) {
        return Cow::Borrowed(s);
    }
    let mut buf = String::with_capacity(s.len());
    for piece in Pieces::new(s) {
        if let Piece::Text(t) = piece {
            buf.push_str(t);
        }
    }
    Cow::Owned(buf)
}

/// Cheap pre-check: no escape introducer byte means no escape sequence.
#[cfg(feature = "alloc")]
pub(crate) fn has_escape(s: &str) -> bool {
    s.as_bytes()
        .iter()
        .enumerate()
        .any(|(i, &c)| (c == ESC || c == 0xC2) && escape_len(s.as_bytes(), i).is_some())
}

/// Whether an escape sequence is complete, or ran off the end of the input.
///
/// An unterminated sequence matters because anything appended after it gets
/// swallowed: `"\x1b[31"` followed by padding spaces is one long escape, not a
/// colour code and three columns.
#[cfg(feature = "alloc")]
pub(crate) fn is_terminated(e: &str) -> bool {
    let b = e.as_bytes();
    let n = b.len();
    match (b.first(), b.get(1)) {
        // CSI: complete once a final byte arrives.
        (Some(&ESC), Some(b'[')) | (Some(&0xC2), Some(&0x9B)) => {
            n > 2 && matches!(b.last(), Some(&c) if (0x40..=0x7E).contains(&c))
        }
        // OSC, DCS, SOS, PM, APC: complete at BEL or a string terminator.
        (Some(&ESC), Some(b']' | b'P' | b'X' | b'^' | b'_'))
        | (Some(&0xC2), Some(&(0x9D | 0x90 | 0x98 | 0x9E | 0x9F))) => {
            n > 2
                && (b[n - 1] == BEL
                    || (b[n - 2] == ESC && b[n - 1] == b'\\')
                    || (b[n - 2] == 0xC2 && b[n - 1] == 0x9C))
        }
        // nF sequence: intermediates then a final byte.
        (Some(&ESC), Some(&c)) if (0x20..=0x2F).contains(&c) => {
            matches!(b.last(), Some(&c) if (0x30..=0x7E).contains(&c))
        }
        (Some(&ESC), Some(_)) => true, // two-byte Fp/Fe/Fs form
        (Some(&ESC), None) => false,   // a lone ESC
        // Not an escape sequence at all; callers only pass `Piece::Escape`.
        _ => true,
    }
}

/// Drop a trailing escape sequence that never terminates.
///
/// Only the last piece can be unterminated, since such a sequence runs to the
/// end of the input by definition.
#[cfg(feature = "alloc")]
pub(crate) fn trim_dangling(s: &str) -> &str {
    if !has_escape(s) {
        return s;
    }
    let mut last = None;
    let mut pos = 0;
    for p in Pieces::new(s) {
        last = match p {
            Piece::Escape(e) => Some((pos, e)),
            Piece::Text(_) => None,
        };
        pos += p.as_str().len();
    }
    match last {
        // Removing one dangling escape can expose another behind it, as in
        // `ESC ESC -`, so keep going until the tail is clean.
        Some((at, e)) if !is_terminated(e) => trim_dangling(&s[..at]),
        _ => s,
    }
}

#[cfg(all(test, feature = "alloc"))]
mod tests {
    use super::*;

    /// `escape_len` is asked about positions the scanner has reached, but its
    /// contract covers the empty and past-the-end cases too.
    #[test]
    fn escape_len_at_the_boundary() {
        assert_eq!(escape_len(b"", 0), None);
        assert_eq!(escape_len(b"ab", 2), None);
        assert_eq!(escape_len(b"a", 0), None);
        assert_eq!(escape_len(b"\x1b[0m", 0), Some(4));
    }

    /// `is_terminated` decides whether appending text to a string would be
    /// swallowed by a half-finished escape, so every introducer form matters.
    #[test]
    fn termination_of_every_escape_form() {
        for complete in [
            "\x1b[0m",
            "\x1b[1;31m",
            "\u{9b}0m", // CSI
            "\x1b]0;t\x07",
            "\x1b]0;t\x1b\\", // OSC by BEL and by ST
            "\x1b]0;t\u{9c}",
            "\x1bPq\x1b\\", // OSC by C1 ST, DCS
            "\x1b(B",
            "\x1b$)C", // nF
            "\x1b7",
            "\x1bM", // two-byte Fp/Fe
            "not an escape",
            "", // defensive: never passed
        ] {
            assert!(is_terminated(complete), "{complete:?} should be complete");
        }
        for dangling in [
            "\x1b",
            "\x1b[",
            "\x1b[31",
            "\x1b[1;", // CSI cut short
            "\x1b]",
            "\x1b]8;;http://x",
            "\x1bP", // string sequence never closed
            "\x1b$",
            "\x1b ", // nF awaiting its final byte
        ] {
            assert!(!is_terminated(dangling), "{dangling:?} should be dangling");
        }
    }
}
