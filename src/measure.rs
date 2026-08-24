//! Column measurement.

use crate::ansi::{escape_len, Pieces};
use crate::grapheme::{first_cluster, Graphemes};
use crate::tables::{entry, WIDTH_MASK};

/// How to count East Asian Ambiguous characters.
///
/// These are characters like `±`, `£`, `°` and the box-drawing set, which
/// legacy CJK fonts render double-width and everything else renders
/// single-width. There is no correct answer available from the text alone: it
/// depends on the font the terminal is using.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum Ambiguous {
    /// One column. Correct for Western locales and the default.
    #[default]
    Narrow,
    /// Two columns. Match this to a terminal running a CJK font, typically by
    /// checking whether `LANG`/`LC_CTYPE` names a CJK locale.
    Wide,
}

/// How to count C0/C1 control characters.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum Control {
    /// Zero columns: the terminal consumes them without moving the cursor.
    /// This is the default and matches what actually happens on screen.
    #[default]
    Zero,
    /// Two columns, as caret notation (`^C`) renders them in pagers and
    /// `cat -v`-style output.
    Caret,
}

/// How much of a grapheme cluster counts towards the width.
///
/// Real terminals fall into camps here, and the difference is measurable: see
/// `probe/` and the recorded results in `results/`. This is the single biggest
/// source of disagreement between terminals, so it is a policy rather than a
/// guess.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum Clusters {
    /// Every grapheme cluster is one glyph, sized by its base character, with
    /// presentation selectors and flag pairing honoured.
    ///
    /// The default. Reproduces kitty 0.48 exactly on all 32 probe cases, and
    /// follows UAX #29 and UTS #51.
    #[default]
    WholeGlyph,
    /// Every code point counts on its own, the way `wcwidth` does: no ZWJ
    /// collapsing, no presentation selectors, no flag pairing.
    ///
    /// Matches VTE (GNOME Terminal, Tilix, Terminator, xfce4-terminal) and
    /// Alacritty. Under it a ZWJ family emoji is eight columns, because that
    /// is genuinely what those terminals draw.
    CodePoints,
}

/// A measurement policy: how to turn text into a number of terminal columns.
///
/// The free functions in the crate root use [`Width::DEFAULT`]. Build your own
/// when you need CJK ambiguous-width handling or a different tab stop:
///
/// ```
/// use cellwidth::{Ambiguous, Width};
///
/// const CJK: Width = Width::DEFAULT.ambiguous(Ambiguous::Wide);
/// assert_eq!(Width::DEFAULT.of("±"), 1);
/// assert_eq!(CJK.of("±"), 2);
/// ```
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct Width {
    ambiguous: Ambiguous,
    control: Control,
    clusters: Clusters,
    tab_stop: usize,
    ansi: bool,
}

impl Default for Width {
    fn default() -> Self {
        Width::DEFAULT
    }
}

impl Width {
    /// Sensible defaults: ambiguous characters are narrow, controls are zero
    /// columns, tab stops every 8 columns, escape sequences are skipped, and an
    /// emoji ZWJ sequence counts as the single glyph it is meant to render as.
    pub const DEFAULT: Width = Width {
        ambiguous: Ambiguous::Narrow,
        control: Control::Zero,
        clusters: Clusters::WholeGlyph,
        tab_stop: 8,
        ansi: true,
    };

    /// The same as [`Width::DEFAULT`], named for when the contrast with
    /// [`Width::LEGACY`] is the point.
    pub const MODERN: Width = Width::DEFAULT;

    /// Matches terminals that count each code point separately, as `wcwidth`
    /// does.
    ///
    /// This is VTE (GNOME Terminal, Tilix, Terminator, xfce4-terminal) and
    /// Alacritty. Under it a ZWJ family emoji is eight columns, not two,
    /// because that is genuinely what those terminals draw.
    pub const LEGACY: Width = Width {
        clusters: Clusters::CodePoints,
        ..Width::DEFAULT
    };

    /// How much of a grapheme cluster counts towards the width.
    pub const fn clusters(mut self, c: Clusters) -> Self {
        self.clusters = c;
        self
    }

    /// How to count East Asian Ambiguous characters.
    pub const fn ambiguous(mut self, a: Ambiguous) -> Self {
        self.ambiguous = a;
        self
    }

    /// How to count control characters.
    pub const fn control(mut self, c: Control) -> Self {
        self.control = c;
        self
    }

    /// Distance between tab stops. `0` makes a tab occupy no columns.
    ///
    /// Because a tab's width depends on where it starts, measurements assume
    /// the text begins at column 0.
    pub const fn tab_stop(mut self, n: usize) -> Self {
        self.tab_stop = n;
        self
    }

    /// Whether to recognise and skip ANSI escape sequences (default `true`).
    ///
    /// Turn this off to measure escape sequences as the literal characters they
    /// are made of, which is what you want when displaying them for debugging.
    pub const fn ansi(mut self, enabled: bool) -> Self {
        self.ansi = enabled;
        self
    }

    /// Columns occupied by a single `char`.
    ///
    /// Beware that a `char` is not a user-perceived character: the width of
    /// `'\u{301}'` (combining acute) is 0, and the width of a flag is only
    /// defined for the pair. Prefer [`Width::of`] for real text.
    pub fn of_char(&self, c: char) -> usize {
        let cp = c as u32;
        if cp < 0x7F {
            return if cp >= 0x20 {
                1
            } else if c == '\t' {
                self.tab_stop
            } else {
                self.control_width()
            };
        }
        if (0x7F..=0x9F).contains(&cp) {
            return self.control_width();
        }
        if self.clusters == Clusters::CodePoints {
            match cp {
                // A skin tone modifier drawn on its own is a Wide glyph.
                0x1F3FB..=0x1F3FF => return 2,
                // A regional indicator is East Asian Neutral; only the pairing
                // rule, which this policy does not apply, makes it wide.
                0x1F1E6..=0x1F1FF => return 1,
                _ => {}
            }
        }
        match entry(cp) & WIDTH_MASK {
            0 => 0,
            2 => 2,
            // East Asian Ambiguous: the answer depends on the terminal's font,
            // so it is a policy choice rather than a property of the text.
            3 if self.ambiguous == Ambiguous::Wide => 2,
            _ => 1,
        }
    }

    fn control_width(&self) -> usize {
        match self.control {
            Control::Zero => 0,
            Control::Caret => 2,
        }
    }

    /// Columns occupied by one grapheme cluster, starting at column `col`.
    ///
    /// `col` only matters for tabs. Escape sequences are not expected here;
    /// [`Width::of`] strips them before splitting into clusters.
    pub(crate) fn advance(&self, cluster: &str, col: usize) -> usize {
        let bytes = cluster.as_bytes();
        // Fast path: a lone ASCII printable, which is the overwhelming majority.
        if bytes.len() == 1 {
            let b = bytes[0];
            if (0x20..0x7F).contains(&b) {
                return 1;
            }
            if b == b'\t' {
                return self.tab_advance(col);
            }
            return self.control_width();
        }

        // wcwidth semantics: no cluster rules at all, just add up the pieces.
        // Tabs cannot appear here -- a tab is one byte and takes the fast path
        // above -- so the running column is not needed.
        if self.clusters == Clusters::CodePoints {
            return cluster.chars().map(|c| self.of_char(c)).sum();
        }

        let mut sum = 0;
        let mut first = None;
        let mut ri = 0;
        let mut modifier = false;
        let mut vs16 = false;
        let mut vs15 = false;
        for c in cluster.chars() {
            match c {
                '\u{FE0F}' => vs16 = true,
                '\u{FE0E}' => vs15 = true,
                '\u{1F1E6}'..='\u{1F1FF}' => ri += 1,
                '\u{1F3FB}'..='\u{1F3FF}' => modifier = true,
                _ => {}
            }
            let w = self.of_char(c);
            sum += w;
            first.get_or_insert(w);
        }

        // A regional indicator pair is one flag glyph. Grapheme clustering
        // never puts more than two in a cluster, but it can attach combining
        // marks to them, so the count of other characters is not a condition.
        if ri == 2 {
            return 2;
        }
        // An explicit presentation selector overrides the base character: `❤`
        // is a 1-column dingbat, `❤️` is a 2-column emoji.
        // A skin tone modifier forces emoji presentation too: UTS #51 makes an
        // emoji modifier sequence always emoji-presented, so the text-default
        // hand sign U+270C is one column but U+270C U+1F3FD is two.
        if vs16 || modifier {
            return 2;
        }
        if vs15 {
            return 1;
        }
        // The cluster is one glyph, sized by the character it is built on: a
        // ZWJ sequence by its lead emoji, an accented letter by the letter, an
        // Indic conjunct by its first consonant.
        first.filter(|&w| w > 0).unwrap_or(sum)
    }

    fn tab_advance(&self, col: usize) -> usize {
        if self.tab_stop == 0 {
            0
        } else {
            self.tab_stop - (col % self.tab_stop)
        }
    }

    /// Columns occupied by a string.
    ///
    /// Assumes a single line starting at column 0; split on newlines yourself
    /// if you have more than one. Escape sequences count as zero unless
    /// [`Width::ansi`] is off.
    ///
    /// ```
    /// use cellwidth::Width;
    /// assert_eq!(Width::DEFAULT.of("日本語"), 6);
    /// ```
    pub fn of(&self, s: &str) -> usize {
        self.of_at(s, 0)
    }

    /// Columns `s` occupies when it starts at column `start`.
    ///
    /// Only tabs make this differ from [`Width::of`], but when they do it
    /// matters: a tab after four characters advances four columns, and the same
    /// tab after eight advances eight. Width is not additive across a tab, so
    /// anything laying text out in pieces has to measure each piece where it
    /// will actually sit.
    ///
    /// ```
    /// use cellwidth::Width;
    /// let w = Width::DEFAULT;
    /// assert_eq!(w.of("\t"), 8);
    /// assert_eq!(w.of_at("\t", 4), 4);
    /// assert_eq!(w.of_at("\t", 8), 8);
    /// ```
    pub fn of_at(&self, s: &str, start: usize) -> usize {
        let mut col = start;
        if !self.ansi {
            return self.of_text(s, start);
        }
        for piece in Pieces::new(s) {
            if let crate::ansi::Piece::Text(t) = piece {
                col += self.of_text(t, col);
            }
        }
        col - start
    }

    /// Width of a run known to contain no escape sequences.
    fn of_text(&self, s: &str, start_col: usize) -> usize {
        let mut col = start_col;
        // Pure-ASCII printable runs need no segmentation at all.
        if s.is_ascii() && s.bytes().all(|b| (0x20..0x7F).contains(&b)) {
            return s.len();
        }
        // The per-code-point model needs no segmentation: every cluster's
        // advance is the sum of its characters', and a tab is always a cluster
        // of its own, so the running column is the same either way.
        if self.clusters == Clusters::CodePoints {
            for c in s.chars() {
                col += if c == '\t' {
                    self.tab_advance(col)
                } else {
                    self.of_char(c)
                };
            }
            return col - start_col;
        }
        for cluster in Graphemes::new(s) {
            col += self.advance(cluster, col);
        }
        col - start_col
    }

    /// Columns occupied by one grapheme cluster, as if printed at column 0.
    ///
    /// ```
    /// use cellwidth::Width;
    /// assert_eq!(Width::DEFAULT.of_grapheme("👍🏽"), 2);
    /// ```
    pub fn of_grapheme(&self, cluster: &str) -> usize {
        self.advance(cluster, 0)
    }

    /// Whether this policy is skipping escape sequences.
    #[cfg(feature = "alloc")]
    pub(crate) fn ansi_enabled(&self) -> bool {
        self.ansi
    }

    /// Byte length of an escape sequence at `i`, or `None` if escapes are off
    /// or none starts there.
    pub(crate) fn escape_at(&self, s: &str, i: usize) -> Option<usize> {
        if self.ansi {
            escape_len(s.as_bytes(), i)
        } else {
            None
        }
    }

    /// Longest prefix of `s` that fits in `max` columns.
    ///
    /// Borrows: no allocation, ever. Cuts only at cluster boundaries and never
    /// inside an escape sequence. Escape sequences already passed are kept,
    /// which means the result can leave styling switched on -- use
    /// [`Width::truncate_ellipsis`] or [`Width::cell`] if you need the reset
    /// handled for you.
    ///
    /// ```
    /// use cellwidth::Width;
    /// assert_eq!(Width::DEFAULT.truncate("日本語テキスト", 7), "日本語");
    /// assert_eq!(Width::DEFAULT.truncate("héllo", 3), "hél");
    /// ```
    pub fn truncate<'a>(&self, s: &'a str, max: usize) -> &'a str {
        let mut i = 0;
        let mut col = 0;
        while i < s.len() {
            if let Some(len) = self.escape_at(s, i) {
                i += len;
                continue;
            }
            // Non-empty by the loop condition, so this always advances.
            let cluster = first_cluster(&s[i..]);
            let w = self.advance(cluster, col);
            if col + w > max {
                break;
            }
            col += w;
            i += cluster.len();
        }
        &s[..i]
    }
}
