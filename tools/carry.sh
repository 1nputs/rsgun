#!/bin/sh
# Packs what another machine needs to carry on and cannot fetch: the ROM sets and what is made from
# them that takes hours to remake. ROM-derived: for your own machines, never for a repository.
#
#     tools/carry.sh [OUT.tar]        # default out/carry.tar (about 200 MB)
#
# Then on the other machine, in a fresh clone: tools/setup_debian.sh OUT.tar
#
# It holds: roms/ (rsgun.zip, stvbios.zip; from ../roms/ if not here), out/evidence/ (the runs the
# translation and the compares start from: play1, the attract mode, the marathon, its save states,
# the overlay images, the memory dump), the last whole-game check's reports (which functions each
# 10000-frame segment calls: how a compare's segment is picked), build/leftovers/ (the cart's
# leftover types, for Ghidra) and build/ghidra/c/ (Ghidra's drafts; about 10 minutes to remake).
set -e
cd "$(dirname "$0")/.."
OUT=${1:-out/carry.tar}
STAGE=$(mktemp -d)
trap 'rm -rf "$STAGE"' EXIT

mkdir -p "$STAGE/roms"
for z in rsgun.zip stvbios.zip; do
    if [ -f "roms/$z" ]; then cp "roms/$z" "$STAGE/roms/"
    elif [ -f "../roms/$z" ]; then cp "../roms/$z" "$STAGE/roms/"
    else echo "$z not found in roms/ or ../roms/" >&2; exit 1
    fi
done

set -- out/evidence build/leftovers
[ -d build/ghidra/c ] && set -- "$@" build/ghidra/c
REPORTS=$(ls -dt out/port/marathon-*/ 2>/dev/null | head -1)
REPORTS=${REPORTS%/}
if [ -n "$REPORTS" ]; then
    mkdir -p "$STAGE/$REPORTS"
    cp "$REPORTS"/f*.txt "$STAGE/$REPORTS/"
fi
mkdir -p "$(dirname "$OUT")"
tar -cf "$OUT" -C "$STAGE" .
tar -rf "$OUT" "$@"
echo "$OUT: $(du -h "$OUT" | cut -f1) (roms/, $* ${REPORTS:+$REPORTS/f*.txt})"
