//! Grapheme cluster segmentation (UAX #29, extended grapheme clusters).
//!
//! A "character" as a user thinks of it is a grapheme cluster, not a `char`.
//! `é` may be two `char`s, a flag is two, and a family emoji is seven. Width,
//! truncation and cursor movement all have to work in clusters or they produce
//! mojibake.

use crate::tables::{entry, GCB_MASK, GCB_SHIFT};

/// Grapheme_Cluster_Break property values, plus the refinements UAX #29 needs
/// for emoji (`ExtPict`) and Indic conjuncts (`InCB*`).
///
/// Discriminants are written by `tools/gen_tables.py`; keep them in sync.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
pub(crate) enum Gcb {
    Other = 0,
    Cr = 1,
    Lf = 2,
    Control = 3,
    Extend = 4,
    Zwj = 5,
    RegionalIndicator = 6,
    Prepend = 7,
    SpacingMark = 8,
    L = 9,
    V = 10,
    T = 11,
    Lv = 12,
    Lvt = 13,
    ExtPict = 14,
    InCBLinker = 15,
    InCBConsonant = 16,
    InCBExtend = 17,
}

/// Every 5-bit value the table can hold, so the lookup is total.
#[rustfmt::skip]
const GCB_TABLE: [Gcb; 32] = {
    use Gcb::*;
    [
        Other, Cr, Lf, Control, Extend, Zwj, RegionalIndicator, Prepend,
        SpacingMark, L, V, T, Lv, Lvt, ExtPict, InCBLinker, InCBConsonant,
        InCBExtend, Other, Other, Other, Other, Other, Other, Other, Other,
        Other, Other, Other, Other, Other, Other,
    ]
};

impl Gcb {
    /// GB9: characters that glue themselves to whatever came before.
    fn is_extend_like(self) -> bool {
        matches!(
            self,
            Gcb::Extend | Gcb::InCBExtend | Gcb::InCBLinker | Gcb::Zwj
        )
    }

    /// InCB=Extend for GB9c. ZWJ carries InCB=Extend even though its
    /// Grapheme_Cluster_Break value is ZWJ.
    fn is_incb_extend(self) -> bool {
        matches!(self, Gcb::InCBExtend | Gcb::InCBLinker | Gcb::Zwj)
    }
}

pub(crate) fn gcb(c: char) -> Gcb {
    GCB_TABLE[((entry(c as u32) >> GCB_SHIFT) & GCB_MASK) as usize]
}

/// Incremental UAX #29 boundary detection.
///
/// Rules GB1-GB999. The three rules that need memory rather than just the
/// adjacent pair are tracked as flags: GB11 (emoji ZWJ sequences), GB12/13
/// (regional indicator pairs) and GB9c (Indic conjuncts).
#[derive(Default, Clone)]
pub(crate) struct Breaker {
    prev: Option<Gcb>,
    /// Number of unbroken regional indicators immediately behind us.
    ri_run: usize,
    /// We are inside `ExtPict Extend*`, so a following ZWJ can join (GB11).
    pict_seq: bool,
    /// Seen InCB=Consonant, then only InCB=Extend/Linker since (GB9c).
    incb_consonant: bool,
    /// ...and at least one of them was a Linker.
    incb_linker: bool,
    /// Legacy grapheme clusters: GB9a, GB9b and GB9c do not apply.
    legacy: bool,
}

impl Breaker {
    /// A breaker for legacy (non-extended) grapheme clusters.
    pub(crate) fn legacy() -> Self {
        Breaker {
            legacy: true,
            ..Breaker::default()
        }
    }

    /// Feed the next character; returns `true` if a cluster boundary falls
    /// *before* it.
    pub(crate) fn is_boundary(&mut self, c: char) -> bool {
        let right = gcb(c);
        let brk = match self.prev {
            None => true, // GB1: start of text
            Some(left) => self.decide(left, right),
        };
        self.advance(right, brk);
        brk
    }

    fn decide(&self, l: Gcb, r: Gcb) -> bool {
        // GB3, GB4, GB5: control characters stand alone, but CR LF is one.
        if l == Gcb::Cr && r == Gcb::Lf {
            return false;
        }
        if matches!(l, Gcb::Cr | Gcb::Lf | Gcb::Control)
            || matches!(r, Gcb::Cr | Gcb::Lf | Gcb::Control)
        {
            return true;
        }
        // GB6, GB7, GB8: Hangul syllable composition.
        if l == Gcb::L && matches!(r, Gcb::L | Gcb::V | Gcb::Lv | Gcb::Lvt) {
            return false;
        }
        if matches!(l, Gcb::Lv | Gcb::V) && matches!(r, Gcb::V | Gcb::T) {
            return false;
        }
        if matches!(l, Gcb::Lvt | Gcb::T) && r == Gcb::T {
            return false;
        }
        // GB9: extenders and ZWJ never start a cluster.
        if r.is_extend_like() {
            return false;
        }
        // GB9a, GB9b and GB9c only apply to extended clusters.
        if !self.legacy {
            if r == Gcb::SpacingMark || l == Gcb::Prepend {
                return false;
            }
            // GB9c: consonant + linker + consonant is one Indic conjunct.
            if r == Gcb::InCBConsonant && self.incb_consonant && self.incb_linker {
                return false;
            }
        }
        // GB11: emoji ZWJ sequence, e.g. the family emoji.
        if l == Gcb::Zwj && self.pict_seq && r == Gcb::ExtPict {
            return false;
        }
        // GB12, GB13: regional indicators pair up into flags.
        if l == Gcb::RegionalIndicator && r == Gcb::RegionalIndicator && self.ri_run % 2 == 1 {
            return false;
        }
        true // GB999
    }

    fn advance(&mut self, r: Gcb, brk: bool) {
        self.ri_run = if r == Gcb::RegionalIndicator {
            if brk {
                1
            } else {
                self.ri_run + 1
            }
        } else {
            0
        };

        if r == Gcb::ExtPict {
            self.pict_seq = true;
        } else if !matches!(
            r,
            Gcb::Extend | Gcb::InCBExtend | Gcb::InCBLinker | Gcb::Zwj
        ) {
            self.pict_seq = false;
        }

        if r == Gcb::InCBConsonant {
            self.incb_consonant = true;
            self.incb_linker = false;
        } else if r == Gcb::InCBLinker {
            if self.incb_consonant {
                self.incb_linker = true;
            }
        } else if !r.is_incb_extend() {
            self.incb_consonant = false;
            self.incb_linker = false;
        }

        self.prev = Some(r);
    }
}

/// Iterator over the grapheme clusters of a string, yielded as subslices.
///
/// Created by [`graphemes`](crate::graphemes).
#[derive(Clone)]
pub struct Graphemes<'a> {
    s: &'a str,
    /// Byte offset of the start of the cluster being accumulated.
    start: usize,
    /// Byte offset of the next character to inspect.
    pos: usize,
    breaker: Breaker,
}

impl<'a> Graphemes<'a> {
    pub(crate) fn new(s: &'a str) -> Self {
        Graphemes {
            s,
            start: 0,
            pos: 0,
            breaker: Breaker::default(),
        }
    }

    /// Legacy (non-extended) grapheme clusters, for the compatibility shim.
    pub(crate) fn new_legacy(s: &'a str) -> Self {
        Graphemes {
            breaker: Breaker::legacy(),
            ..Graphemes::new(s)
        }
    }

    /// The part of the input not yet returned, including any partially
    /// accumulated cluster.
    pub fn remainder(&self) -> &'a str {
        &self.s[self.start..]
    }
}

impl<'a> Iterator for Graphemes<'a> {
    type Item = &'a str;

    fn next(&mut self) -> Option<&'a str> {
        if self.start >= self.s.len() {
            return None;
        }
        while let Some(c) = self.s[self.pos..].chars().next() {
            let at = self.pos;
            if self.breaker.is_boundary(c) && at > self.start {
                let cluster = &self.s[self.start..at];
                self.start = at;
                self.pos = at + c.len_utf8();
                return Some(cluster);
            }
            self.pos = at + c.len_utf8();
        }
        let cluster = &self.s[self.start..];
        self.start = self.s.len();
        Some(cluster)
    }

    fn size_hint(&self) -> (usize, Option<usize>) {
        let bytes = self.s.len() - self.start;
        ((bytes != 0) as usize, Some(bytes))
    }
}

impl core::iter::FusedIterator for Graphemes<'_> {}

/// First grapheme cluster of a string, or `""` if it is empty.
pub(crate) fn first_cluster(s: &str) -> &str {
    Graphemes::new(s).next().unwrap_or("")
}

/// Whether this character would attach itself to whatever precedes it.
///
/// A cell or field beginning with one of these is not self-contained: it will
/// merge into the border or padding drawn before it and quietly cost a column.
#[cfg(feature = "alloc")]
pub(crate) fn attaches_to_previous(c: char) -> bool {
    matches!(
        gcb(c),
        Gcb::Extend
            | Gcb::SpacingMark
            | Gcb::Zwj
            | Gcb::InCBExtend
            | Gcb::InCBLinker
            | Gcb::V
            | Gcb::T
    )
}
