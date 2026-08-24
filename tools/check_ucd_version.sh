#!/usr/bin/env bash
# Fail if Unicode has published a version newer than the one our tables were
# generated from. Run on a schedule in CI: the tables are committed, so nothing
# else will ever tell us the data has moved on.
set -euo pipefail

ours=$(grep -oP 'UNICODE_VERSION: &str = "\K[^"]+' src/tables.rs)

# Buffer the whole body rather than piping into `head`: unicode.org ignores
# range requests, and a closed pipe makes curl exit non-zero under `pipefail`.
body=$(curl -sS --max-time 30 \
  https://www.unicode.org/Public/UCD/latest/ucd/EastAsianWidth.txt)
latest=$(printf '%s' "$body" | sed -n '1p' \
  | grep -oP 'EastAsianWidth-\K[0-9.]+(?=\.txt)' || true)

echo "tables generated from: $ours"
echo "latest published:      $latest"

if [ -z "$latest" ]; then
  echo "::error::could not read the published Unicode version"
  exit 2
fi
if [ "$ours" != "$latest" ]; then
  echo "::warning::Unicode $latest is out; cellwidth's tables are $ours."
  echo "Regenerate:  python3 tools/gen_tables.py <ucd-dir> > src/tables.rs && cargo test"
  exit 1
fi
echo "up to date"
