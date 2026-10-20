#!/bin/sh
# The Python environment the tools run in: .venv at the repository root.
set -e
cd "$(dirname "$0")/.."
python3 -m venv .venv
.venv/bin/pip install -q --upgrade pip
.venv/bin/pip install -q 'capstone>=5,<6' pillow
echo ".venv ready: .venv/bin/python"
