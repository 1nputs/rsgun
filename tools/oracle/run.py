"""Run the game on the oracle (Mednafen with tools/patches/mednafen), dumping state at chosen frames.

    .venv/bin/python tools/oracle/run.py --frames N [--dump F,F,...] [--trace F:N] [--out DIR] [--window]

Each dump is DIR/fFFFFFF/, each trace DIR/tFFFFFF/ (work RAM, VDP1/VDP2 memory, sound RAM, both SH-2s' registers, the screen as
PPM and PNG); see tools/patches/mednafen/oracle.inc. Mednafen keeps its settings and the ST-V BIOS chips in
out/mednafen-home (made on first use from the user's stvbios.zip). Headless by default: no window, no
sound, no frame skipping, unthrottled. Deterministic: the SMPC clock is not set from the host, and no
battery RAM, EEPROM or clock state carries over from an earlier run.
"""
import argparse
import os
import pathlib
import subprocess
import sys
import zipfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
MEDNAFEN = ROOT / "ref/mednafen/bin/mednafen"
HOME = ROOT / "out/mednafen-home"
BIOS = ("epr-20091.ic8", "epr-17952a.ic8", "epr-17954a.ic8")


def roms_dir() -> pathlib.Path:
    for d in (ROOT / "roms", ROOT.parent / "roms"):
        if (d / "rsgun.zip").exists():
            return d
    sys.exit("rsgun.zip not found in roms/ or ../roms/")


def prepare_home(roms: pathlib.Path) -> None:
    fw = HOME / "firmware"
    fw.mkdir(parents=True, exist_ok=True)
    with zipfile.ZipFile(roms / "stvbios.zip") as zf:
        for name in BIOS:
            if not (fw / name).exists():
                (fw / name).write_bytes(zf.read(name))


def ppm_to_png(ppm: pathlib.Path) -> None:
    try:
        from PIL import Image
    except ImportError:
        return
    Image.open(ppm).save(ppm.with_suffix(".png"))


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--frames", type=int, required=True, help="exit after this many frames")
    ap.add_argument("--dump", default="", help="frames to dump at, comma separated (default: the last)")
    ap.add_argument("--out", default=str(ROOT / "out/oracle"), help="dump directory")
    ap.add_argument("--trace", default="", help="F:N[,F:N...]: trace both SH-2s through frames F to F+N-1 (to DIR/tFFFFFF)")
    ap.add_argument("--hashes", default="", help="write a line of memory hashes per frame to this file")
    ap.add_argument("--window", action="store_true", help="show a window, at normal speed, with sound")
    ap.add_argument("extra", nargs="*", help="more Mednafen arguments")
    a = ap.parse_args()

    roms = roms_dir()
    prepare_home(roms)
    out = pathlib.Path(a.out).resolve()
    out.mkdir(parents=True, exist_ok=True)
    env = dict(os.environ, MEDNAFEN_HOME=str(HOME), STV_ORACLE_DIR=str(out),
               STV_ORACLE_DUMP=a.dump or str(a.frames), STV_ORACLE_EXIT=str(a.frames),
               STV_ORACLE_TRACE=a.trace,
               STV_ORACLE_HASHES=str(pathlib.Path(a.hashes).resolve()) if a.hashes else "")
    # Deterministic: no clock from the host, no battery RAM, EEPROM or clock state from an earlier run.
    for f in (HOME / "sav").glob("*"):
        f.unlink()
    args = [str(MEDNAFEN), "-video.fs", "0", "-video.frameskip", "0", "-ss.smpc.autortc", "0"]
    if not a.window:
        env.update(SDL_VIDEODRIVER="dummy", SDL_AUDIODRIVER="dummy")
        args += ["-sound", "0", "-nothrottle", "1", "-video.driver", "softfb"]
    args += a.extra + [str(roms / "rsgun.zip")]
    log = out / "mednafen.log"
    with open(log, "w") as fp:
        rc = subprocess.run(args, env=env, stdout=fp, stderr=subprocess.STDOUT).returncode
    for ppm in sorted(out.glob("f*/screen.ppm")):
        ppm_to_png(ppm)
    if rc != 0:
        sys.exit(f"mednafen exited with {rc}; see {log}")
    print(f"in {out}: {', '.join(p.name for p in sorted(out.glob('[ft]0*')))}")


if __name__ == "__main__":
    main()
