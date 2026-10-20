#!/bin/sh
# Fetch and build everything under ref/ (git-ignored): the oracle (Mednafen with our hooks), a JDK, Ghidra.
# Each download is pinned by version and SHA-256. Needs: curl, tar, unzip, a C++ compiler, SDL2 and ALSA
# development headers. Run from anywhere; safe to run again (finished steps are skipped).
set -e
cd "$(dirname "$0")/.."
ROOT=$(pwd)
REF=$ROOT/ref
mkdir -p "$REF/dl"

MEDNAFEN=mednafen-1.32.1
MEDNAFEN_SHA=de7eb94ab66212ae7758376524368a8ab208234b33796625ca630547dbc83832
JDK_DIR=jdk-21.0.12.1+1
JDK_TGZ=OpenJDK21U-jdk_x64_linux_hotspot_21.0.12.1_1.tar.gz
JDK_URL=https://github.com/adoptium/temurin21-binaries/releases/download/jdk-21.0.12.1%2B1/$JDK_TGZ
JDK_SHA=ce79869e1307ed8ee1e2baa86a412b1eb5b75d10a01006d788a6f968bcfaee94
GHIDRA_DIR=ghidra_12.1.4_PUBLIC
GHIDRA_ZIP=ghidra_12.1.4_PUBLIC_20260921.zip
GHIDRA_URL=https://github.com/NationalSecurityAgency/ghidra/releases/download/Ghidra_12.1.4_build/$GHIDRA_ZIP
GHIDRA_SHA=ddac49f903da9d5bac833e5cc79395098b9c33cfd3279be5f31bd00387d2d4db

fetch() { # url file sha256
    if [ ! -f "$REF/dl/$2" ]; then
        echo "fetching $2"
        curl -sSfL -o "$REF/dl/$2.part" "$1"
        mv "$REF/dl/$2.part" "$REF/dl/$2"
    fi
    echo "$3  $REF/dl/$2" | sha256sum -c --quiet - || { echo "$2: SHA-256 mismatch" >&2; exit 1; }
}

# --- Mednafen: Saturn/ST-V only, with tools/patches/mednafen applied ---------------------------------
fetch https://mednafen.github.io/releases/files/$MEDNAFEN.tar.xz $MEDNAFEN.tar.xz $MEDNAFEN_SHA
if [ ! -d "$REF/$MEDNAFEN" ]; then
    tar -xJf "$REF/dl/$MEDNAFEN.tar.xz" -C "$REF"
    mv "$REF/mednafen" "$REF/$MEDNAFEN"
fi
PATCHED=$REF/$MEDNAFEN/.stv-oracle-patched
if [ ! -f "$PATCHED" ]; then
    (cd "$REF/$MEDNAFEN" && patch -p1 < "$ROOT/tools/patches/mednafen/oracle.patch")
    touch "$PATCHED"
fi
cp "$ROOT/tools/patches/mednafen/oracle.inc" "$REF/$MEDNAFEN/src/ss/oracle.inc"
if [ ! -f "$REF/$MEDNAFEN/config.status" ]; then
    (cd "$REF/$MEDNAFEN" && ./configure --prefix="$REF/mednafen" --without-libflac --disable-jack \
        --disable-apple2 --disable-gb --disable-gba --disable-lynx --disable-md --disable-nes --disable-ngp \
        --disable-pce --disable-pce-fast --disable-pcfx --disable-psx --disable-sms --disable-snes \
        --disable-snes-faust --disable-ssfplay --disable-vb --disable-wswan --enable-ss > "$REF/mednafen-configure.log")
fi
echo "building Mednafen (log: ref/mednafen-build.log)"
(cd "$REF/$MEDNAFEN" && make -j"$(nproc)" > "$REF/mednafen-build.log" 2>&1 && make install > /dev/null)

# --- JDK and Ghidra ---------------------------------------------------------------------------------
fetch "$JDK_URL" "$JDK_TGZ" $JDK_SHA
[ -d "$REF/$JDK_DIR" ] || tar -xzf "$REF/dl/$JDK_TGZ" -C "$REF"
fetch "$GHIDRA_URL" "$GHIDRA_ZIP" $GHIDRA_SHA
[ -d "$REF/$GHIDRA_DIR" ] || unzip -q "$REF/dl/$GHIDRA_ZIP" -d "$REF"
ln -sfn "$JDK_DIR" "$REF/jdk"
ln -sfn "$GHIDRA_DIR" "$REF/ghidra"

# --- Python tools: Capstone, and PyGhidra from Ghidra's own wheels ----------------------------------
[ -x .venv/bin/python ] || tools/setup_venv.sh
.venv/bin/pip install -q --no-index -f "$REF/$GHIDRA_DIR/Ghidra/Features/PyGhidra/pypkg/dist" pyghidra
echo "ref/ ready: ref/mednafen/bin/mednafen, ref/jdk, ref/ghidra"
