//! Drop-in replacements for the traits of `unicode-width` and
//! `unicode-segmentation`.
//!
//! Both crates expose their functionality as extension traits on `str` and
//! `char`. The traits here have the same names and the same method signatures,
//! so a crate that uses either can switch by changing an import:
//!
//! ```
//! // use unicode_width::UnicodeWidthStr;
//! use cellwidth::compat::UnicodeWidthStr;
//!
//! assert_eq!("日本語".width(), 6);
//! assert_eq!("°C".width(), 2);
//! assert_eq!("°C".width_cjk(), 3);
//! ```
//!
//! ```
//! // use unicode_segmentation::UnicodeSegmentation;
//! use cellwidth::compat::UnicodeSegmentation;
//!
//! let g: Vec<&str> = "a👍🏽🇯🇵".graphemes(true).collect();
//! assert_eq!(g, ["a", "👍🏽", "🇯🇵"]);
//! ```
//!
//! # What changes when you switch
//!
//! The method names are the same; the answers are cellwidth's. In particular,
//! [`UnicodeWidthStr::width`] counts ANSI escape sequences as zero columns,
//! measures whole grapheme clusters, and expands tabs, which is the reason to
//! switch. The `unicode-width` figures below are measured, not quoted:
//!
//! ```
//! use cellwidth::compat::UnicodeWidthStr;
//!
//! assert_eq!("\x1b[31mred\x1b[0m".width(), 3); // unicode-width 0.2.2: 12
//! assert_eq!("क्षि".width(), 1);                  // unicode-width 0.2.2: 3
//! assert_eq!("a\tb".width(), 9);                 // unicode-width 0.2.2: 3
//! assert_eq!("👨‍👩‍👧‍👦".width(), 2);                  // both agree
//! ```
//!
//! [`UnicodeWidthChar`] keeps `unicode-width`'s convention of returning `None`
//! for a control character. The single-`char` answers otherwise follow
//! [`Width::DEFAULT`], and every place they differ from `unicode-width` is
//! listed and pinned in `oracles/tests/differential.rs`.
//!
//! [`UnicodeSegmentation`] provides the grapheme cluster methods only; word and
//! sentence segmentation are outside this crate's scope. Its iterators are
//! forward-only, so code that relied on `.rev()` will not compile rather than
//! misbehave.

use crate::grapheme::Graphemes;
use crate::{Ambiguous, Width};

const CJK: Width = Width::DEFAULT.ambiguous(Ambiguous::Wide);

mod private {
    pub trait Sealed {}
    impl Sealed for str {}
    impl Sealed for char {}
}

/// Display width of a string, with the interface of `unicode_width::UnicodeWidthStr`.
pub trait UnicodeWidthStr: private::Sealed {
    /// Columns the string occupies, per [`Width::DEFAULT`]: East Asian
    /// Ambiguous characters are one column wide.
    fn width(&self) -> usize;

    /// Columns the string occupies with East Asian Ambiguous characters two
    /// columns wide, as CJK locales expect.
    fn width_cjk(&self) -> usize;
}

impl UnicodeWidthStr for str {
    #[inline]
    fn width(&self) -> usize {
        Width::DEFAULT.of(self)
    }

    #[inline]
    fn width_cjk(&self) -> usize {
        CJK.of(self)
    }
}

/// Display width of a `char`, with the interface of `unicode_width::UnicodeWidthChar`.
///
/// A `char` is not a user-perceived character; prefer [`UnicodeWidthStr`] for
/// real text.
pub trait UnicodeWidthChar: private::Sealed {
    /// Columns the character occupies, or `None` if it is a control character.
    /// East Asian Ambiguous characters are one column wide.
    fn width(self) -> Option<usize>;

    /// Columns the character occupies, or `None` if it is a control character.
    /// East Asian Ambiguous characters are two columns wide.
    fn width_cjk(self) -> Option<usize>;
}

impl UnicodeWidthChar for char {
    #[inline]
    fn width(self) -> Option<usize> {
        if self.is_control() {
            None
        } else {
            Some(Width::DEFAULT.of_char(self))
        }
    }

    #[inline]
    fn width_cjk(self) -> Option<usize> {
        if self.is_control() {
            None
        } else {
            Some(CJK.of_char(self))
        }
    }
}

/// Grapheme cluster iteration, with the interface of
/// `unicode_segmentation::UnicodeSegmentation`.
pub trait UnicodeSegmentation: private::Sealed {
    /// Iterate the grapheme clusters of the string (UAX #29).
    ///
    /// `is_extended` selects extended clusters, which is what every other
    /// function in this crate uses and what a user perceives as a character.
    /// `false` selects legacy clusters, which do not apply rules GB9a, GB9b
    /// and GB9c: spacing marks, prepended characters and Indic conjuncts each
    /// stand alone.
    fn graphemes(&self, is_extended: bool) -> Graphemes<'_>;

    /// Like [`graphemes`](Self::graphemes), yielding each cluster with its
    /// byte offset.
    fn grapheme_indices(&self, is_extended: bool) -> GraphemeIndices<'_>;
}

impl UnicodeSegmentation for str {
    #[inline]
    fn graphemes(&self, is_extended: bool) -> Graphemes<'_> {
        if is_extended {
            Graphemes::new(self)
        } else {
            Graphemes::new_legacy(self)
        }
    }

    #[inline]
    fn grapheme_indices(&self, is_extended: bool) -> GraphemeIndices<'_> {
        GraphemeIndices {
            inner: self.graphemes(is_extended),
            pos: 0,
        }
    }
}

/// Iterator over `(byte offset, cluster)` pairs.
///
/// Created by [`UnicodeSegmentation::grapheme_indices`].
#[derive(Clone)]
pub struct GraphemeIndices<'a> {
    inner: Graphemes<'a>,
    pos: usize,
}

impl<'a> Iterator for GraphemeIndices<'a> {
    type Item = (usize, &'a str);

    fn next(&mut self) -> Option<(usize, &'a str)> {
        let cluster = self.inner.next()?;
        let at = self.pos;
        self.pos += cluster.len();
        Some((at, cluster))
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        self.inner.size_hint()
    }
}

impl core::iter::FusedIterator for GraphemeIndices<'_> {}
