#!/bin/sh
# Builds the analysis driver (driver.rs) against a patched private copy of crates/stv (the patch adds
# a state snapshot at a breakpoint, a last-writer map of work RAM and sound RAM, and an EEPROM log).
# Nothing in crates/ is changed. Usage: tools/boot/reach/build.sh WORKDIR
#   then: WORKDIR/target/release/driver ROOT OUT 0x06004000    (env: EEPROM BACKUP RTC AREA EELOG)
set -e
here=$(cd "$(dirname "$0")" && pwd)
root=$(cd "$here/../../.." && pwd)
work=${1:?a work directory}
mkdir -p "$work/driver/src"
rm -rf "$work/stv"
cp -r "$root/crates/stv" "$work/stv"
(cd "$work/stv" && patch -p1 < "$here/stv-reach.patch")
cp "$here/driver.rs" "$work/driver/src/main.rs"
cat > "$work/Cargo.toml" <<'T'
[workspace]
members = ["stv", "driver"]
resolver = "3"
[workspace.package]
edition = "2024"
license = "MIT"
version = "0.1.0"
[workspace.lints.rust]
unsafe_code = "forbid"
[profile.release]
opt-level = 3
T
cat > "$work/driver/Cargo.toml" <<'T'
[package]
name = "driver"
edition.workspace = true
version.workspace = true
[dependencies]
stv = { path = "../stv" }
T
cd "$work" && cargo build --release
