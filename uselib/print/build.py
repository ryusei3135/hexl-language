import subprocess
from pathlib import Path

ROOT = Path(__file__).parent
SRC_C = ROOT / "c"
SRC_A = ROOT / "asm"
BUILD = ROOT

BUILD.mkdir(exist_ok=True)

print("ROOT =", ROOT)
print("SRC  =", SRC_C)
print("BUILD =", BUILD)

c_files = list(SRC_C.rglob("*.c"))
asm_files = list(SRC_A.rglob("*.s"))

[[print("  ", f) for f in files] for files in [asm_files, c_files]]

objects = []

def files_com(file_list):
    for f in file_list:
        obj = BUILD / (f.stem + ".o")

        subprocess.run([
            "gcc",
            "-c",
            str(f),
            "-o",
            str(obj),
            "-O2",
            "-fPIC",
        ], check=True)

        objects.append(obj)

files_com(c_files)
files_com(asm_files)

print("Objects:")
[print("  ", obj) for obj in objects]

if not objects:
    raise RuntimeError(
        f"No C or Assembly files found in {SRC_C}"
    )

# static library
subprocess.run([
    "ar",
    "rcs",
    str(BUILD / "libprint.a"),
    *map(str, objects),
], check=True)

print("libprint.a created")