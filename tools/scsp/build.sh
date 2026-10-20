#!/bin/sh
# Builds build/scsp_oracle: the oracle's SCSP on its own (scsp_oracle.cpp), from the patched Mednafen
# tree tools/fetch_refs.sh made in ref/.
set -e
cd "$(dirname "$0")/../.."
M=ref/mednafen-1.32.1
mkdir -p build
g++ -std=gnu++11 -O2 -fsigned-char -fwrapv -fno-fast-math -fno-strict-aliasing -DHAVE_CONFIG_H \
    -I$M/include -iquote $M/src -I$M/intl -w -o build/scsp_oracle tools/scsp/scsp_oracle.cpp
echo build/scsp_oracle
