#!/bin/sh
# Sets a Debian (or Ubuntu) machine up to build, play and keep porting rsgun, from a fresh clone.
#
#     tools/setup_debian.sh [CARRY.tar] [--make-evidence]
#
# CARRY.tar is what tools/carry.sh packed on a machine that has the work (the ROM sets, the
# evidence, the last whole-game check's reports, the leftovers, Ghidra's drafts): unpacked here
# first. Without it, put rsgun.zip and stvbios.zip (MAME's sets) in roms/; --make-evidence then
# records the evidence on this machine (play1 and the attract mode in minutes; the marathon and its
# save states take hours).
#
# Steps (each skipped when its output is there, so it can be run again): the packages (sudo apt);
# Rust (rustup, with clippy); the Python tools and ref/ (tools/setup_venv.sh, tools/fetch_refs.sh:
# Mednafen with the oracle's hooks, a JDK, Ghidra, PyGhidra); the cart and BIOS images and the
# EEPROM (build/rom); the leftovers' types (build/leftovers); the translation (build/port) and the
# release build with it; Ghidra's drafts (build/ghidra/c); the overlays' disassembly (build/dis).
set -e
cd "$(dirname "$0")/.."
ROOT=$(pwd)
CARRY=
MAKE_EVIDENCE=
for a in "$@"; do
    case "$a" in
        --make-evidence) MAKE_EVIDENCE=1 ;;
        *) CARRY=$a ;;
    esac
done
step() { printf '\n== %s\n' "$*"; }

step "packages"
sudo apt-get update -q
sudo apt-get install -y -q build-essential pkg-config curl unzip xz-utils patch git \
    python3 python3-venv libsdl2-dev libasound2-dev zlib1g-dev

step "Rust"
# rustup's (in ~/.cargo/bin), not Debian's packaged cargo: that one is too old for the 2024 edition
if [ -f "$HOME/.cargo/env" ]; then . "$HOME/.cargo/env"; fi
if ! command -v rustup > /dev/null; then
    curl -sSf https://sh.rustup.rs | sh -s -- -y --profile minimal
    . "$HOME/.cargo/env"
fi
if command -v rustup > /dev/null; then
    rustup toolchain install stable --profile minimal -c clippy -c rustfmt
fi
cargo --version

if [ -n "$CARRY" ]; then
    step "unpacking $CARRY"
    tar -xf "$CARRY" -C "$ROOT"
fi
if [ ! -f roms/rsgun.zip ] && [ ! -f ../roms/rsgun.zip ]; then
    echo "rsgun.zip and stvbios.zip (MAME's sets) go in roms/ (or pass tools/carry.sh's tarball)" >&2
    exit 1
fi

step "Python tools and ref/ (Mednafen, JDK, Ghidra: a few minutes)"
[ -x .venv/bin/python ] || tools/setup_venv.sh
tools/fetch_refs.sh

step "the cart and BIOS images, the EEPROM"
[ -f build/rom/rsgun-cart.bin ] || .venv/bin/python tools/rom/mkcart.py
[ -f build/rom/eeprom.bin ] || .venv/bin/python tools/oracle/eeprom.py

step "the leftovers' types"
if [ ! -f build/leftovers/dma_types.h ]; then
    mkdir -p build/leftovers
    python3 -I tools/rom/coffdump.py build/rom/rsgun-cart.bin 288000 --header > build/leftovers/dma_types.h
fi

step "the release build (the interpreter only: the translation needs the evidence)"
cargo build --release

if [ ! -f out/evidence/play1.txt ] || [ ! -f out/evidence/attract.txt ]; then
    if [ -z "$MAKE_EVIDENCE" ]; then
        echo "no out/evidence: pass tools/carry.sh's tarball, or --make-evidence" >&2
        exit 1
    fi
    step "evidence: play1 and the attract mode (minutes)"
    mkdir -p out/evidence
    target/release/rsgun --headless --script tools/oracle/play1.txt --frames 4000 \
        --evidence out/evidence/play1.txt --dump 3000 --dump-dir out/evidence/dumps
    target/release/rsgun --headless --frames 7200 --evidence out/evidence/attract.txt
fi
if [ -n "$MAKE_EVIDENCE" ] && [ ! -f out/evidence/marathon.txt ]; then
    step "evidence: the marathon (hours)"
    python3 -I tools/port/marathon.py > out/evidence/marathon-input.txt
    target/release/rsgun --headless --script out/evidence/marathon-input.txt --frames 210000 \
        --states "$(seq -s, 10000 10000 200000)" --dump-dir out/evidence/marathon-states
    cp out/evidence/marathon-states/f140000.state out/evidence/marathon-140000.state
    target/release/rsgun --headless --script out/evidence/marathon-input.txt --frames 150000 \
        --evidence out/evidence/marathon.txt
    target/release/rsgun --headless --script out/evidence/marathon-input.txt \
        --load-state out/evidence/marathon-140000.state --frames 210000 --evidence out/evidence/marathon-end.txt
fi

step "the translation (build/port) and the build with it"
EVIDENCE=out/evidence/play1.txt,out/evidence/attract.txt
for e in marathon marathon-end; do
    if [ -f "out/evidence/$e.txt" ]; then EVIDENCE="$EVIDENCE,out/evidence/$e.txt"; fi
done
if [ ! -f build/port/generated.rs ]; then
    python3 -I tools/port/sh2rust.py --image out/evidence/dumps/f003000/wram-h.bin --evidence "$EVIDENCE" \
        --out build/port/generated.rs
fi
cargo build --release

step "Ghidra's drafts (build/ghidra/c: about 10 minutes)"
[ -d build/ghidra/c ] || .venv/bin/python tools/ghidra/project.py

step "the overlays' disassembly (build/dis/FINGERPRINT.dis)"
mkdir -p build/dis
for image in out/evidence/*.overlays/*.bin; do
    [ -f "$image" ] || continue
    dis=build/dis/$(basename "$image" .bin).dis
    # the window is 0x25000 bytes at 0x06080000
    [ -f "$dis" ] || .venv/bin/python -I tools/rom/dis.py "$image" 0 06080000 75776 > "$dis"
done
ls build/dis

step "done"
echo "cargo is in ~/.cargo/bin: new shells have it on PATH; in one opened before this ran (or a Claude"
echo "Code session started before it), run: . \"\$HOME/.cargo/env\" (or restart it)."
echo "Play: cargo run --release -p rsgun [-- --native]. Check: tools/port/check.sh (about 11 minutes"
echo "on 8 cores), tools/port/check_native.sh. The guide: docs/HANDOFF.md."
