#!/bin/sh
# Compare mode over the stage 4 route (STG03): the marathon's script taking stage 4 at STG01's
# end (tools/port/marathon.py --stage4, out/evidence/route4-input.txt), from the marathon's
# f060000 state (segment r060000: the choice and STG03's start) and from each of the route's
# states (out/evidence/route4-states, one every 10000 frames from 70000) for the 10000 frames
# after it, several segments at once. Fails on any divergence. Run tools/port/check.sh first.
#
#     tools/port/check_route4.sh [JOBS [FRAMES [OUT]]]
#
# As check_marathon.sh: FRAMES (10000) a segment, OUT (out/port/route4) the reports' directory,
# RSGUN=PATH another binary. The evidence and states: tools/setup_debian.sh's commands.
set -e
cd "$(dirname "$0")/../.."
JOBS=${1:-6}
export FRAMES=${2:-10000}
export OUT=${3:-out/port/route4}
rm -rf "$OUT"
mkdir -p "$OUT"
cp "${RSGUN:-target/release/rsgun}" "$OUT/rsgun"
{ echo out/evidence/marathon-states/f060000.state; ls out/evidence/route4-states/f*.state; } | xargs -P "$JOBS" -I{} sh -c '
    s={}; f=$(basename "$s" .state); start=$(expr "${f#f}" + 0)
    case "$s" in out/evidence/marathon-states/*) f=r${f#f} ;; esac
    "$OUT/rsgun" --headless --load-state "$s" --script out/evidence/route4-input.txt \
        --frames $((start + FRAMES)) --save "$OUT/save-$f" --compare "$OUT/$f.txt" > "$OUT/$f.log" 2>&1
'
fail=0
for r in "$OUT"/[fr]*.txt; do
    line=$(head -1 "$r")
    echo "$(basename "$r" .txt): $line"
    case "$line" in *", 0 diverging;"*) ;; *) fail=1 ;; esac
done
[ $fail = 0 ] && echo "every call equal" || { echo "divergences: see $OUT/*.txt"; exit 1; }
