#!/bin/sh
# Compare mode over the whole game: the marathon's input script (tools/port/marathon.py) from power
# on and from each of its save states (one every 10000 frames, out/evidence/marathon-states/) for
# the 10000 frames after it, several segments at once. Fails on any divergence. Run
# tools/port/check.sh first (it translates and builds).
#
#     tools/port/check_marathon.sh [JOBS]
#
# The states come from one interpreted run (about 45 minutes):
#     rsgun --headless --script out/evidence/marathon-input.txt --frames 210000 \
#         --states 10000,20000,...,200000 --dump-dir out/evidence/marathon-states
# Output: out/port/marathon/ (a report a segment).
set -e
cd "$(dirname "$0")/../.."
JOBS=${1:-6}
rm -rf out/port/marathon
mkdir -p out/port/marathon
# a copy of the binary, so it can be rebuilt meanwhile
cp target/release/rsgun out/port/marathon/rsgun
# The first segment runs from power on (no state: "f000000").
{ echo f000000; ls out/evidence/marathon-states/f*.state; } | xargs -P "$JOBS" -I{} sh -c '
    s={}; f=$(basename "$s" .state); start=$(expr "${f#f}" + 0)
    if [ "$f" = f000000 ]; then load=""; else load="--load-state $s"; fi
    out/port/marathon/rsgun --headless $load --script out/evidence/marathon-input.txt \
        --frames $((start + 10000)) --save "out/port/marathon/save-$f" --compare "out/port/marathon/$f.txt" \
        > "out/port/marathon/$f.log" 2>&1
'
fail=0
for r in out/port/marathon/f*.txt; do
    line=$(head -1 "$r")
    echo "$(basename "$r" .txt): $line"
    case "$line" in *", 0 diverging;"*) ;; *) fail=1 ;; esac
done
[ $fail = 0 ] && echo "every call equal" || { echo "divergences: see out/port/marathon/*.txt"; exit 1; }
