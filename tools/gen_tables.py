#!/usr/bin/env python3
"""Generate src/tables.rs from the Unicode Character Database.

Build-time only. The generated tables are committed, so the crate itself
has zero dependencies (and no build script).

Usage: python3 tools/gen_tables.py <ucd-dir> > src/tables.rs
"""
import sys, os, re
from collections import defaultdict

UCD = sys.argv[1]
def path(n): return os.path.join(UCD, os.path.basename(n))

def parse_ranges(fname, wanted):
    """Yield (lo, hi, prop) for `prop in wanted` from a semicolon UCD file."""
    out = defaultdict(list)
    with open(path(fname), encoding="utf-8") as f:
        for line in f:
            line = line.split("#", 1)[0].strip()
            if not line:
                continue
            parts = [p.strip() for p in line.split(";")]
            prop = "; ".join(parts[1:])
            if prop not in wanted:
                continue
            cps = parts[0]
            if ".." in cps:
                lo, hi = (int(x, 16) for x in cps.split(".."))
            else:
                lo = hi = int(cps, 16)
            out[prop].append((lo, hi))
    return out

def unicode_data():
    """category per code point, expanding First>/Last> ranges."""
    cats = {}
    first = None
    with open(path("UnicodeData.txt"), encoding="utf-8") as f:
        for line in f:
            fs = line.split(";")
            cp, name, cat = int(fs[0], 16), fs[1], fs[2]
            if name.endswith(", First>"):
                first = (cp, cat)
                continue
            if name.endswith(", Last>"):
                for c in range(first[0], cp + 1):
                    cats[c] = first[1]
                first = None
                continue
            cats[cp] = cat
    return cats

def coalesce(ranges):
    """Sort and merge adjacent/overlapping ranges."""
    rs = sorted(ranges)
    out = []
    for lo, hi in rs:
        if out and lo <= out[-1][1] + 1:
            out[-1] = (out[-1][0], max(out[-1][1], hi))
        else:
            out.append((lo, hi))
    return out

def from_set(cps):
    out, run = [], None
    for c in sorted(cps):
        if run and c == run[1] + 1:
            run = (run[0], c)
            out[-1] = run
        else:
            run = (c, c)
            out.append(run)
    return out

# ---------------------------------------------------------------- width data
eaw = parse_ranges("EastAsianWidth.txt", {"W", "F", "A"})
wide = coalesce(eaw["W"] + eaw["F"])
ambiguous = coalesce(eaw["A"])

dcp = parse_ranges("DerivedCoreProperties.txt",
                   {"Grapheme_Extend", "Default_Ignorable_Code_Point",
                    "InCB; Linker", "InCB; Consonant", "InCB; Extend"})
cats = unicode_data()

# Nonspacing and enclosing marks are the characters that genuinely do not
# advance the cursor. Grapheme_Extend is deliberately *not* used here: it also
# contains Other_Grapheme_Extend, a set of visible spacing marks (Bengali
# vowel sign AA and friends) that exists for segmentation, not for display.
zero = {cp for cp, cat in cats.items() if cat in ("Mn", "Me")}
for lo, hi in dcp["Default_Ignorable_Code_Point"]:
    zero.update(range(lo, hi + 1))
# Format characters (Cf) are not rendered: ZWJ, ZWNJ, bidi controls, VS, ...
zero.update(cp for cp, cat in cats.items() if cat == "Cf")
# Conjoining Hangul Jungseong/Jongseong compose onto the preceding Choseong.
zero.update(range(0x1160, 0x1200))   # V + T
zero.update(range(0xD7B0, 0xD7C7))   # extended V
zero.update(range(0xD7CB, 0xD7FC))   # extended T

emoji = parse_ranges("emoji-data.txt",
                     {"Emoji_Presentation", "Extended_Pictographic", "Emoji_Modifier"})
# Skin tone modifiers are Grapheme_Cluster_Break=Extend but not Grapheme_Extend,
# so they need adding by hand: they recolour the preceding emoji rather than
# taking cells of their own.
for lo, hi in emoji["Emoji_Modifier"]:
    zero.update(range(lo, hi + 1))

# Not everything that is ignorable is invisible. A handful of characters are
# Default_Ignorable yet carry an explicit East Asian Width,
# which means the terminal really does advance the cursor for them: the Hangul
# fillers. Nonspacing and enclosing marks (Mn, Me) never advance, whatever their
# East Asian Width says, and emoji modifiers merge into the glyph they follow.
eaw_of = {}
for prop in ("W", "F", "H"):
    for lo, hi in parse_ranges("EastAsianWidth.txt", {prop})[prop]:
        for c in range(lo, hi + 1):
            eaw_of[c] = prop
emod = set()
for lo, hi in emoji["Emoji_Modifier"]:
    emod.update(range(lo, hi + 1))
spacing = {
    c for c in zero
    if c in eaw_of and cats.get(c) not in ("Mn", "Me") and c not in emod
}
zero -= spacing
sys.stderr.write("kept spacing despite combining: "
                 + " ".join(f"U+{c:04X}" for c in sorted(spacing)) + "\n")

zero_ranges = coalesce(from_set(zero))
emoji_pres = coalesce(emoji["Emoji_Presentation"])

# --------------------------------------------------------- grapheme classes
# Values must match the `Gcb` enum in src/grapheme.rs.
GCB = ["Other", "CR", "LF", "Control", "Extend", "ZWJ", "RegionalIndicator",
       "Prepend", "SpacingMark", "L", "V", "T", "LV", "LVT",
       "ExtPict", "InCBLinker", "InCBConsonant", "InCBExtend"]
IDX = {n: i for i, n in enumerate(GCB)}

gbp = parse_ranges("GraphemeBreakProperty.txt",
                   {"CR", "LF", "Control", "Extend", "ZWJ", "Regional_Indicator",
                    "Prepend", "SpacingMark", "L", "V", "T", "LV", "LVT"})
cls = {}
alias = {"Regional_Indicator": "RegionalIndicator"}
for prop, rs in gbp.items():
    name = alias.get(prop, prop)
    for lo, hi in rs:
        for c in range(lo, hi + 1):
            cls[c] = IDX[name]

# Extended_Pictographic only matters for GB11 (ZWJ sequences) and only for
# code points that are otherwise `Other`; it must not mask Extend/Control.
for lo, hi in emoji["Extended_Pictographic"]:
    for c in range(lo, hi + 1):
        if c not in cls:
            cls[c] = IDX["ExtPict"]

# GB9c (Unicode 15.1) needs the Indic conjunct-break properties. Linker and
# InCB=Extend are both GCB=Extend, so they only ever refine that class -- never
# ZWJ, which is GCB=ZWJ but InCB=Extend and is special-cased in the state machine.
for lo, hi in dcp["InCB; Extend"]:
    for c in range(lo, hi + 1):
        if cls.get(c, IDX["Other"]) == IDX["Extend"]:
            cls[c] = IDX["InCBExtend"]
for lo, hi in dcp["InCB; Linker"]:
    for c in range(lo, hi + 1):
        if cls.get(c, IDX["Other"]) == IDX["Extend"]:
            cls[c] = IDX["InCBLinker"]
for lo, hi in dcp["InCB; Consonant"]:
    for c in range(lo, hi + 1):
        if cls.get(c, IDX["Other"]) == IDX["Other"]:
            cls[c] = IDX["InCBConsonant"]

# collapse to (lo, hi, class) runs, dropping Other
runs = []
for c in sorted(cls):
    k = cls[c]
    if runs and runs[-1][2] == k and c == runs[-1][1] + 1:
        runs[-1] = (runs[-1][0], c, k)
    else:
        runs.append((c, c, k))

# --------------------------------------------------------- line break classes
# Line_Break, with the LB1 resolution baked in (AI/SG/XX -> AL, SA -> CM or AL
# depending on category, CJ -> NS) and QU split by Initial/Final punctuation,
# which rules LB15a and LB15b need to tell apart.
LB = ["XX", "AK", "AL", "AP", "AS", "B2", "BA", "BB", "BK", "CB", "CL", "CM",
      "CP", "CR", "EB", "EM", "EX", "GL", "H2", "H3", "HH", "HL", "HY", "ID",
      "IN", "IS", "JL", "JT", "JV", "LF", "NL", "NS", "NU", "OP", "PO", "PR",
      "QU", "QUPi", "QUPf", "RI", "SP", "SY", "VF", "VI", "WJ", "ZW", "ZWJ"]
LBI = {n: i for i, n in enumerate(LB)}
assert len(LB) <= 64, "line break class must fit in six bits"

lb_raw = {}
lb_missing = []
with open(path("LineBreak.txt"), encoding="utf-8") as f:
    for line in f:
        m = re.match(r"#\s*@missing:\s*([0-9A-F]+)\.\.([0-9A-F]+)\s*;\s*(\w+)", line)
        if m:
            lb_missing.append((int(m.group(1), 16), int(m.group(2), 16), m.group(3)))
            continue
        line = line.split("#", 1)[0].strip()
        if not line:
            continue
        cps, lb_cls = [x.strip() for x in line.split(";")]
        lo, hi = (int(x, 16) for x in cps.split("..")) if ".." in cps else (int(cps, 16),) * 2
        for c in range(lo, hi + 1):
            lb_raw[c] = lb_cls

def resolve_lb(cp):
    """LB1: resolve the classes that have no behaviour of their own."""
    v = lb_raw.get(cp)
    if v is None:
        v = "XX"
        for lo, hi, d in lb_missing:
            if lo <= cp <= hi:
                v = d
    cat = cats.get(cp, "Cn")
    if v == "SA":
        return "CM" if cat in ("Mn", "Mc") else "AL"
    if v in ("AI", "SG", "XX"):
        return "AL"
    if v == "CJ":
        return "NS"
    if v == "QU":
        return "QUPi" if cat == "Pi" else "QUPf" if cat == "Pf" else "QU"
    return v

# East Asian F, W or H. Several rules are conditioned on it, and it is not the
# same test as the width table's "wide": halfwidth forms count here.
ea_fwh = set()
for prop in ("F", "W", "H"):
    for lo, hi in parse_ranges("EastAsianWidth.txt", {prop})[prop]:
        ea_fwh.update(range(lo, hi + 1))

# LB30b's second clause: Extended_Pictographic that is also unassigned.
extpict_unassigned = set()
for lo, hi in emoji["Extended_Pictographic"]:
    for c in range(lo, hi + 1):
        if c not in cats:
            extpict_unassigned.add(c)

# ------------------------------------------------------------------- output
# Everything above is collapsed into one byte per code point:
#
#     bits 0-1  width class: 0 = zero, 1 = narrow, 2 = wide, 3 = ambiguous
#     bits 2-6  grapheme cluster break class (see `Gcb` in src/grapheme.rs)
#
# Those bytes are then split into 256-byte pages and deduplicated. Most of the
# code space is uniform, so ~1.1M entries collapse to a few dozen pages, and a
# lookup becomes two array reads instead of three binary searches.
PAGE = 256
NPAGES = 0x110000 // PAGE

def expand(ranges):
    out = set()
    for lo, hi in ranges:
        out.update(range(lo, hi + 1))
    return out

wide_set = expand(wide)
amb_set = expand(ambiguous)
pres_set = expand(emoji_pres)

entries = [0] * 0x110000
for cp in range(0x110000):
    if cp in zero:
        w = 0
    elif cp in wide_set or cp in pres_set:
        w = 2
    elif cp in amb_set:
        w = 3
    else:
        w = 1
    e = w | (cls.get(cp, 0) << 2) | (LBI[resolve_lb(cp)] << 7)
    if cp in ea_fwh:
        e |= 1 << 13
    if cp in extpict_unassigned:
        e |= 1 << 14
    entries[cp] = e

pages = []
index = []
seen = {}
for p in range(NPAGES):
    page = tuple(entries[p * PAGE:(p + 1) * PAGE])
    if page not in seen:
        seen[page] = len(pages)
        pages.append(page)
    index.append(seen[page])

ver = "unknown"
with open(path("EastAsianWidth.txt"), encoding="utf-8") as f:
    m = re.search(r"EastAsianWidth-([\d.]+)\.txt", f.readline())
    if m:
        ver = m.group(1)

print("// @generated by tools/gen_tables.py -- do not edit by hand.")
print(f"// Source: Unicode Character Database {ver}")
print("//")
print("// One byte per code point, as a deduplicated two-level page table:")
print("//   bits 0-1  width class: 0 zero, 1 narrow, 2 wide, 3 ambiguous")
print("//   bits 2-6  grapheme cluster break class (`Gcb` in src/grapheme.rs)")
print(f"// {NPAGES} pages of {PAGE} entries collapse to {len(pages)} distinct pages.")
print()
print("/// The Unicode version these tables were generated from.")
print(f'pub const UNICODE_VERSION: &str = "{ver}";')
print()
print("/// Width class of a code point: 0 zero, 1 narrow, 2 wide, 3 ambiguous.")
print("pub(crate) const WIDTH_MASK: u16 = 0b11;")
print("/// Bits to shift off to reach the grapheme cluster break class.")
print("pub(crate) const GCB_SHIFT: u16 = 2;")
print("/// Mask for the grapheme cluster break class, once shifted.")
print("pub(crate) const GCB_MASK: u16 = 0b1_1111;")
print("/// Bits to shift off to reach the line break class.")
print("pub(crate) const LB_SHIFT: u16 = 7;")
print("/// Mask for the line break class, once shifted.")
print("pub(crate) const LB_MASK: u16 = 0b11_1111;")
print("/// Set when the code point is East Asian Fullwidth, Wide or Halfwidth,")
print("/// which several line breaking rules are conditioned on.")
print("pub(crate) const EAST_ASIAN: u16 = 1 << 13;")
print("/// Set for Extended_Pictographic code points that are unassigned, which")
print("/// rule LB30b treats as emoji bases.")
print("pub(crate) const EXTPICT_UNASSIGNED: u16 = 1 << 14;")
print()
print("// Line break classes, after the LB1 resolution. Keep in sync with the")
print("// `Lb` enum in src/linebreak.rs:")
print("//   " + " ".join(f"{i}={n}" for i, n in enumerate(LB)))
print()
print(f"/// Page number for each block of {PAGE} code points.")
print("pub(crate) static PAGE_INDEX: &[u16] = &[")
for i in range(0, len(index), 24):
    print("    " + "".join(f"{v}," for v in index[i:i + 24]))
print("];")
print()
print(f"/// {len(pages)} distinct pages of {PAGE} entries, concatenated.")
print("#[rustfmt::skip]")
print("pub(crate) static PAGES: &[u16] = &[")
for page in pages:
    for i in range(0, PAGE, 16):
        print("    " + "".join(f"{v}," for v in page[i:i + 16]))
print("];")
print()
print("""/// Table entry for a code point. Width class, grapheme cluster break class,
/// line break class and two flags, packed into one `u16`.
#[inline]
pub(crate) fn entry(cp: u32) -> u16 {
    // `cp` comes from a `char`, so it is always below 0x110000 and in range.
    let page = PAGE_INDEX[(cp >> 8) as usize] as usize;
    PAGES[(page << 8) | (cp as usize & 0xFF)]
}""")

sys.stderr.write(
    f"unicode {ver}: {NPAGES} pages -> {len(pages)} distinct "
    f"({len(pages) * PAGE * 2 + len(index) * 2} bytes of tables)\n")
