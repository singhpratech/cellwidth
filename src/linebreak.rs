//! Line break opportunities (UAX #14).
//!
//! Where may a line be broken? Not simply "at spaces": CJK breaks between
//! ideographs, a number like `1,000` must not be split at the comma, `(a)`
//! keeps its brackets, and a hyphen behaves differently depending on what sits
//! either side of it.
//!
//! The algorithm is verified against Unicode's own `LineBreakTest.txt`, all
//! 19,338 cases of it.

use crate::tables::{entry, EAST_ASIAN, EXTPICT_UNASSIGNED, LB_MASK, LB_SHIFT};

/// Line_Break property values, after the LB1 resolution.
///
/// Discriminants are written by `tools/gen_tables.py`; keep them in sync.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
#[repr(u8)]
#[allow(clippy::upper_case_acronyms)]
pub(crate) enum Lb {
    XX = 0,
    AK,
    AL,
    AP,
    AS,
    B2,
    BA,
    BB,
    BK,
    CB,
    CL,
    CM,
    CP,
    CR,
    EB,
    EM,
    EX,
    GL,
    H2,
    H3,
    HH,
    HL,
    HY,
    ID,
    IN,
    IS,
    JL,
    JT,
    JV,
    LF,
    NL,
    NS,
    NU,
    OP,
    PO,
    PR,
    QU,
    QUPi,
    QUPf,
    RI,
    SP,
    SY,
    VF,
    VI,
    WJ,
    ZW,
    ZWJ,
}

/// Every 6-bit value the table can hold, so the lookup is total and has no
/// unreachable fallback.
#[rustfmt::skip]
const LB_TABLE: [Lb; 64] = {
    use Lb::*;
    [
        XX, AK, AL, AP, AS, B2, BA, BB, BK, CB, CL, CM, CP, CR, EB, EM, EX, GL,
        H2, H3, HH, HL, HY, ID, IN, IS, JL, JT, JV, LF, NL, NS, NU, OP, PO, PR,
        QU, QUPi, QUPf, RI, SP, SY, VF, VI, WJ, ZW, ZWJ,
        XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX, XX,
    ]
};

impl Lb {
    fn of(c: char) -> Lb {
        LB_TABLE[((entry(c as u32) >> LB_SHIFT) & LB_MASK) as usize]
    }

    /// Any of the three QU flavours.
    fn is_qu(self) -> bool {
        matches!(self, Lb::QU | Lb::QUPi | Lb::QUPf)
    }

    /// A mandatory break class.
    fn is_hard(self) -> bool {
        matches!(self, Lb::BK | Lb::CR | Lb::LF | Lb::NL)
    }
}

/// Whether a code point is East Asian Fullwidth, Wide or Halfwidth. Several
/// rules are conditioned on this, and it is a different test from the width
/// table's notion of "wide".
fn east_asian(c: char) -> bool {
    entry(c as u32) & EAST_ASIAN != 0
}

/// Extended_Pictographic and unassigned, which LB30b treats as an emoji base.
fn extpict_unassigned(c: char) -> bool {
    entry(c as u32) & EXTPICT_UNASSIGNED != 0
}

/// What kind of break opportunity this is.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Break {
    /// A line *must* end here: the text contains an explicit line separator.
    Mandatory,
    /// A line *may* end here.
    Allowed,
}

/// Iterator over line break opportunities, as byte offsets into the string.
///
/// Created by [`line_breaks`](crate::line_breaks). Offsets point *after* the
/// character that ends the line, so `&s[..offset]` is a complete line. The last
/// offset is always `s.len()`, per rule LB3.
#[derive(Clone)]
pub struct LineBreaks<'a> {
    s: &'a str,
    /// Byte offset of the character being considered.
    pos: usize,
    /// How many characters have been consumed, so `sot` can be recognised.
    seen: usize,
    /// The previous character, with LB9 already applied.
    prev: Ch,
    /// The one before that, which LB19a needs.
    prev2: Option<Ch>,
    /// The last character that was not a space, for the `X SP* ×` rules.
    nonsp: Ch,
    /// Whatever came immediately before `nonsp`, space or not. `None` at the
    /// start of text. LB15a, LB20a, LB21a and LB28a are all conditioned on it.
    ctx: Option<Ch>,
    /// Consecutive spaces immediately behind us.
    spaces: usize,
    /// Unbroken regional indicators behind us, for LB30a.
    ri: usize,
    /// Tracks the numeric run for LB25.
    num: Num,
    /// The previous character was a zero width joiner, for LB8a.
    prev_zwj: bool,
    /// Emitted the final LB3 break.
    done: bool,
}

/// One character's worth of the properties the rules ask about.
#[derive(Clone, Copy, PartialEq, Eq)]
struct Ch {
    class: Lb,
    /// East Asian Fullwidth, Wide or Halfwidth.
    ea: bool,
    /// U+25CC DOTTED CIRCLE, which LB28a treats as a Brahmic placeholder.
    dotted: bool,
    /// Extended_Pictographic and unassigned, for LB30b.
    pict: bool,
}

impl Ch {
    const NONE: Ch = Ch {
        class: Lb::XX,
        ea: false,
        dotted: false,
        pict: false,
    };

    fn new(c: char, class: Lb) -> Ch {
        Ch {
            class,
            ea: east_asian(c),
            dotted: c == '\u{25CC}',
            pict: extpict_unassigned(c),
        }
    }
}

impl<'a> LineBreaks<'a> {
    pub(crate) fn new(s: &'a str) -> Self {
        LineBreaks {
            s,
            pos: 0,
            seen: 0,
            prev: Ch::NONE,
            prev2: None,
            nonsp: Ch::NONE,
            ctx: None,
            spaces: 0,
            ri: 0,
            num: Num::No,
            prev_zwj: false,
            done: false,
        }
    }
}

/// Tracks `NU (NU | SY | IS)* (CL | CP)?` for rule LB25.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Num {
    No,
    /// Inside the digits, or the symbols and separators that may follow them.
    In,
    /// Past the closing bracket that may follow the digits.
    Closed,
}

impl Iterator for LineBreaks<'_> {
    type Item = (usize, Break);

    fn next(&mut self) -> Option<(usize, Break)> {
        loop {
            let Some(c) = self.s[self.pos..].chars().next() else {
                // LB3: always break at the end of text.
                if self.done {
                    return None;
                }
                self.done = true;
                return Some((self.s.len(), Break::Mandatory));
            };
            let at = self.pos;
            self.pos += c.len_utf8();
            let raw = Lb::of(c);

            // LB9: a combining mark or joiner is part of what precedes it, so
            // it neither breaks nor changes the class the rules compare.
            // LB10: unless there is nothing to attach to, in which case it is
            // an ordinary alphabetic.
            let attaches = self.seen > 0
                && !(self.prev.class.is_hard() || matches!(self.prev.class, Lb::SP | Lb::ZW));
            if matches!(raw, Lb::CM | Lb::ZWJ) && attaches {
                // LB8a still applies to a joiner absorbed this way.
                self.prev_zwj = raw == Lb::ZWJ;
                continue;
            }
            let resolved = match raw {
                Lb::CM | Lb::ZWJ => Lb::AL,
                other => other,
            };
            let ch = Ch::new(c, resolved);

            // LB2: never break at the start of text.
            let verdict = if self.seen == 0 {
                None
            } else {
                self.decide(ch, at + c.len_utf8())
            };
            self.consume(ch, raw == Lb::ZWJ);
            if let Some(kind) = verdict {
                return Some((at, kind));
            }
        }
    }
}

impl LineBreaks<'_> {
    /// Advance the state machine past one character.
    fn consume(&mut self, ch: Ch, was_zwj: bool) {
        if ch.class != Lb::SP {
            self.ctx = if self.seen == 0 {
                None
            } else {
                Some(self.prev)
            };
            self.nonsp = ch;
            self.spaces = 0;
        } else {
            self.spaces += 1;
        }
        self.ri = if ch.class == Lb::RI { self.ri + 1 } else { 0 };
        self.num = match (self.num, ch.class) {
            (_, Lb::NU) => Num::In,
            (Num::In, Lb::SY | Lb::IS) => Num::In,
            (Num::In, Lb::CL | Lb::CP) => Num::Closed,
            _ => Num::No,
        };
        self.prev2 = if self.seen == 0 {
            None
        } else {
            Some(self.prev)
        };
        self.prev = ch;
        self.prev_zwj = was_zwj;
        self.seen += 1;
    }

    /// Class of the character starting at byte offset `i`, if there is one.
    ///
    /// `i` always comes from a character boundary this iterator has already
    /// walked past, so the slice cannot be out of range.
    fn class_at(&self, i: usize) -> Option<Lb> {
        self.s[i..].chars().next().map(Lb::of)
    }

    /// Whether the character at byte offset `i` is East Asian.
    fn ea_at(&self, i: usize) -> Option<bool> {
        self.s[i..].chars().next().map(east_asian)
    }

    /// Should a line break be allowed before the character `cur`, which ends at
    /// byte offset `end`? Rules are checked in the order UAX #14 lists them;
    /// the first that matches decides.
    fn decide(&self, cur: Ch, end: usize) -> Option<Break> {
        use Lb::*;
        let b = self.prev.class;
        let a = cur.class;
        let nsp = self.nonsp.class;
        let brk = Some(Break::Allowed);
        let stay = None;
        // Brahmic "consonant or placeholder", used throughout LB28a.
        let ak = |c: Ch| matches!(c.class, AK | AS) || c.dotted;

        // LB4, LB5: mandatory breaks.
        if b == BK {
            return Some(Break::Mandatory);
        }
        if b == CR && a == LF {
            return stay;
        }
        if matches!(b, CR | LF | NL) {
            return Some(Break::Mandatory);
        }
        // LB6, LB7.
        if a.is_hard() || matches!(a, SP | ZW) {
            return stay;
        }
        // LB8: break after a zero width space, and after any spaces past it.
        if nsp == ZW {
            return brk;
        }
        // LB8a.
        if self.prev_zwj {
            return stay;
        }
        // LB11.
        if a == WJ || b == WJ {
            return stay;
        }
        // LB12, LB12a. An unambiguous hyphen does not bind to what follows.
        if b == GL {
            return stay;
        }
        if a == GL && !matches!(b, SP | BA | HY | HH) {
            return stay;
        }
        // LB13.
        if matches!(a, CL | CP | EX | SY) {
            return stay;
        }
        // LB14.
        if nsp == OP {
            return stay;
        }
        // LB15a: an opening quote, after something that can precede one.
        if nsp == QUPi
            && match self.ctx {
                None => true,
                Some(x) => {
                    x.class.is_hard() || matches!(x.class, OP | GL | SP | ZW) || x.class.is_qu()
                }
            }
        {
            return stay;
        }
        // LB15b: a closing quote, before something that can follow one.
        if a == QUPf {
            match self.class_at(end) {
                None => return stay,
                Some(n)
                    if n.is_qu()
                        || n.is_hard()
                        || matches!(n, SP | GL | WJ | CL | CP | EX | IS | SY | ZW) =>
                {
                    return stay
                }
                _ => {}
            }
        }
        // LB15c, LB15d.
        if b == SP && a == IS && self.class_at(end) == Some(NU) {
            return brk;
        }
        if a == IS {
            return stay;
        }
        // LB16, LB17.
        if matches!(nsp, CL | CP) && a == NS {
            return stay;
        }
        if nsp == B2 && a == B2 {
            return stay;
        }
        // LB18: break after a space.
        if b == SP {
            return brk;
        }
        // LB19, LB19a: quotation marks, unless walled in by East Asian text.
        // An initial quote does not bind to what precedes it, and a final quote
        // does not bind to what follows, so each half of LB19 excludes one.
        if matches!(a, QU | QUPf) && !cur.ea {
            return stay;
        }
        if matches!(b, QU | QUPi) && !self.prev.ea {
            return stay;
        }
        if a.is_qu() && (!self.prev.ea || self.ea_at(end) != Some(true)) {
            return stay;
        }
        if b.is_qu() && (!cur.ea || self.prev2.map(|p| p.ea) != Some(true)) {
            return stay;
        }
        // LB20.
        if a == CB || b == CB {
            return brk;
        }
        // LB20a: a hyphen that starts a word keeps the word with it.
        if matches!(a, AL | HL)
            && matches!(b, HY | HH)
            && match self.ctx {
                None => true,
                Some(x) => x.class.is_hard() || matches!(x.class, SP | ZW | CB | GL),
            }
        {
            return stay;
        }
        // LB21.
        if matches!(a, BA | HH | HY | NS) {
            return stay;
        }
        if b == BB {
            return stay;
        }
        // LB21a: Hebrew keeps its hyphen and whatever follows it. Only a
        // hyphen: `HL BA` does break, which the conformance suite is explicit
        // about.
        if b == HY && self.ctx.map(|x| x.class) == Some(HL) && a != HL {
            return stay;
        }
        // LB21b.
        if b == SY && a == HL {
            return stay;
        }
        // LB22.
        if a == IN {
            return stay;
        }
        // LB23, LB23a.
        if matches!(b, AL | HL) && a == NU {
            return stay;
        }
        if b == NU && matches!(a, AL | HL) {
            return stay;
        }
        if b == PR && matches!(a, ID | EB | EM) {
            return stay;
        }
        if matches!(b, ID | EB | EM) && a == PO {
            return stay;
        }
        // LB24.
        if matches!(b, PR | PO) && matches!(a, AL | HL) {
            return stay;
        }
        if matches!(b, AL | HL) && matches!(a, PR | PO) {
            return stay;
        }
        // LB25: do not break numbers.
        if matches!(b, PR | PO) {
            if a == NU {
                return stay;
            }
            if matches!(a, OP | HY) && self.class_at(end) == Some(NU) {
                return stay;
            }
        }
        if matches!(b, OP | HY | IS) && a == NU {
            return stay;
        }
        if self.num == Num::In && matches!(a, NU | SY | IS | CL | CP) {
            return stay;
        }
        if matches!(self.num, Num::In | Num::Closed) && matches!(a, PO | PR) {
            return stay;
        }
        // LB26, LB27: Hangul syllable blocks.
        if b == JL && matches!(a, JL | JV | H2 | H3) {
            return stay;
        }
        if matches!(b, JV | H2) && matches!(a, JV | JT) {
            return stay;
        }
        if matches!(b, JT | H3) && a == JT {
            return stay;
        }
        if matches!(b, JL | JV | JT | H2 | H3) && a == PO {
            return stay;
        }
        if b == PR && matches!(a, JL | JV | JT | H2 | H3) {
            return stay;
        }
        // LB28.
        if matches!(b, AL | HL) && matches!(a, AL | HL) {
            return stay;
        }
        // LB28a: the orthographic syllables of Brahmic scripts.
        if b == AP && (ak(cur) || a == AS) {
            return stay;
        }
        if ak(self.prev) && matches!(a, VF | VI) {
            return stay;
        }
        if b == VI && ak(cur) && self.ctx.is_some_and(ak) {
            return stay;
        }
        if ak(self.prev) && ak(cur) && self.class_at(end) == Some(VF) {
            return stay;
        }
        // LB29.
        if b == IS && matches!(a, AL | HL) {
            return stay;
        }
        // LB30.
        if matches!(b, AL | HL | NU) && a == OP && !cur.ea {
            return stay;
        }
        if b == CP && !self.prev.ea && matches!(a, AL | HL | NU) {
            return stay;
        }
        // LB30a: flags pair up.
        if b == RI && a == RI && self.ri % 2 == 1 {
            return stay;
        }
        // LB30b.
        if b == EB && a == EM {
            return stay;
        }
        if self.prev.pict && a == EM {
            return stay;
        }
        // LB31: break everywhere else.
        brk
    }
}
