#!/bin/sh
# Dumps the oracle's display state at frames across power on, the title, the story and attract play,
# and composes each with VDP2's renderer (stv-check vdp2): every pixel must match the oracle's picture.
# Frame 4200's story text is typed in during the frame (after the dump), so it is listed apart.
set -e
cd "$(dirname "$0")/../.."
FRAMES="600,1200,1800,2400,3000,3600,4800,5400,6000,6600,7200"
rm -rf out/vdp2
.venv/bin/python -I tools/oracle/run.py --frames 7200 --dump "$FRAMES" --out out/vdp2 > /dev/null
cargo build --release -q -p stv-check
fail=0
for d in out/vdp2/f*; do
    if out=$(./target/release/stv-check vdp2 "$d"); then echo "$out"; else echo "$out" | cut -c1-200; fail=1; fi
done
[ $fail = 0 ] && echo "every frame matches" || { echo "differences: see above"; exit 1; }
