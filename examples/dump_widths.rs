//! Dev tool: print `cp<TAB>width` for every code point, for cross-checking
//! against another implementation. Not part of the library.
fn main() {
    let mut out = String::new();
    for cp in 0u32..=0x10FFFF {
        if let Some(c) = char::from_u32(cp) {
            out.push_str(&format!("{cp}\t{}\n", cellwidth::char_width(c)));
        }
    }
    print!("{out}");
}
