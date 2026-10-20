"""Decompile SH-2 functions from a raw big-endian image with Ghidra (headless, through PyGhidra).

    .venv/bin/python tools/ghidra/decomp.py IMAGE FILE_OFFSET SIZE LOAD_ADDRESS FUNC_ADDRESS...

Loads SIZE bytes of IMAGE from FILE_OFFSET at LOAD_ADDRESS (hex values) into a throwaway project under
build/ghidra, disassembles from each FUNC_ADDRESS, makes a function there and prints Ghidra's C.
Needs ref/ (tools/fetch_refs.sh): Ghidra, its JDK and the pyghidra package in .venv.
"""
import os
import pathlib
import sys
import tempfile

ROOT = pathlib.Path(__file__).resolve().parents[2]
os.environ.setdefault("JAVA_HOME", str(ROOT / "ref/jdk"))
os.environ.setdefault("GHIDRA_INSTALL_DIR", str(ROOT / "ref/ghidra"))

import pyghidra  # noqa: E402


def main() -> None:
    image, off, size, base = sys.argv[1], int(sys.argv[2], 16), int(sys.argv[3], 16), int(sys.argv[4], 16)
    funcs = [int(a, 16) for a in sys.argv[5:]]
    data = pathlib.Path(image).read_bytes()[off:off + size]
    pyghidra.start()
    from ghidra.app.cmd.disassemble import DisassembleCommand
    from ghidra.app.cmd.function import CreateFunctionCmd
    from ghidra.app.decompiler import DecompInterface
    from ghidra.util.task import ConsoleTaskMonitor

    monitor = ConsoleTaskMonitor()
    projdir = ROOT / "build/ghidra"
    projdir.mkdir(parents=True, exist_ok=True)
    name = f"seg_{base:08x}"
    with tempfile.TemporaryDirectory(dir=projdir) as tmp, \
            pyghidra.open_project(tmp, "scratch", create=True) as project:
        seg = pathlib.Path(tmp) / f"{name}.bin"
        seg.write_bytes(data)
        from java.io import File
        loader = (pyghidra.program_loader().project(project).source(File(str(seg))).name(name)
                  .language("SuperH:BE:32:SH-2").compiler("default").loaders("BinaryLoader")
                  .addLoaderArg("-loader-baseAddr", hex(base)))
        with loader.load() as results:
            results.save(monitor)
        with pyghidra.program_context(project, "/" + name) as program:
            space = program.getAddressFactory().getDefaultAddressSpace()
            mem = program.getMemory()
            if not mem.contains(space.getAddress(base)):
                sys.exit(f"loaded at {mem.getMinAddress()}, not {base:#x}")
            with pyghidra.transaction(program):
                for a in funcs:
                    addr = space.getAddress(a)
                    DisassembleCommand(addr, None, True).applyTo(program, monitor)
                    if program.getFunctionManager().getFunctionAt(addr) is None:
                        CreateFunctionCmd(addr).applyTo(program, monitor)
            dec = DecompInterface()
            dec.openProgram(program)
            for a in funcs:
                fn = program.getFunctionManager().getFunctionAt(space.getAddress(a))
                res = dec.decompileFunction(fn, 60, monitor)
                print(res.getDecompiledFunction().getC() if res.decompileCompleted() else res.getErrorMessage())


if __name__ == "__main__":
    main()
