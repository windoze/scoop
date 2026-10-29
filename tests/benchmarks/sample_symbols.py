"""Resolve explicit main-image PCs when macOS sample folds symbol aliases."""

import re
import subprocess

import dwarf_symbols

ADDRESS = r"0x[0-9a-fA-F]+"
FRAME = re.compile(r"\(in [^)]+\).*\[(?P<addresses>0x[^\]]+)\]")


def image_range(text):
    load = re.search(rf"^Load Address:\s+({ADDRESS})\s*$", text, re.MULTILINE)
    if load is None:
        raise ValueError("sample report has no main-image Load Address")
    start = int(load[1], 16)
    for image in re.finditer(rf"^\s*({ADDRESS})\s+-\s+({ADDRESS})\s+", text, re.MULTILINE):
        if int(image[1], 16) == start:
            return start, int(image[2], 16)
    raise ValueError("sample report has no Binary Images range for its main image")


def frame_addresses(line, start, end):
    frame = FRAME.search(line)
    if frame is None:
        return []
    return [
        int(address, 16)
        for address in re.findall(ADDRESS, frame["addresses"])
        if start <= int(address, 16) <= end
    ]


def text_base(output):
    for segment in re.findall(r"Segment \{([^}]+)\}", output):
        if re.search(r"^\s*Name: __TEXT\s*$", segment, re.MULTILINE):
            address = re.search(rf"^\s*vmaddr: ({ADDRESS})\s*$", segment, re.MULTILINE)
            if address:
                return int(address[1], 16)
    raise ValueError("LLVM readobj found no Mach-O __TEXT segment")


def append_frames(text, frames, start, end):
    lines = []
    for line in text.splitlines(keepends=True):
        entries = [
            f"0x{address:x} = {frames[address]}"
            for address in dict.fromkeys(frame_addresses(line, start, end))
            if address in frames
        ]
        if entries:
            suffix = "\n" if line.endswith("\n") else ""
            line = line.removesuffix("\n") + " [Scoop PCs: " + "; ".join(entries) + "]" + suffix
        lines.append(line)
    return "".join(lines)


def annotate(text, names, binary, tools=None):
    start, end = image_range(text)
    addresses = sorted(
        {address for line in text.splitlines() for address in frame_addresses(line, start, end)}
    )
    if not addresses:
        return text
    readobj = str(tools / "llvm-readobj") if tools else "llvm-readobj"
    headers = subprocess.run(
        [readobj, "--macho-segment", str(binary)], capture_output=True, text=True, check=True
    )
    slide = start - text_base(headers.stdout)
    return append_frames(
        text, dwarf_symbols.resolve(binary, addresses, names, tools, slide), start, end
    )
