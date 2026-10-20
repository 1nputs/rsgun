#!/bin/sh
# Remake oracle.patch from the working tree ref/mednafen-1.32.1/src/ss against the pristine tarball, then
# check it applies to a fresh tree. oracle.inc is kept as its own file (fetch_refs.sh copies it in).
set -e
cd "$(dirname "$0")/../../.."
ROOT=$(pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
tar -xJf ref/dl/mednafen-1.32.1.tar.xz -C "$TMP"
mkdir "$TMP/a" "$TMP/b"
cp -r "$TMP/mednafen/src/ss" "$TMP/a/ss"
cp -r ref/mednafen-1.32.1/src/ss "$TMP/b/ss"
(cd "$TMP" && diff -ru -x '*.o' -x '.deps' -x 'Makefile' -x 'oracle.inc' -x '*.a' -x '.dirstamp' a/ss b/ss > raw.patch) || true
sed -E -e '/^diff -ru /d' -e '/^Only in /d' \
    -e 's#^--- a/ss/([^\t]*).*#--- a/src/ss/\1#' -e 's#^\+\+\+ b/ss/([^\t]*).*#+++ b/src/ss/\1#' \
    "$TMP/raw.patch" > tools/patches/mednafen/oracle.patch
(cd "$TMP/mednafen" && patch -p1 --dry-run -s < "$ROOT/tools/patches/mednafen/oracle.patch")
echo "oracle.patch: $(grep -c '^+++ ' tools/patches/mednafen/oracle.patch) files, applies to a fresh tree"
