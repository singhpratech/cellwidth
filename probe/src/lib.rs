//! Shared list of strings whose width real terminals are known, or suspected,
//! to disagree about.

/// One probe case: a stable id, the text to draw, and why it is interesting.
pub struct Case {
    pub id: &'static str,
    pub text: &'static str,
    pub note: &'static str,
}

/// The cases. Anything here is either a sanity check or a place where
/// `cellwidth` had to make a judgement call that only a terminal can settle.
pub const CASES: &[Case] = &[
    // --- sanity: if these are wrong, the harness is wrong, not the terminal ---
    Case {
        id: "ascii",
        text: "abc",
        note: "plain ASCII",
    },
    Case {
        id: "cjk",
        text: "\u{65E5}\u{672C}\u{8A9E}",
        note: "East Asian Wide",
    },
    Case {
        id: "accent-precomp",
        text: "caf\u{E9}",
        note: "precomposed e-acute",
    },
    Case {
        id: "accent-decomp",
        text: "cafe\u{301}",
        note: "combining acute",
    },
    // --- emoji presentation ---
    Case {
        id: "emoji-basic",
        text: "\u{1F600}",
        note: "emoji-presentation base",
    },
    Case {
        id: "zwj-family",
        text: "\u{1F468}\u{200D}\u{1F469}\u{200D}\u{1F467}\u{200D}\u{1F466}",
        note: "ZWJ sequence: one glyph or four?",
    },
    Case {
        id: "zwj-astronaut",
        text: "\u{1F469}\u{1F3FE}\u{200D}\u{1F680}",
        note: "modifier plus ZWJ",
    },
    Case {
        id: "skin-tone",
        text: "\u{1F44D}\u{1F3FD}",
        note: "emoji base plus modifier",
    },
    Case {
        id: "text-emoji-mod",
        text: "\u{270C}\u{1F3FB}",
        note: "text-default base plus modifier",
    },
    Case {
        id: "vs16",
        text: "\u{2764}\u{FE0F}",
        note: "VS16 asks for emoji presentation",
    },
    Case {
        id: "vs15",
        text: "\u{231A}\u{FE0E}",
        note: "VS15 asks for text presentation",
    },
    Case {
        id: "bare-heart",
        text: "\u{2764}",
        note: "no selector: text dingbat",
    },
    Case {
        id: "keycap",
        text: "1\u{FE0F}\u{20E3}",
        note: "keycap sequence",
    },
    Case {
        id: "flag",
        text: "\u{1F1EF}\u{1F1F5}",
        note: "regional indicator pair",
    },
    Case {
        id: "lone-ri",
        text: "\u{1F1EF}",
        note: "unpaired regional indicator",
    },
    // --- the decisions cellwidth had to make on its own ---
    Case {
        id: "bengali-ka-aa",
        text: "\u{0995}\u{09BE}",
        note: "Mc spacing mark that is also Other_Grapheme_Extend",
    },
    Case {
        id: "devanagari-ksi",
        text: "\u{0915}\u{094D}\u{0937}\u{093F}",
        note: "Indic conjunct with shaping",
    },
    Case {
        id: "hangul-filler",
        text: "\u{3164}",
        note: "blank, but East Asian Wide",
    },
    Case {
        id: "choseong-filler",
        text: "\u{115F}",
        note: "default-ignorable and Wide",
    },
    Case {
        id: "halfwidth-dakuten",
        text: "\u{FF76}\u{FF9E}",
        note: "halfwidth katakana plus sound mark",
    },
    Case {
        id: "hangul-tone",
        text: "\u{AC00}\u{302E}",
        note: "Hangul syllable plus tone mark",
    },
    Case {
        id: "jamo-decomposed",
        text: "\u{1112}\u{1161}\u{11AB}",
        note: "L+V+T composing into one syllable",
    },
    Case {
        id: "trigram",
        text: "\u{2630}",
        note: "became Wide in Unicode 16",
    },
    Case {
        id: "counting-rod",
        text: "\u{1D360}",
        note: "became Wide in Unicode 16",
    },
    Case {
        id: "arabic-number-sign",
        text: "\u{0600}9",
        note: "invisible prepend format character",
    },
    Case {
        id: "soft-hyphen",
        text: "a\u{AD}b",
        note: "soft hyphen",
    },
    // --- East Asian Ambiguous: the answer depends on the font ---
    Case {
        id: "amb-plusminus",
        text: "\u{B1}",
        note: "ambiguous",
    },
    Case {
        id: "amb-degree",
        text: "\u{B0}",
        note: "ambiguous",
    },
    Case {
        id: "amb-section",
        text: "\u{A7}",
        note: "ambiguous",
    },
    Case {
        id: "amb-alpha",
        text: "\u{3B1}",
        note: "ambiguous",
    },
    Case {
        id: "amb-arrow",
        text: "\u{2192}",
        note: "ambiguous",
    },
    Case {
        id: "amb-boxdraw",
        text: "\u{2500}",
        note: "ambiguous",
    },
];

/// Render a string as space-separated code points, for the report.
pub fn codepoints(s: &str) -> String {
    s.chars()
        .map(|c| format!("{:04X}", c as u32))
        .collect::<Vec<_>>()
        .join(" ")
}
