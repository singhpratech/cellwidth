//! Laying out a grid of text so the columns line up.
//!
//! This is the thing everyone writes by hand and gets subtly wrong. It uses the
//! rest of the crate: columns are measured in terminal cells, cells are wrapped
//! at UAX #14 opportunities, and colour codes survive the process.

use alloc::borrow::Cow;
use alloc::string::String;
use alloc::vec::Vec;

use crate::ansi::{Piece, Pieces};
use crate::measure::Width;

/// Horizontal alignment within a column.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub enum Align {
    /// Against the left edge. The default.
    #[default]
    Left,
    /// Centred, with an odd column going to the right.
    Center,
    /// Against the right edge. Usually what numbers want.
    Right,
}

/// How wide a column should be.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum Sizing {
    /// As wide as its widest cell. The default.
    #[default]
    Auto,
    /// Exactly this many columns, truncating or padding as needed.
    Fixed(usize),
    /// As wide as its widest cell, but never more than this.
    Max(usize),
}

/// The characters a table is drawn with.
#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
#[non_exhaustive]
pub enum Border {
    /// Box drawing characters. The default.
    #[default]
    Light,
    /// Heavier box drawing characters.
    Heavy,
    /// `+`, `-` and `|`, for terminals or logs that mangle anything else.
    Ascii,
    /// A GitHub-flavoured Markdown table.
    Markdown,
    /// Two spaces between columns and nothing else.
    None,
}

impl Border {
    /// (horizontal, vertical, corner set) as a 11-character alphabet:
    /// h v tl tm tr ml mm mr bl bm br
    fn glyphs(self) -> Option<[&'static str; 11]> {
        Some(match self {
            Border::Light => ["─", "│", "┌", "┬", "┐", "├", "┼", "┤", "└", "┴", "┘"],
            Border::Heavy => ["━", "┃", "┏", "┳", "┓", "┣", "╋", "┫", "┗", "┻", "┛"],
            Border::Ascii => ["-", "|", "+", "+", "+", "+", "+", "+", "+", "+", "+"],
            Border::Markdown => ["-", "|", "", "", "", "|", "|", "|", "", "", ""],
            Border::None => return None,
        })
    }
}

/// One column's configuration.
#[derive(Clone, Debug)]
struct Column {
    header: String,
    align: Align,
    sizing: Sizing,
}

/// A text table that lines up in a terminal.
///
/// ```
/// use cellwidth::{Align, Border, Table};
///
/// let table = Table::new()
///     .column("host")
///     .column_aligned("cost", Align::Right)
///     .row(["\u{6771}\u{4EAC}-01", "1,200"])
///     .row(["berlin-7", "980"])
///     .border(Border::Ascii);
/// let out = table.render(None);
/// // Every line is the same number of columns wide, whatever is in the cells.
/// let widths: Vec<usize> = out.lines().map(cellwidth::width).collect();
/// assert!(widths.windows(2).all(|w| w[0] == w[1]), "{out}");
/// ```
#[derive(Clone, Debug, Default)]
pub struct Table {
    columns: Vec<Column>,
    rows: Vec<Vec<String>>,
    border: Border,
    width: Width,
    padding: usize,
    header: bool,
}

impl Table {
    /// An empty table with light box-drawing borders and a header row.
    pub fn new() -> Self {
        Table {
            columns: Vec::new(),
            rows: Vec::new(),
            border: Border::Light,
            width: Width::DEFAULT,
            padding: 1,
            header: true,
        }
    }

    /// Add a left-aligned, auto-sized column.
    pub fn column(self, header: impl Into<String>) -> Self {
        self.column_with(header, Align::Left, Sizing::Auto)
    }

    /// Add an auto-sized column with the given alignment.
    pub fn column_aligned(self, header: impl Into<String>, align: Align) -> Self {
        self.column_with(header, align, Sizing::Auto)
    }

    /// Add a column, specifying both alignment and sizing.
    pub fn column_with(mut self, header: impl Into<String>, align: Align, sizing: Sizing) -> Self {
        self.columns.push(Column {
            header: header.into(),
            align,
            sizing,
        });
        self
    }

    /// Add a row. Missing cells are left blank; extra cells are ignored.
    pub fn row<I, S>(mut self, cells: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: Into<String>,
    {
        let mut row: Vec<String> = cells.into_iter().map(Into::into).collect();
        row.truncate(self.columns.len());
        while row.len() < self.columns.len() {
            row.push(String::new());
        }
        self.rows.push(row);
        self
    }

    /// Choose the border style.
    pub fn border(mut self, border: Border) -> Self {
        self.border = border;
        self
    }

    /// Spaces between a cell's text and the border. Defaults to one.
    pub fn padding(mut self, cells: usize) -> Self {
        self.padding = cells;
        self
    }

    /// Whether to draw the header row. Defaults to `true`.
    pub fn with_header(mut self, show: bool) -> Self {
        self.header = show;
        self
    }

    /// The measurement policy, for terminals that need a different one.
    pub fn measured_by(mut self, width: Width) -> Self {
        self.width = width;
        self
    }
}

impl Table {
    /// Cell text, ready to measure and lay out.
    ///
    /// Tabs are expanded to spaces first. A tab's width depends on the column
    /// it starts in, but a cell does not know where on the line it will be
    /// drawn, so a raw tab cannot be laid out in a grid at all: expanding it
    /// against the cell's own origin is the only stable answer.
    fn cell_text<'b>(&self, s: &'b str) -> Cow<'b, str> {
        // Skip the walk only for text that plainly needs nothing done to it.
        // Reasoning about which bytes matter is how the C1 forms get missed:
        // U+009D is an OSC introducer, but its two UTF-8 bytes both look like
        // perfectly ordinary text.
        if s.bytes().all(|b| b == b'\n' || (0x20..0x7F).contains(&b)) {
            return Cow::Borrowed(s);
        }
        // Deleting a character can leave its neighbours adjacent, and those
        // neighbours can form an escape introducer that was not there before
        // -- `ESC NUL ]` becomes `ESC ]`. Repeat until the text stops changing;
        // every pass either removes bytes or is the last one.
        let mut cur = self.sanitise(s);
        loop {
            let next = self.sanitise(&cur);
            if next == cur {
                return Cow::Owned(cur);
            }
            cur = next;
        }
    }

    /// One sanitising pass: expand tabs, drop control characters, and drop any
    /// escape sequence carrying a line separator.
    fn sanitise(&self, s: &str) -> String {
        let mut out = String::with_capacity(s.len() + 8);
        let mut col = 0;
        for piece in Pieces::new(s) {
            match piece {
                // A string sequence carrying a raw line separator is malformed
                // -- ECMA-48 allows only printable characters in the payload --
                // and would split a rendered row in half. Drop it rather than
                // let it wreck the grid.
                Piece::Escape(e) if e.contains('\n') || e.contains('\r') => {}
                Piece::Escape(e) => out.push_str(e),
                Piece::Text(t) => {
                    for g in crate::grapheme::Graphemes::new(t) {
                        // Control characters other than a line break have no
                        // place in a cell: they occupy no columns but confuse
                        // anything that reads the output back.
                        if g.chars().all(|c| c.is_control() && c != '\n' && c != '\t') {
                            continue;
                        }
                        if g == "\t" {
                            let advance = self.width.of_at("\t", col).max(1);
                            for _ in 0..advance {
                                out.push(' ');
                            }
                            col += advance;
                        } else {
                            out.push_str(g);
                            col += self.width.of_at(g, col);
                        }
                    }
                }
            }
        }
        // A cell that begins with a combining mark is not self-contained: the
        // mark would attach to the padding space drawn before it, and the row
        // would come up a column short. Unicode's own convention for showing
        // an isolated mark is to put it on a dotted circle.
        if out
            .chars()
            .next()
            .is_some_and(crate::grapheme::attaches_to_previous)
        {
            out.insert(0, '\u{25CC}');
        }
        out
    }

    /// Render the table, fitting it into `max_width` columns if given.
    ///
    /// When the natural width does not fit, the widest columns are narrowed
    /// first and their cells wrapped, so nothing is thrown away until wrapping
    /// alone cannot save it.
    pub fn render(&self, max_width: Option<usize>) -> String {
        if self.columns.is_empty() {
            return String::new();
        }
        let widths = self.resolve_widths(max_width);
        let g = self.border.glyphs();
        let pad = self.padding;
        let mut out = String::new();

        // A rule such as `+-----+-----+`, using the given corner glyphs.
        let rule = |left: &str, mid: &str, right: &str, h: &str| -> String {
            let mut r = String::from(left);
            for (i, w) in widths.iter().enumerate() {
                for _ in 0..w + pad * 2 {
                    r.push_str(h);
                }
                r.push_str(if i + 1 == widths.len() { right } else { mid });
            }
            r
        };

        if let Some(gl) = g {
            if self.border != Border::Markdown {
                out.push_str(&rule(gl[2], gl[3], gl[4], gl[0]));
                out.push('\n');
            }
        }
        if self.header {
            let cells: Vec<&str> = self.columns.iter().map(|c| c.header.as_str()).collect();
            self.push_row(&mut out, &cells, &widths, g);
            if let Some(gl) = g {
                out.push_str(&rule(gl[5], gl[6], gl[7], gl[0]));
                out.push('\n');
            }
        }
        for row in &self.rows {
            let cells: Vec<&str> = row.iter().map(String::as_str).collect();
            self.push_row(&mut out, &cells, &widths, g);
        }
        if let Some(gl) = g {
            if self.border != Border::Markdown {
                out.push_str(&rule(gl[8], gl[9], gl[10], gl[0]));
                out.push('\n');
            }
        }
        while out.ends_with('\n') {
            out.pop();
        }
        out
    }

    /// One logical row, which may occupy several lines if any cell wraps.
    fn push_row(
        &self,
        out: &mut String,
        cells: &[&str],
        widths: &[usize],
        g: Option<[&'static str; 11]>,
    ) {
        // Wrap every cell first; the row is as tall as the tallest one.
        let wrapped: Vec<Vec<String>> = cells
            .iter()
            .zip(widths)
            // `wrap` always yields at least one line for a width of one or
            // more, so a cell is never zero lines tall.
            .map(|(c, &w)| self.width.wrap(&self.cell_text(c), w.max(1)))
            .collect();
        let height = wrapped.iter().map(Vec::len).max().unwrap_or(1);

        for line in 0..height {
            if let Some(gl) = g {
                out.push_str(gl[1]);
            }
            for (i, &w) in widths.iter().enumerate() {
                let text = wrapped[i].get(line).map(String::as_str).unwrap_or("");
                if g.is_some() {
                    for _ in 0..self.padding {
                        out.push(' ');
                    }
                } else if i > 0 {
                    out.push_str("  ");
                }
                out.push_str(&self.align_cell(text, w, self.columns[i].align));
                if let Some(gl) = g {
                    for _ in 0..self.padding {
                        out.push(' ');
                    }
                    out.push_str(gl[1]);
                }
            }
            out.push('\n');
        }
    }

    /// Fit one cell's line into exactly `w` columns.
    fn align_cell<'b>(&self, text: &'b str, w: usize, align: Align) -> Cow<'b, str> {
        let fitted = self.width.cell(text, w);
        match align {
            Align::Left => fitted,
            Align::Right => {
                let have = self.width.of(text);
                if have >= w {
                    fitted
                } else {
                    self.width.pad_start(text, w)
                }
            }
            Align::Center => {
                let have = self.width.of(text);
                if have >= w {
                    fitted
                } else {
                    self.width.center(text, w)
                }
            }
        }
    }

    /// Decide how wide each column should be.
    fn resolve_widths(&self, max_width: Option<usize>) -> Vec<usize> {
        let n = self.columns.len();
        // Natural width: the widest thing that has to go in the column.
        let mut widths: Vec<usize> = self
            .columns
            .iter()
            .enumerate()
            .map(|(i, c)| {
                let header = if self.header {
                    self.width.of(&self.cell_text(&c.header))
                } else {
                    0
                };
                let body = self
                    .rows
                    .iter()
                    .map(|r| self.width.of(&self.cell_text(&r[i])))
                    .max()
                    .unwrap_or(0);
                match c.sizing {
                    Sizing::Fixed(w) => w,
                    Sizing::Max(w) => header.max(body).min(w),
                    Sizing::Auto => header.max(body),
                }
            })
            .collect();

        let Some(limit) = max_width else {
            return widths;
        };
        // Everything that is not a column: borders and padding.
        let chrome = match self.border {
            Border::None => 2 * n.saturating_sub(1),
            _ => (n + 1) + 2 * self.padding * n,
        };
        let mut total: usize = widths.iter().sum::<usize>() + chrome;
        if total <= limit {
            return widths;
        }

        // Narrow the widest flexible column, one column at a time, until it
        // fits or nothing can give any more. Fixed columns are never touched.
        let floor = 1;
        while let Some((i, _)) = widths
            .iter()
            .enumerate()
            .filter(|(i, &w)| w > floor && !matches!(self.columns[*i].sizing, Sizing::Fixed(_)))
            .max_by_key(|(_, &w)| w)
        {
            widths[i] -= 1;
            total -= 1;
            if total <= limit {
                break;
            }
        }
        widths
    }
}
