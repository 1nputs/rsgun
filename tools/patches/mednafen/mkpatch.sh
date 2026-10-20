#!/bin/sh
# Remake oracle.patch from the working tree ref/mednafen-1.32.1 (src/ss and src/hw_cpu/m68k) against the
# pristine tarball, then check it applies to a fresh tree. oracle.inc is kept as its own file
# (fetch_refs.sh copies it in).
set -e
cd "$(dirname "$0")/../../.."
ROOT=$(pwd)
TMP=$(mktemp -d)
trap 'rm -rf "$TMP"' EXIT
tar -xJf ref/dl/mednafen-1.32.1.tar.xz -C "$TMP"
: > "$TMP/raw.patch"
for dir in ss hw_cpu/m68k; do
    mkdir -p "$TMP/a/$dir" "$TMP/b/$dir"
    cp -r "$TMP/mednafen/src/$dir/." "$TMP/a/$dir/"
    cp -r "ref/mednafen-1.32.1/src/$dir/." "$TMP/b/$dir/"
    (cd "$TMP" && diff -ru -x '*.o' -x '.deps' -x 'Makefile' -x 'oracle.inc' -x '*.a' -x '.dirstamp' "a/$dir" "b/$dir" >> raw.patch) || true
done
sed -E -e '/^diff -ru /d' -e '/^Only in /d' \
    -e 's#^--- a/([^\t]*).*#--- a/src/\1#' -e 's#^\+\+\+ b/([^\t]*).*#+++ b/src/\1#' \
    "$TMP/raw.patch" > tools/patches/mednafen/oracle.patch
(cd "$TMP/mednafen" && patch -p1 --dry-run -s < "$ROOT/tools/patches/mednafen/oracle.patch")
echo "oracle.patch: $(grep -c '^+++ ' tools/patches/mednafen/oracle.patch) files, applies to a fresh tree"
