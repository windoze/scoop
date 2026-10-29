"""Resolve native PCs through LLVM's DWARF reader and preserve inline frames."""

import json
import subprocess
from pathlib import Path


def resolved_frames(output, names, bias):
    result = {}
    for line in output.splitlines():
        value = json.loads(line)
        for record in value if isinstance(value, list) else [value]:
            address = int(record["Address"], 16)
            frames = record.get("Symbol", [])
            if not any(
                frame.get("FunctionName", "").removeprefix("_") in names for frame in frames
            ):
                continue
            chain = []
            for index, frame in enumerate(frames):
                symbol = frame.get("FunctionName", "??").removeprefix("_")
                if symbol == "??":
                    continue
                label = f"{names[symbol]} {{{symbol}}}" if symbol in names else symbol
                start = frame.get("StartAddress")
                if start and address - bias >= int(start, 16):
                    label += f"+0x{address - bias - int(start, 16):x}"
                filename, line_number = frame.get("FileName"), frame.get("Line", 0)
                if filename and line_number:
                    label += f" ({filename}:{line_number})"
                if index < len(frames) - 1:
                    label += " [inline]"
                chain.append(label)
            if chain:
                result[address] = " <- ".join(chain)
    return result


def resolve(binary, addresses, names, tools=None, bias=0):
    if not addresses:
        return {}
    symbolizer = str(tools / "llvm-symbolizer") if tools else "llvm-symbolizer"
    binary = Path(binary)
    # LLVM 22 prefers executable symbols over DWARF names. In a stripped Mach-O,
    # its remaining C entry symbol can incorrectly cover later Scoop functions.
    # Use the matching sidecar directly when present; LLVM still owns all parsing.
    debug_object = next(
        (
            path
            for path in (
                Path(str(binary) + ".dSYM") / "Contents/Resources/DWARF" / binary.name,
                Path(str(binary) + ".debug"),
            )
            if path.is_file()
        ),
        binary,
    )
    result = subprocess.run(
        [
            symbolizer,
            f"--obj={debug_object}",
            f"--adjust-vma={bias}",
            "--functions=linkage",
            "--no-demangle",
            "--inlines",
            "--output-style=JSON",
        ],
        input="".join(f"0x{address:x}\n" for address in addresses),
        capture_output=True,
        text=True,
        check=True,
    )
    return resolved_frames(result.stdout, names, bias)
