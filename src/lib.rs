//! Terminal display width that gets emoji, CJK and ANSI colour right.
//!
//! `"👨‍👩‍👧‍👦".len()` is 25. `"👨‍👩‍👧‍👦".chars().count()` is 7. A terminal draws it in **2**
//! columns. Every CLI table, progress bar, box border and status line needs
//! that third number, and getting it wrong is why so much terminal output is
//! subtly ragged.
//!
//! ```
//! use cellwidth::{width, cell, truncate};
//!
//! assert_eq!(width("café"), 4);            // combining accents don't count
//! assert_eq!(width("日本語"), 6);           // CJK is double-width
//! assert_eq!(width("👨‍👩‍👧‍👦"), 2);              // one glyph, not seven
//! assert_eq!(width("\x1b[31mred\x1b[0m"), 3); // colour codes are free
//!
//! // Cuts land on cluster boundaries, never mid-character.
//! assert_eq!(truncate("日本語テキスト", 7), "日本語");
//!
//! // And this is always exactly 10 columns wide, whatever you feed it.
//! assert_eq!(width(&cell("🇯🇵 Tōkyō 東京", 10)), 10);
//! ```
//!
//! # What makes this correct
//!
//! * Widths come from the East Asian Width property of the Unicode Character
//!   Database, version [`UNICODE_VERSION`], generated into plain sorted tables.
//! * Text is segmented into extended grapheme clusters per UAX #29 -- including
//!   emoji ZWJ sequences, regional indicator flags, skin tone modifiers and
//!   Indic conjuncts -- so a "character" means what a user thinks it means.
//! * Variation selectors are honoured: `❤` is 1 column, `❤️` is 2.
//! * ANSI escape sequences are parsed properly (CSI, OSC, DCS and the 8-bit C1
//!   forms), counted as zero columns, and never cut in half.
//!
//! # Choosing a policy
//!
//! The free functions use [`Width::DEFAULT`]. Where the answer genuinely
//! depends on the terminal -- East Asian Ambiguous characters, tab stops, how
//! old the emoji font is -- build a [`Width`] instead:
//!
//! ```
//! use cellwidth::{Ambiguous, Width};
//!
//! const CJK: Width = Width::DEFAULT.ambiguous(Ambiguous::Wide).tab_stop(4);
//! assert_eq!(CJK.of("°C"), 3);
//! ```
//!
//! # No dependencies
//!
//! None, and no build script: the Unicode tables are generated ahead of time by
//! `tools/gen_tables.py` and committed. `no_std` is supported by disabling the
//! `std` feature; measurement works with no allocator at all, and the
//! `alloc` feature adds the functions that return owned strings.

#![doc(html_logo_url = "https://singhpratech.github.io/cellwidth/icon.svg")]
#![doc(html_favicon_url = "https://singhpratech.github.io/cellwidth/icon.svg")]
#![cfg_attr(not(feature = "std"), no_std)]
#![deny(missing_docs)]
#![forbid(unsafe_code)]

#[cfg(feature = "alloc")]
extern crate alloc;

mod ansi;
#[cfg(feature = "alloc")]
mod fit;
pub(crate) mod grapheme;
pub(crate) mod linebreak;
mod measure;
#[cfg(feature = "alloc")]
mod table;
mod tables;

#[cfg(feature = "alloc")]
pub use crate::ansi::strip_ansi;
pub use crate::ansi::{Piece, Pieces};
#[cfg(feature = "alloc")]
pub use crate::fit::ELLIPSIS;
pub use crate::grapheme::Graphemes;
pub use crate::linebreak::{Break, LineBreaks};
pub use crate::measure::{Ambiguous, Clusters, Control, Width};
#[cfg(feature = "alloc")]
pub use crate::table::{Align, Border, Sizing, Table};
pub use crate::tables::UNICODE_VERSION;

#[cfg(feature = "alloc")]
use alloc::{borrow::Cow, string::String, vec::Vec};

/// Columns a string occupies in a terminal, using [`Width::DEFAULT`].
///
/// ```
/// # use cellwidth::width;
/// assert_eq!(width("hello"), 5);
/// assert_eq!(width("👍🏽"), 2);
/// ```
pub fn width(s: &str) -> usize {
    Width::DEFAULT.of(s)
}

/// Columns a single `char` occupies, using [`Width::DEFAULT`].
///
/// Remember that a `char` is not a user-perceived character; prefer [`width`]
/// for real text.
pub fn char_width(c: char) -> usize {
    Width::DEFAULT.of_char(c)
}

/// Iterate the extended grapheme clusters of a string (UAX #29).
///
/// ```
/// # use cellwidth::graphemes;
/// let g: Vec<&str> = graphemes("a👍🏽🇯🇵").collect();
/// assert_eq!(g, ["a", "👍🏽", "🇯🇵"]);
/// ```
pub fn graphemes(s: &str) -> Graphemes<'_> {
    Graphemes::new(s)
}

/// Iterate the places a line may be broken (UAX #14).
///
/// Yields byte offsets, so `&s[..offset]` is a complete line. The final offset
/// is always `s.len()`.
///
/// ```
/// # use cellwidth::{line_breaks, Break};
/// let b: Vec<_> = line_breaks("a b").collect();
/// assert_eq!(b, [(2, Break::Allowed), (3, Break::Mandatory)]);
/// ```
pub fn line_breaks(s: &str) -> LineBreaks<'_> {
    LineBreaks::new(s)
}

/// Split a string into printable text runs and escape sequences.
///
/// ```
/// # use cellwidth::{pieces, Piece};
/// let p: Vec<Piece> = pieces("\x1b[1mbold\x1b[0m").collect();
/// assert_eq!(p[1], Piece::Text("bold"));
/// ```
pub fn pieces(s: &str) -> Pieces<'_> {
    Pieces::new(s)
}

/// Longest prefix of `s` fitting in `max` columns, using [`Width::DEFAULT`].
///
/// Never allocates and never cuts inside a character or an escape sequence.
pub fn truncate(s: &str, max: usize) -> &str {
    Width::DEFAULT.truncate(s, max)
}

/// Fit `s` into exactly `width` columns, truncating with `…` or padding with
/// spaces. The table-cell workhorse.
///
/// ```
/// # use cellwidth::{cell, width};
/// assert_eq!(cell("hi", 5), "hi   ");
/// assert_eq!(cell("日本語テキスト", 8), "日本語… ");
/// assert_eq!(width(&cell("👨‍👩‍👧‍👦 crew", 6)), 6);
/// ```
#[cfg(feature = "alloc")]
pub fn cell(s: &str, width: usize) -> Cow<'_, str> {
    Width::DEFAULT.cell(s, width)
}

/// Pad `s` with spaces on the right to at least `width` columns.
#[cfg(feature = "alloc")]
pub fn pad_end(s: &str, width: usize) -> Cow<'_, str> {
    Width::DEFAULT.pad_end(s, width)
}

/// Pad `s` with spaces on the left to at least `width` columns.
#[cfg(feature = "alloc")]
pub fn pad_start(s: &str, width: usize) -> Cow<'_, str> {
    Width::DEFAULT.pad_start(s, width)
}

/// Break `s` into lines of at most `width` columns, breaking at whitespace.
#[cfg(feature = "alloc")]
pub fn wrap(s: &str, width: usize) -> Vec<String> {
    Width::DEFAULT.wrap(s, width)
}
