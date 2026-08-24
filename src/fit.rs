//! Cutting text to a column budget without breaking it.
//!
//! Everything here works in whole grapheme clusters and whole escape
//! sequences, so a cut never produces a broken character or a half-written
//! colour code.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ansi::{leaves_style_open, trim_dangling, Piece, Pieces, RESET};
use crate::grapheme::first_cluster;
use crate::measure::Width;

/// The default ellipsis used by [`Width::cell`]: a single-column `…`.
pub const ELLIPSIS: &str = "…";

impl Width {
    /// Fit `s` into `max` columns, marking a cut with `ellipsis`.
    ///
    /// Returns the input untouched if it already fits. Otherwise the result is
    /// at most `max` columns wide including the ellipsis, and a reset is
    /// appended if the cut left styling switched on.
    ///
    /// ```
    /// use cellwidth::Width;
    /// let w = Width::DEFAULT;
    /// assert_eq!(w.truncate_ellipsis("日本語テキスト", 7, "…"), "日本語…");
    /// assert_eq!(w.truncate_ellipsis("short", 10, "…"), "short");
    /// ```
    pub fn truncate_ellipsis<'a>(&self, s: &'a str, max: usize, ellipsis: &str) -> Cow<'a, str> {
        if self.of(s) <= max {
            return Cow::Borrowed(s);
        }
        let e_width = self.of(ellipsis);
        if e_width > max {
            // No room for the marker; a bare cut is the best available answer.
            return Cow::Borrowed(self.truncate(s, max));
        }
        // A dangling escape would swallow the ellipsis, so drop it.
        let head = trim_dangling(self.truncate(s, max - e_width));
        let mut out = String::with_capacity(head.len() + ellipsis.len() + RESET.len());
        out.push_str(head);
        out.push_str(ellipsis);
        if self.ansi_enabled() && leaves_style_open(head) {
            out.push_str(RESET);
        }
        Cow::Owned(out)
    }

    /// Pad `s` with spaces on the right to at least `width` columns.
    ///
    /// Text wider than `width` is returned unchanged; use [`Width::cell`] to
    /// force an exact width.
    pub fn pad_end<'a>(&self, s: &'a str, width: usize) -> Cow<'a, str> {
        self.pad(s, width, Align::Start)
    }

    /// Pad `s` with spaces on the left to at least `width` columns.
    pub fn pad_start<'a>(&self, s: &'a str, width: usize) -> Cow<'a, str> {
        self.pad(s, width, Align::End)
    }

    /// Pad `s` with spaces on both sides to at least `width` columns.
    ///
    /// An odd remainder goes on the right.
    pub fn center<'a>(&self, s: &'a str, width: usize) -> Cow<'a, str> {
        self.pad(s, width, Align::Center)
    }

    fn pad<'a>(&self, s: &'a str, width: usize, align: Align) -> Cow<'a, str> {
        // An unterminated escape at the end would swallow the padding, leaving
        // a cell narrower than asked for. Such a sequence is malformed input;
        // dropping it is the only way to keep the width guarantee.
        let keep = trim_dangling(s).len();
        let have = self.of(&s[..keep]);
        if have >= width {
            return Cow::Borrowed(&s[..keep]);
        }
        let s = &s[..keep];
        let gap = width - have;
        let (left, right) = match align {
            Align::Start => (0, gap),
            Align::End => (gap, 0),
            Align::Center => (gap / 2, gap - gap / 2),
        };
        let mut out = String::with_capacity(s.len() + gap);
        for _ in 0..left {
            out.push(' ');
        }
        out.push_str(s);
        for _ in 0..right {
            out.push(' ');
        }
        Cow::Owned(out)
    }

    /// Fit `s` into exactly `width` columns: truncate with `…` if too long,
    /// pad with spaces if too short.
    ///
    /// This is the table-cell workhorse. The result is always `width` columns
    /// wide, whatever writing system or emoji the input contains.
    ///
    /// ```
    /// use cellwidth::Width;
    /// let w = Width::DEFAULT;
    /// assert_eq!(w.of(&w.cell("日本語テキスト", 8)), 8);
    /// assert_eq!(w.of(&w.cell("👨‍👩‍👧‍👦 family", 8)), 8);
    /// assert_eq!(w.of(&w.cell("ok", 8)), 8);
    /// ```
    pub fn cell<'a>(&self, s: &'a str, width: usize) -> Cow<'a, str> {
        let fitted = self.truncate_ellipsis(s, width, ELLIPSIS);
        let keep = trim_dangling(&fitted).len();
        let have = self.of(&fitted[..keep]);
        if have >= width && keep == fitted.len() {
            return fitted;
        }
        let mut out = String::with_capacity(keep + width - have);
        out.push_str(&fitted[..keep]);
        for _ in 0..width - have {
            out.push(' ');
        }
        Cow::Owned(out)
    }

    /// Break `s` into lines of at most `width` columns.
    ///
    /// Break opportunities come from the Unicode line breaking algorithm
    /// (UAX #14), so this finds the places a reader would accept: between CJK
    /// ideographs, after a hyphen, never inside `1,000` or between a bracket
    /// and what it encloses. Existing newlines are honoured. A word longer than
    /// the line is broken by column.
    ///
    /// Styling open at the end of a line is closed and reopened on the next, so
    /// each line can be printed on its own.
    ///
    /// ```
    /// use cellwidth::Width;
    /// let w = Width::DEFAULT;
    /// assert_eq!(w.wrap("the quick brown fox", 10), ["the quick", "brown fox"]);
    /// // CJK breaks between ideographs, with no spaces in sight.
    /// assert_eq!(w.wrap("\u{65E5}\u{672C}\u{8A9E}\u{30C6}\u{30AD}\u{30B9}\u{30C8}", 6),
    ///            ["\u{65E5}\u{672C}\u{8A9E}", "\u{30C6}\u{30AD}\u{30B9}", "\u{30C8}"]);
    /// // ...but never inside a number.
    /// assert_eq!(w.wrap("cost 1,000 yen", 8), ["cost", "1,000", "yen"]);
    /// ```
    pub fn wrap(&self, s: &str, width: usize) -> Vec<String> {
        let mut out = Vec::new();
        if width == 0 {
            return out;
        }
        // Line breaking must not see escape sequences: their bytes would look
        // like ordinary letters and invite breaks inside them. Analyse the
        // stripped text and map the offsets back.
        let (plain, map) = self.strip_with_offsets(s);

        let mut style: Option<String> = None;
        let mut line_start = 0; // byte offset in `s` where the current line begins
        let mut seg_start = 0; // byte offset in `s` of the pending segment
        let mut col = 0;
        let mut wrote = false;

        for (end_plain, kind) in crate::linebreak::LineBreaks::new(&plain) {
            let end = map_offset(&map, end_plain, s.len());
            let seg = &s[seg_start..end];
            if seg.is_empty() {
                continue;
            }
            let trimmed_len = trim_end_len(seg);

            // Does the segment still fit once trailing spaces are ignored?
            // Measured where it will sit, not from column zero: a tab's width
            // depends on the column it starts in.
            if col > 0 && col + self.of_at(&seg[..trimmed_len], col) > width {
                self.push_line(&mut out, s, line_start, seg_start, &mut style);
                line_start = seg_start;
                col = 0;
                wrote = true;
            }
            // Re-measure: the flush above may have moved the segment to column
            // zero, where a tab in it is a different width.
            let w_trimmed = self.of_at(&seg[..trimmed_len], col);
            // A segment wider than the whole line has to be cut by column.
            // Start from `line_start`, not `seg_start`: anything already
            // pending on the line occupies no columns but is still text, and
            // must ride along with the first chunk rather than be dropped.
            if col == 0 && w_trimmed > width {
                let limit = seg_start + trimmed_len;
                let mut at = line_start;
                while at < limit && self.of(&s[at..limit]) > width {
                    let head = self.truncate(&s[at..limit], width);
                    // Always take at least one cluster, or an over-wide glyph
                    // would loop forever; never past `limit`, or the next slice
                    // would be inverted.
                    let take = head.len().max(first_cluster(&s[at..limit]).len());
                    let stop = (at + take).min(limit);
                    self.push_line(&mut out, s, at, stop, &mut style);
                    wrote = true;
                    at = stop;
                }
                line_start = at;
                col = self.of(&s[at..end]);
                debug_assert!(at <= end);
                seg_start = end;
                if kind == crate::linebreak::Break::Mandatory {
                    self.push_line(&mut out, s, line_start, end, &mut style);
                    line_start = end;
                    col = 0;
                }
                continue;
            }
            col += self.of_at(seg, col);
            seg_start = end;
            if kind == crate::linebreak::Break::Mandatory {
                self.push_line(&mut out, s, line_start, end, &mut style);
                line_start = end;
                col = 0;
                wrote = true;
            }
        }
        if line_start < s.len() || !wrote {
            self.push_line(&mut out, s, line_start, s.len(), &mut style);
        }
        out
    }

    /// Emit `s[from..to]` as a line: trailing spaces trimmed, styling reopened
    /// at the front and closed at the end.
    fn push_line(
        &self,
        out: &mut Vec<String>,
        s: &str,
        from: usize,
        to: usize,
        style: &mut Option<String>,
    ) {
        let raw = &s[from..to.max(from)];
        let body = line_body(raw);
        let mut line = String::with_capacity(body.len() + 8);
        if let Some(open) = style.as_deref() {
            line.push_str(open);
        }
        line.push_str(&body);
        self.track_style(raw, style);
        if self.ansi_enabled() && leaves_style_open(&line) {
            line.push_str(RESET);
        }
        out.push(line);
    }

    /// The text with escape sequences removed, plus a map from each break
    /// offset in the stripped text back to the original.
    ///
    /// The map records where each prefix *ends*, not where the next character
    /// begins. The difference matters when an escape sits between them: a break
    /// after a newline has to land before the escape, or the newline ends up
    /// inside the line instead of terminating it.
    fn strip_with_offsets(&self, s: &str) -> (String, Vec<usize>) {
        let mut plain = String::with_capacity(s.len());
        let mut ends = Vec::with_capacity(s.len() + 1);
        ends.push(0);
        let mut at = 0;
        for piece in Pieces::new(s) {
            if let Piece::Text(t) = piece {
                for (i, ch) in t.char_indices() {
                    for _ in 0..ch.len_utf8() {
                        ends.push(at + i + ch.len_utf8());
                    }
                }
                plain.push_str(t);
            }
            at += piece.as_str().len();
        }
        (plain, ends)
    }
}

/// The printable content of a line: the separator that ended it and any
/// trailing spaces removed, but trailing escape sequences kept.
///
/// The separator is not always last. `"line\n\x1b[0m"` ends with a reset, and
/// dropping the reset to get at the newline would leak colour into the next
/// line, so the two are separated rather than truncated.
fn line_body(s: &str) -> Cow<'_, str> {
    // Everything after the last printable text is escapes, and stays.
    let mut text_end = 0;
    let mut at = 0;
    for piece in Pieces::new(s) {
        at += piece.as_str().len();
        if matches!(piece, Piece::Text(_)) {
            text_end = at;
        }
    }
    let head = &s[..text_end];
    let keep = trim_end_len(head);
    if keep == s.len() {
        return Cow::Borrowed(s);
    }
    if text_end == s.len() {
        return Cow::Borrowed(&s[..keep]);
    }
    let mut out = String::with_capacity(keep + s.len() - text_end);
    out.push_str(&s[..keep]);
    out.push_str(&s[text_end..]);
    Cow::Owned(out)
}

/// Length of `s` once the line separator that ended it, and any trailing
/// spaces, are removed.
fn trim_end_len(s: &str) -> usize {
    // The separator itself is not part of the line it terminates.
    let mut n = match () {
        _ if s.ends_with("\r\n") => s.len() - 2,
        _ => s
            .strip_suffix([
                '\n', '\r', '\u{0B}', '\u{0C}', '\u{85}', '\u{2028}', '\u{2029}',
            ])
            .map_or(s.len(), str::len),
    };
    while let Some(stripped) = s[..n].strip_suffix([' ', '\t']) {
        n = stripped.len();
    }
    n
}

/// Translate a break offset in the escape-stripped text back to the original.
///
/// The very end of the text maps to the very end of the original, so trailing
/// escape sequences -- a reset, typically -- stay on the last line.
fn map_offset(map: &[usize], i: usize, len: usize) -> usize {
    if i + 1 >= map.len() {
        len
    } else {
        map[i]
    }
}

impl Width {
    /// Remember the last styling sequence so wrapped lines can reopen it.
    fn track_style(&self, s: &str, style: &mut Option<String>) {
        if !self.ansi_enabled() {
            return;
        }
        for piece in Pieces::new(s) {
            if let Piece::Escape(e) = piece {
                if leaves_style_open(e) {
                    *style = Some(String::from(e));
                } else if e.ends_with('m') {
                    *style = None;
                }
            }
        }
    }
}

enum Align {
    Start,
    End,
    Center,
}
