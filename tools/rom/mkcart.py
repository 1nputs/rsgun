"""Make the images the SH-2 sees from the user's MAME-format zips, checking every chip's SHA-1.

    python3 tools/rom/mkcart.py [ROMS_DIR] [OUT_DIR]

ROMS_DIR defaults to roms/ in the repository, else roms/ beside it; OUT_DIR to build/rom.

Writes OUT_DIR/rsgun-cart.bin (the cartridge as mapped at $02000000, 0x3000000 bytes, big-endian) and
OUT_DIR/bios-<name>.bin for each ST-V BIOS in stvbios.zip. The chips are stored as MAME loads them
(ROM_LOAD16_WORD_SWAP), so each 16-bit word is swapped back; the layout is MAME's `rsgun` (stv.cpp).
"""
import hashlib
import pathlib
import sys
import zipfile

CART = [  # (chip, cart offset, SHA-1) from MAME's ROM_START( rsgun )
    ("mpr20958.7", 0x0200000, "b4744ab71ccbadda1921ba43dd1148e57c0f84c5"),
    ("mpr20959.2", 0x0400000, "965274a7297cb88e281fcbdd3ec5025c6463cc7b"),
    ("mpr20960.3", 0x0800000, "87c5d077eb1219c35fa65b4e11d5b62e826f5236"),
    ("mpr20961.4", 0x0C00000, "0ec2842622f3e9dc5689abd58aeddc7e5603b97a"),
    ("mpr20962.5", 0x1000000, "0ba0972f1bc7c56f4e0589d3e363523cea988bb0"),
]
BIOS = [  # (chip, SHA-1): the ones Mednafen and the tools use
    ("epr-20091.ic8", "eff0f54c70bce05ff3a289bf30b1027e1c8cd117"),   # Japan 97/08/21
    ("epr-17952a.ic8", "eaf1c3e5d602e1139d2090a78d7e19f04f916794"),  # USA 95/04/25
    ("epr-17954a.ic8", "af79cff317e5b57d49e463af16a9f616ed1eee08"),  # Europe 95/04/25
]


def swapped(data: bytes) -> bytearray:
    out = bytearray(data)
    out[0::2], out[1::2] = data[1::2], data[0::2]
    return out


def chip(zf: zipfile.ZipFile, name: str, sha1: str) -> bytes:
    data = zf.read(name)
    got = hashlib.sha1(data).hexdigest()
    if got != sha1:
        sys.exit(f"{zf.filename}:{name}: SHA-1 {got}, expected {sha1} (not MAME's rsgun set)")
    return data


ROOT = pathlib.Path(__file__).resolve().parents[2]


def default_roms() -> pathlib.Path:
    for d in (ROOT / "roms", ROOT.parent / "roms"):
        if (d / "rsgun.zip").exists():
            return d
    sys.exit("rsgun.zip not found in roms/ or ../roms/; pass ROMS_DIR")


def main() -> None:
    roms = pathlib.Path(sys.argv[1]) if len(sys.argv) > 1 else default_roms()
    out = pathlib.Path(sys.argv[2]) if len(sys.argv) > 2 else ROOT / "build/rom"
    out.mkdir(parents=True, exist_ok=True)
    cart = bytearray(b"\xff" * 0x3000000)
    with zipfile.ZipFile(roms / "rsgun.zip") as zf:
        for name, off, sha1 in CART:
            data = swapped(chip(zf, name, sha1))
            cart[off:off + len(data)] = data
    (out / "rsgun-cart.bin").write_bytes(cart)
    with zipfile.ZipFile(roms / "stvbios.zip") as zf:
        for name, sha1 in BIOS:
            stem = name.split(".")[0].replace("epr-", "epr")
            (out / f"bios-{stem}.bin").write_bytes(swapped(chip(zf, name, sha1)))
    print(f"{out}/rsgun-cart.bin and {len(BIOS)} BIOS images written; every SHA-1 matches")


if __name__ == "__main__":
    main()
