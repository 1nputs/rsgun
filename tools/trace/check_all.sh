#!/bin/sh
# Records the oracle's trace windows (power on, the BIOS, the game's boot, the title, the story, attract
# play) in one run and replays each through the SH-2 core with stv-check. Fails on any difference.
# Output: out/trace/ (about 200 MB).
set -e
cd "$(dirname "$0")/../.."
WINDOWS="2:2,30:1,120:1,300:1,600:1,900:1,1200:1,1500:1,1800:2,2400:1,3000:1,3600:1,4200:1,4800:1,5400:1,6000:1,6600:1,7001:3,7200:1"
LAST=7200
DUMPS=$(echo "$WINDOWS" | tr ',' '\n' | awk -F: '{ printf "%s%d", (NR > 1 ? "," : ""), $1 + $2 - 1 }')
rm -rf out/trace
.venv/bin/python -I tools/oracle/run.py --frames $LAST --dump "$DUMPS" --trace "$WINDOWS" --out out/trace
cargo build --release -q -p stv-check
fail=0
for t in out/trace/t*; do
    echo "== $t"
    ./target/release/stv-check trace "$t" | grep -v '^ ' | head -3 || true
    ./target/release/stv-check trace "$t" > /dev/null || fail=1
done
[ $fail = 0 ] && echo "every window matches" || { echo "differences: see above"; exit 1; }
