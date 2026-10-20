#!/bin/sh
# Runs two machines from power on through the attract mode, one playing the loops its CPUs spin in
# and one executing every instruction (stv-check spin): they must be equal after every frame.
set -e
cd "$(dirname "$0")/../.."
cargo build --release -q -p stv-check
./target/release/stv-check spin --frames "${1:-5400}"
