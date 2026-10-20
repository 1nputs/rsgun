"""A Ghidra project of the game's code as the port has it, for the rewrite: work RAM-H from a dump (the
main program, `.control`, the BIOS's code in RAM), each overlay the evidence saw as an overlay block
over the window, the types of the cart's leftover dma.c object (tools/rom/coffdump.py --header), and a
function at each function the port translated (build/port/generated.rs). Ghidra's C for each of them
is written to build/ghidra/c/, named as the translation names it (f_ADDR, f_ADDR_FINGERPRINT).

    .venv/bin/python tools/ghidra/project.py [--dump DIR] [--overlays DIR,DIR] [--only NAME,NAME]

Defaults: --dump out/evidence/dumps/f003000, --overlays out/evidence/play1.overlays,
out/evidence/attract.overlays. --only decompiles just those functions (the project is kept). Output
(ROM-derived, never committed): build/ghidra/rsgun/ (remade each time but with --only), build/ghidra/c/.
Needs ref/ (tools/fetch_refs.sh): Ghidra, its JDK and the pyghidra package in .venv.
"""
import argparse
import os
import pathlib
import re
import shutil
import sys

ROOT = pathlib.Path(__file__).resolve().parents[2]
os.environ.setdefault("JAVA_HOME", str(ROOT / "ref/jdk"))
os.environ.setdefault("GHIDRA_INSTALL_DIR", str(ROOT / "ref/ghidra"))

import pyghidra  # noqa: E402

WRAM_H = 0x06000000
# the overlay window (crates/rsgun's overlay())
WINDOW = (0x06080000, 0x060A5000)
PROJECT = ROOT / "build/ghidra/rsgun"
OUT = ROOT / "build/ghidra/c"
HEADER = ROOT / "build/leftovers/dma_types.h"


def functions():
    """The functions the port translated: (name, address, overlay fingerprint or None)."""
    text = (ROOT / "build/port/generated.rs").read_text()
    out = []
    for m in re.finditer(r"^pub fn (f_([0-9a-f]{8})(?:_([0-9a-f]{16}))?)\(", text, re.M):
        out.append((m.group(1), int(m.group(2), 16), m.group(3)))
    return out


def header_sizes():
    """Each struct's and union's size, as the debug information gives it."""
    text = HEADER.read_text()
    return {m.group(2): int(m.group(3), 16)
            for m in re.finditer(r"^(struct|union) (\w+) \{\n.*?^\}; /\* size (0x[0-9a-f]+) \*/", text, re.S | re.M)}


def build(project, monitor, dump, overlays):
    from java.io import ByteArrayInputStream, File

    wram = (dump / "wram-h.bin").read_bytes()
    image = PROJECT / "wram-h.bin"
    image.write_bytes(wram)
    loader = (pyghidra.program_loader().project(project).source(File(str(image))).name("rsgun")
              .language("SuperH:BE:32:SH-2").compiler("default").loaders("BinaryLoader")
              .addLoaderArg("-loader-baseAddr", hex(WRAM_H)))
    with loader.load() as results:
        results.save(monitor)
    image.unlink()
    with pyghidra.program_context(project, "/rsgun") as program:
        mem = program.getMemory()
        space = program.getAddressFactory().getDefaultAddressSpace()
        with pyghidra.transaction(program):
            for fp, data in sorted(overlays.items()):
                mem.createInitializedBlock(f"ov_{fp}", space.getAddress(WINDOW[0]), ByteArrayInputStream(data),
                                           len(data), monitor, True)
            from ghidra.app.util.cparser.C import CParser
            dtm = program.getDataTypeManager()
            # Ghidra's parser takes a pointer to an array (long (*p)[3]) for an array of pointers
            text = re.sub(r"(\w+) \(\*(\w+)\)\[\d+\];", r"\1 *\2;", HEADER.read_text())
            CParser(dtm, True, None).parse(text)
            wrong = []
            for name, size in header_sizes().items():
                dt = next((t for t in dtm.getAllDataTypes() if t.getName() == name), None)
                if dt is None or dt.getLength() != size:
                    wrong.append(f"{name}: {dt.getLength() if dt else 'missing'} (debug information {size:#x})")
            if wrong:
                sys.exit("the types' layout differs:\n  " + "\n  ".join(wrong))
        program.save("rsgun: work RAM-H, overlays, dma.c's types", monitor)


def decompile(project, monitor, wanted):
    from ghidra.app.cmd.disassemble import DisassembleCommand
    from ghidra.app.cmd.function import CreateFunctionCmd
    from ghidra.app.decompiler import DecompileOptions, DecompInterface
    from ghidra.program.model.symbol import SourceType

    OUT.mkdir(parents=True, exist_ok=True)
    with pyghidra.program_context(project, "/rsgun") as program:
        factory = program.getAddressFactory()

        def address(a, fp):
            space = factory.getAddressSpace(f"ov_{fp}") if fp else factory.getDefaultAddressSpace()
            return space.getAddress(a) if space else None

        fm = program.getFunctionManager()
        todo = []
        with pyghidra.transaction(program):
            for name, a, fp in wanted:
                addr = address(a, fp)
                if addr is None:
                    print(f"{name}: no overlay {fp} in the project", file=sys.stderr)
                    continue
                DisassembleCommand(addr, None, True).applyTo(program, monitor)
                if fm.getFunctionAt(addr) is None:
                    CreateFunctionCmd(addr).applyTo(program, monitor)
                fn = fm.getFunctionAt(addr)
                if fn is None:
                    print(f"{name}: Ghidra made no function", file=sys.stderr)
                    continue
                fn.setName(name, SourceType.USER_DEFINED)
                todo.append((name, fn))
        with pyghidra.transaction(program):
            n = constant_literals(program, [fn for _, fn in todo])
        print(f"{n} literals marked constant")
        program.save("functions", monitor)
        dec = DecompInterface()
        options = DecompileOptions()
        options.grabFromProgram(program)  # without, read-only and constant data are not folded
        dec.setOptions(options)
        dec.openProgram(program)
        failed = 0
        for name, fn in todo:
            res = dec.decompileFunction(fn, 120, monitor)
            c = res.getDecompiledFunction()
            if c is None:
                failed += 1
                (OUT / f"{name}.c").write_text(f"/* {name}: {res.getErrorMessage()} */\n")
                continue
            (OUT / f"{name}.c").write_text(c.getC())
        print(f"{len(todo) - failed} of {len(wanted)} functions decompiled into {OUT.relative_to(ROOT)}")


def constant_literals(program, fns):
    """Marks the literals the functions load PC-relative (mov.l/mov.w @(disp,PC)) as constant data,
    so the decompiler folds their values in: the image is RAM, which it otherwise takes as
    changeable."""
    from ghidra.program.model.address import Address
    from ghidra.program.model.data import DWordDataType, WordDataType, MutabilitySettingsDefinition

    listing = program.getListing()
    n = 0
    for fn in fns:
        space = fn.getEntryPoint().getAddressSpace()
        for inst in listing.getInstructions(fn.getBody(), True):
            m = inst.getMnemonicString().lower()
            if m not in ("mov.l", "mov.w") or ",pc)" not in str(inst).lower():
                continue
            op = inst.getOpObjects(0)
            if not op:
                continue
            to = op[0] if isinstance(op[0], Address) else space.getAddress(op[0].getUnsignedValue())
            size = 4 if m == "mov.l" else 2
            if listing.getInstructionContaining(to) is not None:
                continue
            data = listing.getDataAt(to)
            if data is None or data.getLength() != size:
                listing.clearCodeUnits(to, to.add(size - 1), False)
                data = listing.createData(to, DWordDataType() if size == 4 else WordDataType())
            MutabilitySettingsDefinition.DEF.setChoice(data, MutabilitySettingsDefinition.CONSTANT)
            n += 1
    return n


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--dump", default=str(ROOT / "out/evidence/dumps/f003000"))
    ap.add_argument("--overlays", default=",".join(str(ROOT / d) for d in
                                                   ("out/evidence/play1.overlays", "out/evidence/attract.overlays")))
    ap.add_argument("--only", default="")
    args = ap.parse_args()
    wanted = functions()
    if args.only:
        names = set(args.only.split(","))
        wanted = [f for f in wanted if f[0] in names]
    pyghidra.start()
    from ghidra.util.task import ConsoleTaskMonitor
    monitor = ConsoleTaskMonitor()
    if not args.only:
        overlays = {}
        for d in args.overlays.split(","):
            for f in pathlib.Path(d).glob("*.bin"):
                overlays[f.stem] = f.read_bytes()
        if PROJECT.exists():
            shutil.rmtree(PROJECT)
        PROJECT.mkdir(parents=True)
    with pyghidra.open_project(str(PROJECT), "rsgun", create=not args.only) as project:
        if not args.only:
            build(project, monitor, pathlib.Path(args.dump), overlays)
        decompile(project, monitor, wanted)


if __name__ == "__main__":
    main()
