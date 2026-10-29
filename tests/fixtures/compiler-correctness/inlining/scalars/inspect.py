"""Check calls to the three scalar kernels by their declared function identities."""

import re
import sys
from pathlib import Path

for path, release in zip(sys.argv[1:], (False, True), strict=True):
    source = Path(path).read_text()
    functions = dict(re.findall(r"^  fun (\S+) @fn(\d+)", source, re.M))
    main = re.search(r"^  fun main @fn\d+.*?(?=^  fun |\Z)", source, re.M | re.S).group()
    for name in ("add", "scale", "scalar"):
        target = functions[name]
        assert (f" = @fn{target} direct" in main) != release, name
