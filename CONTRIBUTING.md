# Contributing to cellwidth

## The one rule

**This crate has no dependencies, and it stays that way.** Not at runtime, not
for tests. `cargo tree --edges normal,dev` prints nothing but `cellwidth`, and
CI runs the test suite with `--offline` on an empty registry to keep it honest.

Code that needs another crate to be tested belongs in `oracles/`, which is a
separate package for exactly this reason.

## Running everything

```sh
cargo test                                        # the crate's own suite, offline
cargo test --manifest-path oracles/Cargo.toml     # differential vs other crates
cargo clippy --all-targets --all-features -- -D warnings
cargo fmt --all --check
cargo +nightly miri test -p cellwidth
cargo llvm-cov -p cellwidth --all-features \
  --ignore-filename-regex 'tables\.rs' \
  --fail-under-lines 100 --fail-under-functions 100
cd fuzz && cargo +nightly fuzz run cell -- -max_total_time=300
```

Line and function coverage are gated at 100% in CI. If a line genuinely cannot
be reached, delete it rather than excluding it: several unreachable branches
were found and removed that way, and two lookup tables were made total so they
have no fallback arm to leave uncovered.

Region coverage sits at 99.95% and is reported but not gated. The merged report
attributes one uncovered region to `src/ansi.rs`, but its own segment data
contains no uncovered segment, and measuring the library tests alone shows the
lines in question covered. It is an artifact of how the per-binary reports are
merged, not a hole in the suite.

## Claims need an oracle

Every expectation in the test suite should trace back to something that is not
this crate's own source: Unicode's published data files, another
implementation, or a property a fuzzer can attack. A test that encodes what the
code already does only pins the bug in place.

When a bug is fixed, the regression test says which oracle caught it.

## Changing the Unicode tables

`src/tables.rs` is generated and committed. Never hand-edit it. To move to a new
Unicode release:

```sh
mkdir -p ucd && cd ucd
base=https://www.unicode.org/Public/UCD/latest/ucd
curl -O $base/EastAsianWidth.txt -O $base/UnicodeData.txt \
     -O $base/DerivedCoreProperties.txt \
     -O $base/auxiliary/GraphemeBreakProperty.txt \
     -O $base/emoji/emoji-data.txt
curl -o ../tests/data/GraphemeBreakTest.txt $base/auxiliary/GraphemeBreakTest.txt
curl -o ../tests/data/emoji-test.txt \
     https://www.unicode.org/Public/emoji/latest/emoji-test.txt
cd .. && python3 tools/gen_tables.py ucd > src/tables.rs && cargo test
```

A scheduled CI job runs `tools/check_ucd_version.sh` weekly and fails when
Unicode publishes a version newer than the tables.

## Width decisions

Where terminals disagree, the answer is a policy on `Width`, not a guess baked
into the tables. Before adding one, check that real terminals actually differ;
before changing a default, say which terminals were tested.

## MSRV

1.75 for the library. Raising it is a breaking change for the purposes of this
project. The `oracles` package may need a newer compiler, which is why the MSRV
job builds and tests `cellwidth` alone.
