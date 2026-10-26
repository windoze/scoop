"""Read callable address ranges from the platform linker's external map."""

import bisect
import itertools
import re

ADDRESS = r"0x[0-9a-fA-F]+"
CALLABLE = re.compile(r"_?scoop\$1\$cb\$[0-9a-f]{64}\Z")


class AddressMap:
    def __init__(self, ranges):
        self.ranges = sorted(set(ranges))
        self.starts = [start for start, _, _ in self.ranges]
        self.ends = list(itertools.accumulate((end for _, end, _ in self.ranges), max))

    def resolve(self, address):
        index = bisect.bisect_right(self.starts, address) - 1
        matches = set()
        while index >= 0 and self.ends[index] > address:
            start, end, symbol = self.ranges[index]
            if address < end:
                matches.add((symbol, address - start))
            index -= 1
        return next(iter(matches)) if len(matches) == 1 else None

    def frames(self, addresses, names, slide=0):
        result = {}
        for address in addresses:
            entry = self.resolve(address - slide)
            if entry and entry[0] in names:
                symbol, offset = entry
                result[address] = f"{names[symbol]} {{{symbol}}}+0x{offset:x}"
        return result


def load(path):
    text = path.read_text(encoding="utf-8")
    if "# Symbols:" in text and "# Sections:" in text:
        return AddressMap(darwin_ranges(text))
    if "Linker script and memory map" in text:
        return AddressMap(elf_ranges(text))
    raise ValueError(f"{path}: expected a Darwin or GNU ELF linker map")


def darwin_ranges(text):
    section = re.compile(rf"^({ADDRESS})\s+({ADDRESS})\s+__TEXT\s+__text\s*$")
    symbol = re.compile(rf"^({ADDRESS})\s+({ADDRESS})\s+\[\s*\d+\]\s+(\S+)\s*$")
    bounds = []
    symbols = []
    sizes = {}
    active = None
    for line in text.splitlines():
        if line.startswith("# "):
            if line in ("# Sections:", "# Symbols:", "# Dead Stripped Symbols:"):
                active = line
            continue
        if active == "# Sections:" and (match := section.fullmatch(line)):
            start, size = int(match[1], 16), int(match[2], 16)
            bounds.append((start, start + size))
        elif active == "# Symbols:" and (match := symbol.fullmatch(line)):
            start, size = int(match[1], 16), int(match[2], 16)
            sizes[start] = max(sizes.get(start, 0), size)
            if CALLABLE.fullmatch(match[3]):
                symbols.append((start, size, match[3].removeprefix("_")))
    for start, size, name in symbols:
        # A zero-sized alias may share an atom with an explicitly sized symbol.
        size = size or sizes[start]
        for lower, upper in bounds:
            if size and lower <= start < start + size <= upper:
                yield start, start + size, name
                break


def elf_ranges(text):
    text = text.split("Linker script and memory map", 1)[1]
    section = re.compile(r"^ (\.\S+)(?:\s+(.*))?$")
    extent = re.compile(rf"\s*({ADDRESS})\s+({ADDRESS})(?:\s+.*)?$")
    symbol = re.compile(rf"\s+({ADDRESS})\s+(\S+)\s*$")
    pending = None
    bounds = None
    symbols = []

    def ranges():
        if bounds is None:
            return
        # Definition-end markers and subsequent real symbols bound a body;
        # never let a lookup escape its explicitly sized input text section.
        ends = sorted({address for address, _ in symbols} | {bounds[1]})
        for address, name in symbols:
            if CALLABLE.fullmatch(name):
                end = ends[bisect.bisect_right(ends, address)]
                if address < end <= bounds[1]:
                    yield address, end, name.removeprefix("_")

    for line in text.splitlines():
        if match := section.fullmatch(line):
            yield from ranges()
            symbols = []
            bounds = None
            pending = match[1]
            line = match[2] or ""
        if pending and (match := extent.fullmatch(line)):
            start, size = int(match[1], 16), int(match[2], 16)
            if pending == ".text" or pending.startswith((".text.", ".gnu.linkonce.t.")):
                bounds = (start, start + size)
            pending = None
        elif bounds and (match := symbol.fullmatch(line)):
            address = int(match[1], 16)
            if bounds[0] <= address < bounds[1]:
                symbols.append((address, match[2]))
    yield from ranges()
