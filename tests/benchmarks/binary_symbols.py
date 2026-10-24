"""Join definition-start aliases to unique callable bodies at the same address."""

import re
import subprocess
from collections import defaultdict


def aliases(output, names):
    bodies = defaultdict(set)
    boundaries = []
    for line in output.splitlines():
        fields = line.split()
        if len(fields) < 3 or fields[1] not in "TtWw":
            continue
        symbol = fields[0].removeprefix("_")
        if symbol.startswith("scoop$1$cb$"):
            bodies[int(fields[2], 16)].add(symbol)
        elif re.fullmatch(r"scoop\$1\$bs\$[0-9a-f]{64}", symbol):
            boundaries.append((symbol, int(fields[2], 16)))
    result = dict(names)
    for boundary, address in boundaries:
        candidates = bodies[address]
        if len(candidates) == 1:
            body = next(iter(candidates))
            if body in names:
                result[boundary] = f"{names[body]} [body={body}]"
    return result


def load(names, binary, tools=None):
    nm = str(tools / "llvm-nm") if tools else "llvm-nm"
    result = subprocess.run(
        [nm, "--defined-only", "--format=posix", str(binary)],
        capture_output=True,
        text=True,
        check=True,
    )
    return aliases(result.stdout, names)
