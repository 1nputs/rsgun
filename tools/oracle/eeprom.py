"""Writes build/rom/eeprom.bin: the ST-V settings EEPROM as the oracle has it at power on (Mednafen
formats it with the game's defaults), so the machine starts from the same contents.

    .venv/bin/python tools/oracle/eeprom.py
"""
import pathlib
import shutil
import subprocess
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]

out = ROOT / "out/oracle/eeprom"
subprocess.run([sys.executable, "-I", str(ROOT / "tools/oracle/run.py"), "--frames", "1", "--trace", "1:1",
                "--out", str(out)], check=True, stdout=subprocess.DEVNULL)
shutil.copy(out / "t000001/start/eeprom.bin", ROOT / "build/rom/eeprom.bin")
print("build/rom/eeprom.bin written")
