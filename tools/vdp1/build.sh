#!/bin/sh
# Builds build/vdp1_oracle: the oracle's VDP1 on its own (vdp1_oracle.cpp, stubs.cpp), linked with the
# VDP1 objects of the patched Mednafen tree tools/fetch_refs.sh built in ref/.
set -e
cd "$(dirname "$0")/../.."
M=ref/mednafen-1.32.1
S=$M/src
F="-std=gnu++11 -O2 -fsigned-char -fwrapv -DHAVE_CONFIG_H -I$M/include -iquote $S -I$M/intl -w"
mkdir -p build
g++ $F -c -o build/vdp1_oracle.o tools/vdp1/vdp1_oracle.cpp
g++ $F -c -o build/vdp1_stubs.o tools/vdp1/stubs.cpp
g++ -no-pie -o build/vdp1_oracle build/vdp1_oracle.o build/vdp1_stubs.o \
    $S/ss/vdp1.o $S/ss/vdp1_line.o $S/ss/vdp1_poly.o $S/ss/vdp1_sprite.o \
    $S/FileStream.o $S/Stream.o $S/error.o $S/VirtualFS.o $S/string/escape.o $S/string/string.o \
    $S/trio/libtrio_a-trio.o $S/trio/libtrio_a-triostr.o $S/trio/libtrio_a-trionan.o -lm
echo build/vdp1_oracle
