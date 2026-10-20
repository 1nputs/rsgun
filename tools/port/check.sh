#!/bin/sh
# The port's check: translates the game's functions (tools/port/sh2rust.py) and runs the game in
# compare mode (rsgun --compare: the SH-2 code runs, each call the port has is checked against it)
# through play1's input script and through the attract mode. Fails on any divergence.
#
#     tools/port/check.sh [PLAY_FRAMES [ATTRACT_FRAMES]]
#
# Needs the evidence and the memory dump the translation is made from (once; about 8 minutes):
#     rsgun --headless --script tools/oracle/play1.txt --frames 4000 --evidence out/evidence/play1.txt \
#         --dump 3000 --dump-dir out/evidence/dumps
#     rsgun --headless --frames 7200 --evidence out/evidence/attract.txt
# Output: out/port/ (the reports).
set -e
cd "$(dirname "$0")/../.."
PLAY=${1:-4000}
ATTRACT=${2:-7200}
python3 -I tools/port/sh2rust.py --image out/evidence/dumps/f003000/wram-h.bin \
    --evidence out/evidence/play1.txt,out/evidence/attract.txt --out build/port/generated.rs
cargo build --release -q -p rsgun
rm -rf out/port
mkdir -p out/port
./target/release/rsgun --headless --save out/port/save-attract --frames "$ATTRACT" --compare out/port/attract.txt \
    > out/port/attract.log 2>&1 &
attract=$!
./target/release/rsgun --headless --save out/port/save-play1 --script tools/oracle/play1.txt --frames "$PLAY" \
    --compare out/port/play1.txt > out/port/play1.log 2>&1
wait $attract
fail=0
for r in play1 attract; do
    echo "$r: $(head -1 out/port/$r.txt)"
    grep -q ", 0 diverging;" out/port/$r.txt || fail=1
done
[ $fail = 0 ] && echo "every call equal" || { echo "divergences: see out/port/*.txt"; exit 1; }
