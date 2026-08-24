#!/usr/bin/env bash
# Record every terminal we can drive on this machine, then print the matrix.
#
#     probe/drivers/run_all.sh
#
# Terminals are run headlessly under Xvfb, so this works over ssh and in CI.
# Any terminal that is not installed is skipped, not faked.
set -uo pipefail
cd "$(dirname "$0")/../.."

mkdir -p tests/data/terminals
cargo build --manifest-path probe/Cargo.toml --release --quiet
PROBE="$PWD/probe/target/release/probe"
export LIBGL_ALWAYS_SOFTWARE=1

run() {  # name, then the command that starts the terminal
  local name=$1; shift
  printf '%-12s ' "$name"
  if timeout 250 "$@" >/dev/null 2>&1 && [ -s "tests/data/terminals/$name.tsv" ]; then
    echo "recorded"
  else
    echo "skipped (not installed, or no reply)"
    rm -f "tests/data/terminals/$name.tsv"
  fi
}

# VTE: GNOME Terminal, Tilix, Terminator, xfce4-terminal, Guake.
if python3 -c "import gi; gi.require_version('Vte','2.91')" 2>/dev/null; then
  run vte xvfb-run -a python3 probe/drivers/vte_driver.py "$PROBE" "$PWD/tests/data/terminals/vte.tsv"
fi

if command -v kitty >/dev/null || [ -x "$HOME/.local/kitty.app/bin/kitty" ]; then
  KITTY=$(command -v kitty || echo "$HOME/.local/kitty.app/bin/kitty")
  run kitty xvfb-run -a -s "-screen 0 1920x1080x24" "$KITTY" --config NONE \
      -o initial_window_width=1500 -e "$PROBE" "$PWD/tests/data/terminals/kitty.tsv"
fi

if command -v alacritty >/dev/null; then
  printf '[window.dimensions]\ncolumns = 200\nlines = 24\n' > /tmp/cw-alacritty.toml
  run alacritty xvfb-run -a -s "-screen 0 1920x1080x24" alacritty \
      --config-file /tmp/cw-alacritty.toml -e "$PROBE" "$PWD/tests/data/terminals/alacritty.tsv" \
      "alacritty $(alacritty --version 2>/dev/null | awk '{print $2}')"
fi

if command -v wezterm-gui >/dev/null; then
  printf 'return { front_end = "Software", initial_cols = 200, initial_rows = 24,\n  check_for_updates = false, enable_wayland = false }\n' > /tmp/cw-wez.lua
  run wezterm xvfb-run -a -s "-screen 0 1920x1080x24" wezterm-gui \
      --config-file /tmp/cw-wez.lua start -- "$PROBE" "$PWD/tests/data/terminals/wezterm.tsv"
fi

echo
cargo run --manifest-path probe/Cargo.toml --bin compare --quiet -- tests/data/terminals/*.tsv
echo
cargo run --manifest-path probe/Cargo.toml --bin score --quiet -- tests/data/terminals/*.tsv
