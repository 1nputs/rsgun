#!/bin/sh
# The port run natively against the interpreter: play1 (to frame 3600) and the attract mode (to
# frame 7200) must end on the same pictures both ways. It covers what compare mode cannot take
# whole (the main loop: one call for as long as a game mode lasts) and the native machinery itself
# (waits, interrupts between blocks). The rewrite's code costs the machine less time than the SH-2
# code it replaces, so a native run slows down less where the game runs out of time, and the game
# goes differently from there: both run with the CPUs 16 times as fast (--cpu-speed 16), where
# neither slows down (at 4 times, the attract mode's change of mode at frame 5544 still slows both,
# by a frame more on the interpreter).
#
#     tools/port/check_native.sh
# Output: out/native/ (the pictures).
set -e
cd "$(dirname "$0")/../.."
cargo build --release -q -p rsgun
rm -rf out/native
mkdir -p out/native
run() {
    ./target/release/rsgun --headless --cold --cpu-speed 16 --save "out/native/save-$1-$2" --frames "$3" --shot "out/native/$1-$2.ppm" \
        $4 $5 $6 > "out/native/$1-$2.log" 2>&1
}
run play1 sh2 3600 --script tools/oracle/play1.txt &
run play1 native 3600 --script tools/oracle/play1.txt --native &
run attract sh2 7200 &
run attract native 7200 --native
wait
fail=0
for r in play1 attract; do
    if cmp -s "out/native/$r-sh2.ppm" "out/native/$r-native.ppm"; then
        echo "$r: the same picture"
    else
        echo "$r: the pictures differ (out/native/$r-*.ppm)"
        fail=1
    fi
done
[ $fail = 0 ] || exit 1
